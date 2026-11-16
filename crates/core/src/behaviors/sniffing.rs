//! Sniffing: a short in-place sniff. The cat does a few bouts of sniffing
//! (sitting with its nose in the air, standing, or leaning forward), lifting
//! its head between bouts. In an unfamiliar location (e.g. right after a wifi
//! scan turned up new networks) it sniffs longer and leans in more.
//!
//! Chains naturally around Investigating and Pawing: a sniff can lead into
//! investigating or pawing at something, and both of those can end in a sniff.

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    behaviors::{common, InvestigatingBehavior, PawingBehavior},
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
};

const SHUFFLE_SPEED: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    Sitting,
    Standing,
    Leaning,
}

impl Stance {
    fn sniff_pose(self) -> PoseId {
        match self {
            Stance::Sitting => PoseId::SittingSideSniffing,
            Stance::Standing => PoseId::StandingSideSniffing,
            Stance::Leaning => PoseId::LeaningForwardSideSniffing,
        }
    }

    /// Head-up pose for the pause between bouts.
    fn rest_pose(self) -> PoseId {
        match self {
            Stance::Sitting => PoseId::SittingSideNeutral,
            Stance::Standing => PoseId::StandingSideNeutral,
            Stance::Leaning => PoseId::LeaningForwardSideNeutral,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Sniffing,
    Pausing,
    Shuffling,
}

pub struct SniffingBehavior {
    phase: Phase,
    phase_timer: f32,
    phase_duration: f32,
    pose_id: PoseId,
    stance: Stance,
    bouts_total: u8,
    bouts_done: u8,
    /// True when the cat entered in an unfamiliar location. Biases stance
    /// and chaining toward investigating.
    unfamiliar: bool,
    shuffle_dir: i32,
    shuffle_target: i32,
    walker_accum: f32,
}

impl SniffingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Sniffing,
            phase_timer: 0.0,
            phase_duration: 2.0,
            pose_id: PoseId::SittingSideSniffing,
            stance: Stance::Sitting,
            bouts_total: 1,
            bouts_done: 0,
            unfamiliar: false,
            shuffle_dir: 0,
            shuffle_target: 0,
            walker_accum: 0.0,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.curiosity >= 20.0
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let hi = (100.0 - ctx.curiosity).max(20.0);
        let mut base = rand::rand_range_f32(rng, 10.0, hi);
        if !ctx.in_familiar_location {
            base *= 0.8;
        }
        base.max(0.0) as u32
    }

    fn pick_stance(&self, ctx: &mut GameContext) -> Stance {
        // Mildly sick: low-effort sitting sniffs only.
        if ctx.sickness >= 2.0 {
            return Stance::Sitting;
        }
        let r = rand::rand_f32(&mut ctx.rng);
        if self.unfamiliar {
            if r < 0.5 {
                Stance::Leaning
            } else if r < 0.75 {
                Stance::Standing
            } else {
                Stance::Sitting
            }
        } else if r < 0.4 {
            Stance::Sitting
        } else if r < 0.8 {
            Stance::Standing
        } else {
            Stance::Leaning
        }
    }

    fn start_bout(&mut self, ctx: &mut GameContext) {
        self.phase = Phase::Sniffing;
        self.phase_timer = 0.0;
        self.phase_duration = rand::rand_range_f32(&mut ctx.rng, 1.5, 3.0);
        self.pose_id = self.stance.sniff_pose();
    }

    fn start_pause(&mut self, ctx: &mut GameContext) {
        self.phase = Phase::Pausing;
        self.phase_timer = 0.0;
        self.phase_duration = rand::rand_range_f32(&mut ctx.rng, 0.6, 1.5);
        self.pose_id = self.stance.rest_pose();
    }

    /// After a pause: maybe change stance, maybe shuffle a few pixels to a
    /// fresh spot (standing only), then sniff again.
    fn after_pause(&mut self, ctx: &mut GameContext, character: &mut Character) {
        if rand::rand_bool(&mut ctx.rng, 0.35) {
            self.stance = self.pick_stance(ctx);
        }
        if self.stance == Stance::Standing && rand::rand_bool(&mut ctx.rng, 0.4) {
            let dist = rand::rand_range_u32(&mut ctx.rng, 3, 8) as i32;
            self.shuffle_dir = if character.mirror_h { 1 } else { -1 };
            self.shuffle_target = (character.pos.x + self.shuffle_dir * dist)
                .clamp(ctx.scene_x_min, ctx.scene_x_max);
            self.walker_accum = 0.0;
            self.phase = Phase::Shuffling;
            self.phase_timer = 0.0;
            self.pose_id = PoseId::WalkingSideNeutral;
        } else {
            self.start_bout(ctx);
        }
    }
}

impl Behavior for SniffingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Sniffing
    }

    fn progress(&self) -> f32 {
        let total = self.bouts_total.max(1) as f32;
        match self.phase {
            Phase::Sniffing => {
                let bout = (self.phase_timer / self.phase_duration).clamp(0.0, 1.0);
                (self.bouts_done as f32 + bout) / total
            }
            Phase::Pausing | Phase::Shuffling => self.bouts_done as f32 / total,
        }
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _character: &mut Character) {
        self.unfamiliar = !ctx.in_familiar_location;
        self.bouts_done = 0;
        self.bouts_total = if ctx.sickness >= 2.0 {
            rand::rand_range_u32(&mut ctx.rng, 1, 2)
        } else if self.unfamiliar {
            rand::rand_range_u32(&mut ctx.rng, 3, 5)
        } else {
            rand::rand_range_u32(&mut ctx.rng, 1, 3)
        } as u8;
        self.stance = self.pick_stance(ctx);
        self.start_bout(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Sniffing => {
                if self.phase_timer >= self.phase_duration {
                    self.bouts_done += 1;
                    if self.bouts_done >= self.bouts_total {
                        return BehaviorState::Completed;
                    }
                    self.start_pause(ctx);
                }
            }
            Phase::Pausing => {
                if self.phase_timer >= self.phase_duration {
                    self.after_pause(ctx, character);
                }
            }
            Phase::Shuffling => {
                let bounced = common::step_walker(
                    character,
                    ctx,
                    self.shuffle_dir,
                    SHUFFLE_SPEED,
                    dt,
                    &mut self.walker_accum,
                );
                let arrived = common::distance_to(character, self.shuffle_target) == 0;
                if arrived || bounced || self.phase_timer >= 2.0 {
                    self.start_bout(ctx);
                }
            }
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        // `advance()` asks for `next()` before recording this behavior, so
        // the head of the history is whatever came before the sniff. Never
        // bounce straight back into it.
        let prev = ctx.recent_behaviors.first().copied();
        let mut rng = ctx.rng;
        let investigate_p = if self.unfamiliar { 0.5 } else { 0.25 };
        if prev != Some(BehaviorId::Investigating)
            && InvestigatingBehavior::can_trigger(ctx)
            && rand::rand_f32(&mut rng) < investigate_p
        {
            return Some(NextBehavior::Investigating);
        }
        if prev != Some(BehaviorId::Pawing)
            && PawingBehavior::can_trigger(ctx)
            && rand::rand_f32(&mut rng) < 0.15
        {
            return Some(NextBehavior::Pawing);
        }
        None
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let bonus = [
            (StatId::Curiosity, -0.05 * progress),
            (StatId::Focus, -0.05 * progress),
            (StatId::Fulfillment, 0.03 * progress),
            (StatId::Maturity, 0.01 * progress),
        ];
        ctx.apply_stat_changes(&bonus);
    }
}
