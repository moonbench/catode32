//! Each pet's weather starts at a point in the shared sequence chosen by its
//! seed, and applying the same seed always gives the same starting weather.

use catode32_core::{context::GameContext, led::Led};

const SEED_A: u64 = 0x0ED3_DAFF_FD3E_74D1;
const SEED_B: u64 = 0x1122_3344_5566_7788;

fn reseeded(seed: u64) -> GameContext {
    let mut ctx = GameContext::new(Led::new());
    ctx.reseed(seed);
    ctx
}

#[test]
fn different_seeds_start_at_different_steps() {
    assert_ne!(reseeded(SEED_A).weather_step, reseeded(SEED_B).weather_step);
}

#[test]
fn same_seed_gives_same_starting_weather() {
    let a = reseeded(SEED_A);
    let b = reseeded(SEED_A);
    assert!(a.weather == b.weather);
    assert_eq!(a.weather_step, b.weather_step);
    assert_eq!(a.weather_timer, b.weather_timer);
}

#[test]
fn reseed_arms_the_weather_timer() {
    assert!(reseeded(SEED_A).weather_timer > 0.0);
}
