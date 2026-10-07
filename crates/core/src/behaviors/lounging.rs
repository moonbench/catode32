use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    scene::SceneId,
    time_system::Weather,
};

const NEUTRAL_LOUNGE: &[PoseId] = &[
    PoseId::LayingSideNeutral,
    PoseId::LayingSideNeutral2,
];

// Added when all four happy gates are met (fullness, comfort, affection, serenity).
// Python: HAPPY_LOUNGE_POSES = NEUTRAL_LOUNGE_POSES + (aloof, happy).
const HAPPY_LOUNGE: &[PoseId] = &[
    PoseId::LayingSideNeutral,
    PoseId::LayingSideNeutral2,
    PoseId::LayingSideAloof,
    PoseId::LayingSideHappy,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Settling,
    Lounging,
    Rousing,
}

pub struct LoungingBehavior {
    phase: Phase,
    phase_timer: f32,
    settle_duration: f32,
    lounge_duration: f32,
    rouse_duration: f32,
    pose_id: PoseId,
    lounge_pose: PoseId,
}

impl LoungingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Settling,
            phase_timer: 0.0,
            settle_duration: 6.0,
            lounge_duration: 60.0,
            rouse_duration: 3.0,
            pose_id: PoseId::KneadingSideNeutral,
            lounge_pose: PoseId::LayingSideNeutral,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.focus > 30.0 && ctx.serenity > 30.0
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let lo = ctx.serenity * 0.5;
        let hi = (ctx.serenity * 1.5).min(90.0);
        let mut base = 100.0 - rand::rand_range_f32(rng, lo, hi);
        if ctx.in_familiar_location {
            base *= 0.8;
        }
        base.max(5.0) as u32
    }
}

impl Behavior for LoungingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Lounging
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Settling => 0.0,
            Phase::Lounging => (self.phase_timer / self.lounge_duration).clamp(0.0, 1.0),
            Phase::Rousing => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::Settling;
        self.phase_timer = 0.0;
        self.settle_duration = rand::rand_range_f32(&mut ctx.rng, 4.0, 10.0);
        self.lounge_duration = rand::rand_range_f32(&mut ctx.rng, 30.0, 120.0);
        self.rouse_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 5.0);
        let happy = ctx.fullness >= 50.0
            && ctx.comfort >= 50.0
            && ctx.affection >= 60.0
            && ctx.serenity >= 60.0;
        let pool = if happy { HAPPY_LOUNGE } else { NEUTRAL_LOUNGE };
        self.lounge_pose = common::pick_pose(&mut ctx.rng, pool);
        self.pose_id = PoseId::KneadingSideNeutral;
    }

    fn update(&mut self, _ctx: &mut GameContext, _: &mut Character, dt: f32) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Settling if self.phase_timer >= self.settle_duration => {
                self.phase = Phase::Lounging;
                self.phase_timer = 0.0;
                self.pose_id = self.lounge_pose;
            }
            Phase::Lounging if self.phase_timer >= self.lounge_duration => {
                self.phase = Phase::Rousing;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::LeaningForwardSideStretch;
            }
            Phase::Rousing if self.phase_timer >= self.rouse_duration => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        let mut rng = ctx.rng;
        // Low serenity -> more likely to knead. 0% at serenity=100, 35% at serenity=0.
        let kneading_p = (100.0 - ctx.serenity) * 0.35;
        if rand::rand_f32(&mut rng) * 100.0 < kneading_p {
            return Some(NextBehavior::Kneading);
        }
        // Low energy -> more likely to nap. 0% at energy=100, 45% at energy=0.
        let napping_p = (100.0 - ctx.energy) * 0.45;
        if rand::rand_f32(&mut rng) * 100.0 < napping_p {
            return Some(NextBehavior::Napping);
        }
        None
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 14> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Fullness, -0.025);
        common::bonus_add(&mut bonus, StatId::Energy, -0.2);
        common::bonus_add(&mut bonus, StatId::Comfort, 1.0);
        common::bonus_add(&mut bonus, StatId::Focus, -0.05);
        common::bonus_add(&mut bonus, StatId::Playfulness, -0.05);
        common::bonus_add(&mut bonus, StatId::Fulfillment, -0.02);
        common::bonus_add(&mut bonus, StatId::Sociability, -0.02);
        common::bonus_add(&mut bonus, StatId::Intelligence, -0.005);
        common::bonus_add(&mut bonus, StatId::Maturity, 0.02);
        common::bonus_add(&mut bonus, StatId::Fitness, -0.015);

        let hf = common::hungry_factor(ctx);
        if hf > 0.0 {
            common::bonus_scale(&mut bonus, StatId::Comfort, 1.0 - 0.5 * hf);
        }
        let ff = common::fed_factor(ctx);
        if ff > 0.0 {
            common::bonus_add(&mut bonus, StatId::Comfort, 0.8 * ff);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 0.25 * ff);
            common::bonus_add(&mut bonus, StatId::Loyalty, 0.03 * ff);
        }

        // apply_location_bonus
        let scene = ctx.last_main_scene;
        if matches!(scene, SceneId::Inside | SceneId::Outside | SceneId::Treehouse) {
            common::bonus_scale(&mut bonus, StatId::Comfort, 1.3);
        }
        if matches!(scene, SceneId::Outside | SceneId::Treehouse)
            && matches!(ctx.weather, Weather::Rain | Weather::Storm | Weather::Snow)
        {
            common::bonus_add(&mut bonus, StatId::Comfort, -6.0);
        }
        let wf = common::serenity_wellbeing_factor(ctx);
        if ctx.in_familiar_location {
            common::bonus_add(&mut bonus, StatId::Serenity, 1.5 * wf);
            common::bonus_scale(&mut bonus, StatId::Comfort, 1.15);
        } else {
            common::bonus_add(&mut bonus, StatId::Serenity, -1.0);
            common::bonus_scale(&mut bonus, StatId::Comfort, 0.9);
        }
        if Some(scene) == ctx.fav_location {
            common::bonus_scale(&mut bonus, StatId::Comfort, 1.2);
            common::bonus_scale(&mut bonus, StatId::Serenity, 1.2);
        } else if Some(scene) == ctx.least_fav_location {
            common::bonus_scale(&mut bonus, StatId::Comfort, 0.85);
            common::bonus_scale(&mut bonus, StatId::Serenity, 0.85);
        }
        if ctx.meteor_shower_happening() {
            common::bonus_add(&mut bonus, StatId::Serenity, 2.0);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 1.5);
            common::bonus_add(&mut bonus, StatId::Comfort, 3.0);
            common::bonus_add(&mut bonus, StatId::Maturity, 0.5);
        }
        if ctx.in_cat_bed {
            common::bonus_add(&mut bonus, StatId::Comfort, 5.0);
            common::bonus_add(&mut bonus, StatId::Serenity, 2.0 * wf);
        }
        let ph = ctx.scene_plant_health as f32;
        if ph != 0.0 {
            common::bonus_add(&mut bonus, StatId::Serenity, ph * 0.2);
            common::bonus_add(&mut bonus, StatId::Comfort, ph * 0.15);
            common::bonus_add(&mut bonus, StatId::Fulfillment, ph * 0.05);
        }
        let (fc, fs) = common::fav_weather_bonus(ctx);
        if fc != 0.0 {
            common::bonus_add(&mut bonus, StatId::Comfort, fc);
        }
        if fs != 0.0 {
            common::bonus_add(&mut bonus, StatId::Serenity, fs);
        }

        for e in bonus.iter_mut() {
            e.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }
}
