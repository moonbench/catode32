//! Player-steered play sessions. Mirrors `entities/behaviors/playing.py`.
//!
//! Seven variants:
//! - `Ball`, `Mouse`, `Hand`: shared sliding-toy physics (push / friction /
//!   wall-bounce), pounce / recover / catch state machine.
//! - `Laser`: user-steered base with sine wobble around it.
//! - `String`, `Feather`: 8-segment verlet rope dangling from a user-steered
//!   anchor; cat pounces toward the tip.
//! - `Bubbles`: bubble-wand sliding toy plus particle bubbles that spawn from
//!   wand motion and fall to the floor / pop.
//!
//! Camera lock and player input routing live in `LocationScene`. The
//! behavior reads `ctx.input` (held + just-pressed bits) and writes the
//! character's eye-frame override via `Behavior::eye_frame_override`.

use embedded_graphics::prelude::Point;
use heapless::Vec;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::{
    assets::{
        character::PoseId,
        items::{
            BUBBLE1, BUBBLE2, BUBBLE_POP, BUBBLE_WAND, HAND_SCRATCH, MOUSE_TOY, YARN_BALL,
        },
    },
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior, PlayVariant},
    behaviors::common,
    context::{GameContext, StatId, ToyVariant},
    entities::character::Character,
    rand,
    render::{Renderer, SpriteOpts},
    scene::SceneId,
};

/// Toy variants the pet can play with on its own (no player input).
const SOLO_PLAY_VARIANTS: &[ToyVariant] = &[
    ToyVariant::Ball,
    ToyVariant::String_,
    ToyVariant::Feather,
    ToyVariant::Mouse,
];

fn is_solo(v: ToyVariant) -> bool {
    SOLO_PLAY_VARIANTS.iter().any(|&s| s == v)
}

/// Returns the list of solo-toy variants currently present in the player's
/// inventory. Used by both `can_trigger` and auto-select variant choice.
pub fn solo_toys_in_inventory(
    ctx: &GameContext,
) -> heapless::Vec<ToyVariant, { SOLO_PLAY_VARIANTS.len() }> {
    let mut out: heapless::Vec<ToyVariant, { SOLO_PLAY_VARIANTS.len() }> =
        heapless::Vec::new();
    for entry in ctx.toys.iter() {
        if is_solo(entry.variant) {
            let _ = out.push(entry.variant);
        }
    }
    out
}

// -- Tuning constants (mirrors playing.py top-of-file constants) ---------

const POUNCE_SLIDE_SPEED: f32 = 28.0;
const POUNCE_SLIDE_DURATION: f32 = 0.9;
const POUNCE_RECOVER_DURATION: f32 = 0.8;
const POUNCE_CATCH_DURATION: f32 = 1.5;
const POUNCE_COUNT_MIN: u32 = 4;
const POUNCE_COUNT_MAX: u32 = 8;

const PLAY_MIN_DURATION: f32 = 15.0;
const TOY_SCREEN_MARGIN: f32 = 8.0;

const BALL_PUSH_FORCE: f32 = 140.0;
const BALL_MAX_SPEED: f32 = 65.0;
const BALL_FRICTION: f32 = 0.20;
const BALL_BOUNCE_DAMPING: f32 = 0.55;
const BALL_ROLL_RANGE: f32 = 60.0;
const BALL_Y_OFFSET: i32 = 8;
const MOUSE_Y_OFFSET: i32 = 4;
const HAND_Y_OFFSET: i32 = 6;
const HAND_ANIM_STEP: f32 = 10.0;
const HAND_PUSH_FORCE: f32 = 300.0;
const HAND_MAX_SPEED: f32 = 90.0;
const HAND_FRICTION: f32 = 0.08;
const HAND_BOUNCE_DAMPING: f32 = 0.20;
const BALL_POUNCE_DELAY_MIN: f32 = 2.5;
const BALL_POUNCE_DELAY_MAX: f32 = 6.0;

const LASER_WOBBLE_AMPLITUDE: f32 = 8.0;
const LASER_WOBBLE_SPEED: f32 = 2.5;
const LASER_USER_SPEED: f32 = 50.0;
const LASER_Y_OFFSET: i32 = 1;
const LASER_POUNCE_DELAY_MIN: f32 = 2.0;
const LASER_POUNCE_DELAY_MAX: f32 = 5.0;
const LASER_DOT_RADIUS: i32 = 2;
const LASER_LINE_TOP_Y: i32 = -64;

const STRING_SEGMENTS: usize = 8;
const STRING_SEG_LEN_TOP: f32 = 20.0;
const STRING_SEG_LEN_BOT: f32 = 4.0;
const STRING_GRAVITY: f32 = 120.0;
const STRING_DAMPING: f32 = 0.45;
const STRING_ITERATIONS: usize = 3;
const STRING_ANCHOR_SPEED: f32 = 60.0;
const STRING_ANCHOR_Y: i32 = -70;
const STRING_POUNCE_DELAY_MIN: f32 = 2.0;
const STRING_POUNCE_DELAY_MAX: f32 = 8.0;
const FEATHER_SEGMENTS: usize = 8;
const FEATHER_WIDTH: f32 = 2.0;

const WAND_PUSH_FORCE: f32 = 200.0;
const WAND_MAX_SPEED: f32 = 80.0;
const WAND_FRICTION: f32 = 0.15;
const WAND_BOUNCE_DAMPING: f32 = 0.4;
const WAND_RANGE: f32 = 60.0;
const WAND_SCREEN_TOP: i32 = 8;
const WAND_POUNCE_DELAY_MIN: f32 = 2.5;
const WAND_POUNCE_DELAY_MAX: f32 = 6.5;
const BUBBLE_MAX: usize = 10;
const BUBBLE_SPAWN_DIST: f32 = 20.0;
const BUBBLE_SPAWN_SPEED_MIN: f32 = 12.0;
const BUBBLE_FALL_SPEED: f32 = 9.0;
const BUBBLE_DRIFT_SPEED: f32 = 2.5;
const BUBBLE_POP_FPS: f32 = 7.0;
const BUBBLE_POP_DURATION: f32 = 4.0 / BUBBLE_POP_FPS;

const BUBBLE_SURPRISED_POSES: &[PoseId] = &[
    PoseId::LeaningForwardSideCrazy,
    PoseId::PlayfulForwardWowed,
    PoseId::SittingForwardShocked,
    PoseId::StandingSideCrazy,
];

const REJECTION_POSES: &[PoseId] = &[
    PoseId::StandingSideNeutralLookingDown,
    PoseId::SittingSideLookingDown,
    PoseId::LayingSideNeutral2,
    PoseId::LayingSideBored,
    PoseId::SittingSillySideNeutral,
    PoseId::StandingSideAnnoyed,
    PoseId::LayingSideAnnoyed,
    PoseId::LayingSideContent,
    PoseId::SittingLickingSideLickingLeg,
];

// -- Eye tracking --------------------------------------------------------

fn compute_eye_frame(offset_x: f32, mirror: bool) -> usize {
    let mut t = (offset_x / BALL_ROLL_RANGE).clamp(-1.0, 1.0);
    if mirror {
        t = -t;
    }
    // Round to nearest, then clamp.
    let raw = 2.0 - t * 2.0;
    let rounded = (raw + 0.5).floor() as i32;
    rounded.clamp(0, 4) as usize
}

// -- Phase enum ----------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Pre-start placeholder. Every variant overrides this in `enter()`.
    Excited,
    Watching,
    Pouncing,
    Recovering,
    Catching,
}

// -- Behavior ------------------------------------------------------------

pub struct PlayingBehavior {
    variant: PlayVariant,
    phase: Phase,
    phase_timer: f32,
    session_timer: f32,
    rejecting: bool,
    rejection_timeout: f32,
    completed_natural: bool,
    progress_v: f32,
    pose_id: PoseId,
    eye_frame: Option<usize>,

    // Cached character screen position (refreshed from ctx.scene_camera_x).
    play_char_x: f32,
    play_char_y: f32,

    // Shared sliding-toy state.
    toy_x: f32,
    toy_vel_x: f32,
    toy_facing_right: bool,
    toy_anim_dist: f32,
    ball_rotation: f32,

    // Pounce.
    pounce_direction: i32,
    pounce_timer: f32,
    pounces_total: u32,
    pounces_done: u32,

    // Laser.
    laser_base_x: f32,
    laser_wobble_phase: f32,
    laser_line_x_top: i32,

    // String / feather verlet rope.
    str_node_count: usize,
    str_seg_lens: [f32; FEATHER_SEGMENTS],
    str_px: [f32; FEATHER_SEGMENTS],
    str_py: [f32; FEATHER_SEGMENTS],
    str_ox: [f32; FEATHER_SEGMENTS],
    str_oy: [f32; FEATHER_SEGMENTS],
    str_anchor_x: f32,
    str_needs_init: bool,

    // Bubbles.
    bubbles: Vec<Bubble, BUBBLE_MAX>,
    wand_spawn_dist: f32,
    had_bubbles: bool,
    bubble_pose_timer: f32,
}

#[derive(Clone, Copy)]
struct Bubble {
    size: u8,    // 0 = BUBBLE1, 1 = BUBBLE2
    x: f32,
    y: f32,
    drift: f32,
    pop_timer: f32, // <0 = falling, >=0 = popping
}

impl PlayingBehavior {
    pub fn new(variant: PlayVariant) -> Self {
        Self {
            variant,
            phase: Phase::Excited,
            phase_timer: 0.0,
            session_timer: 0.0,
            rejecting: false,
            rejection_timeout: 15.0,
            completed_natural: false,
            progress_v: 0.0,
            pose_id: PoseId::SittingForwardShocked,
            eye_frame: None,

            play_char_x: 64.0,
            play_char_y: 40.0,

            toy_x: 64.0,
            toy_vel_x: 0.0,
            toy_facing_right: false,
            toy_anim_dist: 0.0,
            ball_rotation: 0.0,

            pounce_direction: 1,
            pounce_timer: 0.0,
            pounces_total: 3,
            pounces_done: 0,

            laser_base_x: 64.0,
            laser_wobble_phase: 0.0,
            laser_line_x_top: 64,

            str_node_count: STRING_SEGMENTS,
            str_seg_lens: [0.0; FEATHER_SEGMENTS],
            str_px: [0.0; FEATHER_SEGMENTS],
            str_py: [0.0; FEATHER_SEGMENTS],
            str_ox: [0.0; FEATHER_SEGMENTS],
            str_oy: [0.0; FEATHER_SEGMENTS],
            str_anchor_x: 64.0,
            str_needs_init: true,

            bubbles: Vec::new(),
            wand_spawn_dist: 0.0,
            had_bubbles: false,
            bubble_pose_timer: 0.0,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        if ctx.playfulness < 40.0 {
            return false;
        }
        !solo_toys_in_inventory(ctx).is_empty()
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let lo = 100.0 - ctx.playfulness * 1.5;
        let hi = ctx.playfulness * 1.5;
        let mut base = rand::rand_range_f32(rng, lo, hi);
        if ctx.in_familiar_location {
            base *= 0.85;
        }
        base.max(0.0) as u32
    }

    fn rejection_chance(ctx: &GameContext) -> f32 {
        let mut complement = 1.0_f32;
        let thresholds = [
            (ctx.energy, 35.0),
            (ctx.playfulness, 35.0),
            (ctx.fullness, 25.0),
            (ctx.comfort, 30.0),
            (ctx.focus, 25.0),
            (ctx.curiosity, 25.0),
            (ctx.affection, 30.0),
            (ctx.sociability, 25.0),
            (ctx.courage, 25.0),
        ];
        for (val, threshold) in thresholds {
            if val < threshold {
                let deficit = (threshold - val) / threshold;
                complement *= 1.0 - deficit;
            }
        }
        1.0 - complement
    }

    fn scene_bounds(ctx: &GameContext) -> (i32, i32) {
        (ctx.scene_x_min + 15, ctx.scene_x_max - 15)
    }

    // Choose a pounce delay for the current variant.
    fn pounce_delay_range(&self) -> (f32, f32) {
        match self.variant {
            PlayVariant::Laser => (LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX),
            PlayVariant::String | PlayVariant::Feather => {
                (STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX)
            }
            PlayVariant::Bubbles => (WAND_POUNCE_DELAY_MIN, WAND_POUNCE_DELAY_MAX),
            _ => (BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX),
        }
    }

    fn start_sliding_toy(&mut self, ctx: &mut GameContext, pose: PoseId) {
        self.toy_x = self.play_char_x;
        self.toy_vel_x = 0.0;
        self.pounces_total =
            rand::rand_range_u32(&mut ctx.rng, POUNCE_COUNT_MIN, POUNCE_COUNT_MAX);
        self.pounces_done = 0;
        let (lo, hi) = self.pounce_delay_range();
        self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
        self.eye_frame = Some(compute_eye_frame(0.0, false));
        self.phase = Phase::Watching;
        self.pose_id = pose;
    }

    fn start_string(&mut self, ctx: &mut GameContext) {
        self.str_node_count = if matches!(self.variant, PlayVariant::Feather) {
            FEATHER_SEGMENTS
        } else {
            STRING_SEGMENTS
        };
        let n_segs = self.str_node_count - 1;
        for i in 0..n_segs {
            let t = i as f32 / (n_segs.max(1) - 1).max(1) as f32;
            self.str_seg_lens[i] =
                STRING_SEG_LEN_TOP + (STRING_SEG_LEN_BOT - STRING_SEG_LEN_TOP) * t;
        }
        self.str_needs_init = true;
        self.pounces_total =
            rand::rand_range_u32(&mut ctx.rng, POUNCE_COUNT_MIN, POUNCE_COUNT_MAX);
        self.pounces_done = 0;
        let (lo, hi) = self.pounce_delay_range();
        self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
        self.phase = Phase::Watching;
        self.pose_id = PoseId::PlayfulForwardWowed;
    }

    fn refresh_screen_xy(&mut self, ctx: &GameContext, character: &Character) {
        self.play_char_x = (character.pos.x - ctx.scene_camera_x) as f32;
        self.play_char_y = character.pos.y as f32;
    }

    fn update_toy_physics(
        &mut self,
        ctx: &GameContext,
        dt: f32,
        push: f32,
        max_spd: f32,
        friction: f32,
        bounce: f32,
    ) {
        if ctx.input.left {
            self.toy_vel_x -= push * dt;
        }
        if ctx.input.right {
            self.toy_vel_x += push * dt;
        }
        if self.toy_vel_x > max_spd {
            self.toy_vel_x = max_spd;
        } else if self.toy_vel_x < -max_spd {
            self.toy_vel_x = -max_spd;
        }
        self.toy_vel_x *= friction.powf(dt);
        self.toy_x += self.toy_vel_x * dt;
        let lo = TOY_SCREEN_MARGIN;
        let hi = 128.0 - TOY_SCREEN_MARGIN;
        if self.toy_x >= hi {
            self.toy_x = hi;
            self.toy_vel_x = -self.toy_vel_x.abs() * bounce;
        } else if self.toy_x <= lo {
            self.toy_x = lo;
            self.toy_vel_x = self.toy_vel_x.abs() * bounce;
        }
    }

    fn dispatch_sliding_phase(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) {
        let offset = self.toy_x - self.play_char_x;
        match self.phase {
            Phase::Watching => self.update_watching(ctx, dt, offset),
            Phase::Pouncing => self.update_pounce(ctx, character, dt, true),
            Phase::Recovering => self.update_recovering(ctx, dt, offset),
            Phase::Catching => self.update_catching(ctx, character),
            _ => {}
        }
    }

    fn update_watching(&mut self, ctx: &mut GameContext, dt: f32, offset_x: f32) {
        let (lo, hi) = self.pounce_delay_range();
        self.pounce_timer -= dt;
        if self.pounce_timer <= 0.0 {
            if !self.rejecting {
                self.pounces_done += 1;
                self.begin_pounce(offset_x);
                return;
            }
            self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
        }
        self.progress_v = self.pounces_done as f32 / self.pounces_total.max(1) as f32;
    }

    fn begin_pounce(&mut self, offset_x: f32) {
        self.pounce_direction = if offset_x >= 0.0 { 1 } else { -1 };
        self.pose_id = PoseId::LeaningForwardSidePounce;
        self.eye_frame = None;
        self.phase = Phase::Pouncing;
        self.phase_timer = 0.0;
    }

    fn update_pounce(
        &mut self,
        ctx: &GameContext,
        character: &mut Character,
        dt: f32,
        zero_vel: bool,
    ) {
        character.pos.x += (self.pounce_direction as f32 * POUNCE_SLIDE_SPEED * dt) as i32;
        character.mirror_h = self.pounce_direction > 0;
        if self.phase_timer >= POUNCE_SLIDE_DURATION {
            let (x_min, x_max) = Self::scene_bounds(ctx);
            character.pos.x = character.pos.x.clamp(x_min, x_max);
            if zero_vel {
                self.toy_vel_x = 0.0;
            }
            self.phase = Phase::Recovering;
            self.phase_timer = 0.0;
            self.pose_id = PoseId::SittingSillySideHappy;
        }
    }

    fn update_recovering(&mut self, ctx: &mut GameContext, dt: f32, offset_x: f32) {
        if self.phase_timer >= POUNCE_RECOVER_DURATION {
            if self.pounces_done >= self.pounces_total {
                self.phase = Phase::Catching;
                self.phase_timer = 0.0;
            } else {
                let (lo, hi) = self.pounce_delay_range();
                self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
                self.eye_frame = Some(compute_eye_frame(offset_x, false));
                self.phase = Phase::Watching;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::PlayfulForwardWowed;
            }
        }
        let _ = dt;
    }

    fn update_catching(&mut self, ctx: &mut GameContext, character: &mut Character) {
        if self.phase_timer >= POUNCE_CATCH_DURATION {
            self.progress_v = 1.0;
            character.play_bursts(&mut ctx.rng, 5);
            self.completed_natural = true;
        }
    }

    // -- Ball / Mouse / Hand -----------------------------------------------

    fn update_ball(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        self.update_toy_physics(
            ctx,
            dt,
            BALL_PUSH_FORCE,
            BALL_MAX_SPEED,
            BALL_FRICTION,
            BALL_BOUNCE_DAMPING,
        );
        let angle = self.toy_vel_x * dt / (YARN_BALL.width as f32 / 2.0)
            * (180.0 / core::f32::consts::PI);
        self.ball_rotation = (self.ball_rotation + angle) % 360.0;
        if self.ball_rotation < 0.0 {
            self.ball_rotation += 360.0;
        }
        self.eye_frame =
            Some(compute_eye_frame(self.toy_x - self.play_char_x, character.mirror_h));
        self.dispatch_sliding_phase(ctx, character, dt);
    }

    fn update_mouse(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        self.update_toy_physics(
            ctx,
            dt,
            BALL_PUSH_FORCE,
            BALL_MAX_SPEED,
            BALL_FRICTION,
            BALL_BOUNCE_DAMPING,
        );
        if self.toy_vel_x.abs() > 2.0 {
            self.toy_facing_right = self.toy_vel_x > 0.0;
        }
        self.eye_frame =
            Some(compute_eye_frame(self.toy_x - self.play_char_x, character.mirror_h));
        self.dispatch_sliding_phase(ctx, character, dt);
    }

    fn update_hand(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        self.update_toy_physics(
            ctx,
            dt,
            HAND_PUSH_FORCE,
            HAND_MAX_SPEED,
            HAND_FRICTION,
            HAND_BOUNCE_DAMPING,
        );
        self.toy_anim_dist += self.toy_vel_x.abs() * dt;
        if self.toy_vel_x.abs() > 2.0 {
            self.toy_facing_right = self.toy_vel_x > 0.0;
        }
        self.eye_frame =
            Some(compute_eye_frame(self.toy_x - self.play_char_x, character.mirror_h));
        self.dispatch_sliding_phase(ctx, character, dt);
    }

    // -- Laser -------------------------------------------------------------

    fn update_laser(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        if ctx.input.left {
            self.laser_base_x -= LASER_USER_SPEED * dt;
        }
        if ctx.input.right {
            self.laser_base_x += LASER_USER_SPEED * dt;
        }
        let lo = TOY_SCREEN_MARGIN;
        let hi = 128.0 - TOY_SCREEN_MARGIN;
        if self.laser_base_x < lo {
            self.laser_base_x = lo;
        } else if self.laser_base_x > hi {
            self.laser_base_x = hi;
        }
        self.laser_wobble_phase += LASER_WOBBLE_SPEED * dt;
        self.toy_x = self.laser_base_x + LASER_WOBBLE_AMPLITUDE * self.laser_wobble_phase.sin();
        self.eye_frame =
            Some(compute_eye_frame(self.toy_x - self.play_char_x, character.mirror_h));
        self.dispatch_sliding_phase(ctx, character, dt);
    }

    // -- String / Feather verlet rope --------------------------------------

    fn str_init_positions(
        &mut self,
        ctx: &mut GameContext,
        anchor_sx: f32,
        anchor_sy: f32,
        floor_y: f32,
    ) {
        let n = self.str_node_count;
        let curve_amp = rand::rand_range_f32(&mut ctx.rng, 4.0, 9.0);
        let curve_dir: f32 = if rand::rand_bool(&mut ctx.rng, 0.5) {
            1.0
        } else {
            -1.0
        };
        let mut cumulative_y = 0.0_f32;
        for i in 0..n {
            let t = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.0
            };
            let x_off = curve_dir * curve_amp * (t * core::f32::consts::PI).sin();
            self.str_px[i] = anchor_sx + x_off;
            self.str_py[i] = anchor_sy + cumulative_y;
            self.str_ox[i] = self.str_px[i];
            self.str_oy[i] = self.str_py[i];
            if i < n - 1 {
                cumulative_y += self.str_seg_lens[i];
            }
        }
        self.str_anchor_x = anchor_sx;

        // Settle for 30 frames at dt = 1/12.
        let settle_dt = 1.0 / 12.0;
        for _ in 0..30 {
            self.str_step_physics(anchor_sx, anchor_sy, floor_y, settle_dt);
        }

        self.str_needs_init = false;
    }

    fn str_step_physics(&mut self, anchor_sx: f32, anchor_sy: f32, floor_y: f32, dt: f32) {
        let n = self.str_node_count;
        let damp = STRING_DAMPING.powf(dt);
        for i in 1..n {
            let px = self.str_px[i];
            let py = self.str_py[i];
            let ox = self.str_ox[i];
            let oy = self.str_oy[i];
            let vx = (px - ox) * damp;
            let vy = (py - oy) * damp + STRING_GRAVITY * dt * dt;
            self.str_ox[i] = px;
            self.str_oy[i] = py;
            self.str_px[i] = px + vx;
            self.str_py[i] = py + vy;
        }
        self.str_ox[0] = self.str_px[0];
        self.str_oy[0] = self.str_py[0];
        self.str_px[0] = anchor_sx;
        self.str_py[0] = anchor_sy;

        for _ in 0..STRING_ITERATIONS {
            for i in 0..(n - 1) {
                let ax = self.str_px[i];
                let ay = self.str_py[i];
                let bx = self.str_px[i + 1];
                let by = self.str_py[i + 1];
                let mut dx = bx - ax;
                let mut dy = by - ay;
                let mut dist = (dx * dx + dy * dy).sqrt();
                if dist < 0.001 {
                    dist = 0.001;
                    dx = 0.0;
                    dy = 0.0;
                }
                let correction = (dist - self.str_seg_lens[i]) / dist * 0.5;
                let cx = dx * correction;
                let cy = dy * correction;
                if i == 0 {
                    self.str_px[i + 1] -= cx * 2.0;
                    self.str_py[i + 1] -= cy * 2.0;
                } else {
                    self.str_px[i] += cx;
                    self.str_py[i] += cy;
                    self.str_px[i + 1] -= cx;
                    self.str_py[i + 1] -= cy;
                }
            }
        }
        for i in 1..n {
            if self.str_py[i] > floor_y {
                self.str_py[i] = floor_y;
                self.str_oy[i] = floor_y;
            }
        }
    }

    fn update_string_physics(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) {
        self.refresh_screen_xy(ctx, character);
        let char_x = self.play_char_x;
        let char_y = self.play_char_y;
        let floor_y = char_y;
        let anchor_sy = char_y + STRING_ANCHOR_Y as f32;

        if self.str_needs_init {
            self.str_init_positions(ctx, char_x, anchor_sy, floor_y);
        }

        if ctx.input.left {
            self.str_anchor_x -= STRING_ANCHOR_SPEED * dt;
        }
        if ctx.input.right {
            self.str_anchor_x += STRING_ANCHOR_SPEED * dt;
        }
        if self.str_anchor_x < TOY_SCREEN_MARGIN {
            self.str_anchor_x = TOY_SCREEN_MARGIN;
        } else if self.str_anchor_x > 128.0 - TOY_SCREEN_MARGIN {
            self.str_anchor_x = 128.0 - TOY_SCREEN_MARGIN;
        }

        self.str_step_physics(self.str_anchor_x, anchor_sy, floor_y, dt);

        let tip_sx = self.str_px[self.str_node_count - 1];
        self.eye_frame = Some(compute_eye_frame(tip_sx - char_x, character.mirror_h));

        if matches!(self.phase, Phase::Watching) {
            self.pounce_timer -= dt;
            if self.pounce_timer <= 0.0 {
                if !self.rejecting {
                    self.pounces_done += 1;
                    let offset = self.str_px[self.str_node_count - 1] - char_x;
                    self.begin_pounce(offset);
                    return;
                }
                let (lo, hi) = self.pounce_delay_range();
                self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
            }
            self.progress_v = self.pounces_done as f32 / self.pounces_total.max(1) as f32;
        }
    }

    fn update_string_pounce(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) {
        character.pos.x += (self.pounce_direction as f32 * POUNCE_SLIDE_SPEED * dt) as i32;
        character.mirror_h = self.pounce_direction > 0;
        self.update_string_physics(ctx, character, dt);
        if self.phase_timer >= POUNCE_SLIDE_DURATION {
            let (x_min, x_max) = Self::scene_bounds(ctx);
            character.pos.x = character.pos.x.clamp(x_min, x_max);
            self.phase = Phase::Recovering;
            self.phase_timer = 0.0;
            self.pose_id = PoseId::SittingSillySideHappy;
        }
    }

    fn update_string_recovering(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) {
        self.update_string_physics(ctx, character, dt);
        if self.phase_timer >= POUNCE_RECOVER_DURATION {
            if self.pounces_done >= self.pounces_total {
                self.phase = Phase::Catching;
                self.phase_timer = 0.0;
            } else {
                let (lo, hi) = self.pounce_delay_range();
                self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
                self.phase = Phase::Watching;
                self.phase_timer = 0.0;
                self.pose_id = PoseId::PlayfulForwardWowed;
            }
        }
    }

    fn update_string(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        match self.phase {
            Phase::Watching => self.update_string_physics(ctx, character, dt),
            Phase::Pouncing => self.update_string_pounce(ctx, character, dt),
            Phase::Recovering => self.update_string_recovering(ctx, character, dt),
            Phase::Catching => self.update_catching(ctx, character),
            _ => {}
        }
    }

    // -- Bubbles -----------------------------------------------------------

    fn update_bubble_particles(&mut self, dt: f32, ground_y: f32) {
        let mut i = 0;
        while i < self.bubbles.len() {
            let mut keep = true;
            let b = &mut self.bubbles[i];
            if b.pop_timer < 0.0 {
                b.y += BUBBLE_FALL_SPEED * dt;
                b.x += b.drift * dt;
                if b.y >= ground_y {
                    b.y = ground_y;
                    b.pop_timer = 0.0;
                }
            } else {
                b.pop_timer += dt;
                if b.pop_timer >= BUBBLE_POP_DURATION {
                    keep = false;
                }
            }
            if keep {
                i += 1;
            } else {
                self.bubbles.swap_remove(i);
            }
        }
    }

    fn update_bubbles_watching(&mut self, ctx: &mut GameContext, dt: f32) {
        let has_bubbles = !self.bubbles.is_empty();
        if has_bubbles && !self.had_bubbles {
            let idx = rand::rand_range_u32(
                &mut ctx.rng,
                0,
                (BUBBLE_SURPRISED_POSES.len() as u32) - 1,
            ) as usize;
            self.pose_id = BUBBLE_SURPRISED_POSES[idx];
            self.had_bubbles = true;
            self.bubble_pose_timer = rand::rand_range_f32(&mut ctx.rng, 1.5, 3.0);
        } else if !has_bubbles && self.had_bubbles {
            self.pose_id = PoseId::SittingForwardNeutral;
            self.had_bubbles = false;
        } else if has_bubbles {
            self.bubble_pose_timer -= dt;
            if self.bubble_pose_timer <= 0.0 {
                let idx = rand::rand_range_u32(
                    &mut ctx.rng,
                    0,
                    (BUBBLE_SURPRISED_POSES.len() as u32) - 1,
                ) as usize;
                self.pose_id = BUBBLE_SURPRISED_POSES[idx];
                self.bubble_pose_timer = rand::rand_range_f32(&mut ctx.rng, 1.5, 3.0);
            }
        }

        self.pounce_timer -= dt;
        if self.pounce_timer <= 0.0 {
            if !self.rejecting {
                self.pounces_done += 1;
                let target = rand::rand_range_f32(
                    &mut ctx.rng,
                    -WAND_RANGE * 0.7,
                    WAND_RANGE * 0.7,
                );
                self.begin_pounce(target);
                return;
            }
            let (lo, hi) = self.pounce_delay_range();
            self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
        }
        self.progress_v = self.pounces_done as f32 / self.pounces_total.max(1) as f32;
    }

    fn update_bubbles_recovering(&mut self, ctx: &mut GameContext, dt: f32) {
        if self.phase_timer >= POUNCE_RECOVER_DURATION {
            if self.pounces_done >= self.pounces_total {
                self.phase = Phase::Catching;
                self.phase_timer = 0.0;
            } else {
                let (lo, hi) = self.pounce_delay_range();
                self.pounce_timer = rand::rand_range_f32(&mut ctx.rng, lo, hi);
                self.phase = Phase::Watching;
                self.phase_timer = 0.0;
                if !self.bubbles.is_empty() {
                    let idx = rand::rand_range_u32(
                        &mut ctx.rng,
                        0,
                        (BUBBLE_SURPRISED_POSES.len() as u32) - 1,
                    ) as usize;
                    self.pose_id = BUBBLE_SURPRISED_POSES[idx];
                    self.had_bubbles = true;
                    self.bubble_pose_timer = rand::rand_range_f32(&mut ctx.rng, 1.5, 3.0);
                } else {
                    self.pose_id = PoseId::SittingForwardNeutral;
                    self.had_bubbles = false;
                }
            }
        }
        let _ = dt;
    }

    fn update_bubbles_catching(&mut self, ctx: &mut GameContext, character: &mut Character) {
        if self.phase_timer >= POUNCE_CATCH_DURATION && self.bubbles.is_empty() {
            self.progress_v = 1.0;
            character.play_bursts(&mut ctx.rng, 5);
            self.completed_natural = true;
        }
    }

    fn update_bubbles(&mut self, ctx: &mut GameContext, character: &mut Character, dt: f32) {
        self.update_bubble_particles(dt, self.play_char_y);

        if matches!(self.phase, Phase::Catching) {
            self.update_bubbles_catching(ctx, character);
            return;
        }

        self.update_toy_physics(
            ctx,
            dt,
            WAND_PUSH_FORCE,
            WAND_MAX_SPEED,
            WAND_FRICTION,
            WAND_BOUNCE_DAMPING,
        );
        if self.toy_vel_x.abs() > 2.0 {
            self.toy_facing_right = self.toy_vel_x > 0.0;
        }

        let speed = self.toy_vel_x.abs();
        if speed >= BUBBLE_SPAWN_SPEED_MIN && self.bubbles.len() < BUBBLE_MAX {
            self.wand_spawn_dist += speed * dt;
            while self.wand_spawn_dist >= BUBBLE_SPAWN_DIST
                && self.bubbles.len() < BUBBLE_MAX
            {
                self.wand_spawn_dist -= BUBBLE_SPAWN_DIST;
                let drift = rand::rand_range_f32(
                    &mut ctx.rng,
                    -BUBBLE_DRIFT_SPEED,
                    BUBBLE_DRIFT_SPEED,
                );
                let size = rand::rand_range_u32(&mut ctx.rng, 0, 1) as u8;
                let wy = (WAND_SCREEN_TOP + BUBBLE_WAND.height as i32 / 2) as f32;
                let _ = self.bubbles.push(Bubble {
                    size,
                    x: self.toy_x,
                    y: wy,
                    drift,
                    pop_timer: -1.0,
                });
            }
        }

        match self.phase {
            Phase::Watching => self.update_bubbles_watching(ctx, dt),
            Phase::Pouncing => self.update_pounce(ctx, character, dt, false),
            Phase::Recovering => self.update_bubbles_recovering(ctx, dt),
            _ => {}
        }
    }

    fn play_bonus_table(&self) -> heapless::Vec<(StatId, f32), 10> {
        let mut b: heapless::Vec<(StatId, f32), 10> = heapless::Vec::new();
        let (play, energy, focus, fit, ful, cour) = match self.variant {
            PlayVariant::String => (-8.0, -3.0, -1.0, 1.5, 1.5, 0.4),
            PlayVariant::Feather => (-6.0, -5.0, -1.0, 1.5, 1.5, 0.4),
            PlayVariant::Ball => (-8.0, -4.0, -1.0, 1.5, 1.5, 0.4),
            PlayVariant::Mouse => (-8.0, -4.0, -1.0, 1.5, 1.5, 0.4),
            PlayVariant::Hand => (-6.0, -3.0, -1.0, 1.0, 1.0, 0.3),
            PlayVariant::Laser => (-6.0, -3.0, -1.0, 1.5, 1.5, 0.4),
            PlayVariant::Bubbles => (-6.0, -3.0, -1.0, 1.5, 1.5, 0.4),
        };
        let _ = b.push((StatId::Playfulness, play));
        let _ = b.push((StatId::Energy, energy));
        let _ = b.push((StatId::Focus, focus));
        let _ = b.push((StatId::Fitness, fit));
        let _ = b.push((StatId::Fulfillment, ful));
        let _ = b.push((StatId::Courage, cour));
        b
    }
}

impl Behavior for PlayingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Playing
    }
    fn progress(&self) -> f32 {
        self.progress_v.clamp(0.0, 1.0)
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }
    fn eye_frame_override(&self) -> Option<usize> {
        self.eye_frame
    }
    fn captures_dpad(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.refresh_screen_xy(ctx, character);
        self.session_timer = 0.0;
        self.phase_timer = 0.0;
        self.eye_frame = None;
        self.progress_v = 0.0;
        self.completed_natural = false;
        self.bubbles.clear();

        let chance = Self::rejection_chance(ctx);
        self.rejecting = rand::rand_f32(&mut ctx.rng) < chance;

        match self.variant {
            PlayVariant::Ball => {
                self.ball_rotation = 0.0;
                self.start_sliding_toy(ctx, PoseId::PlayfulForwardWowed);
            }
            PlayVariant::Mouse => {
                self.toy_facing_right = false;
                self.start_sliding_toy(ctx, PoseId::PlayfulForwardWowed);
            }
            PlayVariant::Hand => {
                self.toy_facing_right = false;
                self.toy_anim_dist = 0.0;
                self.start_sliding_toy(ctx, PoseId::PlayfulForwardWowed);
            }
            PlayVariant::Laser => {
                self.laser_base_x = self.play_char_x;
                self.laser_wobble_phase = 0.0;
                self.laser_line_x_top =
                    rand::rand_range_u32(&mut ctx.rng, 20, 108) as i32;
                self.start_sliding_toy(ctx, PoseId::PlayfulForwardWowed);
            }
            PlayVariant::String | PlayVariant::Feather => {
                self.start_string(ctx);
            }
            PlayVariant::Bubbles => {
                self.toy_facing_right = true;
                self.wand_spawn_dist = 0.0;
                self.bubbles.clear();
                self.had_bubbles = false;
                self.bubble_pose_timer = 0.0;
                self.start_sliding_toy(ctx, PoseId::SittingForwardNeutral);
            }
        }

        if self.rejecting {
            let idx = rand::rand_range_u32(
                &mut ctx.rng,
                0,
                (REJECTION_POSES.len() as u32) - 1,
            ) as usize;
            self.pose_id = REJECTION_POSES[idx];
            self.rejection_timeout = rand::rand_range_f32(&mut ctx.rng, 10.0, 20.0);
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        self.session_timer += dt;

        self.refresh_screen_xy(ctx, character);

        if self.rejecting && self.session_timer >= self.rejection_timeout {
            return BehaviorState::Completed;
        }

        // Player-initiated stop. Pre-minimum sessions don't earn bonuses.
        if ctx.input.b_just_pressed {
            if self.session_timer < PLAY_MIN_DURATION {
                self.progress_v = 0.0;
            }
            return BehaviorState::Completed;
        }

        match self.variant {
            PlayVariant::Ball => self.update_ball(ctx, character, dt),
            PlayVariant::Mouse => self.update_mouse(ctx, character, dt),
            PlayVariant::Hand => self.update_hand(ctx, character, dt),
            PlayVariant::Laser => self.update_laser(ctx, character, dt),
            PlayVariant::String | PlayVariant::Feather => self.update_string(ctx, character, dt),
            PlayVariant::Bubbles => self.update_bubbles(ctx, character, dt),
        }

        if self.completed_natural {
            BehaviorState::Completed
        } else {
            BehaviorState::Running
        }
    }

    fn next(&self, _ctx: &GameContext) -> Option<NextBehavior> {
        if self.rejecting {
            Some(NextBehavior::Meandering)
        } else {
            None
        }
    }

    fn exit(&mut self, ctx: &mut GameContext, completed: bool) {
        if completed && !self.rejecting {
            ctx.milestone_played = true;
            // Decrement durability on the toy that was used.
            let variant = self.variant;
            let toy_variant = match variant {
                PlayVariant::String => Some(crate::context::ToyVariant::String_),
                PlayVariant::Feather => Some(crate::context::ToyVariant::Feather),
                PlayVariant::Mouse => Some(crate::context::ToyVariant::Mouse),
                PlayVariant::Ball => Some(crate::context::ToyVariant::Ball),
                PlayVariant::Bubbles => Some(crate::context::ToyVariant::Bubbles),
                PlayVariant::Laser => Some(crate::context::ToyVariant::Laser),
                PlayVariant::Hand => None,
            };
            if let Some(tv) = toy_variant {
                if let Some(idx) = ctx.find_toy(tv) {
                    let t = &mut ctx.toys[idx];
                    t.durability = t.durability.saturating_sub(1);
                }
            }
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        if self.rejecting {
            return;
        }
        let mut bonus = self.play_bonus_table();

        // apply_location_bonus (does NOT call super, no fav_weather)
        if matches!(
            ctx.last_main_scene,
            SceneId::Outside | SceneId::Treehouse | SceneId::Inside
        ) {
            common::bonus_scale(&mut bonus, StatId::Energy, 0.75);
            common::bonus_scale(&mut bonus, StatId::Playfulness, 0.75);
            common::bonus_add(&mut bonus, StatId::Fitness, 1.0);
        }
        common::bonus_add(&mut bonus, StatId::Loyalty, 0.5);

        // Favourite / least-favourite toy modifier (after location bonus, before
        // progress scaling).
        let fav_match = ctx
            .fav_toy
            .map(|tv| tv.to_play_variant() == self.variant)
            .unwrap_or(false);
        let least_match = ctx
            .least_fav_toy
            .map(|tv| tv.to_play_variant() == self.variant)
            .unwrap_or(false);
        if fav_match {
            common::bonus_scale(&mut bonus, StatId::Fitness, 1.2);
            common::bonus_scale(&mut bonus, StatId::Fulfillment, 1.2);
            common::bonus_scale(&mut bonus, StatId::Courage, 1.2);
            common::bonus_scale(&mut bonus, StatId::Loyalty, 1.2);
        } else if least_match {
            common::bonus_scale(&mut bonus, StatId::Fitness, 0.85);
            common::bonus_scale(&mut bonus, StatId::Fulfillment, 0.85);
            common::bonus_scale(&mut bonus, StatId::Courage, 0.85);
            common::bonus_scale(&mut bonus, StatId::Loyalty, 0.85);
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
        let char_x = char_screen.x;
        let char_y = char_screen.y;
        let _ = mirror_h;

        match self.variant {
            PlayVariant::Ball => self.draw_ball(renderer, char_y),
            PlayVariant::Mouse => self.draw_mouse(renderer, char_y),
            PlayVariant::Hand => self.draw_hand(renderer, char_y),
            PlayVariant::Laser => self.draw_laser(renderer, char_y),
            PlayVariant::String | PlayVariant::Feather => self.draw_string(renderer),
            PlayVariant::Bubbles => self.draw_bubbles(renderer),
        }

        let _ = char_x;
    }
}

impl PlayingBehavior {
    fn in_visible_phase(&self) -> bool {
        matches!(
            self.phase,
            Phase::Watching | Phase::Pouncing | Phase::Recovering
        )
    }

    fn draw_simple_toy(
        &self,
        renderer: &mut Renderer,
        char_y: i32,
        sprite: &crate::render::Sprite,
        y_off: i32,
        frame: usize,
        mirror_h: bool,
    ) {
        if !self.in_visible_phase() {
            return;
        }
        let tx = self.toy_x as i32 - sprite.width as i32 / 2;
        let ty = char_y - y_off - sprite.height as i32 / 2;
        renderer.draw_sprite(
            sprite,
            Point::new(tx, ty),
            SpriteOpts {
                frame,
                mirror_h,
                ..Default::default()
            },
        );
    }

    fn draw_ball(&self, renderer: &mut Renderer, char_y: i32) {
        let frame = ((self.ball_rotation / 90.0) as usize) % 4;
        self.draw_simple_toy(renderer, char_y, &YARN_BALL, BALL_Y_OFFSET, frame, false);
    }

    fn draw_mouse(&self, renderer: &mut Renderer, char_y: i32) {
        self.draw_simple_toy(
            renderer,
            char_y,
            &MOUSE_TOY,
            MOUSE_Y_OFFSET,
            0,
            self.toy_facing_right,
        );
    }

    fn draw_hand(&self, renderer: &mut Renderer, char_y: i32) {
        let frame = (self.toy_anim_dist / HAND_ANIM_STEP) as usize % 2;
        self.draw_simple_toy(
            renderer,
            char_y,
            &HAND_SCRATCH,
            HAND_Y_OFFSET,
            frame,
            self.toy_facing_right,
        );
    }

    fn draw_laser(&self, renderer: &mut Renderer, char_y: i32) {
        if !self.in_visible_phase() {
            return;
        }
        let dot_x = self.toy_x as i32;
        let dot_y = char_y - LASER_Y_OFFSET;
        renderer.draw_line(
            Point::new(self.laser_line_x_top, LASER_LINE_TOP_Y),
            Point::new(dot_x, dot_y),
        );
        renderer.draw_circle_filled(Point::new(dot_x, dot_y), LASER_DOT_RADIUS);
    }

    fn draw_string(&self, renderer: &mut Renderer) {
        if !self.in_visible_phase() || self.str_needs_init {
            return;
        }
        let n = self.str_node_count;
        if matches!(self.variant, PlayVariant::Feather) {
            for i in 0..(n - 2) {
                renderer.draw_line(
                    Point::new(self.str_px[i] as i32, self.str_py[i] as i32),
                    Point::new(self.str_px[i + 1] as i32, self.str_py[i + 1] as i32),
                );
            }
            self.draw_feather_tip(
                renderer,
                self.str_px[n - 2],
                self.str_py[n - 2],
                self.str_px[n - 1],
                self.str_py[n - 1],
            );
        } else {
            for i in 0..(n - 1) {
                renderer.draw_line(
                    Point::new(self.str_px[i] as i32, self.str_py[i] as i32),
                    Point::new(self.str_px[i + 1] as i32, self.str_py[i + 1] as i32),
                );
            }
            let tx = self.str_px[n - 1] as i32;
            let ty = self.str_py[n - 1] as i32;
            renderer.draw_circle_filled(Point::new(tx, ty), 1);
        }
    }

    fn draw_feather_tip(
        &self,
        renderer: &mut Renderer,
        base_x: f32,
        base_y: f32,
        tip_x: f32,
        tip_y: f32,
    ) {
        let ddx = tip_x - base_x;
        let ddy = tip_y - base_y;
        let mag = (ddx * ddx + ddy * ddy).sqrt();
        if mag < 0.001 {
            return;
        }
        let fx = ddx / mag;
        let fy = ddy / mag;
        let nx = fy;
        let ny = -fx;

        let l = mag;
        let w = FEATHER_WIDTH;

        let p = |fl: f32, wl: f32| {
            Point::new(
                (base_x + fx * fl + nx * wl) as i32,
                (base_y + fy * fl + ny * wl) as i32,
            )
        };

        let a = p(0.0, 0.0);
        let b = p(l, 0.0);
        let c = p(l * 1.036, w * 0.33);
        let d = p(l * 0.964, w);
        let e = p(l * 0.321, w);
        let f = p(l * 0.214, 0.0);
        let g = p(l * 0.321, w * 0.33);
        let h = p(l * 0.964, w * 0.33);
        let ii = p(l * 0.357, w * 0.67);
        let jj = p(l * 0.929, w * 0.67);

        // Erase three internal lines to carve detail.
        renderer.draw_line_color(f, g, false);
        renderer.draw_line_color(g, h, false);
        renderer.draw_line_color(ii, jj, false);
        // Outline edges.
        renderer.draw_line(a, b);
        renderer.draw_line(c, d);
        renderer.draw_line(d, e);
        renderer.draw_line(e, f);
    }

    fn draw_bubbles(&self, renderer: &mut Renderer) {
        if !matches!(
            self.phase,
            Phase::Watching | Phase::Pouncing | Phase::Recovering | Phase::Catching
        ) {
            return;
        }

        if !matches!(self.phase, Phase::Catching) {
            let wx = self.toy_x as i32 - BUBBLE_WAND.width as i32 / 2;
            renderer.draw_sprite(
                &BUBBLE_WAND,
                Point::new(wx, WAND_SCREEN_TOP),
                SpriteOpts {
                    mirror_h: !self.toy_facing_right,
                    ..Default::default()
                },
            );
        }

        let pw = BUBBLE_POP.width as i32 / 2;
        let ph = BUBBLE_POP.height as i32 / 2;
        for b in &self.bubbles {
            let bx = b.x as i32;
            let by = b.y as i32;
            if b.pop_timer < 0.0 {
                if b.size == 0 {
                    renderer.draw_sprite(&BUBBLE1, Point::new(bx - 3, by - 3), SpriteOpts::default());
                } else {
                    renderer.draw_sprite(&BUBBLE2, Point::new(bx - 4, by - 4), SpriteOpts::default());
                }
            } else {
                let frame = ((b.pop_timer * BUBBLE_POP_FPS) as usize).min(3);
                renderer.draw_sprite(
                    &BUBBLE_POP,
                    Point::new(bx - pw, by - ph),
                    SpriteOpts {
                        frame,
                        ..Default::default()
                    },
                );
            }
        }
    }
}
