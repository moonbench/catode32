use crate::{
    assets::character::PoseId,
    behavior::{BehaviorId, NextBehavior},
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    scene::SceneId,
    time_system::Weather,
};

/// Aggregate `delta` into `bonus` under `stat`, summing with any existing entry
/// rather than appending a duplicate. This way the final `apply_stat_changes`
/// sees a single delta per stat. This is important because damping is non-linear
/// and stacking duplicates would drift away from the intended balance.
pub fn bonus_add<const N: usize>(
    bonus: &mut heapless::Vec<(StatId, f32), N>,
    stat: StatId,
    delta: f32,
) {
    if let Some(entry) = bonus.iter_mut().find(|e| e.0 == stat) {
        entry.1 += delta;
    } else {
        let _ = bonus.push((stat, delta));
    }
}

/// Multiply the existing entry for `stat` by `factor`. If no entry exists,
/// the result would be 0, and applying a 0 delta is a no-op, so we just skip.
pub fn bonus_scale<const N: usize>(
    bonus: &mut heapless::Vec<(StatId, f32), N>,
    stat: StatId,
    factor: f32,
) {
    if let Some(entry) = bonus.iter_mut().find(|e| e.0 == stat) {
        entry.1 *= factor;
    }
}

/// Penalty intensity when below the hunger floor. 0 at fullness>=30, 1 at fullness=0.
pub fn hungry_factor(ctx: &GameContext) -> f32 {
    ((30.0 - ctx.fullness) / 30.0).max(0.0)
}

/// Bonus intensity when above the well-fed ceiling. 0 at fullness<=90, 1 at fullness=100.
pub fn fed_factor(ctx: &GameContext) -> f32 {
    ((ctx.fullness - 90.0) / 10.0).max(0.0)
}

/// Shared neutral idle pose pool (also reused by lounging / startled-recovery
/// fall-through).
#[allow(dead_code)]
pub const NEUTRAL_POSES: &[PoseId] = &[
    PoseId::SittingSideNeutral,
    PoseId::SittingSideLookingDown,
    PoseId::SittingForwardNeutral,
    PoseId::SittingForwardSleepy,
    PoseId::SittingForwardContent,
    PoseId::SittingSillySideNeutral,
    PoseId::StandingSideNeutral,
];

#[allow(dead_code)]
pub const HAPPY_POSES: &[PoseId] = &[
    PoseId::SittingSideHappy,
    PoseId::SittingSideAloof,
    PoseId::SittingForwardHappy,
    PoseId::SittingForwardAloof,
    PoseId::SittingSillySideHappy,
    PoseId::SittingSillySideAloof,
];

#[allow(dead_code)]
pub const UPSET_POSES: &[PoseId] = &[
    PoseId::SittingSideAngry,
    PoseId::SittingSideAnnoyed,
    PoseId::SittingSillySideAngry,
    PoseId::SittingSillySideAnnoyed,
];

/// Pick a uniformly random element from a slice, advancing the RNG.
pub fn pick_pose(rng: &mut u32, poses: &[PoseId]) -> PoseId {
    if poses.is_empty() {
        return PoseId::SittingSideNeutral;
    }
    let idx = rand::rand_range_u32(rng, 0, (poses.len() - 1) as u32) as usize;
    poses[idx]
}

/// Step `character.pos.x` along `dir` (+/-1) at `pixels_per_second` over `dt`,
/// clamping to scene bounds. Returns true if the walker bounced into a wall.
pub fn step_walker(
    character: &mut Character,
    ctx: &GameContext,
    dir: i32,
    pixels_per_second: f32,
    dt: f32,
    accum: &mut f32,
) -> bool {
    *accum += pixels_per_second * dt;
    let whole = *accum as i32;
    if whole == 0 {
        return false;
    }
    *accum -= whole as f32;

    let mut nx = character.pos.x + whole * dir.signum();
    let mut bounced = false;
    if nx < ctx.scene_x_min {
        nx = ctx.scene_x_min;
        bounced = true;
    }
    if nx > ctx.scene_x_max {
        nx = ctx.scene_x_max;
        bounced = true;
    }
    character.pos.x = nx;
    // Face the walking direction. `mirror_h = true` flips the natively-left-facing
    // sprite to face right.
    character.mirror_h = dir > 0;
    bounced
}

/// Distance from character.x to a target_x.
pub fn distance_to(character: &Character, target_x: i32) -> i32 {
    (character.pos.x - target_x).abs()
}

/// Scene transition table for auto scene-exit selection.
fn transitions_for(scene: SceneId) -> &'static [SceneId] {
    match scene {
        SceneId::Inside => &[SceneId::Bedroom, SceneId::Kitchen, SceneId::Outside],
        SceneId::Bedroom => &[SceneId::Inside, SceneId::Kitchen],
        SceneId::Kitchen => &[SceneId::Inside, SceneId::Bedroom],
        SceneId::Outside => &[SceneId::Inside, SceneId::Treehouse],
        SceneId::Treehouse => &[SceneId::Outside],
        _ => &[],
    }
}

const SICK_OUTDOOR: &[SceneId] = &[SceneId::Outside, SceneId::Treehouse];

pub fn is_outdoor(scene: SceneId) -> bool {
    SICK_OUTDOOR.iter().any(|s| *s == scene)
}

/// Caught outdoors in rain/snow/storm: very likely to head indoors right away.
/// Ignores the low-energy gate (a tired cat still seeks shelter) and is
/// guaranteed once the pet is already sick.
pub fn shelter_exit(ctx: &mut GameContext) -> Option<NextBehavior> {
    if ctx.on_vacation || !is_outdoor(ctx.last_main_scene) {
        return None;
    }
    let p = match ctx.weather {
        Weather::Storm => 0.9,
        Weather::Rain | Weather::Snow => 0.7,
        _ => return None,
    };
    let p = if ctx.sickness >= 2.0 { 1.0 } else { p };
    let roll = rand::rand_f32(&mut ctx.rng);
    println!("Shelter p={:.2} roll={:.3} ({:?})", p, roll, ctx.last_main_scene);
    if roll > p {
        return None;
    }
    // Treehouse only connects to Outside; from there the next roll goes Inside.
    let dest = if ctx.last_main_scene == SceneId::Treehouse {
        SceneId::Outside
    } else {
        SceneId::Inside
    };
    println!("\x1b[32mSeeking shelter -> {:?}\x1b[0m", dest);
    Some(NextBehavior::GoTo(crate::behavior::GoToParams {
        target_x: ctx.scene_x_min,
        speed: 12.0,
        pending_scene: Some(dest),
        then: None,
    }))
}

pub fn auto_select_scene_exit(ctx: &mut GameContext) -> Option<NextBehavior> {
    // Pet stays put on vacation. The player explicitly chooses "Go home".
    if ctx.on_vacation {
        return None;
    }
    let current = ctx.last_main_scene;
    let options = transitions_for(current);
    if options.is_empty() {
        return None;
    }
    if ctx.energy < 15.0 {
        return None;
    }

    let outdoor_bad = matches!(ctx.weather, Weather::Rain | Weather::Storm | Weather::Snow);
    let temp_extreme = ctx.temperature < -1.0 || ctx.temperature > 33.0;
    let currently_outdoor = is_outdoor(current);

    // Per-destination weights.
    let mut weights: heapless::Vec<f32, 4> = heapless::Vec::new();
    for &dest in options {
        let mut w = 1.0_f32;
        if dest == SceneId::Kitchen {
            w += (50.0 - ctx.fullness).max(0.0) * 0.04;
        }
        if dest == SceneId::Bedroom {
            w += (50.0 - ctx.comfort).max(0.0) * 0.02;
            w += (50.0 - ctx.energy).max(0.0) * 0.02;
        }
        if matches!(dest, SceneId::Outside | SceneId::Treehouse) && outdoor_bad {
            w *= 0.2;
        }
        if matches!(dest, SceneId::Outside | SceneId::Treehouse) && temp_extreme {
            w *= 0.2;
        }
        if currently_outdoor && outdoor_bad && dest == SceneId::Inside {
            w *= 3.0;
        }
        if currently_outdoor && temp_extreme && dest == SceneId::Inside {
            w *= 2.5;
        }
        let _ = weights.push(w);
    }

    let max_w = weights.iter().copied().fold(0.0_f32, f32::max);
    let mut p = (0.08 + (max_w - 1.0) * 0.05).min(0.4);
    if currently_outdoor && outdoor_bad {
        let floor = if ctx.weather == Weather::Storm { 0.70 } else { 0.45 };
        p = p.max(floor);
    }
    if currently_outdoor && temp_extreme {
        p = p.max(0.35);
    }

    let roll = rand::rand_f32(&mut ctx.rng);
    println!(
        "Scene exit p={:.3} roll={:.3} ({:?}) weights: {:?}",
        p,
        roll,
        current,
        &weights[..]
    );
    if roll > p {
        return None;
    }

    let total: f32 = weights.iter().sum();
    let mut r = rand::rand_range_f32(&mut ctx.rng, 0.0, total);
    let mut chosen = options[options.len() - 1];
    for (i, &w) in weights.iter().enumerate() {
        r -= w;
        if r <= 0.0 {
            chosen = options[i];
            break;
        }
    }

    Some(NextBehavior::GoTo(crate::behavior::GoToParams {
        target_x: ctx.scene_x_min,
        speed: 12.0,
        pending_scene: Some(chosen),
        then: None,
    }))
}

/// Sickness-tier auto-select blocking.
pub fn sick_blocks(id: BehaviorId, ctx: &GameContext) -> bool {
    if ctx.sickness < 2.0 {
        return false;
    }
    let mild = matches!(
        id,
        BehaviorId::Zoomies
            | BehaviorId::Mischief
            | BehaviorId::Hunting
            | BehaviorId::Investigating
            | BehaviorId::Observing
            | BehaviorId::Playing
    );
    if mild {
        return true;
    }
    if ctx.sickness >= 5.0 {
        let clear = matches!(
            id,
            BehaviorId::Lounging
                | BehaviorId::SelfGrooming
                | BehaviorId::Pacing
                | BehaviorId::Vocalizing
                | BehaviorId::Hiding
        );
        if clear {
            return true;
        }
    }
    false
}

/// Used by base.apply_location_bonus for the favourite-weather bonus.
pub fn fav_weather_bonus(ctx: &GameContext) -> (f32, f32) {
    use crate::context::FavWeather;
    if let Some(fav) = ctx.fav_weather {
        let matches = match (fav, ctx.weather) {
            (FavWeather::Sunny, Weather::Clear | Weather::Windy) => true,
            (FavWeather::Rainy, Weather::Rain | Weather::Storm) => true,
            (FavWeather::Snowy, Weather::Snow) => true,
            (FavWeather::Overcast, Weather::Cloudy | Weather::Overcast) => true,
            _ => false,
        };
        if matches {
            return (0.8, 0.3 * serenity_wellbeing_factor(ctx));
        }
    }
    (0.0, 0.0)
}

/// 0..1 multiplier for serenity gains based on how well the pet's needs are met.
pub fn serenity_wellbeing_factor(ctx: &GameContext) -> f32 {
    let mut score = 0u8;
    if ctx.fullness >= 60.0 {
        score += 1;
    }
    if ctx.energy >= 60.0 {
        score += 1;
    }
    if ctx.cleanliness >= 60.0 {
        score += 1;
    }
    if ctx.affection >= 50.0 {
        score += 1;
    }
    score as f32 / 4.0
}
