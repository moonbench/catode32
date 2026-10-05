use embedded_graphics::prelude::Point;
use micromath::F32Ext;

use crate::{
    assets::{
        character::PoseId,
        items::{HAND, HAND_SCRATCH, HAND_SCRATCH_FPS},
    },
    behavior::{AffectionVariant, Behavior, BehaviorId, BehaviorState},
    character::head_top_y,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    render::{Renderer, SpriteOpts},
    ui::bubble::{self, BubbleIcon},
};

const REJECTION_STAT_MULTIPLIER: f32 = 0.5;

const SICK_POSES: &[PoseId] = &[PoseId::LayingSideSulking, PoseId::LayingSideSulking2];

const REJECTION_POSES: &[PoseId] = &[
    PoseId::LayingSideNeutral2,
    PoseId::LayingSideBored,
    PoseId::LayingSideAnnoyed,
    PoseId::LayingSideContent,
];

struct VariantCfg {
    pose: PoseId,
    duration: f32,
}

impl AffectionVariant {
    fn cfg(self) -> VariantCfg {
        match self {
            AffectionVariant::Kiss => VariantCfg {
                pose: PoseId::SittingSideHappy,
                duration: 2.5,
            },
            AffectionVariant::Pets => VariantCfg {
                pose: PoseId::SittingSillySideHappy,
                duration: 5.0,
            },
            AffectionVariant::Scratching => VariantCfg {
                pose: PoseId::SittingSillySideHappy,
                duration: 6.0,
            },
        }
    }
}

pub struct AffectionBehavior {
    variant: AffectionVariant,
    phase_timer: f32,
    duration: f32,
    pose_id: PoseId,
    rejecting: bool,
    sick: bool,
    show_bubble: bool,
}

impl AffectionBehavior {
    #[allow(dead_code)]
    pub fn new(variant: AffectionVariant) -> Self {
        Self {
            variant,
            phase_timer: 0.0,
            duration: 5.0,
            pose_id: PoseId::SittingSideHappy,
            rejecting: false,
            sick: false,
            show_bubble: false,
        }
    }

    fn rejection_chance(ctx: &GameContext) -> f32 {
        let mut complement = 1.0f32;
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

impl Behavior for AffectionBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Affection
    }
    fn progress(&self) -> f32 {
        (self.phase_timer / self.duration).clamp(0.0, 1.0)
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        let cfg = self.variant.cfg();
        self.duration = cfg.duration;
        self.phase_timer = 0.0;

        let chance = Self::rejection_chance(ctx);
        self.rejecting = rand::rand_f32(&mut ctx.rng) < chance;
        self.sick = ctx.sickness >= 2.0;

        if self.rejecting {
            self.show_bubble = false;
            let i = rand::rand_range_u32(&mut ctx.rng, 0, (REJECTION_POSES.len() as u32) - 1)
                as usize;
            self.pose_id = REJECTION_POSES[i];
        } else if self.sick {
            self.show_bubble = true;
            let i = rand::rand_range_u32(&mut ctx.rng, 0, (SICK_POSES.len() as u32) - 1) as usize;
            self.pose_id = SICK_POSES[i];
        } else {
            self.show_bubble = true;
            self.pose_id = cfg.pose;
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        if self.phase_timer >= self.duration {
            character.play_bursts(&mut ctx.rng, 5);
            BehaviorState::Completed
        } else {
            BehaviorState::Running
        }
    }

    fn exit(&mut self, ctx: &mut GameContext, completed: bool) {
        if completed && !self.rejecting && !self.sick {
            ctx.milestone_petted = true;
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus = match self.variant {
            AffectionVariant::Kiss => kiss_bonus(),
            AffectionVariant::Pets => pets_bonus(),
            AffectionVariant::Scratching => scratching_bonus(),
        };

        // Well-fed boost.
        let fed_factor = ((ctx.fullness - 90.0) / 10.0).max(0.0);
        if fed_factor > 0.0 {
            add_to(&mut bonus, StatId::Affection, 2.0 * fed_factor);
            add_to(&mut bonus, StatId::Loyalty, 0.2 * fed_factor);
            add_to(&mut bonus, StatId::Fulfillment, 0.5 * fed_factor);
        }

        // Familiar-location modifier.
        if ctx.in_familiar_location {
            scale(&mut bonus, StatId::Affection, 1.2);
            add_to(&mut bonus, StatId::Serenity, 0.5);
        } else {
            scale(&mut bonus, StatId::Affection, 0.85);
        }

        // Scene plant health bonus / penalty.
        let ph = ctx.scene_plant_health as f32;
        if ph != 0.0 {
            add_to(&mut bonus, StatId::Affection, ph * 0.1);
            add_to(&mut bonus, StatId::Comfort, ph * 0.1);
            add_to(&mut bonus, StatId::Fulfillment, ph * 0.05);
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
        if self.show_bubble {
            bubble::draw_above_char(
                renderer,
                BubbleIcon::Heart,
                char_screen.x,
                char_screen.y,
                self.progress(),
                mirror_h,
            );
        }

        let head_top = head_top_y(self.pose_id.data(), char_screen, mirror_h);

        match self.variant {
            AffectionVariant::Pets => {
                let sweep_speed = 1.2_f32;
                let raw = (self.phase_timer * sweep_speed) % 2.0;
                let t = if raw <= 1.0 { raw } else { 2.0 - raw };

                let arc_span = 30.0_f32;
                let arc_lift = 5.0_f32;
                // Palm rests on the head at the bottom of the arc.
                let base_y = (head_top - HAND.height as i32) as f32 - arc_lift;

                let offset = (arc_span * (t - 0.5)) as i32;
                let hand_x = if mirror_h {
                    char_screen.x + offset
                } else {
                    char_screen.x - offset
                } - (HAND.width as i32) / 2;
                let hand_y =
                    (base_y + arc_lift * (core::f32::consts::PI * t).sin()) as i32;

                renderer.draw_sprite(
                    &HAND,
                    Point::new(hand_x, hand_y),
                    SpriteOpts {
                        mirror_h,
                        ..Default::default()
                    },
                );
            }
            AffectionVariant::Scratching => {
                let scratch_speed = 3.5_f32;
                let raw = (self.phase_timer * scratch_speed) % 2.0;
                let t = if raw <= 1.0 { raw } else { 2.0 - raw };

                let jitter_span = 8.0_f32;
                let vertical_range = 4.0_f32;
                // Fingertips dig a little into the fur at the bottom of each stroke.
                let dig_in = 3;
                let base_y = (head_top - HAND_SCRATCH.height as i32 + dig_in) as f32
                    - vertical_range;

                let offset = (jitter_span * (t - 0.5)) as i32;
                let hand_x = if mirror_h {
                    char_screen.x + offset
                } else {
                    char_screen.x - offset
                } - (HAND_SCRATCH.width as i32) / 2;
                let hand_y = (base_y + vertical_range * t) as i32;

                let frame = (self.phase_timer * HAND_SCRATCH_FPS) as usize
                    % HAND_SCRATCH.frames.len();

                renderer.draw_sprite(
                    &HAND_SCRATCH,
                    Point::new(hand_x, hand_y),
                    SpriteOpts {
                        mirror_h,
                        frame,
                        ..Default::default()
                    },
                );
            }
            AffectionVariant::Kiss => {}
        }
    }
}

// --- bonus tables (mirrors VARIANTS in affection.py) --------------------

type Bonus = heapless::Vec<(StatId, f32), 14>;

fn add_to(bonus: &mut Bonus, stat: StatId, delta: f32) {
    if let Some(entry) = bonus.iter_mut().find(|e| e.0 == stat) {
        entry.1 += delta;
    } else {
        let _ = bonus.push((stat, delta));
    }
}

fn scale(bonus: &mut Bonus, stat: StatId, factor: f32) {
    if let Some(entry) = bonus.iter_mut().find(|e| e.0 == stat) {
        entry.1 *= factor;
    }
}

fn kiss_bonus() -> Bonus {
    let mut b: Bonus = heapless::Vec::new();
    let _ = b.push((StatId::Affection, 8.0));
    let _ = b.push((StatId::Fulfillment, 2.5));
    let _ = b.push((StatId::Comfort, 5.0));
    let _ = b.push((StatId::Focus, 3.0));
    let _ = b.push((StatId::Playfulness, 2.0));
    let _ = b.push((StatId::Curiosity, 2.0));
    let _ = b.push((StatId::Sociability, 2.0));
    let _ = b.push((StatId::Serenity, 0.25));
    let _ = b.push((StatId::Maturity, 0.2));
    let _ = b.push((StatId::Loyalty, 1.0));
    let _ = b.push((StatId::Mischievousness, -0.1));
    let _ = b.push((StatId::Courage, 0.3));
    b
}

fn pets_bonus() -> Bonus {
    let mut b: Bonus = heapless::Vec::new();
    let _ = b.push((StatId::Affection, 4.0));
    let _ = b.push((StatId::Fulfillment, 1.5));
    let _ = b.push((StatId::Comfort, 5.0));
    let _ = b.push((StatId::Focus, 1.0));
    let _ = b.push((StatId::Playfulness, 3.5));
    let _ = b.push((StatId::Curiosity, 2.0));
    let _ = b.push((StatId::Sociability, 1.0));
    let _ = b.push((StatId::Serenity, 0.25));
    let _ = b.push((StatId::Maturity, 0.2));
    let _ = b.push((StatId::Loyalty, 1.0));
    let _ = b.push((StatId::Mischievousness, -0.1));
    let _ = b.push((StatId::Courage, 0.3));
    b
}

fn scratching_bonus() -> Bonus {
    let mut b: Bonus = heapless::Vec::new();
    let _ = b.push((StatId::Affection, 3.0));
    let _ = b.push((StatId::Fulfillment, 2.0));
    let _ = b.push((StatId::Comfort, 9.0));
    let _ = b.push((StatId::Focus, 0.5));
    let _ = b.push((StatId::Playfulness, 4.5));
    let _ = b.push((StatId::Curiosity, 1.5));
    let _ = b.push((StatId::Sociability, 1.0));
    let _ = b.push((StatId::Serenity, 0.5));
    let _ = b.push((StatId::Maturity, 0.1));
    let _ = b.push((StatId::Loyalty, 1.0));
    let _ = b.push((StatId::Mischievousness, 0.1));
    let _ = b.push((StatId::Courage, 0.4));
    b
}
