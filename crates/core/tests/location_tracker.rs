//! Decay/promote and quorum behaviour of the location tracker.

use catode32_core::{
    context::GameContext,
    led::Led,
    location_tracker::{process, ScanBeacon},
};

fn ap(id: u8) -> ScanBeacon {
    ScanBeacon {
        addr: [0, 0, 0, 0, 0, id],
        name: heapless::String::new(),
        rssi: -60,
    }
}

fn ctx() -> GameContext {
    GameContext::new(Led::new())
}

fn learn(ctx: &mut GameContext, ids: &[u8], scans: usize, quorum: usize) {
    let aps: Vec<ScanBeacon> = ids.iter().map(|i| ap(*i)).collect();
    for _ in 0..scans {
        process(ctx, &aps, quorum);
    }
}

#[test]
fn entries_promote_after_repeated_sightings() {
    let mut c = ctx();
    learn(&mut c, &[1, 2], 5, 1);
    assert_eq!(c.loc_familiar.len(), 2);
    assert!(c.loc_recent.is_empty());
}

#[test]
fn quorum_one_needs_any_familiar_entry() {
    let mut c = ctx();
    learn(&mut c, &[1, 2], 5, 1);
    process(&mut c, &[ap(1)], 1);
    assert!(c.in_familiar_location);
    process(&mut c, &[ap(9)], 1);
    assert!(!c.in_familiar_location);
}

#[test]
fn quorum_two_needs_two_familiar_entries() {
    let mut c = ctx();
    learn(&mut c, &[1, 2], 5, 2);
    process(&mut c, &[ap(1)], 2);
    assert!(!c.in_familiar_location);
    process(&mut c, &[ap(1), ap(2)], 2);
    assert!(c.in_familiar_location);
}

#[test]
fn unseen_entries_decay_away() {
    let mut c = ctx();
    process(&mut c, &[ap(1)], 1);
    for _ in 0..4 {
        process(&mut c, &[], 1);
    }
    assert!(c.loc_recent.is_empty());
}
