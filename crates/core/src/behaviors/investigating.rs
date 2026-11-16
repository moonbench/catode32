use embedded_graphics::prelude::Point;

use crate::{
    assets::{
        character::PoseId,
        icons::{QUESTION_MARK, QUESTION_MARK_FILL, QUESTION_MARK_H, QUESTION_MARK_W},
    },
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    render::{Renderer, SpriteOpts},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Approaching,
    Sniffing,
    Reacting,
}

pub struct InvestigatingBehavior {
    phase: Phase,
    phase_timer: f32,
    approach_duration: f32,
    sniff_duration: f32,
    react_duration: f32,
    pose_id: PoseId,
}

impl InvestigatingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Approaching,
            phase_timer: 0.0,
            approach_duration: 2.0,
            sniff_duration: 16.0,
            react_duration: 2.0,
            pose_id: PoseId::SittingSideLookingDown,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.curiosity >= 40.0
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let hi = (100.0 - ctx.curiosity).max(10.0);
        let mut base = rand::rand_range_f32(rng, 10.0, hi);
        if !ctx.in_familiar_location {
            base *= 0.8;
        }
        base.max(0.0) as u32
    }
}

impl Behavior for InvestigatingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Investigating
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Approaching => 0.0,
            Phase::Sniffing => (self.phase_timer / self.sniff_duration).clamp(0.0, 1.0),
            Phase::Reacting => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::Approaching;
        self.phase_timer = 0.0;
        self.approach_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 4.0);
        self.sniff_duration = rand::rand_range_f32(&mut ctx.rng, 13.0, 20.0);
        self.react_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 3.0);
        self.pose_id = PoseId::SittingSideLookingDown;
    }

    fn update(&mut self, _ctx: &mut GameContext, _: &mut Character, dt: f32) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Approaching if self.phase_timer >= self.approach_duration => {
                self.phase = Phase::Sniffing;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::LeaningForwardSideSniffing;
            }
            Phase::Sniffing if self.phase_timer >= self.sniff_duration => {
                self.phase = Phase::Reacting;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::SittingSideLookingDown;
            }
            Phase::Reacting if self.phase_timer >= self.react_duration => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        let mut rng = ctx.rng;
        let r = rand::rand_f32(&mut rng);
        // Head of the history is whatever preceded this investigation.
        let after_sniff = ctx.recent_behaviors.first() == Some(&BehaviorId::Sniffing);
        if r < 0.3 {
            Some(NextBehavior::Observing)
        } else if r < 0.55 && !after_sniff {
            Some(NextBehavior::Sniffing)
        } else {
            None
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let bonus = [
            (StatId::Focus, -0.25 * progress),
            (StatId::Comfort, -0.2 * progress),
            (StatId::Playfulness, -0.25 * progress),
            (StatId::Serenity, -0.02 * progress),
            (StatId::Curiosity, -0.05 * progress),
            (StatId::Maturity, 0.025 * progress),
            (StatId::Fulfillment, 0.05 * progress),
        ];
        ctx.apply_stat_changes(&bonus);
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _ctx: &GameContext,
        char_screen: Point,
        mirror_h: bool,
    ) {
        if matches!(self.phase, Phase::Reacting) {
            return;
        }
        let qmark_y = char_screen.y - 42;
        let qmark_x = if mirror_h {
            char_screen.x + 18
        } else {
            char_screen.x - QUESTION_MARK_W as i32 - 18
        };
        let pos = Point::new(qmark_x, qmark_y);
        // Punch-through fill behind the outline (matches draw_sprite_obj's
        // fill_frames handling in the Python renderer).
        renderer.draw_sprite_raw(
            QUESTION_MARK_FILL,
            QUESTION_MARK_W,
            QUESTION_MARK_H,
            pos,
            SpriteOpts {
                transparent: true,
                transparent_color: true,
                invert: true,
                ..Default::default()
            },
        );
        renderer.draw_sprite_raw(
            QUESTION_MARK,
            QUESTION_MARK_W,
            QUESTION_MARK_H,
            pos,
            SpriteOpts {
                transparent: true,
                ..Default::default()
            },
        );
    }
}
