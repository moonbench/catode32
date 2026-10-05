use embedded_graphics::prelude::Point;
use crate::t;

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    context::{GameContext, StatId},
    entities::character::Character,
    render::Renderer,
};

const CHATTER_DURATION: f32 = 10.0;
const SETTLE_DURATION: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Chattering,
    Settling,
}

pub struct ChatteringBehavior {
    phase: Phase,
    phase_timer: f32,
    pose_id: PoseId,
}

impl ChatteringBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Chattering,
            phase_timer: 0.0,
            pose_id: PoseId::SittingSillySideAnnoyed,
        }
    }
}

impl Behavior for ChatteringBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Chattering
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Chattering => (self.phase_timer / CHATTER_DURATION).clamp(0.0, 1.0),
            Phase::Settling => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, _ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::Chattering;
        self.phase_timer = 0.0;
        self.pose_id = PoseId::SittingSillySideAnnoyed;
    }

    fn update(&mut self, _ctx: &mut GameContext, _: &mut Character, dt: f32) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Chattering if self.phase_timer >= CHATTER_DURATION => {
                self.phase = Phase::Settling;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::SittingSideAloof;
            }
            Phase::Settling if self.phase_timer >= SETTLE_DURATION => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn next(&self, _ctx: &GameContext) -> Option<NextBehavior> {
        Some(NextBehavior::Observing)
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let bonus = [
            (StatId::Focus, -0.15 * progress),
            (StatId::Curiosity, -0.05 * progress),
            (StatId::Intelligence, -0.0025 * progress),
        ];
        ctx.apply_stat_changes(&bonus);
    }

    fn draw(&self, renderer: &mut Renderer, _ctx: &GameContext, char_screen: Point, mirror_h: bool) {
        #[cfg(not(feature = "desktop"))]
        use micromath::F32Ext;
        if self.phase != Phase::Chattering {
            return;
        }
        // Three stacked "ek" texts on the side the cat is facing, each blinking
        // on/off on a 1.2s cycle with 0.3s phase offsets.
        const CYCLE: f32 = 1.2;
        const ON_DURATION: f32 = 0.8;
        let base_x = char_screen.x + if mirror_h { 16 } else { -36 };
        let base_y = char_screen.y - 10;
        for i in 0..3 {
            let raw = self.phase_timer - i as f32 * 0.3;
            let age = raw - (raw / CYCLE).floor() * CYCLE;
            if age < ON_DURATION {
                renderer.draw_text(t!("ek"), Point::new(base_x, base_y - i * 9));
            }
        }
    }
}
