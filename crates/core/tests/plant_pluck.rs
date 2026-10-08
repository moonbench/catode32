//! Plucking a potted plant leaves its empty pot in place; empty pots go back
//! to inventory only via `stow_empty_pot`.

use catode32_core::{
    assets::plants::{PlantStage, PotKind},
    context::{GameContext, SeedKind},
    led::Led,
    plant_system::{
        get_plant, get_plant_mut, place_empty_pot, plant_in_ground, plant_seed_into_pot,
        pluck_plant, stow_empty_pot, PlantLayer,
    },
    scene::SceneId,
};

const SMALL_SLOT: usize = 0;

fn ctx() -> GameContext {
    let mut ctx = GameContext::new(Led::new());
    ctx.plants.clear();
    ctx.pots = [1; 4];
    ctx.seeds[SeedKind::CatGrass as usize] = 5;
    ctx
}

fn potted_plant(ctx: &mut GameContext) -> u32 {
    let id = place_empty_pot(
        ctx,
        SceneId::Inside,
        PlantLayer::Midground,
        10,
        0,
        PotKind::Small,
    )
    .unwrap();
    assert!(plant_seed_into_pot(ctx, id, SeedKind::CatGrass));
    id
}

#[test]
fn pluck_live_potted_plant_leaves_empty_pot() {
    let mut ctx = ctx();
    let id = potted_plant(&mut ctx);
    let pots_before = ctx.pots[SMALL_SLOT];

    assert!(pluck_plant(&mut ctx, id));

    let p = get_plant(&ctx, id).expect("pot should remain in scene");
    assert_eq!(p.stage, PlantStage::EmptyPot);
    assert_eq!(p.pot, PotKind::Small);
    assert_eq!(p.x, 10);
    assert!(p.seed.is_none());
    assert_eq!(ctx.pots[SMALL_SLOT], pots_before);
}

#[test]
fn pluck_dead_potted_plant_keeps_pot() {
    let mut ctx = ctx();
    let id = potted_plant(&mut ctx);
    get_plant_mut(&mut ctx, id).unwrap().stage = PlantStage::YoungDead;

    assert!(pluck_plant(&mut ctx, id));
    assert_eq!(get_plant(&ctx, id).unwrap().stage, PlantStage::EmptyPot);
}

#[test]
fn pluck_ground_plant_removes_it() {
    let mut ctx = ctx();
    let id = plant_in_ground(
        &mut ctx,
        SceneId::Outside,
        PlantLayer::Midground,
        0,
        0,
        SeedKind::CatGrass,
    )
    .unwrap();

    assert!(pluck_plant(&mut ctx, id));
    assert!(get_plant(&ctx, id).is_none());
}

#[test]
fn pluck_empty_pot_is_noop() {
    let mut ctx = ctx();
    let id = place_empty_pot(
        &mut ctx,
        SceneId::Inside,
        PlantLayer::Midground,
        0,
        0,
        PotKind::Small,
    )
    .unwrap();

    assert!(!pluck_plant(&mut ctx, id));
    assert!(get_plant(&ctx, id).is_some());
}

#[test]
fn stow_empty_pot_refunds_inventory() {
    let mut ctx = ctx();
    let id = potted_plant(&mut ctx);
    let pots_before = ctx.pots[SMALL_SLOT];

    // Not empty yet: stowing does nothing.
    assert!(!stow_empty_pot(&mut ctx, id));
    assert!(get_plant(&ctx, id).is_some());

    pluck_plant(&mut ctx, id);
    assert!(stow_empty_pot(&mut ctx, id));
    assert!(get_plant(&ctx, id).is_none());
    assert_eq!(ctx.pots[SMALL_SLOT], pots_before + 1);
}
