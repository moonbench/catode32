use embedded_graphics::prelude::Point;

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    render::Renderer,
    time_system::Weather,
    ui::bubble::{self, BubbleIcon},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    WindingUp,
    Vocalizing,
    Settling,
}

pub struct VocalizingBehavior {
    phase: Phase,
    phase_timer: f32,
    windup_duration: f32,
    vocalize_duration: f32,
    settle_duration: f32,
    pose_id: PoseId,
}

impl VocalizingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::WindingUp,
            phase_timer: 0.0,
            windup_duration: 1.5,
            vocalize_duration: 10.0,
            settle_duration: 2.0,
            pose_id: PoseId::SittingForwardNeutral,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        // Vacation overstay always allows a vocalize so the pet can ask to go home.
        if ctx.wants_to_go_home {
            return true;
        }
        const NEED: f32 = 60.0;
        let happy = ctx.energy > 35.0 && ctx.playfulness > 40.0;
        let needs_unmet = ctx.fullness < NEED
            || ctx.comfort < NEED
            || ctx.fulfillment < NEED
            || ctx.affection < NEED
            || ctx.sociability < NEED;
        // Outdoor weather / temperature complaint. Must mirror the
        // complaint branch in `priority` so it can always fire.
        if common::is_outdoor(ctx.last_main_scene)
            && (matches!(
                ctx.weather,
                Weather::Rain | Weather::Storm | Weather::Snow
            ) || ctx.temperature < 2.0
                || ctx.temperature > 30.0)
        {
            return true;
        }
        happy || needs_unmet
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        // Wants-to-go-home wins most selection rounds.
        if ctx.wants_to_go_home {
            return rand::rand_range_f32(rng, 2.0, 8.0).max(0.0) as u32;
        }
        // Outdoor weather complaint.
        if common::is_outdoor(ctx.last_main_scene) {
            let weather_bad = matches!(
                ctx.weather,
                Weather::Rain | Weather::Storm | Weather::Snow
            );
            let temp_complaint = ctx.temperature < 2.0 || ctx.temperature > 30.0;
            if (weather_bad || temp_complaint)
                && ctx
                    .recent_index(BehaviorId::Vocalizing)
                    .is_none()
            {
                let mut urgency = 1.0_f32;
                if ctx.weather == Weather::Storm {
                    urgency = 1.5;
                }
                if ctx.temperature < -1.0 || ctx.temperature > 33.0 {
                    urgency = urgency.max(2.0);
                }
                return (rand::rand_range_f32(rng, 12.0, 22.0) / urgency).max(0.0) as u32;
            }
        }
        // Outdoor chatty: ESP-NOW radio active (typically outdoor scenes).
        if ctx
            .espnow
            .as_ref()
            .is_some_and(|e| e.is_active())
            && ctx.recent_index(BehaviorId::Vocalizing).is_none()
        {
            return rand::rand_range_f32(rng, 5.0, 15.0).max(0.0) as u32;
        }

        const NEED: f32 = 40.0;
        let urgency = (NEED - ctx.fullness)
            .max(NEED - ctx.sociability)
            .max(NEED - ctx.affection)
            .max(NEED - ctx.comfort)
            .max(NEED - ctx.playfulness)
            .max(0.0);
        if urgency > 0.0 {
            return (65.0 - urgency * 3.0).max(5.0) as u32;
        }
        let hi = ((200.0 - ctx.energy - ctx.playfulness) * 0.5).max(25.0);
        rand::rand_range_f32(rng, 25.0, hi).max(0.0) as u32
    }
}

impl Behavior for VocalizingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Vocalizing
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::WindingUp => 0.0,
            Phase::Vocalizing => (self.phase_timer / self.vocalize_duration).clamp(0.0, 1.0),
            Phase::Settling => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::WindingUp;
        self.phase_timer = 0.0;
        self.windup_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 2.0);
        // Python __init__ sets vocalize_duration = uniform(6, 24) but start()
        // overrides with randint(6, 15). Runtime value is the override.
        self.vocalize_duration = rand::rand_range_u32(&mut ctx.rng, 6, 15) as f32;
        self.settle_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 3.0);
        self.pose_id = PoseId::SittingForwardNeutral;

        let icon = pick_icon(ctx);
        print_reason(ctx, icon);
        ctx.pending_popup_icon = Some(icon);
    }

    fn update(&mut self, ctx: &mut GameContext, _: &mut Character, dt: f32) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::WindingUp if self.phase_timer >= self.windup_duration => {
                self.phase = Phase::Vocalizing;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::YellingForwardLiftAndYell;
                // Signal LocationScene to broadcast this vocalization
                // over ESP-NOW if the radio is acquired for this scene.
                // The icon hint reuses the same `pending_popup_icon`
                // value that drives the on-screen bubble.
                ctx.pending_vocalize_broadcast = ctx.pending_popup_icon;
            }
            Phase::Vocalizing if self.phase_timer >= self.vocalize_duration => {
                self.phase = Phase::Settling;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::SittingSideNeutral;
            }
            Phase::Settling if self.phase_timer >= self.settle_duration => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        let mut rng = ctx.rng;
        if rand::rand_f32(&mut rng) < 0.2 {
            Some(NextBehavior::Zoomies)
        } else {
            None
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let bonus = [
            (StatId::Energy, -0.75 * progress),
            (StatId::Comfort, -0.3 * progress),
            (StatId::Serenity, -0.015 * progress),
        ];
        ctx.apply_stat_changes(&bonus);
    }

    fn draw(&self, renderer: &mut Renderer, ctx: &GameContext, char_screen: Point, mirror_h: bool) {
        if self.phase != Phase::Vocalizing {
            return;
        }
        let icon = ctx
            .pending_popup_icon
            .and_then(BubbleIcon::from_name)
            .unwrap_or(BubbleIcon::Exclaim);
        bubble::draw_above_char(
            renderer,
            icon,
            char_screen.x,
            char_screen.y,
            self.progress(),
            mirror_h,
        );
    }
}

const NEED_THRESHOLD: f32 = 60.0;

fn pick_icon(ctx: &GameContext) -> &'static str {
    if ctx.wants_to_go_home {
        return "home";
    }
    if common::is_outdoor(ctx.last_main_scene) {
        if ctx.temperature < 2.0 {
            return "cold";
        }
        if ctx.temperature > 30.0 {
            return "hot";
        }
        match ctx.weather {
            Weather::Rain | Weather::Storm => return "wet",
            Weather::Snow => return "cold",
            _ => {}
        }
    }
    let needs = [
        (ctx.fullness, "hunger"),
        (ctx.comfort, "discomfort"),
        (ctx.fulfillment, "bored"),
        (ctx.affection, "lonely"),
    ];
    let (worst_stat, worst_icon) = needs
        .iter()
        .copied()
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal))
        .unwrap();
    if worst_stat < NEED_THRESHOLD {
        worst_icon
    } else {
        "exclaim"
    }
}


fn print_reason(ctx: &GameContext, icon: &'static str) {
    match icon {
        "home" => println!("[Vocalizing] Reason: home (wants to go home)"),
        "cold" | "hot" | "wet" => println!(
            "[Vocalizing] Reason: {} (temperature: {:.1}, weather: {:?})",
            icon, ctx.temperature, ctx.weather
        ),
        "hunger" => println!("[Vocalizing] Reason: hunger (fullness: {:6.4})", ctx.fullness),
        "discomfort" => println!("[Vocalizing] Reason: discomfort (comfort: {:6.4})", ctx.comfort),
        "bored" => println!("[Vocalizing] Reason: bored (fulfillment: {:6.4})", ctx.fulfillment),
        "lonely" => println!("[Vocalizing] Reason: lonely (affection: {:6.4})", ctx.affection),
        _ => println!(
            "[Vocalizing] Reason: {} (energy: {:6.4}, playfulness: {:6.4}, sociability: {:6.4})",
            icon, ctx.energy, ctx.playfulness, ctx.sociability
        ),
    }
}
