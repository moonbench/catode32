//! The adoption seed mixer: every input bit must reach the root, and the
//! expansion must keep producing distinct pet seeds from any root.

use std::collections::HashSet;

use catode32_core::{
    pet_seed::{absorb, next_expanded},
    platform::rng::{EntropySource, Rng},
};

#[test]
fn absorb_reacts_to_every_input_bit() {
    let mut base = 0x0123_4567_89AB_CDEF;
    absorb(&mut base, 0);
    for bit in 0..64 {
        let mut flipped = 0x0123_4567_89AB_CDEF;
        absorb(&mut flipped, 1u64 << bit);
        assert_ne!(base, flipped, "input bit {bit} was lost");
    }
}

#[test]
fn expansion_does_not_repeat() {
    let mut state = 0xDEAD_BEEF_u64;
    let seen: HashSet<u64> = (0..10_000).map(|_| next_expanded(&mut state)).collect();
    assert_eq!(seen.len(), 10_000);
}

#[test]
fn expansion_escapes_a_zero_root() {
    let mut state = 0;
    let first = next_expanded(&mut state);
    let second = next_expanded(&mut state);
    assert_ne!(first, 0);
    assert_ne!(first, second);
}

#[test]
fn entropy_source_fills_every_sample() {
    let rng = Rng::new();
    let mut samples = [0u32; 32];
    EntropySource::new().collect(&rng, false, &mut samples);
    assert!(samples.iter().all(|&s| s != 0));
}
