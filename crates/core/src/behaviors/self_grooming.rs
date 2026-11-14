use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preparing,
    Grooming,
    Finishing,
}

pub struct SelfGroomingBehavior {
    phase: Phase,
    phase_timer: f32,
    prepare_duration: f32,
    groom_duration: f32,
    finish_duration: f32,
    pose_id: PoseId,
    /// Pose held during the Grooming phase (leg or raised paw).
    groom_pose: PoseId,
    after_meal: bool,
}

impl SelfGroomingBehavior {
    pub fn new(after_meal: bool) -> Self {
        Self {
            phase: Phase::Preparing,
            phase_timer: 0.0,
            prepare_duration: 2.0,
            groom_duration: 25.0,
            finish_duration: 2.0,
            pose_id: PoseId::SittingSideNeutral,
            groom_pose: PoseId::SittingLickingSideLickingLeg,
            after_meal,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.cleanliness < 57.0 && ctx.energy > 30.0
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let bulk = rand::rand_range_f32(rng, ctx.cleanliness * 0.5, ctx.cleanliness * 1.5);
        let extra = rand::rand_range_f32(rng, 0.0, (ctx.energy * 0.25).max(10.0));
        (bulk + extra).max(0.0) as u32
    }
}

/// Post-meal wash. Eating may set `ctx.post_meal_groom_in` to 1..=5; the
/// manager ticks it down on each completed behavior. Once it reaches 0 this
/// fires at the next unchained pick (bypassing the cleanliness gate, since
/// the meal is the reason to groom), then clears.
pub fn post_meal_groom(ctx: &mut GameContext) -> Option<NextBehavior> {
    if ctx.post_meal_groom_in != Some(0) {
        return None;
    }
    ctx.post_meal_groom_in = None;
    if common::sick_blocks(BehaviorId::SelfGrooming, ctx) || ctx.energy <= 15.0 {
        return None;
    }
    println!("[PostMeal] Grooming");
    Some(NextBehavior::SelfGrooming(true))
}

impl Behavior for SelfGroomingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::SelfGrooming
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Preparing => 0.0,
            Phase::Grooming => (self.phase_timer / self.groom_duration).clamp(0.0, 1.0),
            Phase::Finishing => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::Preparing;
        self.phase_timer = 0.0;
        self.prepare_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 3.0);
        self.groom_duration = rand::rand_range_f32(&mut ctx.rng, 10.0, 45.0);
        self.finish_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 3.0);
        self.pose_id = PoseId::SittingSideNeutral;
        self.groom_pose = if self.after_meal || rand::rand_bool(&mut ctx.rng, 0.5) {
            PoseId::SittingLickingSideLickingPaw
        } else {
            PoseId::SittingLickingSideLickingLeg
        };
    }

    fn update(&mut self, _ctx: &mut GameContext, _: &mut Character, dt: f32) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Preparing if self.phase_timer >= self.prepare_duration => {
                self.phase = Phase::Grooming;
                self.phase_timer = 0.0;
                self.pose_id = self.groom_pose;
            }
            Phase::Grooming if self.phase_timer >= self.groom_duration => {
                self.phase = Phase::Finishing;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::SittingSideHappy;
            }
            Phase::Finishing if self.phase_timer >= self.finish_duration => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 8> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Energy, -0.75);
        common::bonus_add(&mut bonus, StatId::Comfort, 0.5);
        common::bonus_add(&mut bonus, StatId::Focus, -0.5);
        common::bonus_add(&mut bonus, StatId::Cleanliness, 15.0);
        common::bonus_add(&mut bonus, StatId::Fulfillment, 0.05);

        // apply_location_bonus (does NOT call super, no fav_weather)
        if ctx.in_familiar_location {
            common::bonus_scale(&mut bonus, StatId::Cleanliness, 1.1);
            common::bonus_add(&mut bonus, StatId::Serenity, 0.5);
        } else {
            common::bonus_scale(&mut bonus, StatId::Cleanliness, 0.9);
        }

        for e in bonus.iter_mut() {
            e.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }
}
