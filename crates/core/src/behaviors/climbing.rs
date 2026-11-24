//! Hopping up onto furniture and back down.
//!
//! A scene lists its raised spots in `ctx.perches`. Before a settled
//! behavior (sleeping, grooming, ...) the manager sometimes sends the cat up
//! one first; before anything that moves around, it brings the cat back
//! down. While the cat is up there `ctx.perch` is set and the location scene
//! pins it to the furniture every frame, so it stays put as the layers
//! scroll at their own parallax speeds.

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState},
    behaviors::common,
    context::GameContext,
    entities::character::Character,
    environment::{parallax_offset, Layer},
    rand,
};

/// A raised spot the cat can sit on.
#[derive(Clone, Copy)]
pub struct Perch {
    /// The layer the furniture is drawn in.
    pub layer: Layer,
    /// Where the cat may sit, in that layer's world x.
    pub x_min: i32,
    pub x_max: i32,
    /// The cat's `pos.y` when sitting on top.
    pub top_y: i32,
}

impl Perch {
    /// The foreground world-x that's on screen at the same spot as `x` on
    /// this perch's layer.
    pub fn fg_x(&self, x: i32, camera_x: i32) -> i32 {
        x - parallax_offset(camera_x, self.layer) + camera_x
    }
}

/// Where the cat is sitting while up on a perch.
#[derive(Clone, Copy)]
pub struct Perched {
    pub idx: u8,
    /// In the perch's layer.
    pub x: i32,
    /// The cat's `pos.y` back on the floor.
    pub floor_y: i32,
}

/// Behaviors that keep still enough to happen up on the furniture. Anything
/// else hops down first.
pub fn perch_safe(id: BehaviorId) -> bool {
    matches!(
        id,
        BehaviorId::Idle
            | BehaviorId::Sleeping
            | BehaviorId::Napping
            | BehaviorId::Stretching
            | BehaviorId::Kneading
            | BehaviorId::Lounging
            | BehaviorId::Observing
            | BehaviorId::Chattering
            | BehaviorId::Vocalizing
            | BehaviorId::SelfGrooming
            | BehaviorId::BeingGroomed
            | BehaviorId::Sulking
            | BehaviorId::Affection
            | BehaviorId::Attention
            | BehaviorId::Startled
            | BehaviorId::Hearing
            | BehaviorId::Training
            | BehaviorId::Climbing
    )
}

/// Chance the cat hops up somewhere before starting `id`.
pub fn climb_chance(id: BehaviorId) -> f32 {
    match id {
        BehaviorId::Sleeping | BehaviorId::Napping => 0.5,
        BehaviorId::Lounging | BehaviorId::Kneading => 0.4,
        BehaviorId::SelfGrooming => 0.35,
        BehaviorId::Observing => 0.25,
        BehaviorId::Idle => 0.15,
        _ => 0.0,
    }
}

const WALK_SPEED: f32 = 16.0;
/// How far short of the spot the cat stops before hopping up, and how far
/// out it lands hopping down.
const HOP_DX: i32 = 12;
const CROUCH_TIME: f32 = 0.4;
const JUMP_TIME: f32 = 0.45;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Walk,
    Crouch,
    Jump,
    Done,
}

pub struct ClimbingBehavior {
    up: bool,
    phase: Phase,
    t: f32,
    pose_id: PoseId,
    walker_accum: f32,
    /// Going up: the perch and the spot on it.
    target: (u8, i32),
    floor_y: i32,
    from: (i32, i32),
    /// Going down: where the cat lands, in foreground world-x.
    land_x: i32,
}

impl ClimbingBehavior {
    pub fn new(up: bool) -> Self {
        Self {
            up,
            phase: Phase::Done,
            t: 0.0,
            pose_id: PoseId::SittingSideNeutral,
            walker_accum: 0.0,
            target: (0, 0),
            floor_y: 0,
            from: (0, 0),
            land_x: 0,
        }
    }

    fn perched(&self) -> Perched {
        Perched { idx: self.target.0, x: self.target.1, floor_y: self.floor_y }
    }
}

impl Behavior for ClimbingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Climbing
    }

    fn progress(&self) -> f32 {
        0.5
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.t = 0.0;
        if self.up {
            if ctx.perch.is_some() || ctx.perches.is_empty() {
                return;
            }
            let idx = rand::rand_range_u32(&mut ctx.rng, 0, ctx.perches.len() as u32 - 1) as usize;
            let p = ctx.perches[idx];
            let x = p.x_min + rand::rand_range_u32(&mut ctx.rng, 0, (p.x_max - p.x_min) as u32) as i32;
            self.target = (idx as u8, x);
            self.floor_y = character.pos.y;
            self.phase = Phase::Walk;
            self.pose_id = PoseId::WalkingSideNeutral;
        } else if let Some(at) = ctx.perch {
            self.target = (at.idx, at.x);
            self.floor_y = at.floor_y;
            // Hop down the way the cat is facing, unless that's into a wall.
            let x = character.pos.x;
            let dir = if character.mirror_h { 1 } else { -1 };
            let land = |d: i32| (x + d * HOP_DX).clamp(ctx.scene_x_min, ctx.scene_x_max);
            self.land_x = land(dir);
            if (self.land_x - x).abs() < HOP_DX / 2 {
                self.land_x = land(-dir);
            }
            self.phase = Phase::Crouch;
            self.pose_id = PoseId::LeaningForwardSideNeutral;
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        let cam = ctx.scene_camera_x;
        let spot_x = |ctx: &GameContext, target: (u8, i32)| {
            ctx.perches.get(target.0 as usize).map(|p| p.fg_x(target.1, cam))
        };
        match self.phase {
            Phase::Walk => {
                let Some(fx) = spot_x(ctx, self.target) else {
                    return BehaviorState::Completed;
                };
                // Stop a hop short, on whichever side the cat is coming from.
                let side = if character.pos.x <= fx { -1 } else { 1 };
                let goal = (fx + side * HOP_DX).clamp(ctx.scene_x_min, ctx.scene_x_max);
                if (character.pos.x - goal).abs() <= 1 {
                    character.pos.x = goal;
                    if fx != goal {
                        character.mirror_h = fx > goal;
                    }
                    self.phase = Phase::Crouch;
                    self.t = 0.0;
                    self.pose_id = PoseId::LeaningForwardSidePounce;
                } else {
                    let dir = if goal > character.pos.x { 1 } else { -1 };
                    common::step_walker(character, ctx, dir, WALK_SPEED, dt, &mut self.walker_accum);
                    self.pose_id = PoseId::WalkingSideNeutral;
                }
            }
            Phase::Crouch => {
                self.t += dt;
                if self.t >= CROUCH_TIME {
                    self.phase = Phase::Jump;
                    self.t = 0.0;
                    self.from = (character.pos.x, character.pos.y);
                    self.pose_id = PoseId::RunningSideNeutral;
                    if !self.up {
                        ctx.perch = None;
                        character.mirror_h = self.land_x > character.pos.x;
                    }
                }
            }
            Phase::Jump => {
                self.t += dt;
                let s = (self.t / JUMP_TIME).min(1.0);
                let (tx, ty) = if self.up {
                    let Some(fx) = spot_x(ctx, self.target) else {
                        return BehaviorState::Completed;
                    };
                    (fx, ctx.perches[self.target.0 as usize].top_y)
                } else {
                    (self.land_x, self.floor_y)
                };
                let (x0, y0) = self.from;
                // A taller arc for the hop up than the drop down.
                let arc = if self.up { 6.0 + (y0 - ty).max(0) as f32 * 0.35 } else { 4.0 };
                character.pos.x = x0 + ((tx - x0) as f32 * s) as i32;
                character.pos.y = y0 + ((ty - y0) as f32 * s - 4.0 * arc * s * (1.0 - s)) as i32;
                if s >= 1.0 {
                    if self.up {
                        ctx.perch = Some(self.perched());
                    }
                    character.pos.y = ty;
                    self.phase = Phase::Done;
                    self.pose_id = PoseId::SittingSideNeutral;
                }
            }
            Phase::Done => return BehaviorState::Completed,
        }
        BehaviorState::Running
    }

    fn exit(&mut self, ctx: &mut GameContext, completed: bool) {
        // Cut off mid-hop up: finish it, so the cat isn't left in mid-air.
        // A cut-off hop down is put back on the floor by the location scene.
        if !completed && self.up && self.phase == Phase::Jump {
            ctx.perch = Some(self.perched());
        }
    }
}
