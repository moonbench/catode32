//! Drinking: the cat walks over to the scene's water bowl, leans down and
//! laps at it. Only eligible where a scene sets `ctx.water_bowl_x`.

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
};

/// How far from the bowl's center the cat stands, so the tongue in the
/// lapping pose dips into the bowl.
const LAP_OFFSET_X: i32 = 30;
const WALK_SPEED: f32 = 14.0;
const APPROACH_TIMEOUT: f32 = 15.0;
const LOWER_DURATION: f32 = 1.0;
const FINISH_DURATION: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Approaching,
    Lowering,
    Lapping,
    Finishing,
}

pub struct DrinkingBehavior {
    phase: Phase,
    phase_timer: f32,
    lap_duration: f32,
    stand_x: i32,
    face_right: bool,
    walker_accum: f32,
    pose_id: PoseId,
    /// Set if the cat never made it to the bowl; no bonus then.
    gave_up: bool,
}

impl DrinkingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Approaching,
            phase_timer: 0.0,
            lap_duration: 6.0,
            stand_x: 0,
            face_right: false,
            walker_accum: 0.0,
            pose_id: PoseId::WalkingSideNeutral,
            gave_up: false,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.water_bowl_x.is_some() && ctx.visit.is_none()
    }

    pub fn priority(_ctx: &GameContext, rng: &mut u32) -> u32 {
        rand::rand_range_f32(rng, 20.0, 90.0) as u32
    }

    fn start_lowering(&mut self, character: &mut Character) {
        self.phase = Phase::Lowering;
        self.phase_timer = 0.0;
        self.pose_id = PoseId::LeaningForwardSideNeutral;
        character.mirror_h = self.face_right;
    }
}

/// Where to stand to drink from a bowl centered at `bowl_x`: on whichever
/// side is nearer the cat, unless that side is off the walkable strip.
/// Returns (stand_x, face_right).
fn drinking_spot(bowl_x: i32, cat_x: i32, x_min: i32, x_max: i32) -> (i32, bool) {
    let left = bowl_x - LAP_OFFSET_X;
    let right = bowl_x + LAP_OFFSET_X;
    let left_ok = left >= x_min;
    let right_ok = right <= x_max;
    let use_left = match (left_ok, right_ok) {
        (true, true) => cat_x < bowl_x,
        (true, false) => true,
        _ => false,
    };
    if use_left {
        (left, true)
    } else {
        (right.min(x_max), false)
    }
}

impl Behavior for DrinkingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Drinking
    }

    fn progress(&self) -> f32 {
        if self.gave_up {
            return 0.0;
        }
        match self.phase {
            Phase::Approaching | Phase::Lowering => 0.0,
            Phase::Lapping => (self.phase_timer / self.lap_duration).clamp(0.0, 1.0),
            Phase::Finishing => 1.0,
        }
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.phase_timer = 0.0;
        self.walker_accum = 0.0;
        self.gave_up = false;
        self.lap_duration = rand::rand_range_f32(&mut ctx.rng, 4.0, 10.0);
        let bowl_x = ctx.water_bowl_x.unwrap_or(character.pos.x);
        let (stand_x, face_right) =
            drinking_spot(bowl_x, character.pos.x, ctx.scene_x_min, ctx.scene_x_max);
        self.stand_x = stand_x;
        self.face_right = face_right;
        if common::distance_to(character, stand_x) <= 1 {
            character.pos.x = stand_x;
            self.start_lowering(character);
        } else {
            self.phase = Phase::Approaching;
            self.pose_id = PoseId::WalkingSideNeutral;
            character.mirror_h = stand_x > character.pos.x;
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
                let dir = if character.pos.x < self.stand_x { 1 } else { -1 };
                let bounced = common::step_walker(
                    character,
                    ctx,
                    dir,
                    WALK_SPEED,
                    dt,
                    &mut self.walker_accum,
                );
                if common::distance_to(character, self.stand_x) <= 1 {
                    character.pos.x = self.stand_x;
                    self.start_lowering(character);
                } else if bounced || self.phase_timer >= APPROACH_TIMEOUT {
                    self.gave_up = true;
                    return BehaviorState::Completed;
                }
            }
            Phase::Lowering => {
                if self.phase_timer >= LOWER_DURATION {
                    self.phase = Phase::Lapping;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::LeaningForwardSideLapping;
                }
            }
            Phase::Lapping => {
                if self.phase_timer >= self.lap_duration {
                    self.phase = Phase::Finishing;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::LeaningForwardSideNeutral;
                }
            }
            Phase::Finishing => {
                if self.phase_timer >= FINISH_DURATION {
                    return BehaviorState::Completed;
                }
            }
        }
        BehaviorState::Running
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 4> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Comfort, 1.0);
        common::bonus_add(&mut bonus, StatId::Serenity, 0.5);
        for e in bonus.iter_mut() {
            e.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }
}
