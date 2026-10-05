use embedded_graphics::prelude::Point;

use crate::{
    assets::character::PoseId,
    behaviors::{auto_select, ActiveBehavior},
    context::{FoodItem, GameContext},
    entities::character::Character,
    render::Renderer,
};

/// Where an Eating behavior was kicked off from. `Item` is player-fed food
/// (decremented from inventory at trigger time) and looks up the full
/// per-item FOOD_CONFIG. `CaughtSnack` is the post-hunt nibble that uses a
/// MOUSE_TOY sprite and a cut-down bonus table.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EatingSource {
    Item(FoodItem),
    CaughtSnack,
}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BehaviorId {
    Idle,
    Sleeping,
    Napping,
    Stretching,
    Kneading,
    Lounging,
    Investigating,
    Observing,
    Chattering,
    Zoomies,
    Vocalizing,
    SelfGrooming,
    BeingGroomed,
    Hunting,
    GiftBringing,
    Pacing,
    Sulking,
    Mischief,
    Hiding,
    Training,
    Playing,
    Affection,
    Attention,
    Eating,
    Startled,
    Meandering,
    GoTo,
    Hearing,
    Greeting,
}

crate::enum_str_method! {
    BehaviorId::name;
    Idle          => "idle",
    Sleeping      => "sleeping",
    Napping       => "napping",
    Stretching    => "stretching",
    Kneading      => "kneading",
    Lounging      => "lounging",
    Investigating => "investigating",
    Observing     => "observing",
    Chattering    => "chattering",
    Zoomies       => "zoomies",
    Vocalizing    => "vocalizing",
    SelfGrooming  => "self_grooming",
    BeingGroomed  => "being_groomed",
    Hunting       => "hunting",
    GiftBringing  => "gift_bringing",
    Pacing        => "pacing",
    Sulking       => "sulking",
    Mischief      => "mischief",
    Hiding        => "hiding",
    Training      => "training",
    Playing       => "playing",
    Affection     => "affection",
    Attention     => "attention",
    Eating        => "eating",
    Startled      => "startled",
    Meandering    => "meandering",
    GoTo          => "go_to",
    Hearing       => "hearing",
    Greeting      => "greeting",
}

#[allow(dead_code)]
impl BehaviorId {
    /// Interaction (player-initiated) behaviors that shouldn't auto-resume
    /// when re-entering a scene.
    pub fn is_interaction(self) -> bool {
        matches!(
            self,
            BehaviorId::Affection
                | BehaviorId::Attention
                | BehaviorId::BeingGroomed
                | BehaviorId::Eating
                | BehaviorId::Playing
                | BehaviorId::GiftBringing
                | BehaviorId::Chattering
                | BehaviorId::GoTo
                | BehaviorId::Hearing
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BehaviorState {
    Running,
    Completed,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayVariant {
    Ball,
    String,
    Feather,
    Mouse,
    Hand,
    Laser,
    Bubbles,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AffectionVariant {
    Kiss,
    Pets,
    Scratching,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttentionVariant {
    Psst,
    PointBird,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrainingKind {
    Intelligence,
    Behavior,
    Fitness,
    Sociability,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GiftKind {
    Fish,
    Mouse,
}

/// What a behavior wants to chain to on natural completion. The manager turns
/// `None` into auto-select, and unknown variants into Idle.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum NextBehavior {
    Idle,
    Sleeping,
    Napping,
    Stretching,
    Kneading,
    Lounging,
    Investigating,
    Observing,
    Chattering,
    Zoomies,
    Vocalizing,
    SelfGrooming,
    Hunting,
    Pacing,
    Sulking,
    Mischief,
    Hiding,
    Meandering,
    Hearing(Option<&'static str>),
    Startled,
    Greeting,
    Playing(PlayVariant),
    Affection(AffectionVariant),
    Attention(AttentionVariant),
    BeingGroomed,
    Eating(EatingSource),
    GiftBringing(GiftKind),
    Training(TrainingKind),
    GoTo(GoToParams),
}

#[derive(Clone, Copy, Debug)]
pub struct GoToParams {
    pub target_x: i32,
    pub speed: f32,
    pub pending_scene: Option<crate::scene::SceneId>,
    pub then: Option<GoToThen>,
}

/// A limited set of behaviors go_to can chain into on arrival. Kept flat so
/// `NextBehavior` doesn't need to be recursive.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum GoToThen {
    Sleeping,
    Napping,
    Lounging,
    Idle,
}

impl GoToThen {
    pub fn into_next(self) -> NextBehavior {
        match self {
            GoToThen::Sleeping => NextBehavior::Sleeping,
            GoToThen::Napping => NextBehavior::Napping,
            GoToThen::Lounging => NextBehavior::Lounging,
            GoToThen::Idle => NextBehavior::Idle,
        }
    }
}

#[allow(dead_code)]
pub trait Behavior {
    fn id(&self) -> BehaviorId;

    fn name(&self) -> &'static str {
        self.id().name()
    }

    fn progress(&self) -> f32;
    fn pose(&self) -> PoseId;

    fn enter(&mut self, _ctx: &mut GameContext, _character: &mut Character) {}
    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState;
    fn exit(&mut self, _ctx: &mut GameContext, _completed: bool) {}

    fn apply_completion_bonus(&self, _ctx: &mut GameContext, _progress: f32) {}

    fn next(&self, _ctx: &GameContext) -> Option<NextBehavior> {
        None
    }

    /// Optional draw hook for particles, bubbles, item sprites, etc.
    /// `char_screen` is the on-screen character anchor; `mirror_h` matches the
    /// character so behavior visuals stay aligned with the pose.
    fn draw(
        &self,
        _renderer: &mut Renderer,
        _ctx: &GameContext,
        _char_screen: Point,
        _mirror_h: bool,
    ) {
    }

    /// Only `playing` uses this. Lets a behavior force a specific eye frame
    /// (e.g. eye-tracking a toy).
    fn eye_frame_override(&self) -> Option<usize> {
        None
    }

    /// Wind down quickly on wake-from-sleep. Only sleeping / napping override.
    /// Implementations may consult / clear `ctx.pending_wake_greeting` to honor
    /// or veto the wake transition based on the cat's state.
    fn mark_almost_done(&mut self, _ctx: &mut GameContext) {}

    /// True if this behavior steals the d-pad while it's running. The location
    /// scene uses this to suppress camera panning so the player can steer the
    /// active toy instead. Only PlayingBehavior overrides today.
    fn captures_dpad(&self) -> bool {
        false
    }
}

pub struct BehaviorManager {
    current: ActiveBehavior,
    started: bool,
    /// Sickness accrued from exposure/neglect but not yet applied to ctx.
    sickness_pending: f32,
}

#[allow(dead_code)]
impl BehaviorManager {
    pub fn new() -> Self {
        Self {
            current: ActiveBehavior::from_next(NextBehavior::Idle),
            started: false,
            sickness_pending: 0.0,
        }
    }

    pub fn start(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.started = true;
        let name = self.current.as_dyn().name();
        ctx.current_behavior_name = Some(name);
        println!("[\x1b[32mBehavior started\x1b[0m] {}", name);
        self.current.as_dyn_mut().enter(ctx, character);
    }

    pub fn update(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        if !self.started {
            self.start(ctx, character);
        }
        self.accumulate_sickness(ctx, dt);
        let state = self.current.as_dyn_mut().update(ctx, character, dt);
        if state == BehaviorState::Completed {
            self.advance(ctx, character, true);
        }
    }

    /// Accrue sickness over real time spent exposed to bad weather or neglect.
    /// Rested time (sleeping / napping) doesn't count. Applied in small steps
    /// so the log isn't spammed every frame.
    fn accumulate_sickness(&mut self, ctx: &mut GameContext, dt: f32) {
        use crate::time_system::Weather;
        if matches!(self.current_id(), BehaviorId::Sleeping | BehaviorId::Napping) {
            return;
        }
        let mut per_min = 0.0;
        if crate::behaviors::common::is_outdoor(ctx.last_main_scene) {
            per_min += match ctx.weather {
                Weather::Storm => SICK_PER_MIN_STORM,
                Weather::Rain | Weather::Snow => SICK_PER_MIN_RAIN,
                _ => 0.0,
            };
        }
        if ctx.fullness < 10.0 {
            per_min += SICK_PER_MIN_NEGLECT;
        }
        if ctx.cleanliness < 15.0 {
            per_min += SICK_PER_MIN_NEGLECT;
        }
        if per_min <= 0.0 {
            return;
        }
        self.sickness_pending += per_min * dt / 60.0;
        if self.sickness_pending >= SICK_APPLY_STEP {
            let delta = self.sickness_pending;
            self.sickness_pending = 0.0;
            ctx.sickness = (ctx.sickness + delta).min(10.0);
            println!("[Sickness] +{:.2} -> {:.2}", delta, ctx.sickness);
        }
    }

    /// Player-initiated trigger. Interrupts the current behavior.
    pub fn trigger(
        &mut self,
        next: NextBehavior,
        ctx: &mut GameContext,
        character: &mut Character,
    ) {
        self.current.as_dyn_mut().exit(ctx, false);
        self.swap_to(next, ctx, character);
    }

    /// Skip the current behavior (debug / dev hook). Falls back to auto-select.
    pub fn skip(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.current.as_dyn_mut().exit(ctx, false);
        let next = auto_select(ctx);
        self.swap_to(next, ctx, character);
    }

    pub fn mark_almost_done(&mut self, ctx: &mut GameContext) {
        self.current.as_dyn_mut().mark_almost_done(ctx);
    }

    pub fn current_id(&self) -> BehaviorId {
        self.current.as_dyn().id()
    }

    pub fn current_name(&self) -> &'static str {
        self.current.as_dyn().name()
    }

    pub fn current_progress(&self) -> f32 {
        self.current.as_dyn().progress()
    }

    pub fn current_pose(&self) -> PoseId {
        self.current.as_dyn().pose()
    }

    pub fn current_eye_frame_override(&self) -> Option<usize> {
        self.current.as_dyn().eye_frame_override()
    }

    pub fn current_captures_dpad(&self) -> bool {
        self.current.as_dyn().captures_dpad()
    }

    pub fn draw_overlay(
        &self,
        renderer: &mut Renderer,
        ctx: &GameContext,
        char_screen: Point,
        mirror_h: bool,
    ) {
        self.current
            .as_dyn()
            .draw(renderer, ctx, char_screen, mirror_h);
    }

    fn advance(&mut self, ctx: &mut GameContext, character: &mut Character, completed: bool) {
        let id = self.current.as_dyn().id();
        let progress = self.current.as_dyn().progress();
        // Caught outside in bad weather: head for shelter, overriding any chain.
        let shelter = if completed && id != BehaviorId::GoTo && ctx.pending_scene.is_none() {
            crate::behaviors::common::shelter_exit(ctx)
        } else {
            None
        };
        let chained = if shelter.is_some() {
            shelter
        } else if completed {
            self.current.as_dyn().next(ctx).filter(|n| {
                let next_id = ActiveBehavior::from_next(n.clone()).as_dyn().id();
                !crate::behaviors::common::sick_blocks(next_id, ctx)
            })
        } else {
            None
        };
        self.current.as_dyn_mut().exit(ctx, completed);
        if completed {
            ctx.record_behavior(id);
            self.current.as_dyn().apply_completion_bonus(ctx, progress);
        }

        let next = if let Some(n) = chained {
            n
        } else if completed {
            // Wake-from-sleep greeting takes precedence over auto-select.
            if ctx.pending_wake_greeting {
                ctx.pending_wake_greeting = false;
                if let Some(g) = wake_greeting(ctx) {
                    println!("[WakeReact] Greeting picked");
                    g
                } else {
                    auto_select(ctx)
                }
            } else {
                auto_select(ctx)
            }
        } else {
            auto_select(ctx)
        };

        self.swap_to(next, ctx, character);
    }

    fn swap_to(
        &mut self,
        next: NextBehavior,
        ctx: &mut GameContext,
        character: &mut Character,
    ) {
        ctx.in_cat_bed = false;
        character.draw_y_offset = 0;
        let next = maybe_redirect_to_bed(next, ctx, character);
        self.current = ActiveBehavior::from_next(next);
        let id = self.current.as_dyn().id();
        if matches!(id, BehaviorId::Sleeping | BehaviorId::Napping) {
            if let Some(bx) = ctx.cat_bed_x {
                if (character.pos.x - bx).abs() < 8 {
                    ctx.in_cat_bed = true;
                    character.draw_y_offset = -4;
                }
            }
        }
        let name = self.current.as_dyn().name();
        ctx.current_behavior_name = Some(name);
        println!("[\x1b[32mBehavior started\x1b[0m] {}", name);
        self.current.as_dyn_mut().enter(ctx, character);
    }
}

// Sickness gained per minute awake in each condition. Rates stack. An hour
// outside in the rain is ~+2.4; a single sleep/nap recovers 1.0 (3.0 w/ medicine).
const SICK_PER_MIN_STORM: f32 = 0.08;
const SICK_PER_MIN_RAIN: f32 = 0.04;
const SICK_PER_MIN_NEGLECT: f32 = 0.03;
const SICK_APPLY_STEP: f32 = 0.05;

// When a sleep-type behavior starts in a room with a cat bed and the cat is
// not already near it, 60% of the time walk to the bed first and chain into
// the original behavior on arrival.
const BED_REDIRECT_CHANCE: f32 = 0.6;
const BED_NEAR_THRESHOLD: i32 = 64;

fn maybe_redirect_to_bed(
    next: NextBehavior,
    ctx: &mut GameContext,
    character: &Character,
) -> NextBehavior {
    let then = match next {
        NextBehavior::Sleeping => GoToThen::Sleeping,
        NextBehavior::Napping => GoToThen::Napping,
        _ => return next,
    };
    let Some(bx) = ctx.cat_bed_x else {
        return next;
    };
    if (character.pos.x - bx).abs() < BED_NEAR_THRESHOLD {
        return next;
    }
    if !crate::rand::rand_bool(&mut ctx.rng, BED_REDIRECT_CHANCE) {
        return next;
    }
    NextBehavior::GoTo(GoToParams {
        target_x: bx,
        speed: 12.0,
        pending_scene: None,
        then: Some(then),
    })
}

fn wake_greeting(ctx: &GameContext) -> Option<NextBehavior> {
    if ctx.sickness >= 8.0 {
        return None;
    }
    const NEED: f32 = 50.0;
    let unmet = ctx.fullness < NEED
        || ctx.affection < NEED
        || ctx.comfort < NEED
        || ctx.fulfillment < NEED;
    let happy = ctx.energy > 40.0 && ctx.playfulness > 45.0;
    let mut rng = ctx.rng;
    let r = crate::rand::rand_f32(&mut rng);
    if unmet && r < 0.75 {
        Some(NextBehavior::Vocalizing)
    } else if happy && r < 0.55 {
        Some(NextBehavior::Vocalizing)
    } else {
        None
    }
}
