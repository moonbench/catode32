//! The guardrail: a save that exists but can't be read is never
//! overwritten. Its own test binary because the lock is process-wide.

#![cfg(feature = "desktop")]

mod common;

use std::fs;

use catode32_core::{
    context::GameContext,
    led::Led,
    ring,
    save::{self, LoadOutcome},
    storage,
};
use common::{maxed::maxed_context, CwdGuard, MemFlash};

fn fresh() -> GameContext {
    GameContext::new(Led::new())
}

#[test]
fn unreadable_saves_lock_storage_until_explicitly_erased() {
    // Case 1: every v1 snapshot's pet section is damaged.
    {
        let _g = CwdGuard::enter();
        let mut ctx = maxed_context();
        assert!(save::save(&mut ctx));
        assert!(save::save(&mut ctx));

        let mut image = fs::read(storage::IMAGE_PATH).unwrap();
        let mut flash = MemFlash::from_bytes(image.clone());
        for s in ring::scan(&mut flash).snaps.iter() {
            image[s.slot * ring::SLOT_SIZE + ring::HEADER_LEN + 8] ^= 0x01;
        }
        fs::write(storage::IMAGE_PATH, &image).unwrap();

        let mut back = fresh();
        assert_eq!(save::load(&mut back), LoadOutcome::Unreadable);
        assert!(storage::is_locked());

        // Nothing may write while locked: the image stays byte-identical.
        let mut other = fresh();
        assert!(!save::save(&mut other));
        save::save_if_needed(&mut other);
        assert_eq!(fs::read(storage::IMAGE_PATH).unwrap(), image);

        // The explicit erase is the only way out.
        assert!(storage::erase_all());
        assert!(!storage::is_locked());
        assert!(!save::has_save());
        assert!(save::save(&mut other));
    }

    // Case 2: a legacy v0 save that doesn't parse.
    {
        let _g = CwdGuard::enter();
        fs::write(storage::LEGACY_PATH, b"{\"pet_seed\": 12, \"plants\": [ oops").unwrap();
        let mut back = fresh();
        assert_eq!(save::load(&mut back), LoadOutcome::Unreadable);
        assert!(storage::is_locked());
        assert!(!save::save(&mut back));
        assert!(fs::metadata(storage::IMAGE_PATH).is_err(), "no snapshot written over it");
        assert!(storage::erase_all());
        assert!(!storage::is_locked());
    }
}
