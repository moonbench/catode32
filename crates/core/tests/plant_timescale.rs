//! Plant thresholds are in in-game hours (15 real minutes each). Guards
//! against tuning drifting back to the old 1-hour-per-real-minute scale,
//! where unwatered indoor plants stayed "OK" for weeks of real time.

use catode32_core::{
    assets::plants::{PlantStage, PotKind},
    context::SeedKind,
    plant_system::{spec_for, tick_plant, Plant, PlantLayer},
    scene::SceneId,
    time_system::{Season, Weather},
};

fn plant(seed: SeedKind, scene: SceneId, stage: PlantStage) -> Plant {
    Plant {
        id: 0,
        seed: Some(seed),
        scene,
        layer: PlantLayer::Midground,
        x: 0,
        y_snap: 0,
        pot: PotKind::Small,
        stage,
        age_hours: 0,
        water_debt: 0.0,
        fertilizer: 0.0,
        planted_day: Some(0),
        mirror: false,
        aged: false,
    }
}

fn tick_hours(p: &mut Plant, hours: u32, weather: Weather) {
    for _ in 0..hours {
        tick_plant(p, Season::Summer, weather);
    }
}

#[test]
fn indoor_rose_survives_a_real_day() {
    // 1 real day = 96 in-game hours.
    let mut p = plant(SeedKind::Rose, SceneId::Inside, PlantStage::Growing);
    tick_hours(&mut p, 96, Weather::Clear);
    assert!(!p.stage.is_wilted() && !p.stage.is_dead());
    assert!(p.water_debt > 0.0);
}

#[test]
fn unwatered_indoor_rose_dies_within_25_game_days() {
    let mut p = plant(SeedKind::Rose, SceneId::Inside, PlantStage::Growing);
    tick_hours(&mut p, 25 * 24, Weather::Clear);
    assert!(p.stage.is_dead());
}

#[test]
fn indoor_plant_wilts_before_death_threshold() {
    let spec = spec_for(SeedKind::CatGrass);
    let mut p = plant(SeedKind::CatGrass, SceneId::Kitchen, PlantStage::Young);
    let mut hours = 0;
    while !p.stage.is_wilted() && hours < 10_000 {
        tick_plant(&mut p, Season::Summer, Weather::Clear);
        hours += 1;
    }
    assert!(p.stage.is_wilted());
    assert!(p.water_debt <= spec.death as f32);
}

#[test]
fn rain_keeps_outdoor_plants_watered() {
    let mut p = plant(SeedKind::CatGrass, SceneId::Outside, PlantStage::Growing);
    tick_hours(&mut p, 25 * 24, Weather::Rain);
    assert_eq!(p.water_debt, 0.0);
    assert!(!p.stage.is_wilted());
}
