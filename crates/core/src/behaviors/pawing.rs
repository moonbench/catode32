//! Pawing: the cat walks up to something in the room (cat bed, floor plant,
//! bookshelf, or a corner of the room), faces it, and bats at it with a paw.
//!
//! Also hosts the paw-geometry helpers `PlayingBehavior` uses to paw at toys.

use heapless::Vec;

use crate::{
    assets::{character::PoseId, plants::pot_sprite},
    behavior::{Behavior, BehaviorId, BehaviorState},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    plant_system::PlantLayer,
    rand,
    scene::SceneId,
};

// -- Shared paw helpers --------------------------------------------------

pub const PAWING_POSES: &[PoseId] = &[
    PoseId::PawingSideAngry,
    PoseId::PawingSideNeutral,
    PoseId::PawingSideHappy,
    PoseId::PawingSideHappy2,
];

/// Body frame where the paw is raised and reaching high (`body_paw_3`).
pub const PAW_FRAME_HIGH: usize = 2;
/// Body frame where the paw is reaching out low (`body_paw_4`).
pub const PAW_FRAME_LOW: usize = 3;

/// Horizontal reach of the paw tip, measured from the cat's x in the
/// direction it faces. Generous on both ends so near misses still connect.
pub const PAW_HIT_X_MIN: f32 = 4.0;
pub const PAW_HIT_X_MAX: f32 = 26.0;

pub fn is_pawing_pose(pose: PoseId) -> bool {
    PAWING_POSES.iter().any(|&p| p == pose)
}

/// Pick a pawing pose for the cat's mood. `playful` is true during play
/// sessions, where the cat is having fun rather than being demanding.
pub fn paw_pose(ctx: &mut GameContext, playful: bool) -> PoseId {
    let r = rand::rand_f32(&mut ctx.rng);
    if playful {
        if r < 0.4 {
            PoseId::PawingSideHappy
        } else if r < 0.8 {
            PoseId::PawingSideHappy2
        } else {
            PoseId::PawingSideNeutral
        }
    } else if ctx.mischievousness > 60.0 && r < 0.5 {
        PoseId::PawingSideAngry
    } else if r < 0.75 {
        PoseId::PawingSideNeutral
    } else {
        PoseId::PawingSideHappy
    }
}

/// The body frame the character is currently showing, if it's in a pawing
/// pose. Behaviors update before the new pose is applied each frame, so this
/// reads the pose the character is actually displaying.
pub fn current_paw_frame(character: &Character) -> Option<usize> {
    if is_pawing_pose(character.pose_id) {
        Some(character.anim.body_frame(character.pose_id.data()))
    } else {
        None
    }
}

/// Returns the reach frame the paw just moved into, or None. Fires once per
/// reach frame per swipe (high, then low).
pub fn swipe_edge(prev: Option<usize>, now: Option<usize>) -> Option<usize> {
    match now {
        Some(f) if (f == PAW_FRAME_HIGH || f == PAW_FRAME_LOW) && prev != now => Some(f),
        _ => None,
    }
}

/// Does a paw swipe on `frame` hit a point `ahead` px in front of the cat
/// (positive = the direction it faces) and `height` px above the floor?
pub fn paw_hits(frame: usize, ahead: f32, height: f32) -> bool {
    if !(PAW_HIT_X_MIN..=PAW_HIT_X_MAX).contains(&ahead) {
        return false;
    }
    match frame {
        PAW_FRAME_HIGH => (6.0..=24.0).contains(&height),
        PAW_FRAME_LOW => (0.0..=16.0).contains(&height),
        _ => false,
    }
}

// -- Targets ---------------------------------------------------------------

/// How far from an object's edge the cat stands so the paw reaches it.
const STANDOFF: i32 = 14;
const BED_STANDOFF: i32 = 10;
const CORNER_INSET: i32 = 6;
/// A room edge within this distance of the bed is "the bed", not a corner.
const CORNER_BED_CLEARANCE: i32 = 40;

const INDOOR_SCENES: &[SceneId] = &[SceneId::Inside, SceneId::Bedroom, SceneId::Kitchen];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TargetKind {
    Bed,
    Plant,
    Bookshelf,
    Corner,
    /// Nothing nearby to paw at: bat at the air where the cat stands.
    Air,
}

#[derive(Clone, Copy, Debug)]
struct Target {
    stand_x: i32,
    face_right: bool,
    kind: TargetKind,
}

impl TargetKind {
    fn weight(self) -> f32 {
        match self {
            TargetKind::Bed | TargetKind::Bookshelf => 3.0,
            TargetKind::Plant | TargetKind::Corner => 1.5,
            TargetKind::Air => 0.0,
        }
    }
}

/// Everything in the current room the cat could walk up to and paw at.
/// `from_x` decides which side of an object the cat approaches from.
fn targets(ctx: &GameContext, from_x: i32) -> Vec<Target, 12> {
    let mut out: Vec<Target, 12> = Vec::new();
    let in_bounds = |x: i32| x >= ctx.scene_x_min && x <= ctx.scene_x_max;
    let push = |out: &mut Vec<Target, 12>, stand_x: i32, face_right: bool, kind| {
        if in_bounds(stand_x) {
            let _ = out.push(Target { stand_x, face_right, kind });
        }
    };

    // Approach an object spanning [left, right] from whichever reachable side
    // is nearer the cat.
    let approach = |out: &mut Vec<Target, 12>, left: i32, right: i32, standoff: i32, kind| {
        let lx = left - standoff;
        let rx = right + standoff;
        let l_ok = in_bounds(lx);
        let r_ok = in_bounds(rx);
        let use_left = match (l_ok, r_ok) {
            (true, true) => (from_x - lx).abs() <= (from_x - rx).abs(),
            (true, false) => true,
            (false, true) => false,
            (false, false) => return,
        };
        if use_left {
            push(out, lx, true, kind);
        } else {
            push(out, rx, false, kind);
        }
    };

    if let Some((left, right)) = ctx.cat_bed_span {
        approach(&mut out, left, right, BED_STANDOFF, TargetKind::Bed);
    }

    if let Some(right) = ctx.bookshelf_right_x {
        approach(&mut out, i32::MIN / 2, right, STANDOFF, TargetKind::Bookshelf);
    }

    // Floor plants share the cat's (foreground) layer.
    for plant in ctx.plants.iter() {
        if plant.scene != ctx.last_main_scene
            || plant.layer != PlantLayer::Foreground
            || plant.y_snap < 60
        {
            continue;
        }
        let w = pot_sprite(plant.pot).map(|s| s.width as i32).unwrap_or(12);
        approach(&mut out, plant.x, plant.x + w, STANDOFF, TargetKind::Plant);
    }

    if INDOOR_SCENES.contains(&ctx.last_main_scene) {
        let bed_near = |edge: i32| {
            ctx.cat_bed_span
                .map(|(l, r)| (edge - l).abs() < CORNER_BED_CLEARANCE || (edge - r).abs() < CORNER_BED_CLEARANCE)
                .unwrap_or(false)
        };
        if ctx.bookshelf_right_x.is_none() && !bed_near(ctx.scene_x_min) {
            push(&mut out, ctx.scene_x_min + CORNER_INSET, false, TargetKind::Corner);
        }
        if !bed_near(ctx.scene_x_max) {
            push(&mut out, ctx.scene_x_max - CORNER_INSET, true, TargetKind::Corner);
        }
    }

    out
}

fn pick_target(ctx: &mut GameContext, from_x: i32, facing_right: bool) -> Target {
    let options = targets(ctx, from_x);
    let total: f32 = options.iter().map(|t| t.kind.weight()).sum();
    if total <= 0.0 {
        return Target { stand_x: from_x, face_right: facing_right, kind: TargetKind::Air };
    }
    let mut r = rand::rand_range_f32(&mut ctx.rng, 0.0, total);
    for t in options.iter() {
        r -= t.kind.weight();
        if r <= 0.0 {
            return *t;
        }
    }
    options[options.len() - 1]
}

// -- Behavior --------------------------------------------------------------

const WALK_SPEED: f32 = 14.0;
const APPROACH_TIMEOUT: f32 = 15.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Approaching,
    Pawing,
    Pausing,
    Settling,
}

pub struct PawingBehavior {
    phase: Phase,
    phase_timer: f32,
    pose_id: PoseId,
    target: Target,
    walker_accum: f32,
    bouts_total: u8,
    bouts_done: u8,
    bout_duration: f32,
    pause_duration: f32,
    settle_duration: f32,
}

impl PawingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Approaching,
            phase_timer: 0.0,
            pose_id: PoseId::WalkingSideNeutral,
            target: Target { stand_x: 0, face_right: false, kind: TargetKind::Air },
            walker_accum: 0.0,
            bouts_total: 1,
            bouts_done: 0,
            bout_duration: 2.5,
            pause_duration: 1.0,
            settle_duration: 1.5,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.energy > 25.0
            && (ctx.playfulness >= 35.0 || ctx.mischievousness >= 40.0)
            && !targets(ctx, (ctx.scene_x_min + ctx.scene_x_max) / 2).is_empty()
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let drive = ctx.playfulness.max(ctx.mischievousness);
        let hi = (100.0 - drive).max(15.0);
        rand::rand_range_f32(rng, 15.0, hi).max(0.0) as u32
    }

    fn start_bout(&mut self, ctx: &mut GameContext, character: &mut Character) {
        character.mirror_h = self.target.face_right;
        self.phase = Phase::Pawing;
        self.phase_timer = 0.0;
        self.pose_id = paw_pose(ctx, false);
        self.bout_duration = rand::rand_range_f32(&mut ctx.rng, 2.0, 3.5);
    }
}

impl Behavior for PawingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Pawing
    }

    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Approaching => 0.0,
            Phase::Pawing => {
                let bout = (self.phase_timer / self.bout_duration).clamp(0.0, 1.0);
                (self.bouts_done as f32 + bout) / self.bouts_total.max(1) as f32
            }
            Phase::Pausing => self.bouts_done as f32 / self.bouts_total.max(1) as f32,
            Phase::Settling => 1.0,
        }
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.target = pick_target(ctx, character.pos.x, character.mirror_h);
        self.phase_timer = 0.0;
        self.walker_accum = 0.0;
        self.bouts_done = 0;
        self.bouts_total = rand::rand_range_u32(&mut ctx.rng, 1, 3) as u8;
        self.pause_duration = rand::rand_range_f32(&mut ctx.rng, 0.8, 1.6);
        self.settle_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 2.0);
        println!("Pawing at {:?}", self.target);
        if common::distance_to(character, self.target.stand_x) <= 1 {
            self.start_bout(ctx, character);
        } else {
            self.phase = Phase::Approaching;
            self.pose_id = PoseId::WalkingSideNeutral;
            character.mirror_h = self.target.stand_x > character.pos.x;
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Approaching => {
                let dir = if character.pos.x < self.target.stand_x { 1 } else { -1 };
                let bounced = common::step_walker(
                    character,
                    ctx,
                    dir,
                    WALK_SPEED,
                    dt,
                    &mut self.walker_accum,
                );
                let arrived = common::distance_to(character, self.target.stand_x) <= 1;
                if arrived {
                    character.pos.x = self.target.stand_x;
                }
                if arrived || bounced || self.phase_timer >= APPROACH_TIMEOUT {
                    self.start_bout(ctx, character);
                }
            }
            Phase::Pawing => {
                character.mirror_h = self.target.face_right;
                if self.phase_timer >= self.bout_duration {
                    self.bouts_done += 1;
                    self.phase_timer = 0.0;
                    if self.bouts_done >= self.bouts_total {
                        self.phase = Phase::Settling;
                        self.pose_id = if rand::rand_bool(&mut ctx.rng, 0.5) {
                            PoseId::SittingSillySideNeutral
                        } else {
                            PoseId::SittingSillySideHappy
                        };
                    } else {
                        self.phase = Phase::Pausing;
                        self.pose_id = PoseId::SittingSillySideNeutral;
                    }
                }
            }
            Phase::Pausing => {
                if self.phase_timer >= self.pause_duration {
                    self.start_bout(ctx, character);
                }
            }
            Phase::Settling => {
                if self.phase_timer >= self.settle_duration {
                    return BehaviorState::Completed;
                }
            }
        }
        BehaviorState::Running
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let bonus = [
            (StatId::Playfulness, -0.3 * progress),
            (StatId::Focus, -0.1 * progress),
            (StatId::Curiosity, -0.05 * progress),
            (StatId::Fulfillment, 0.05 * progress),
        ];
        ctx.apply_stat_changes(&bonus);
    }
}
