//! Playdate familiarity: playtime with a peer accumulates in
//! `ctx.friends`, drives `friendship_level`, and survives save/load in
//! the MicroPython-compatible `"friends": {mac: {n, t, c}}` shape.
//!
//! The save tests flip the process CWD to a temp dir so the repo's real
//! `./catode32-save.json` isn't touched.

#![cfg(feature = "desktop")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use catode32_core::{
    context::{GameContext, VisitRole, VisitState, FRIENDS_MAX, FRIEND_FULL_SECS},
    led::Led,
    save,
};

const A: [u8; 6] = [0xaa, 0xbb, 0xcc, 0x00, 0x11, 0x22];
const B: [u8; 6] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

fn lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

struct CwdGuard {
    prev: PathBuf,
    tmp: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CwdGuard {
    fn enter() -> Self {
        let _lock = lock();
        let prev = env::current_dir().expect("get cwd");
        let mut tmp = env::temp_dir();
        tmp.push(format!(
            "catode32-friends-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&tmp).expect("create tmp");
        env::set_current_dir(&tmp).expect("set cwd");
        Self { prev, tmp, _lock }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.prev);
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

fn visit(mac: [u8; 6], name: &str, secs: f32) -> VisitState {
    let mut peer_name = heapless::String::new();
    let _ = peer_name.push_str(name);
    VisitState {
        peer_mac: mac,
        peer_name,
        role: VisitRole::Inviter,
        play_time: secs,
        greeted: true,
    }
}

#[test]
fn playtime_accumulates_into_familiarity() {
    let mut ctx = GameContext::new(Led::new());
    assert_eq!(ctx.friendship_level(&A), 0.0);

    ctx.record_visit_end(&visit(A, "Mochi", FRIEND_FULL_SECS / 4.0));
    assert!((ctx.friendship_level(&A) - 0.25).abs() < 1e-4);

    ctx.record_visit_end(&visit(A, "Mochi2", FRIEND_FULL_SECS));
    assert_eq!(ctx.friendship_level(&A), 1.0);
    assert_eq!(ctx.friends.len(), 1);
    assert_eq!(ctx.friends[0].visits, 2);
    assert_eq!(ctx.friends[0].name.as_str(), "Mochi2");
    assert_eq!(ctx.friendship_level(&B), 0.0);
}

#[test]
fn very_short_visits_are_ignored() {
    let mut ctx = GameContext::new(Led::new());
    ctx.record_visit_end(&visit(A, "Blip", 5.0));
    assert!(ctx.friends.is_empty());
}

#[test]
fn full_list_evicts_least_played() {
    let mut ctx = GameContext::new(Led::new());
    for i in 0..FRIENDS_MAX {
        let mac = [0, 0, 0, 0, 0, i as u8];
        ctx.update_friend(mac, "Cat", 100.0 + i as f32);
    }
    assert_eq!(ctx.friends.len(), FRIENDS_MAX);

    // A brief newcomer can't displace anyone: the least-played friend
    // (peer 0, 100 s) has spent longer with us.
    ctx.update_friend(A, "Brief", 50.0);
    assert_eq!(ctx.friends.len(), FRIENDS_MAX);
    assert!(!ctx.friends.iter().any(|f| f.mac == A));
    assert!(ctx.friendship_level(&[0, 0, 0, 0, 0, 0]) > 0.0);

    // A newcomer who stayed longer than the bottom friend replaces them.
    ctx.update_friend(B, "Stayed", 150.0);
    assert_eq!(ctx.friends.len(), FRIENDS_MAX);
    assert_eq!(ctx.friendship_level(&[0, 0, 0, 0, 0, 0]), 0.0);
    assert!(ctx.friends.iter().any(|f| f.mac == B));

    // Existing friends still accumulate when the list is full.
    ctx.update_friend([0, 0, 0, 0, 0, 1], "Cat", 1000.0);
    assert_eq!(ctx.friendship_level(&[0, 0, 0, 0, 0, 1]), 1.0);
}

#[test]
fn friends_round_trip_through_save() {
    let _g = CwdGuard::enter();
    let mut ctx = GameContext::new(Led::new());
    ctx.update_friend(A, "Mochi", 450.0);
    ctx.update_friend(B, "Biscuit", 30.0);
    assert!(save::save(&mut ctx));

    let json = fs::read_to_string("catode32-save.json").expect("read save");
    assert!(json.contains(r#""friends":{"aa:bb:cc:00:11:22":{"n":"Mochi","#), "{json}");

    let mut loaded = GameContext::new(Led::new());
    assert!(save::load(&mut loaded));
    assert_eq!(loaded.friends.len(), 2);
    assert!((loaded.friendship_level(&A) - 0.5).abs() < 1e-4);
    let b = loaded.friends.iter().find(|f| f.mac == B).unwrap();
    assert_eq!(b.name.as_str(), "Biscuit");
    assert_eq!(b.visits, 1);
}

#[test]
fn loads_micropython_friends_format() {
    let _g = CwdGuard::enter();
    fs::write(
        "catode32-save.json",
        r#"{"friends":{"aa:bb:cc:00:11:22":{"n":"Mochi","t":912.5,"c":3},"bogus":{"n":"x","t":1.0,"c":1}}}"#,
    )
    .expect("write save");

    let mut ctx = GameContext::new(Led::new());
    assert!(save::load(&mut ctx));
    assert_eq!(ctx.friends.len(), 1);
    assert_eq!(ctx.friendship_level(&A), 1.0);
    assert_eq!(ctx.friends[0].visits, 3);
}
