//! Smoke tests for the desktop platform shims. These run on the host with
//! `cargo test -p catode32-core --no-default-features --features desktop
//! --target <host>` (the firmware target can't run binaries).

#![cfg(feature = "desktop")]

use catode32_core::platform::{
    rng::Rng,
    time::{Duration, Instant},
};

#[test]
fn duration_round_trip() {
    let d = Duration::from_millis(1234);
    assert_eq!(d.as_millis(), 1234);
    assert_eq!(d.as_micros(), 1234 * 1000);
    assert_eq!(Duration::from_secs(2).as_millis(), 2000);
}

#[test]
fn instant_monotonic_within_a_call() {
    let a = Instant::now();
    let b = Instant::now();
    // Either same nanosecond (very tight) or b strictly after a, never before.
    assert!(b >= a);
}

#[test]
fn instant_plus_duration_is_later() {
    let a = Instant::now();
    let later = a + Duration::from_millis(100);
    assert!(later > a);
}

#[test]
fn rng_changes_each_call() {
    let rng = Rng::new();
    let a = rng.random();
    let b = rng.random();
    let c = rng.random();
    // Three consecutive xorshift outputs collide with vanishing
    // probability; if all three match, the impl is broken.
    assert!(!(a == b && b == c));
}

#[test]
fn rng_fills_buffer() {
    let rng = Rng::new();
    let mut buf = [0u8; 32];
    rng.read(&mut buf);
    // Catastrophic failure mode: read() left every byte zero.
    assert!(buf.iter().any(|&b| b != 0));
}

/// One test for the whole crash record: it lives in a process-wide static,
/// so separate tests would race.
#[test]
fn crash_record_round_trip() {
    use catode32_core::platform::persist::{crash_take, crash_write, CRASH_MSG_MAX, CRASH_PCS_MAX};

    assert!(crash_take().is_none());

    crash_write("game.rs:42 boom", &[0x4200_1000, 0x4200_2000]);
    let rec = crash_take().expect("record written");
    assert_eq!(rec.msg.as_str(), "game.rs:42 boom");
    assert_eq!(rec.pcs.as_slice(), &[0x4200_1000, 0x4200_2000]);
    // Taking clears it.
    assert!(crash_take().is_none());

    // Overlong input truncates on a char boundary and drops extra PCs.
    let long: std::string::String = "é".repeat(CRASH_MSG_MAX);
    let pcs = [1u32; CRASH_PCS_MAX + 3];
    crash_write(&long, &pcs);
    let rec = crash_take().expect("record written");
    assert!(rec.msg.len() <= CRASH_MSG_MAX);
    assert!(rec.msg.chars().all(|c| c == 'é'));
    assert_eq!(rec.pcs.len(), CRASH_PCS_MAX);
}
