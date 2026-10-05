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

const SLEEP_POSES: &[PoseId] = &[
    PoseId::SleepingSideSploot,
    PoseId::SleepingSideModest,
    PoseId::SleepingSideCrossed,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Considering,
    Settling,
    Sleeping,
    Waking,
}

pub struct SleepingBehavior {
    phase: Phase,
    phase_timer: f32,
    considering_duration: f32,
    settle_duration: f32,
    sleep_duration: f32,
    wake_duration: f32,
    pose_id: PoseId,
    sleep_pose: PoseId,
    z_timer: f32,
}

impl SleepingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Considering,
            phase_timer: 0.0,
            considering_duration: 2.0,
            settle_duration: 5.0,
            sleep_duration: 240.0,
            wake_duration: 5.0,
            pose_id: PoseId::SittingSideLookingDown,
            sleep_pose: PoseId::SleepingSideModest,
            z_timer: 0.0,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        if ctx.sickness >= 8.0 {
            return true;
        }
        let threshold = if ctx.sickness >= 5.0 {
            95.0
        } else if ctx.sickness >= 2.0 {
            75.0
        } else {
            let mut t = if ctx.time_hours >= 21 || ctx.time_hours < 6 {
                70.0
            } else {
                40.0
            };
            if ctx.last_main_scene == SceneId::Bedroom {
                t += 20.0;
            }
            t
        };
        ctx.energy < threshold
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let lo = ctx.energy * 0.25;
        let hi = (ctx.energy * 2.0).max(lo);
        let mut base = rand::rand_range_f32(rng, lo, hi);
        if ctx.time_hours >= 19 || ctx.time_hours < 6 {
            base *= 0.4;
        }
        if ctx.last_main_scene == SceneId::Bedroom {
            base *= 0.55;
        }
        base.max(0.0) as u32
    }
}

impl Behavior for SleepingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Sleeping
    }

    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Considering | Phase::Settling => 0.0,
            Phase::Sleeping => (self.phase_timer / self.sleep_duration).clamp(0.0, 1.0),
            Phase::Waking => 1.0,
        }
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _character: &mut Character) {
        self.phase = Phase::Considering;
        self.phase_timer = 0.0;
        self.considering_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 4.0);
        self.settle_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 10.0);
        self.sleep_duration = rand::rand_range_u32(&mut ctx.rng, 120, 360) as f32;
        self.wake_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 10.0);
        self.sleep_pose = common::pick_pose(&mut ctx.rng, SLEEP_POSES);
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
            Phase::Considering if self.phase_timer >= self.considering_duration => {
                self.phase = Phase::Settling;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::LeaningForwardSideNeutral;
            }
            Phase::Settling if self.phase_timer >= self.settle_duration => {
                self.phase = Phase::Sleeping;
                self.phase_timer = 0.0;
                self.pose_id = self.sleep_pose;
            }
            Phase::Sleeping if self.phase_timer >= self.sleep_duration => {
                self.phase = Phase::Waking;
                self.phase_timer = 0.0;
                // Python does NOT change pose on wake; the sleep pose is kept.
            }
            Phase::Waking if self.phase_timer >= self.wake_duration => {
                // Opportunistic save at the wake transition.
                crate::save::save_if_needed(ctx);
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
        common::bonus_add(&mut bonus, StatId::Energy, 35.0);
        common::bonus_add(&mut bonus, StatId::Focus, 9.0);
        common::bonus_add(&mut bonus, StatId::Comfort, 10.0);
        common::bonus_add(&mut bonus, StatId::Playfulness, 15.0);
        common::bonus_add(&mut bonus, StatId::Fullness, -5.0);
        common::bonus_add(&mut bonus, StatId::Curiosity, 0.2);
        common::bonus_add(&mut bonus, StatId::Cleanliness, -1.9);
        common::bonus_add(&mut bonus, StatId::Intelligence, -0.05);
        common::bonus_add(&mut bonus, StatId::Fitness, -0.2);

        if ctx.fullness > 60.0 {
            common::bonus_add(&mut bonus, StatId::Energy, 12.0);
        }
        if ctx.playfulness > 75.0 {
            common::bonus_scale(&mut bonus, StatId::Playfulness, 0.5);
        }
        if ctx.focus > 75.0 {
            common::bonus_scale(&mut bonus, StatId::Focus, 0.5);
        } else if ctx.focus < 30.0 {
            common::bonus_scale(&mut bonus, StatId::Focus, 3.0);
        }

        let hf = common::hungry_factor(ctx);
        if hf > 0.0 {
            common::bonus_add(&mut bonus, StatId::Focus, -4.0 * hf);
            common::bonus_add(&mut bonus, StatId::Serenity, -1.5 * hf);
            common::bonus_add(&mut bonus, StatId::Fulfillment, -0.5 * hf);
        }
        let ff = common::fed_factor(ctx);
        if ff > 0.0 {
            common::bonus_add(&mut bonus, StatId::Focus, 3.0 * ff);
            common::bonus_add(&mut bonus, StatId::Serenity, 1.5 * ff);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 0.5 * ff);
            common::bonus_add(&mut bonus, StatId::Loyalty, 0.15 * ff);
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
            common::bonus_scale(&mut bonus, StatId::Energy, 1.3);
            common::bonus_scale(&mut bonus, StatId::Comfort, 1.25);
        }
        if matches!(scene, SceneId::Outside | SceneId::Treehouse)
            && matches!(ctx.weather, Weather::Rain | Weather::Storm | Weather::Snow)
        {
            common::bonus_add(&mut bonus, StatId::Comfort, -10.0);
        }
        let wf = common::serenity_wellbeing_factor(ctx);
        if ctx.in_familiar_location {
            common::bonus_add(&mut bonus, StatId::Serenity, 2.25 * wf);
        } else {
            common::bonus_add(&mut bonus, StatId::Serenity, -2.0);
            common::bonus_scale(&mut bonus, StatId::Comfort, 0.85);
        }
        if ctx.meteor_shower_happening() {
            common::bonus_add(&mut bonus, StatId::Serenity, 3.0);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 1.5);
        }
        if ctx.in_cat_bed {
            common::bonus_scale(&mut bonus, StatId::Energy, 1.15);
            common::bonus_add(&mut bonus, StatId::Comfort, 5.0);
            common::bonus_add(&mut bonus, StatId::Serenity, 1.5 * wf);
        }
        let ph = ctx.scene_plant_health as f32;
        if ph != 0.0 {
            common::bonus_add(&mut bonus, StatId::Serenity, ph * 0.15);
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
        if self.phase != Phase::Sleeping {
            return;
        }
        let base_x = char_screen.x + if mirror_h { 20 } else { -20 };
        let base_y = char_screen.y - 35;
        const WAVE_SPEED: f32 = 3.0;
        const WAVE_AMP: f32 = 3.0;
        const SPACING_X: i32 = 8;
        const SPACING_Y: i32 = -2;
        for i in 0..4 {
            let phase_offset = i as f32 * 0.8;
            let wave = (self.z_timer * WAVE_SPEED - phase_offset).sin() * WAVE_AMP;
            let x = base_x + i * SPACING_X;
            let y = base_y + i * SPACING_Y + wave as i32;
            renderer.draw_text(t!("z"), Point::new(x, y));
        }
    }

    fn mark_almost_done(&mut self, ctx: &mut GameContext) {
        // Only meaningful while still sleeping; let considering/settling/waking finish.
        if self.phase != Phase::Sleeping {
            return;
        }
        // More serene cats sleep more deeply and are less likely to stir.
        let stay_chance = 0.1 + (ctx.serenity / 100.0) * 0.7;
        if rand::rand_f32(&mut ctx.rng) < stay_chance {
            ctx.pending_wake_greeting = false;
            return;
        }
        // Rouse in ~3 seconds.
        self.sleep_duration = self.phase_timer + 3.0;
    }
}
