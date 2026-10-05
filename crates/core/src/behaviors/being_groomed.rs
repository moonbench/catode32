use embedded_graphics::prelude::Point;
use micromath::F32Ext;

use crate::{
    assets::{character::PoseId, items::HAIR_BRUSH},
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    character::head_top_y,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    render::{Renderer, SpriteOpts},
    ui::bubble::{self, BubbleIcon},
};

const ACCEPT_DURATION: f32 = 1.5;
const SATISFY_DURATION: f32 = 1.5;
const BUBBLE_WINDOW: f32 = 5.0;

const REJECTION_STAT_MULTIPLIER: f32 = 0.5;

const REJECTION_POSES: &[PoseId] = &[
    PoseId::LayingSideNeutral2,
    PoseId::LayingSideBored,
    PoseId::LayingSideAnnoyed,
    PoseId::LayingSideContent,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Accepting,
    Enjoying,
    Satisfied,
}

pub struct BeingGroomedBehavior {
    phase: Phase,
    phase_timer: f32,
    enjoy_duration: f32,
    pose_id: PoseId,
    rejecting: bool,
}

impl BeingGroomedBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Accepting,
            phase_timer: 0.0,
            enjoy_duration: 16.0,
            pose_id: PoseId::LeaningForwardSideNeutral,
            rejecting: false,
        }
    }

    fn rejection_chance(ctx: &GameContext) -> f32 {
        let mut complement = 1.0_f32;
        let thresholds = [
            (ctx.affection, 25.0),
            (ctx.comfort, 30.0),
            (ctx.sociability, 25.0),
            (ctx.courage, 20.0),
        ];
        for (val, threshold) in thresholds {
            if val < threshold {
                let deficit = (threshold - val) / threshold;
                complement *= 1.0 - deficit;
            }
        }
        1.0 - complement
    }
}

impl Behavior for BeingGroomedBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::BeingGroomed
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Enjoying => (self.phase_timer / self.enjoy_duration).clamp(0.0, 1.0),
            Phase::Satisfied => 1.0,
            Phase::Accepting => 0.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase_timer = 0.0;
        self.enjoy_duration = rand::rand_range_u32(&mut ctx.rng, 10, 30) as f32;
        let chance = Self::rejection_chance(ctx);
        self.rejecting = rand::rand_f32(&mut ctx.rng) < chance;

        if self.rejecting {
            // Skip straight to "enjoying" so the cat just lays there
            // for the full enjoy_duration without changing pose.
            self.phase = Phase::Enjoying;
            let i = rand::rand_range_u32(&mut ctx.rng, 0, (REJECTION_POSES.len() as u32) - 1)
                as usize;
            self.pose_id = REJECTION_POSES[i];
        } else {
            self.phase = Phase::Accepting;
            self.pose_id = PoseId::LeaningForwardSideNeutral;
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
            Phase::Accepting => {
                if self.phase_timer >= ACCEPT_DURATION {
                    self.phase = Phase::Enjoying;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::LayingSideBliss;
                }
            }
            Phase::Enjoying => {
                if self.phase_timer >= self.enjoy_duration {
                    if self.rejecting {
                        return BehaviorState::Completed;
                    }
                    self.phase = Phase::Satisfied;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::SittingSideHappy;
                    character.play_bursts(&mut ctx.rng, 5);
                }
            }
            Phase::Satisfied => {
                if self.phase_timer >= SATISFY_DURATION {
                    character.play_bursts(&mut ctx.rng, 5);
                    return BehaviorState::Completed;
                }
            }
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        if self.rejecting {
            return None;
        }
        let mut rng = ctx.rng;
        if ctx.cleanliness < 70.0
            && ctx.energy > 30.0
            && rand::rand_f32(&mut rng) < 0.4
        {
            Some(NextBehavior::SelfGrooming)
        } else {
            None
        }
    }

    fn exit(&mut self, ctx: &mut GameContext, completed: bool) {
        if completed && !self.rejecting {
            ctx.milestone_groomed = true;
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 12> = heapless::Vec::new();
        let _ = bonus.push((StatId::Focus, -1.0));
        let _ = bonus.push((StatId::Playfulness, 2.5));
        let _ = bonus.push((StatId::Comfort, 2.0));
        let _ = bonus.push((StatId::Cleanliness, 20.0));
        let _ = bonus.push((StatId::Affection, 2.0));
        let _ = bonus.push((StatId::Sociability, 2.5));
        let _ = bonus.push((StatId::Fulfillment, 2.5));
        let _ = bonus.push((StatId::Maturity, 0.25));
        let _ = bonus.push((StatId::Serenity, 1.0));
        let _ = bonus.push((StatId::Mischievousness, -0.1));
        let _ = bonus.push((StatId::Courage, 0.15));

        // Location bonus.
        if ctx.in_familiar_location {
            if let Some(e) = bonus.iter_mut().find(|e| e.0 == StatId::Affection) {
                e.1 *= 1.2;
            }
            if let Some(e) = bonus.iter_mut().find(|e| e.0 == StatId::Serenity) {
                e.1 += 0.5;
            }
        } else if let Some(e) = bonus.iter_mut().find(|e| e.0 == StatId::Serenity) {
            e.1 *= 0.7;
        }

        if self.rejecting {
            for entry in bonus.iter_mut() {
                entry.1 *= REJECTION_STAT_MULTIPLIER;
            }
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
        if !matches!(self.phase, Phase::Enjoying) {
            return;
        }

        // Heart bubble appears for a 5s window centred around the midpoint.
        if !self.rejecting {
            let bubble_start = self.enjoy_duration / 2.0;
            let bubble_end = bubble_start + BUBBLE_WINDOW;
            if self.phase_timer >= bubble_start && self.phase_timer < bubble_end {
                let bubble_progress = (self.phase_timer - bubble_start) / BUBBLE_WINDOW;
                bubble::draw_above_char(
                    renderer,
                    BubbleIcon::Heart,
                    char_screen.x,
                    char_screen.y,
                    bubble_progress.clamp(0.0, 1.0),
                    mirror_h,
                );
            }
        }

        // Brush triangle-wave sweep with parabolic arc lift.
        let sweep_speed = 0.7_f32;
        let raw = (self.phase_timer * sweep_speed) % 2.0;
        let t = if raw <= 1.0 { raw } else { 2.0 - raw };

        let arc_span = 32.0_f32;
        let arc_lift = 6.0_f32;
        // Bristles rest on the head at the bottom of the arc.
        let head_top = head_top_y(self.pose_id.data(), char_screen, mirror_h);
        let base_y = (head_top - HAIR_BRUSH.height as i32) as f32 - arc_lift;

        let offset = (arc_span * (t - 0.5)) as i32;
        let brush_x = if mirror_h {
            char_screen.x + offset
        } else {
            char_screen.x - offset
        } - (HAIR_BRUSH.width as i32) / 2;
        let brush_y = (base_y + arc_lift * (core::f32::consts::PI * t).sin()) as i32;

        renderer.draw_sprite(
            &HAIR_BRUSH,
            Point::new(brush_x, brush_y),
            SpriteOpts {
                mirror_h,
                ..Default::default()
            },
        );
    }
}
