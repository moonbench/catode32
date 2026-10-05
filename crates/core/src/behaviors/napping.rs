use embedded_graphics::prelude::Point;
use crate::t;

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    render::Renderer,
    scene::SceneId,
    time_system::Weather,
};

const NAP_POSES: &[PoseId] = &[
    PoseId::SleepingSideModest,
    PoseId::SleepingSideCrossed,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Settling,
    Napping,
    Waking,
}

pub struct NappingBehavior {
    phase: Phase,
    phase_timer: f32,
    settle_duration: f32,
    nap_duration: f32,
    wake_duration: f32,
    pose_id: PoseId,
    nap_pose: PoseId,
    z_timer: f32,
}

impl NappingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Settling,
            phase_timer: 0.0,
            settle_duration: 3.0,
            nap_duration: 20.0,
            wake_duration: 3.0,
            pose_id: PoseId::SittingSideLookingDown,
            nap_pose: PoseId::SleepingSideModest,
            z_timer: 0.0,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        if ctx.sickness >= 8.0 {
            return true;
        }
        let threshold = if ctx.sickness >= 5.0 {
            97.0
        } else if ctx.sickness >= 2.0 {
            85.0
        } else {
            let mut t = if ctx.time_hours >= 21 || ctx.time_hours < 6 {
                85.0
            } else {
                60.0
            };
            if ctx.last_main_scene == SceneId::Bedroom {
                t += 20.0;
            }
            t
        };
        ctx.energy < threshold
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let lo = ctx.energy * 0.3;
        let hi = (ctx.energy * 2.5).max(ctx.energy * 0.5);
        let mut base = rand::rand_range_f32(rng, lo, hi);
        if ctx.time_hours >= 19 || ctx.time_hours < 6 {
            base *= 0.5;
        }
        if ctx.last_main_scene == SceneId::Bedroom {
            base *= 0.55;
        }
        base.max(0.0) as u32
    }
}

impl Behavior for NappingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Napping
    }

    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Settling => 0.0,
            Phase::Napping => (self.phase_timer / self.nap_duration).clamp(0.0, 1.0),
            Phase::Waking => 1.0,
        }
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _character: &mut Character) {
        self.phase = Phase::Settling;
        self.phase_timer = 0.0;
        self.settle_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 5.0);
        // Python init: uniform(30, 120), but start() overrides with randint(12, 45).
        // The runtime value is the start() override, so we match that.
        self.nap_duration = rand::rand_range_u32(&mut ctx.rng, 12, 45) as f32;
        self.wake_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 5.0);
        self.nap_pose = common::pick_pose(&mut ctx.rng, NAP_POSES);
        self.pose_id = PoseId::SittingSideLookingDown;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        self.z_timer += dt;
        match self.phase {
            Phase::Settling if self.phase_timer >= self.settle_duration => {
                self.phase = Phase::Napping;
                self.phase_timer = 0.0;
                self.pose_id = self.nap_pose;
                crate::save::save_if_needed(ctx);
            }
            Phase::Napping if self.phase_timer >= self.nap_duration => {
                self.phase = Phase::Waking;
                self.phase_timer = 0.0;
                // Python does NOT change pose on wake; the nap pose is kept.
            }
            Phase::Waking if self.phase_timer >= self.wake_duration => {
                // Bedroom-location burst (Python apply_location_bonus).
                if ctx.last_main_scene == SceneId::Bedroom {
                    character.play_bursts(&mut ctx.rng, 5);
                }
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn next(&self, _ctx: &GameContext) -> Option<NextBehavior> {
        Some(NextBehavior::Stretching)
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 14> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Energy, 22.0);
        common::bonus_add(&mut bonus, StatId::Focus, 6.0);
        common::bonus_add(&mut bonus, StatId::Playfulness, 13.0);
        common::bonus_add(&mut bonus, StatId::Fullness, -1.0);
        common::bonus_add(&mut bonus, StatId::Comfort, 4.0);
        common::bonus_add(&mut bonus, StatId::Curiosity, 0.1);
        common::bonus_add(&mut bonus, StatId::Cleanliness, -0.8);
        common::bonus_add(&mut bonus, StatId::Intelligence, -0.025);
        common::bonus_add(&mut bonus, StatId::Fitness, -0.1);

        if ctx.fullness > 60.0 {
            common::bonus_add(&mut bonus, StatId::Energy, 4.0);
        }
        if ctx.playfulness > 75.0 {
            common::bonus_scale(&mut bonus, StatId::Playfulness, 0.5);
        }
        if ctx.focus > 75.0 {
            common::bonus_scale(&mut bonus, StatId::Focus, 0.5);
        } else if ctx.focus < 25.0 {
            common::bonus_scale(&mut bonus, StatId::Focus, 2.0);
        }

        let hf = common::hungry_factor(ctx);
        if hf > 0.0 {
            common::bonus_add(&mut bonus, StatId::Focus, -2.0 * hf);
            common::bonus_add(&mut bonus, StatId::Serenity, -0.75 * hf);
        }
        let ff = common::fed_factor(ctx);
        if ff > 0.0 {
            common::bonus_add(&mut bonus, StatId::Focus, 1.5 * ff);
            common::bonus_add(&mut bonus, StatId::Serenity, 0.75 * ff);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 0.25 * ff);
        }

        let medicine = ctx.medicine_pending;
        let sickness = ctx.sickness;
        if sickness >= 8.0 {
            common::bonus_scale(&mut bonus, StatId::Energy, 0.35);
        } else if sickness >= 5.0 {
            common::bonus_scale(&mut bonus, StatId::Energy, 0.5);
        } else if sickness >= 2.0 {
            common::bonus_scale(&mut bonus, StatId::Energy, 0.7);
        }
        if medicine {
            ctx.medicine_pending = false;
        }
        if sickness > 0.0 {
            let recovery = if medicine { 3.0 } else { 1.0 };
            ctx.sickness = (sickness - recovery).max(0.0);
        }

        // apply_location_bonus
        let scene = ctx.last_main_scene;
        if scene == SceneId::Bedroom {
            common::bonus_scale(&mut bonus, StatId::Energy, 1.2);
            common::bonus_scale(&mut bonus, StatId::Comfort, 1.2);
        }
        if matches!(scene, SceneId::Outside | SceneId::Treehouse)
            && matches!(ctx.weather, Weather::Rain | Weather::Storm | Weather::Snow)
        {
            common::bonus_add(&mut bonus, StatId::Comfort, -7.0);
        }
        let wf = common::serenity_wellbeing_factor(ctx);
        if ctx.in_familiar_location {
            common::bonus_add(&mut bonus, StatId::Serenity, 1.5 * wf);
        } else {
            common::bonus_add(&mut bonus, StatId::Serenity, -1.0);
            common::bonus_scale(&mut bonus, StatId::Comfort, 0.9);
        }
        if ctx.meteor_shower_happening() {
            common::bonus_add(&mut bonus, StatId::Serenity, 1.5);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 0.75);
        }
        if ctx.in_cat_bed {
            common::bonus_scale(&mut bonus, StatId::Energy, 1.15);
            common::bonus_add(&mut bonus, StatId::Comfort, 5.0);
            common::bonus_add(&mut bonus, StatId::Serenity, 1.5 * wf);
        }
        let ph = ctx.scene_plant_health as f32;
        if ph != 0.0 {
            common::bonus_add(&mut bonus, StatId::Serenity, ph * 0.1);
            common::bonus_add(&mut bonus, StatId::Comfort, ph * 0.1);
        }
        let (fc, fs) = common::fav_weather_bonus(ctx);
        if fc != 0.0 {
            common::bonus_add(&mut bonus, StatId::Comfort, fc);
        }
        if fs != 0.0 {
            common::bonus_add(&mut bonus, StatId::Serenity, fs);
        }

        for entry in bonus.iter_mut() {
            entry.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _ctx: &GameContext,
        char_screen: Point,
        mirror_h: bool,
    ) {
        #[cfg(not(feature = "desktop"))]
        use micromath::F32Ext;
        if self.phase != Phase::Napping {
            return;
        }
        let base_x = char_screen.x + if mirror_h { 18 } else { -18 };
        let base_y = char_screen.y - 28;
        let wave = (self.z_timer * 2.5).sin() * 2.0;
        renderer.draw_text(t!("z"), Point::new(base_x, base_y + wave as i32));
    }

    fn mark_almost_done(&mut self, ctx: &mut GameContext) {
        // Only meaningful while still napping; let settling/waking finish.
        if self.phase != Phase::Napping {
            return;
        }
        // Lighter sleep, so the stay-asleep ceiling is lower than for sleeping.
        let stay_chance = 0.05 + (ctx.serenity / 100.0) * 0.45;
        if rand::rand_f32(&mut ctx.rng) < stay_chance {
            // Cancel the wake greeting; the pet stays asleep.
            ctx.pending_wake_greeting = false;
            return;
        }
        // Rouse in ~3 seconds.
        self.nap_duration = self.phase_timer + 3.0;
    }
}
