//! Plant growth state machine + tick logic.
//!
//! Plants live in `ctx.plants`. Ticking is global: `tick_plants(ctx)` runs
//! once per in-game hour from `LocationScene::update` and advances every
//! plant, not just the ones in the current scene.

use crate::{
    assets::plants::{PlantStage, PotKind},
    context::{GameContext, SeedKind},
    rand,
    scene::SceneId,
    time_system::{Season, Weather},
};

/// Per-type thresholds (all in in-game hours). Time scale: 1 in-game hour =
/// 15 real minutes, so 1 real day = 96 in-game hours. Targets: wilt after
/// ~1-2 real days unwatered, advance a stage every ~2-5 real days.
#[derive(Clone, Copy)]
pub struct PlantTypeSpec {
    pub wilt: u32,
    pub death: u32,
    pub recover: u32,
    /// Cumulative seedling -> young -> growing -> mature -> thriving.
    pub stage_hours: [u32; 4],
    pub water_rate: f32,
    pub dormant_in_winter: bool,
    pub indoor_max: Option<PlantStage>,
    /// Annual lifespan (sunflower only). Zero = no lifespan.
    pub max_age_hours: u32,
    pub max_age_death_window: u32,
}

pub const fn spec_for(seed: SeedKind) -> PlantTypeSpec {
    match seed {
        SeedKind::CatGrass => PlantTypeSpec {
            wilt: 96,
            death: 288,
            recover: 16,
            stage_hours: [192, 240, 288, 336],
            water_rate: 0.5,
            dormant_in_winter: false,
            indoor_max: None,
            max_age_hours: 0,
            max_age_death_window: 0,
        },
        SeedKind::Freesia => PlantTypeSpec {
            wilt: 96,
            death: 288,
            recover: 8,
            stage_hours: [288, 336, 432, 480],
            water_rate: 0.75,
            dormant_in_winter: true,
            indoor_max: None,
            max_age_hours: 0,
            max_age_death_window: 0,
        },
        SeedKind::Rose => PlantTypeSpec {
            wilt: 96,
            death: 288,
            recover: 8,
            stage_hours: [336, 432, 480, 480],
            water_rate: 1.0,
            dormant_in_winter: false,
            indoor_max: None,
            max_age_hours: 0,
            max_age_death_window: 0,
        },
        SeedKind::Sunflower => PlantTypeSpec {
            wilt: 192,
            death: 384,
            recover: 16,
            stage_hours: [192, 288, 384, 480],
            water_rate: 1.0,
            dormant_in_winter: false,
            indoor_max: Some(PlantStage::Growing),
            max_age_hours: 1824,
            max_age_death_window: 192,
        },
    }
}

/// Water debt multiplier per growth stage. Wilted plants drain at a flat
/// 1.0/hr regardless of this table.
fn stage_water_mult(stage: PlantStage) -> f32 {
    match stage.base() {
        PlantStage::Seedling => 0.4,
        PlantStage::Young => 0.6,
        PlantStage::Growing => 0.8,
        PlantStage::Mature => 0.9,
        PlantStage::Thriving => 1.0,
        _ => 1.0,
    }
}

/// Maximum growth stage permitted per pot type.
fn pot_cap(pot: PotKind) -> PlantStage {
    match pot {
        PotKind::Small => PlantStage::Young,
        PotKind::Medium => PlantStage::Growing,
        PotKind::Large => PlantStage::Mature,
        PotKind::Planter => PlantStage::Mature,
        PotKind::Ground => PlantStage::Thriving,
    }
}

fn stage_index(stage: PlantStage) -> Option<usize> {
    Some(match stage.base() {
        PlantStage::Seedling => 0,
        PlantStage::Young => 1,
        PlantStage::Growing => 2,
        PlantStage::Mature => 3,
        PlantStage::Thriving => 4,
        _ => return None,
    })
}

fn stage_from_index(i: usize) -> PlantStage {
    match i {
        0 => PlantStage::Seedling,
        1 => PlantStage::Young,
        2 => PlantStage::Growing,
        3 => PlantStage::Mature,
        _ => PlantStage::Thriving,
    }
}

// Fertilizer thresholds.
const FERT_DECAY: f32 = 0.225;
const FERT_NO_MAX: f32 = 5.0;
const FERT_LOW_MAX: f32 = 20.0;
const FERT_OK_MAX: f32 = 120.0;
const FERT_ADD: f32 = 100.0;
const FERT_CAP: f32 = 200.0;

fn fert_ok(fert: f32) -> bool {
    fert > FERT_LOW_MAX && fert <= FERT_OK_MAX
}

pub fn fert_label(fert: f32) -> &'static str {
    if fert <= FERT_NO_MAX {
        t!("Fert: No")
    } else if fert <= FERT_LOW_MAX {
        t!("Fert: Low")
    } else if fert <= FERT_OK_MAX {
        t!("Fert: OK")
    } else {
        t!("Fert: Over")
    }
}

// --- Weather modifier for outdoor plants (net effect after the standard +1) ---
const RAIN_DEBT_DELTA: f32 = -11.0; // net -10/h
const STORM_DEBT_DELTA: f32 = -21.0; // net -20/h

// ---------------------------------------------------------------------------
// Plant data structure
// ---------------------------------------------------------------------------

/// A single plant entry. `seed` is `None` for empty pots; once a seed is
/// planted it stays set for the rest of the plant's life.
#[derive(Clone, Copy)]
pub struct Plant {
    pub id: u32,
    pub seed: Option<SeedKind>,
    pub scene: SceneId,
    pub layer: PlantLayer,
    pub x: i32,
    pub y_snap: i32,
    pub pot: PotKind,
    pub stage: PlantStage,
    pub age_hours: u32,
    pub water_debt: f32,
    pub fertilizer: f32,
    pub planted_day: Option<u32>,
    pub mirror: bool,
    /// Set when the plant has entered end-of-life wilt (sunflower annual).
    /// Watering won't revive it; it dies after `max_age_death_window` hours.
    pub aged: bool,
}

/// Layer hint stored on each plant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlantLayer {
    Background,
    Midground,
    Foreground,
}

impl PlantLayer {
    pub fn to_env(self) -> crate::environment::Layer {
        match self {
            PlantLayer::Background => crate::environment::Layer::Background,
            PlantLayer::Midground => crate::environment::Layer::Midground,
            PlantLayer::Foreground => crate::environment::Layer::Foreground,
        }
    }

    pub fn parallax(self) -> f32 {
        crate::environment::PARALLAX[self.to_env() as usize]
    }
}

// ---------------------------------------------------------------------------
// Internal advance / wilt / death logic
// ---------------------------------------------------------------------------

fn can_advance(plant: &Plant, spec: &PlantTypeSpec) -> bool {
    let cur_idx = match stage_index(plant.stage) {
        Some(i) if i + 1 < 5 => i,
        _ => return false,
    };
    let next_idx = cur_idx + 1;
    let next_stage = stage_from_index(next_idx);

    let cap_idx = stage_index(pot_cap(plant.pot)).unwrap_or(0);
    if next_idx > cap_idx {
        return false;
    }
    if let Some(indoor_max) = spec.indoor_max {
        if plant.scene != SceneId::Outside {
            let im = stage_index(indoor_max).unwrap_or(0);
            if next_idx > im {
                return false;
            }
        }
    }
    if next_stage == PlantStage::Thriving && !fert_ok(plant.fertilizer) {
        return false;
    }
    true
}

fn maturation_threshold(spec: &PlantTypeSpec, stage_idx: usize) -> u32 {
    let mut total = 0u32;
    for i in 0..=(stage_idx.min(spec.stage_hours.len() - 1)) {
        total = total.saturating_add(spec.stage_hours[i]);
    }
    total
}

/// Advance one in-game hour for a single plant. Mirrors `tick_plant`.
pub fn tick_plant(plant: &mut Plant, season: Season, weather: Weather) {
    // Inert stages skip ticking entirely.
    let stage = plant.stage;
    if stage == PlantStage::EmptyPot || stage.is_dead() {
        return;
    }
    let seed = match plant.seed {
        Some(s) => s,
        None => return,
    };
    let spec = spec_for(seed);

    // --- Outdoor winter handling ---
    let outside = plant.scene == SceneId::Outside;
    if outside && season == Season::Winter {
        if spec.dormant_in_winter
            && stage != PlantStage::Dormant
            && stage != PlantStage::EmptyPot
            && !stage.is_dead()
        {
            plant.stage = PlantStage::Dormant;
        }
        // All outdoor plants pause in winter.
        return;
    }

    // --- Dormant freesia waking ---
    if stage == PlantStage::Dormant {
        if season != Season::Winter {
            plant.stage = PlantStage::Seedling;
            plant.water_debt = 0.0;
        }
        return;
    }

    // --- Hour accumulation ---
    plant.age_hours = plant.age_hours.saturating_add(1);
    let debt_inc = if stage.is_wilted() {
        1.0
    } else {
        spec.water_rate * stage_water_mult(stage)
    };
    plant.water_debt += debt_inc;
    plant.fertilizer = (plant.fertilizer - FERT_DECAY).max(0.0);

    // --- Rain / storm watering for outdoor plants ---
    if outside {
        match weather {
            Weather::Rain => plant.water_debt += RAIN_DEBT_DELTA,
            Weather::Storm => plant.water_debt += STORM_DEBT_DELTA,
            _ => {}
        }
        if plant.water_debt < 0.0 {
            plant.water_debt = 0.0;
        }
    }

    // --- Annual lifecycle (sunflower) ---
    if spec.max_age_hours > 0 {
        let age = plant.age_hours;
        if age >= spec.max_age_hours + spec.max_age_death_window {
            // Use the wilted stage's dead variant so the corpse art reflects
            // what the plant looked like before it expired.
            plant.stage = stage.dead_variant();
            return;
        }
        if age >= spec.max_age_hours {
            if !stage.is_wilted() {
                plant.stage = stage.wilted_variant();
                plant.aged = true;
            }
            return;
        }
    }

    let debt = plant.water_debt;

    if stage.is_wilted() {
        // Player watered -> debt low -> recover.
        if debt <= spec.recover as f32 {
            plant.stage = stage.base();
            return;
        }
        if debt > spec.death as f32 {
            plant.stage = stage.dead_variant();
            return;
        }
    } else {
        // Thriving knockback: if fertilizer left the OK window, drop to mature.
        if stage == PlantStage::Thriving && !fert_ok(plant.fertilizer) {
            plant.stage = PlantStage::Mature;
            return;
        }
        if debt > spec.wilt as f32 {
            plant.stage = stage.wilted_variant();
            return;
        }
        if let Some(idx) = stage_index(stage) {
            if can_advance(plant, &spec) {
                let threshold = maturation_threshold(&spec, idx);
                if plant.age_hours >= threshold {
                    plant.stage = stage_from_index(idx + 1);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Global tick, called from LocationScene::update
// ---------------------------------------------------------------------------

/// Advance every plant by however many in-game hours have elapsed since the
/// last call. Caps at 24h to avoid runaway ticks after a long pause.
pub fn tick_plants(ctx: &mut GameContext) {
    let current_hour = ctx.day_number.saturating_mul(24) + ctx.time_hours as u32;
    let last = match ctx.last_plant_tick_hour {
        Some(h) => h,
        None => {
            ctx.last_plant_tick_hour = Some(current_hour);
            return;
        }
    };
    if current_hour <= last {
        return;
    }
    let elapsed = (current_hour - last).min(24);
    let season = ctx.season;
    let weather = ctx.weather;
    for _ in 0..elapsed {
        for plant in ctx.plants.iter_mut() {
            tick_plant(plant, season, weather);
        }
    }
    ctx.last_plant_tick_hour = Some(current_hour);
}

// ---------------------------------------------------------------------------
// Player actions
// ---------------------------------------------------------------------------

/// Reset water debt after the player waters a plant.
pub fn water_plant(plant: &mut Plant) {
    plant.water_debt = 0.0;
}

/// Apply one unit of fertilizer (adds 100, capped at 200).
pub fn fertilize_plant(plant: &mut Plant) {
    plant.fertilizer = (plant.fertilizer + FERT_ADD).min(FERT_CAP);
}

/// Plant a seed into an existing empty pot. Returns true on success.
pub fn plant_seed_into_pot(ctx: &mut GameContext, pot_id: u32, seed: SeedKind) -> bool {
    if ctx.seeds[seed as usize] == 0 {
        return false;
    }
    let day = ctx.day_number;
    let mirror = rand::rand_bool(&mut ctx.rng, 0.5);
    for plant in ctx.plants.iter_mut() {
        if plant.id == pot_id && plant.stage == PlantStage::EmptyPot {
            plant.seed = Some(seed);
            plant.stage = PlantStage::Seedling;
            plant.age_hours = 0;
            plant.water_debt = 0.0;
            plant.fertilizer = 0.0;
            plant.planted_day = Some(day);
            plant.mirror = mirror;
            plant.aged = false;
            ctx.seeds[seed as usize] -= 1;
            return true;
        }
    }
    false
}

/// Place an empty pot. Returns the new plant id, or None if no pot in inventory
/// or the plant list is full.
pub fn place_empty_pot(
    ctx: &mut GameContext,
    scene: SceneId,
    layer: PlantLayer,
    x: i32,
    y_snap: i32,
    pot: PotKind,
) -> Option<u32> {
    let pot_slot = match pot {
        PotKind::Small => 0,
        PotKind::Medium => 1,
        PotKind::Large => 2,
        PotKind::Planter => 3,
        PotKind::Ground => return None,
    };
    if ctx.pots[pot_slot] == 0 {
        return None;
    }
    let mirror = rand::rand_bool(&mut ctx.rng, 0.5);
    let id = ctx.next_plant_id;
    let plant = Plant {
        id,
        seed: None,
        scene,
        layer,
        x,
        y_snap,
        pot,
        stage: PlantStage::EmptyPot,
        age_hours: 0,
        water_debt: 0.0,
        fertilizer: 0.0,
        planted_day: None,
        mirror,
        aged: false,
    };
    if ctx.plants.push(plant).is_err() {
        return None;
    }
    ctx.next_plant_id = ctx.next_plant_id.wrapping_add(1);
    ctx.pots[pot_slot] -= 1;
    Some(id)
}

/// Plant a seed directly in the ground (no pot).
pub fn plant_in_ground(
    ctx: &mut GameContext,
    scene: SceneId,
    layer: PlantLayer,
    x: i32,
    y_snap: i32,
    seed: SeedKind,
) -> Option<u32> {
    if ctx.seeds[seed as usize] == 0 {
        return None;
    }
    let mirror = rand::rand_bool(&mut ctx.rng, 0.5);
    let id = ctx.next_plant_id;
    let plant = Plant {
        id,
        seed: Some(seed),
        scene,
        layer,
        x,
        y_snap,
        pot: PotKind::Ground,
        stage: PlantStage::Seedling,
        age_hours: 0,
        water_debt: 0.0,
        fertilizer: 0.0,
        planted_day: Some(ctx.day_number),
        mirror,
        aged: false,
    };
    if ctx.plants.push(plant).is_err() {
        return None;
    }
    ctx.next_plant_id = ctx.next_plant_id.wrapping_add(1);
    ctx.seeds[seed as usize] -= 1;
    Some(id)
}

/// Pluck a plant. Ground plants are removed entirely; potted plants (live or
/// dead) are cleared back to an empty pot left in place. Returns false if the
/// plant isn't found or is already an empty pot.
pub fn pluck_plant(ctx: &mut GameContext, id: u32) -> bool {
    let Some(idx) = ctx.plants.iter().position(|p| p.id == id) else {
        return false;
    };
    let plant = &mut ctx.plants[idx];
    if plant.stage == PlantStage::EmptyPot {
        return false;
    }
    if plant.pot == PotKind::Ground {
        ctx.plants.swap_remove(idx);
        return true;
    }
    plant.seed = None;
    plant.stage = PlantStage::EmptyPot;
    plant.age_hours = 0;
    plant.water_debt = 0.0;
    plant.fertilizer = 0.0;
    plant.planted_day = None;
    plant.aged = false;
    true
}

/// Pick up an empty pot from the scene and return it to inventory. Returns
/// false if the plant isn't found or isn't an empty pot.
pub fn stow_empty_pot(ctx: &mut GameContext, id: u32) -> bool {
    let Some(idx) = ctx.plants.iter().position(|p| p.id == id) else {
        return false;
    };
    let p = ctx.plants[idx];
    if p.stage != PlantStage::EmptyPot {
        return false;
    }
    let Some(slot) = pot_slot(p.pot) else {
        return false;
    };
    ctx.pots[slot] = ctx.pots[slot].saturating_add(1);
    ctx.plants.swap_remove(idx);
    true
}

fn pot_slot(pot: PotKind) -> Option<usize> {
    match pot {
        PotKind::Small => Some(0),
        PotKind::Medium => Some(1),
        PotKind::Large => Some(2),
        PotKind::Planter => Some(3),
        PotKind::Ground => None,
    }
}

/// Move a plant to a new scene/layer/x/y. Health and stage are preserved.
pub fn move_plant(
    ctx: &mut GameContext,
    id: u32,
    scene: SceneId,
    layer: PlantLayer,
    x: i32,
    y_snap: i32,
) -> bool {
    for p in ctx.plants.iter_mut() {
        if p.id == id {
            p.scene = scene;
            p.layer = layer;
            p.x = x;
            p.y_snap = y_snap;
            return true;
        }
    }
    false
}

fn pot_rank(p: PotKind) -> i32 {
    match p {
        PotKind::Small => 0,
        PotKind::Medium => 1,
        PotKind::Large => 2,
        PotKind::Planter => 3,
        PotKind::Ground => 4,
    }
}

/// Repot into a larger pot from inventory. Returns false if not an upgrade,
/// not in inventory, or the target/current pot is `ground`.
pub fn repot_plant(ctx: &mut GameContext, id: u32, target: PotKind) -> bool {
    let idx = match ctx.plants.iter().position(|p| p.id == id) {
        Some(i) => i,
        None => return false,
    };
    let current = ctx.plants[idx].pot;
    if current == PotKind::Ground || target == PotKind::Ground {
        return false;
    }
    if pot_rank(target) <= pot_rank(current) {
        return false;
    }
    let target_slot = match pot_slot(target) {
        Some(s) => s,
        None => return false,
    };
    if ctx.pots[target_slot] == 0 {
        return false;
    }
    ctx.pots[target_slot] -= 1;
    if let Some(cur_slot) = pot_slot(current) {
        ctx.pots[cur_slot] = ctx.pots[cur_slot].saturating_add(1);
    }
    ctx.plants[idx].pot = target;
    true
}

// ---------------------------------------------------------------------------
// Stat helpers (used by behavior bonuses and scene_plant_health)
// ---------------------------------------------------------------------------

pub fn count_healthy_plants(ctx: &GameContext, scene: SceneId) -> usize {
    ctx.plants
        .iter()
        .filter(|p| p.scene == scene)
        .filter(|p| {
            !matches!(
                p.stage,
                PlantStage::EmptyPot | PlantStage::Dead | PlantStage::Dormant
            ) && !p.stage.is_wilted()
        })
        .count()
}

pub fn count_dead_plants(ctx: &GameContext, scene: SceneId) -> usize {
    ctx.plants
        .iter()
        .filter(|p| p.scene == scene && p.stage.is_dead())
        .count()
}

/// Aggregate score used by behavior bonuses: thriving +2, healthy +1,
/// wilted/dormant -1, dead -2 (per plant in the scene).
pub fn scene_plant_health_score(ctx: &GameContext, scene: SceneId) -> i32 {
    let mut score = 0i32;
    for p in ctx.plants.iter() {
        if p.scene != scene {
            continue;
        }
        match p.stage {
            PlantStage::Thriving => score += 2,
            PlantStage::EmptyPot => {}
            stage if stage.is_dead() => score -= 2,
            stage if stage.is_wilted() => score -= 1,
            _ => score += 1,
        }
    }
    score
}

// ---------------------------------------------------------------------------
// Inspect helpers
// ---------------------------------------------------------------------------

use crate::t;
use heapless::String;

pub const INSPECT_LINE_LEN: usize = 16;
pub const INSPECT_MAX_LINES: usize = 5;
pub type InspectLines = heapless::Vec<String<INSPECT_LINE_LEN>, INSPECT_MAX_LINES>;

fn seed_label(seed: SeedKind) -> &'static str {
    match seed {
        SeedKind::CatGrass => t!("Cat Grass"),
        SeedKind::Freesia => t!("Freesia"),
        SeedKind::Rose => t!("Rose"),
        SeedKind::Sunflower => t!("Sunflower"),
    }
}

fn pot_inspect_label(pot: PotKind) -> &'static str {
    pot.label()
}

fn push_line(lines: &mut InspectLines, s: &str) {
    let mut line: String<INSPECT_LINE_LEN> = String::new();
    let _ = line.push_str(&s[..s.len().min(INSPECT_LINE_LEN)]);
    let _ = lines.push(line);
}

fn push_prefixed(lines: &mut InspectLines, prefix: &str, value: &str) {
    let mut line: String<INSPECT_LINE_LEN> = String::new();
    let _ = line.push_str(prefix);
    let remaining = INSPECT_LINE_LEN - line.len();
    let _ = line.push_str(&value[..value.len().min(remaining)]);
    let _ = lines.push(line);
}

/// Build the lines shown by the Inspect submenu. Each line is at most 16 chars.
pub fn inspect_lines(plant: &Plant) -> InspectLines {
    let mut out: InspectLines = heapless::Vec::new();

    if plant.stage == PlantStage::EmptyPot || plant.seed.is_none() {
        push_line(&mut out, t!("Empty pot"));
        push_prefixed(&mut out, t!("Pot: "), pot_inspect_label(plant.pot));
        return out;
    }

    let seed = plant.seed.unwrap();
    push_line(&mut out, seed_label(seed));
    push_prefixed(&mut out, t!("Pot: "), pot_inspect_label(plant.pot));

    if plant.stage.is_dead() {
        push_line(&mut out, t!("Stage: Dead"));
        return out;
    }
    if plant.stage == PlantStage::Dormant {
        push_line(&mut out, t!("Stage: Dormant"));
        return out;
    }

    let spec = spec_for(seed);
    let debt = plant.water_debt;
    let aged = plant.aged;
    let wilted = plant.stage.is_wilted();
    let recovering = wilted && !aged && debt <= spec.recover as f32;

    if recovering {
        push_line(&mut out, t!("Stage: Recovering"));
        return out;
    }

    push_prefixed(&mut out, t!("Stage: "), plant.stage.base().label());

    let status = if aged {
        t!("Natural lifespan")
    } else if wilted {
        let remaining = spec.death as f32 - debt;
        if remaining <= (spec.death / 4) as f32 {
            t!("Water: Critical")
        } else {
            t!("Water: Dry!")
        }
    } else {
        let wilt = spec.wilt as f32;
        if debt == 0.0 {
            t!("Water: Full")
        } else if debt < wilt / 3.0 {
            t!("Water: OK")
        } else if debt < wilt * 2.0 / 3.0 {
            t!("Water: Low")
        } else {
            t!("Water: Urgent")
        }
    };
    push_line(&mut out, status);

    push_prefixed(&mut out, t!("Fert: "), fert_label(plant.fertilizer));
    out
}

/// Lookup helper used by menu action handlers.
pub fn get_plant_mut(ctx: &mut GameContext, id: u32) -> Option<&mut Plant> {
    ctx.plants.iter_mut().find(|p| p.id == id)
}

pub fn get_plant(ctx: &GameContext, id: u32) -> Option<&Plant> {
    ctx.plants.iter().find(|p| p.id == id)
}
