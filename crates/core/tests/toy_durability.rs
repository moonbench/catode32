//! A toy whose durability reaches 0 is broken until it is bought again.

use catode32_core::{
    behavior::PlayVariant,
    context::{GameContext, ToyVariant},
    led::Led,
};

#[test]
fn worn_out_toy_is_broken_until_refreshed() {
    let mut ctx = GameContext::new(Led::new());
    ctx.toys.clear();

    // Not owned: not broken (the menu won't offer it anyway).
    assert!(!ctx.toy_is_broken(PlayVariant::String));

    assert!(ctx.add_toy(ToyVariant::String_));
    assert!(!ctx.toy_is_broken(PlayVariant::String));

    let idx = ctx.find_toy(ToyVariant::String_).unwrap();
    ctx.toys[idx].durability = 0;
    assert!(ctx.toy_is_broken(PlayVariant::String));
    assert!(!ctx.toy_is_broken(PlayVariant::Feather));

    assert!(ctx.refresh_toy(ToyVariant::String_));
    assert!(!ctx.toy_is_broken(PlayVariant::String));
}

#[test]
fn hand_is_never_broken() {
    let ctx = GameContext::new(Led::new());
    assert!(!ctx.toy_is_broken(PlayVariant::Hand));
}

#[test]
fn feather_ball_wears_out_separately_from_yarn_ball() {
    let mut ctx = GameContext::new(Led::new());
    ctx.toys.clear();
    assert!(ctx.add_toy(ToyVariant::Ball));
    assert!(ctx.add_toy(ToyVariant::FeatherBall));
    assert_eq!(PlayVariant::FeatherBall.to_toy_variant(), Some(ToyVariant::FeatherBall));
    assert_eq!(ToyVariant::FeatherBall.to_play_variant(), PlayVariant::FeatherBall);

    let idx = ctx.find_toy(ToyVariant::FeatherBall).unwrap();
    ctx.toys[idx].durability = 0;
    assert!(ctx.toy_is_broken(PlayVariant::FeatherBall));
    assert!(!ctx.toy_is_broken(PlayVariant::Ball));

    assert!(ctx.refresh_toy(ToyVariant::FeatherBall));
    assert!(!ctx.toy_is_broken(PlayVariant::FeatherBall));
}
