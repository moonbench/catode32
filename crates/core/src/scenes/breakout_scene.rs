//! Breakout / brick-breaker minigame. Special bricks drop a paw to catch, or
//! now and then a powerup: a longer paddle, multi-ball, a magnet, a barrier
//! under the paddle, a fireball that punches through bricks up to the top,
//! claws that scratch bricks on A, a hairball that blows up a 3x3 patch, or a
//! treat magnet that pulls falling paws toward the paddle.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use super::pawpong_scene::{
    draw_glyph, GLYPH_BARRIER, GLYPH_FIRE, GLYPH_LONG, GLYPH_MAGNET, GLYPH_MULTI, GLYPH_SIZE,
};
use crate::{
    assets::{minigame_assets::PAW_SMALL1, minigame_character::CAT_AVATAR1},
    board::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_bool, rand_range_u32, xorshift32},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
};

const DISPLAY_W: i32 = DISPLAY_WIDTH as i32;
const DISPLAY_H: i32 = DISPLAY_HEIGHT as i32;
const CHAR_W: i32 = 6; // FONT_6X10

// Brick dimensions
const BRICK_WIDTH: i32 = 6;
const BRICK_HEIGHT: i32 = 3;
const BRICK_GAP: i32 = 1;

// Ball / paddle
const BALL_SIZE: i32 = 3;
const PADDLE_WIDTH: i32 = 20;
const PADDLE_HEIGHT: i32 = 2;
const PADDLE_Y: i32 = 60;

// Paddle physics
const PADDLE_MAX_SPEED: f32 = 120.0;
const PADDLE_ACCELERATION: f32 = 800.0;
const PADDLE_FRICTION: f32 = 300.0;

// Ball physics
const BALL_SPEED: f32 = 36.0;
/// Bounce angle from vertical at the very end of the paddle (60 degrees).
const MAX_ANGLE: f32 = core::f32::consts::PI / 3.0;
const MAX_BALLS: usize = 3;

// Brick grid layout
const BRICK_ROWS: usize = 5;
const BRICK_COLS: usize = 18;
const BRICK_COUNT: usize = BRICK_ROWS * BRICK_COLS;
const BRICK_START_X: i32 = 2;
const BRICK_START_Y: i32 = 2;
const SPECIAL_BRICK_COUNT: usize = 12;

// Falling paws and powerups
const DROP_FALL_SPEED: f32 = 40.0;
/// Powerups fall in a box the size of a paw: a border around a 5x5 glyph.
const DROP_SIZE: i32 = GLYPH_SIZE + 2;
/// Cap on simultaneous drops. At most `SPECIAL_BRICK_COUNT` can be spawned
/// across a game; doubled for safety headroom.
const MAX_DROPS: usize = 32;
/// Chance a special brick drops a powerup instead of a paw.
const POWERUP_CHANCE: f32 = 1.0 / 3.0;

// Powerups
const LONG_PADDLE_WIDTH: i32 = 30;
/// Paddle hits the long paddle lasts for.
const LONG_HITS: u8 = 5;
/// Angle between the ball that splits and each new ball.
const MULTI_SPREAD: f32 = 0.4;
const MAGNET_CATCHES: u8 = 3;
/// A held ball launches on its own after this long so play can't stall.
const MAGNET_MAX_HOLD: f32 = 3.0;
/// A barrier is a one-hit wall along the bottom row.
const BARRIER_Y: i32 = DISPLAY_H - 1;
/// Claws: A scratches a pair of shots up from the ends of the paddle.
const CLAWS_TIME: f32 = 8.0;
const CLAW_COOLDOWN: f32 = 0.35;
const SHOT_SPEED: f32 = 90.0;
const SHOT_H: i32 = 3;
const MAX_SHOTS: usize = 6;
/// Treat magnet: falling drops drift toward the paddle this fast.
const TREAT_TIME: f32 = 30.0;
const TREAT_PULL: f32 = 30.0;

// 5x5 glyphs for the powerups Paw Pong doesn't have.
const GLYPH_CLAWS: [u8; 5] = [0b10101, 0b10101, 0b10101, 0b01010, 0b01010];
const GLYPH_HAIRBALL: [u8; 5] = [0b01010, 0b10101, 0b01110, 0b10101, 0b01010];
const GLYPH_TREAT: [u8; 5] = [0b10101, 0b00000, 0b01110, 0b11111, 0b01110];

// Cat avatar
const CAT_X: i32 = 0;
const CAT_Y: i32 = DISPLAY_H - 18; // CAT_AVATAR1 height = 18

/// Active powerups show above the score in the cat's column, three to a row.
const HUD_STEP: i32 = GLYPH_SIZE + 1;
const HUD_PER_ROW: i32 = 3;

const BRICK_EMPTY: u8 = 0;
const BRICK_NORMAL: u8 = 1;
const BRICK_SPECIAL: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Playing,
    Win,
    Lose,
}

#[derive(Clone, Copy)]
enum Powerup {
    Long,
    Multi,
    Magnet,
    Barrier,
    Fireball,
    Claws,
    Hairball,
    Treat,
}

impl Powerup {
    const ALL: [Powerup; 8] = [
        Powerup::Long,
        Powerup::Multi,
        Powerup::Magnet,
        Powerup::Barrier,
        Powerup::Fireball,
        Powerup::Claws,
        Powerup::Hairball,
        Powerup::Treat,
    ];

    fn glyph(self) -> &'static [u8; 5] {
        match self {
            Powerup::Long => &GLYPH_LONG,
            Powerup::Multi => &GLYPH_MULTI,
            Powerup::Magnet => &GLYPH_MAGNET,
            Powerup::Barrier => &GLYPH_BARRIER,
            Powerup::Fireball => &GLYPH_FIRE,
            Powerup::Claws => &GLYPH_CLAWS,
            Powerup::Hairball => &GLYPH_HAIRBALL,
            Powerup::Treat => &GLYPH_TREAT,
        }
    }
}

/// A ball stuck to the magnet paddle.
#[derive(Clone, Copy)]
struct Held {
    /// Ball centre relative to the paddle centre.
    offset: f32,
    timer: f32,
}

#[derive(Clone, Copy)]
struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// Left the paddle as a fireball and hasn't reached the top yet.
    fire: bool,
    held: Option<Held>,
}

/// A falling paw, or a powerup when `kind` is set.
#[derive(Clone, Copy)]
struct Falling {
    x: f32,
    y: f32,
    kind: Option<Powerup>,
}

/// A claw scratch heading up the screen.
#[derive(Clone, Copy)]
struct Shot {
    x: i32,
    y: f32,
}

pub struct BreakoutScene {
    state: State,
    paddle_x: f32,
    paddle_vx: f32,
    balls: Vec<Ball, MAX_BALLS>,
    bricks: [u8; BRICK_COUNT],
    bricks_remaining: u16,
    drops: Vec<Falling, MAX_DROPS>,
    shots: Vec<Shot, MAX_SHOTS>,
    score: i32,

    long_hits: u8,
    magnet: u8,
    barrier: bool,
    /// The next ball off the paddle is a fireball.
    fireball_ready: bool,
    claws_timer: f32,
    claw_cooldown: f32,
    /// The next brick a ball hits blows up its neighbours too.
    hairball: bool,
    treat_timer: f32,

    session_bricks: u32,
    session_paws: u32,
    session_won: bool,
}

impl BreakoutScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            paddle_x: 0.0,
            paddle_vx: 0.0,
            balls: Vec::new(),
            bricks: [BRICK_EMPTY; BRICK_COUNT],
            bricks_remaining: 0,
            drops: Vec::new(),
            shots: Vec::new(),
            score: 0,
            long_hits: 0,
            magnet: 0,
            barrier: false,
            fireball_ready: false,
            claws_timer: 0.0,
            claw_cooldown: 0.0,
            hairball: false,
            treat_timer: 0.0,
            session_bricks: 0,
            session_paws: 0,
            session_won: false,
        }
    }

    fn reset_game(&mut self, rng: &mut u32, reset_score: bool) {
        self.state = State::Ready;
        self.paddle_x = ((DISPLAY_W - PADDLE_WIDTH) / 2) as f32;
        self.paddle_vx = 0.0;
        self.balls.clear();
        let _ = self.balls.push(Ball { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, fire: false, held: None });
        self.position_ball_on_paddle();
        self.create_bricks(rng);
        self.drops.clear();
        self.shots.clear();
        self.long_hits = 0;
        self.magnet = 0;
        self.barrier = false;
        self.fireball_ready = false;
        self.claws_timer = 0.0;
        self.claw_cooldown = 0.0;
        self.hairball = false;
        self.treat_timer = 0.0;
        if reset_score {
            self.score = 0;
        }
    }

    fn create_bricks(&mut self, rng: &mut u32) {
        for b in &mut self.bricks {
            *b = BRICK_NORMAL;
        }
        self.bricks_remaining = BRICK_COUNT as u16;

        // Pick SPECIAL_BRICK_COUNT distinct indices without replacement.
        let mut indices: Vec<u16, BRICK_COUNT> = Vec::new();
        for i in 0..BRICK_COUNT {
            let _ = indices.push(i as u16);
        }
        let specials = SPECIAL_BRICK_COUNT.min(BRICK_COUNT);
        for _ in 0..specials {
            let len = indices.len() as u32;
            let idx = (xorshift32(rng) % len) as usize;
            let brick_idx = indices.swap_remove(idx) as usize;
            self.bricks[brick_idx] = BRICK_SPECIAL;
        }
    }

    fn paddle_width(&self) -> i32 {
        if self.long_hits > 0 { LONG_PADDLE_WIDTH } else { PADDLE_WIDTH }
    }

    fn position_ball_on_paddle(&mut self) {
        let x = self.paddle_x + (self.paddle_width() / 2) as f32 - (BALL_SIZE / 2) as f32;
        if let Some(b) = self.balls.first_mut() {
            b.x = x;
            b.y = (PADDLE_Y - BALL_SIZE) as f32;
        }
    }

    fn brick_xy(idx: usize) -> (i32, i32) {
        let row = (idx / BRICK_COLS) as i32;
        let col = (idx % BRICK_COLS) as i32;
        (
            BRICK_START_X + col * (BRICK_WIDTH + BRICK_GAP),
            BRICK_START_Y + row * (BRICK_HEIGHT + BRICK_GAP),
        )
    }

    fn launch_ball(&mut self) {
        // 30 degrees from vertical.
        let angle = core::f32::consts::PI / 6.0;
        if let Some(b) = self.balls.first_mut() {
            b.vx = BALL_SPEED * angle.sin();
            b.vy = -BALL_SPEED * angle.cos();
        }
    }

    fn update_paddle(&mut self, dt: f32) {
        if self.paddle_vx > 0.0 {
            self.paddle_vx = (self.paddle_vx - PADDLE_FRICTION * dt).max(0.0);
        } else if self.paddle_vx < 0.0 {
            self.paddle_vx = (self.paddle_vx + PADDLE_FRICTION * dt).min(0.0);
        }
        self.paddle_x += self.paddle_vx * dt;
        self.clamp_paddle();
    }

    fn clamp_paddle(&mut self) {
        // Don't overlap the cat avatar on the left.
        let min_x = CAT_AVATAR1.width as f32 + 1.0;
        let max_x = (DISPLAY_W - self.paddle_width()) as f32;
        if self.paddle_x < min_x {
            self.paddle_x = min_x;
            self.paddle_vx = 0.0;
        } else if self.paddle_x > max_x {
            self.paddle_x = max_x;
            self.paddle_vx = 0.0;
        }
    }

    fn tick_timers(&mut self, dt: f32) {
        self.claws_timer = (self.claws_timer - dt).max(0.0);
        self.claw_cooldown = (self.claw_cooldown - dt).max(0.0);
        self.treat_timer = (self.treat_timer - dt).max(0.0);
    }

    fn activate(&mut self, kind: Powerup) {
        match kind {
            Powerup::Long => {
                if self.long_hits == 0 {
                    // Grow about the centre.
                    self.paddle_x -= ((LONG_PADDLE_WIDTH - PADDLE_WIDTH) / 2) as f32;
                }
                self.long_hits = LONG_HITS;
                self.clamp_paddle();
            }
            Powerup::Multi => self.split_ball(),
            Powerup::Magnet => self.magnet = MAGNET_CATCHES,
            Powerup::Barrier => self.barrier = true,
            Powerup::Fireball => self.fireball_ready = true,
            Powerup::Claws => self.claws_timer = CLAWS_TIME,
            Powerup::Hairball => self.hairball = true,
            Powerup::Treat => self.treat_timer = TREAT_TIME,
        }
    }

    /// Multi-ball: two more balls fan out from a free ball (or straight up
    /// off the paddle if every ball is held), kept within the paddle's
    /// bounce angles.
    fn split_ball(&mut self) {
        let Some(&first) = self.balls.first() else {
            return;
        };
        let b = self
            .balls
            .iter()
            .copied()
            .find(|b| b.held.is_none())
            .unwrap_or(Ball { vx: 0.0, vy: -BALL_SPEED, held: None, ..first });
        let up = if b.vy < 0.0 { -1.0 } else { 1.0 };
        let base = b.vx.atan2(b.vy.abs());
        for spread in [MULTI_SPREAD, -MULTI_SPREAD] {
            let angle = (base + spread).clamp(-MAX_ANGLE, MAX_ANGLE);
            let _ = self.balls.push(Ball {
                vx: BALL_SPEED * angle.sin(),
                vy: up * BALL_SPEED * angle.cos(),
                ..b
            });
        }
    }

    fn update_drops(&mut self, dt: f32) {
        let fall = DROP_FALL_SPEED * dt;
        let paddle_w = self.paddle_width() as f32;
        let paddle_left = self.paddle_x;
        let paddle_right = paddle_left + paddle_w;
        let paddle_top = PADDLE_Y as f32;
        let paddle_bottom = paddle_top + PADDLE_HEIGHT as f32;
        let cat_left = CAT_X as f32;
        let cat_top = CAT_Y as f32;
        let cat_right = cat_left + CAT_AVATAR1.width as f32;
        let cat_bottom = cat_top + CAT_AVATAR1.height as f32;
        let screen_h = DISPLAY_H as f32;
        let size = DROP_SIZE as f32;
        let pull_x = paddle_left + paddle_w / 2.0 - size / 2.0;
        let max_pull = TREAT_PULL * dt;

        let mut i = 0;
        while i < self.drops.len() {
            self.drops[i].y += fall;
            if self.treat_timer > 0.0 {
                let dx = pull_x - self.drops[i].x;
                self.drops[i].x += dx.clamp(-max_pull, max_pull);
            }
            let p = self.drops[i];
            let pr = p.x + size;
            let pb = p.y + size;

            let caught_paddle = pb >= paddle_top
                && p.y < paddle_bottom
                && pr > paddle_left
                && p.x < paddle_right;
            let caught_cat = !caught_paddle
                && pb >= cat_top
                && p.y < cat_bottom
                && pr > cat_left
                && p.x < cat_right;

            if caught_paddle || caught_cat {
                self.drops.swap_remove(i);
                match p.kind {
                    Some(kind) => self.activate(kind),
                    None => {
                        self.score += 1;
                        self.session_paws += 1;
                    }
                }
            } else if p.y > screen_h {
                self.drops.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Clears a brick, dropping a paw or powerup if it was special.
    fn break_brick(&mut self, rng: &mut u32, idx: usize) {
        if self.bricks[idx] == BRICK_SPECIAL {
            let (bx, by) = Self::brick_xy(idx);
            let kind = if rand_bool(rng, POWERUP_CHANCE) {
                Some(Powerup::ALL[rand_range_u32(rng, 0, Powerup::ALL.len() as u32 - 1) as usize])
            } else {
                None
            };
            let _ = self.drops.push(Falling {
                x: (bx + BRICK_WIDTH / 2) as f32 - DROP_SIZE as f32 / 2.0,
                y: (by + BRICK_HEIGHT) as f32,
                kind,
            });
        }

        self.bricks[idx] = BRICK_EMPTY;
        self.bricks_remaining -= 1;
        self.session_bricks += 1;
        if self.bricks_remaining == 0 {
            self.state = State::Win;
            self.session_won = true;
        }
    }

    /// Hairball: breaks a brick and every brick around it.
    fn blast(&mut self, rng: &mut u32, idx: usize) {
        let row = (idx / BRICK_COLS) as i32;
        let col = (idx % BRICK_COLS) as i32;
        for r in row - 1..=row + 1 {
            for c in col - 1..=col + 1 {
                if r < 0 || c < 0 || r >= BRICK_ROWS as i32 || c >= BRICK_COLS as i32 {
                    continue;
                }
                let i = r as usize * BRICK_COLS + c as usize;
                if self.bricks[i] != BRICK_EMPTY {
                    self.break_brick(rng, i);
                    if self.state != State::Playing {
                        return;
                    }
                }
            }
        }
    }

    /// Sends a ball off the paddle at an angle set by where it sits on it,
    /// using up the fireball if there is one.
    fn send_off_paddle(&mut self, b: &mut Ball) {
        let ball_center_x = b.x + BALL_SIZE as f32 / 2.0;
        // -1 left edge, 0 center, 1 right edge -> +/-60 degrees.
        let mut hit_pos = (ball_center_x - self.paddle_x) / self.paddle_width() as f32;
        hit_pos = ((hit_pos - 0.5) * 2.0).clamp(-1.0, 1.0);
        let angle = hit_pos * MAX_ANGLE;
        b.y = (PADDLE_Y - BALL_SIZE) as f32;
        b.vx = BALL_SPEED * angle.sin();
        b.vy = -BALL_SPEED * angle.cos();
        b.held = None;
        b.fire = core::mem::take(&mut self.fireball_ready);
    }

    /// Keeps magnet balls on the paddle and launches them on A or once
    /// they've been held too long. Returns whether any ball was held.
    fn update_held(&mut self, launch_pressed: bool, dt: f32) -> bool {
        let half = self.paddle_width() as f32 / 2.0;
        let mut any = false;
        for i in 0..self.balls.len() {
            let mut b = self.balls[i];
            let Some(mut held) = b.held else {
                continue;
            };
            any = true;
            held.timer += dt;
            // The paddle may have shrunk since the catch.
            held.offset = held.offset.clamp(-half, half);
            b.x = self.paddle_x + half + held.offset - BALL_SIZE as f32 / 2.0;
            b.y = (PADDLE_Y - BALL_SIZE) as f32;
            b.held = Some(held);
            if launch_pressed || held.timer >= MAGNET_MAX_HOLD {
                self.send_off_paddle(&mut b);
            }
            self.balls[i] = b;
        }
        any
    }

    fn fire_claws(&mut self) {
        self.claw_cooldown = CLAW_COOLDOWN;
        let left = self.paddle_x as i32 + 1;
        let right = self.paddle_x as i32 + self.paddle_width() - 2;
        for x in [left, right] {
            let _ = self.shots.push(Shot { x, y: (PADDLE_Y - SHOT_H) as f32 });
        }
    }

    /// The lowest brick in column `x` overlapping rows `top..bottom`.
    fn shot_target(&self, x: i32, top: i32, bottom: i32) -> Option<usize> {
        let rel = x - BRICK_START_X;
        if rel < 0 || rel % (BRICK_WIDTH + BRICK_GAP) >= BRICK_WIDTH {
            return None;
        }
        let col = (rel / (BRICK_WIDTH + BRICK_GAP)) as usize;
        if col >= BRICK_COLS {
            return None;
        }
        (0..BRICK_ROWS).rev().map(|r| r * BRICK_COLS + col).find(|&idx| {
            let (_, by) = Self::brick_xy(idx);
            self.bricks[idx] != BRICK_EMPTY && by < bottom && by + BRICK_HEIGHT > top
        })
    }

    fn update_shots(&mut self, rng: &mut u32, dt: f32) {
        let mut i = 0;
        while i < self.shots.len() {
            let s = self.shots[i];
            let y = s.y - SHOT_SPEED * dt;
            // Check everything the shot swept past this frame.
            if let Some(idx) = self.shot_target(s.x, y as i32, s.y as i32 + SHOT_H) {
                self.shots.swap_remove(i);
                self.break_brick(rng, idx);
                if self.state != State::Playing {
                    return;
                }
            } else if y + (SHOT_H as f32) < 0.0 {
                self.shots.swap_remove(i);
            } else {
                self.shots[i].y = y;
                i += 1;
            }
        }
    }

    fn handle_wall_collisions(b: &mut Ball) {
        if b.x <= 0.0 {
            b.x = 0.0;
            b.vx = b.vx.abs();
        }
        if b.x + BALL_SIZE as f32 >= DISPLAY_W as f32 {
            b.x = (DISPLAY_W - BALL_SIZE) as f32;
            b.vx = -b.vx.abs();
        }
        if b.y <= 0.0 {
            b.y = 0.0;
            b.vy = b.vy.abs();
            b.fire = false;
        }
    }

    fn handle_cat_collision(b: &mut Ball) {
        let cat_x = CAT_X as f32;
        let cat_y = CAT_Y as f32;
        let cat_right = cat_x + CAT_AVATAR1.width as f32;
        let cat_bottom = cat_y + CAT_AVATAR1.height as f32;
        let ball_size = BALL_SIZE as f32;
        let ball_right = b.x + ball_size;
        let ball_bottom = b.y + ball_size;

        if b.x < cat_right
            && ball_right > cat_x
            && b.y < cat_bottom
            && ball_bottom > cat_y
        {
            let overlap_left = ball_right - cat_x;
            let overlap_right = cat_right - b.x;
            let overlap_top = ball_bottom - cat_y;
            let overlap_bottom = cat_bottom - b.y;
            let min_overlap = overlap_left
                .min(overlap_right)
                .min(overlap_top)
                .min(overlap_bottom);

            if min_overlap == overlap_left {
                b.x = cat_x - ball_size;
                b.vx = -b.vx.abs();
            } else if min_overlap == overlap_right {
                b.x = cat_right;
                b.vx = b.vx.abs();
            } else if min_overlap == overlap_top {
                b.y = cat_y - ball_size;
                b.vy = -b.vy.abs();
            } else {
                b.y = cat_bottom;
                b.vy = b.vy.abs();
            }
        }
    }

    /// Bounces the ball off the paddle, or catches it if the magnet is on.
    fn handle_paddle_collision(&mut self, b: &mut Ball) {
        let paddle_top = PADDLE_Y as f32;
        let paddle_left = self.paddle_x;
        let paddle_right = self.paddle_x + self.paddle_width() as f32;
        let ball_bottom = b.y + BALL_SIZE as f32;
        let ball_center_x = b.x + BALL_SIZE as f32 / 2.0;

        if b.vy > 0.0
            && ball_bottom >= paddle_top
            && b.y < paddle_top + PADDLE_HEIGHT as f32
            && ball_center_x >= paddle_left
            && ball_center_x <= paddle_right
        {
            if self.magnet > 0 {
                self.magnet -= 1;
                b.y = paddle_top - BALL_SIZE as f32;
                b.vx = 0.0;
                b.vy = 0.0;
                b.fire = false;
                b.held = Some(Held {
                    offset: ball_center_x - (paddle_left + paddle_right) / 2.0,
                    timer: 0.0,
                });
            } else {
                self.send_off_paddle(b);
            }
            if self.long_hits > 0 {
                self.long_hits -= 1;
                if self.long_hits == 0 {
                    // Shrink about the centre.
                    self.paddle_x += ((LONG_PADDLE_WIDTH - PADDLE_WIDTH) / 2) as f32;
                    self.clamp_paddle();
                }
            }
        }
    }

    fn handle_barrier(&mut self, b: &mut Ball) {
        if self.barrier && b.vy > 0.0 && b.y + BALL_SIZE as f32 >= BARRIER_Y as f32 {
            self.barrier = false;
            b.y = (BARRIER_Y - BALL_SIZE) as f32;
            b.vy = -b.vy.abs();
        }
    }

    /// Breaks the brick the ball touches and bounces off it. A fireball
    /// breaks every brick it touches and keeps going.
    fn handle_brick_collisions(&mut self, rng: &mut u32, b: &mut Ball) {
        let ball_size = BALL_SIZE as f32;
        let ball_right = b.x + ball_size;
        let ball_bottom = b.y + ball_size;
        let bw = BRICK_WIDTH as f32;
        let bh = BRICK_HEIGHT as f32;

        for idx in 0..BRICK_COUNT {
            if self.bricks[idx] == BRICK_EMPTY {
                continue;
            }
            let (bxi, byi) = Self::brick_xy(idx);
            let bx = bxi as f32;
            let by = byi as f32;
            let bx_right = bx + bw;
            let by_bottom = by + bh;

            if b.x < bx_right
                && ball_right > bx
                && b.y < by_bottom
                && ball_bottom > by
            {
                if core::mem::take(&mut self.hairball) {
                    self.blast(rng, idx);
                } else {
                    self.break_brick(rng, idx);
                }
                if self.state != State::Playing {
                    return;
                }
                if b.fire {
                    continue;
                }

                let dx_left = ball_right - bx;
                let dx_right = bx_right - b.x;
                let dy_top = ball_bottom - by;
                let dy_bottom = by_bottom - b.y;
                let min_d = dx_left.min(dx_right).min(dy_top).min(dy_bottom);
                if min_d == dy_top || min_d == dy_bottom {
                    b.vy = -b.vy;
                } else {
                    b.vx = -b.vx;
                }
                return;
            }
        }
    }

    /// Moves every free ball, dropping those that fall off the bottom. Losing
    /// the last one ends the game.
    fn update_balls(&mut self, rng: &mut u32, dt: f32) {
        // Sub-step ball physics so it can't tunnel through bricks at low FPS.
        let max_step = (BALL_SIZE - 1) as f32; // 2 px
        let steps = ((BALL_SPEED * dt / max_step) as i32 + 1).max(1);
        let sub_dt = dt / steps as f32;
        let mut i = 0;
        while i < self.balls.len() {
            let mut b = self.balls[i];
            if b.held.is_some() {
                i += 1;
                continue;
            }
            for _ in 0..steps {
                b.x += b.vx * sub_dt;
                b.y += b.vy * sub_dt;
                Self::handle_wall_collisions(&mut b);
                Self::handle_cat_collision(&mut b);
                self.handle_paddle_collision(&mut b);
                if b.held.is_some() {
                    break;
                }
                self.handle_barrier(&mut b);
                self.handle_brick_collisions(rng, &mut b);
                if self.state != State::Playing {
                    return;
                }
            }
            if b.y > DISPLAY_H as f32 {
                self.balls.swap_remove(i);
            } else {
                self.balls[i] = b;
                i += 1;
            }
        }
        if self.balls.is_empty() {
            self.state = State::Lose;
        }
    }

    fn draw_bricks(&self, renderer: &mut Renderer) {
        for idx in 0..BRICK_COUNT {
            let bt = self.bricks[idx];
            if bt == BRICK_EMPTY {
                continue;
            }
            let (x, y) = Self::brick_xy(idx);
            renderer.draw_rect(
                Point::new(x, y),
                Size::new(BRICK_WIDTH as u32, BRICK_HEIGHT as u32),
                bt == BRICK_NORMAL,
            );
        }
    }

    /// Draws the active powerups above the score, filling rows upward.
    fn draw_hud(&self, renderer: &mut Renderer) {
        let active = [
            (self.long_hits > 0, Powerup::Long),
            (self.magnet > 0, Powerup::Magnet),
            (self.fireball_ready, Powerup::Fireball),
            (self.claws_timer > 0.0, Powerup::Claws),
            (self.hairball, Powerup::Hairball),
            (self.treat_timer > 0.0, Powerup::Treat),
        ];
        let base_y = CAT_Y - 10 - 1 - GLYPH_SIZE;
        let mut n = 0;
        for (on, kind) in active {
            if on {
                let x = CAT_X + 1 + (n % HUD_PER_ROW) * HUD_STEP;
                let y = base_y - (n / HUD_PER_ROW) * HUD_STEP;
                draw_glyph(renderer, kind.glyph(), x, y);
                n += 1;
            }
        }
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        if self.session_bricks == 0 && self.session_paws == 0 {
            return;
        }
        // Note: divides session_bricks by 144.0, not the actual brick count
        // (90). Changing it would shift the reward curve.
        let brick_reward = (self.session_bricks as f32 / 144.0).sqrt();
        let paw_reward = (self.session_paws as f32 / 20.0).sqrt();

        let fulfillment_mult = if self.session_won { 4.0 } else { 2.0 };
        ctx.apply_stat_changes(&[
            (StatId::Playfulness, 5.0 * brick_reward),
            (StatId::Focus,       3.0 * brick_reward + 4.0 * paw_reward),
            (StatId::Fitness,     6.0 * brick_reward),
            (StatId::Sociability, 3.0 * paw_reward),
            (StatId::Loyalty,     1.0 * brick_reward),
            (StatId::Fulfillment, fulfillment_mult * brick_reward),
        ]);
        let coins = (2.0 * brick_reward + 3.0 * paw_reward) as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }
}

impl Scene for BreakoutScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.session_bricks = 0;
        self.session_paws = 0;
        self.session_won = false;
        self.reset_game(&mut rng, true);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        self.apply_rewards(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        // Allow exit at any time.
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }

        let mut rng = ctx.rng;

        // State transitions on A.
        let a = buttons.was_just_pressed(Button::A);
        let was_playing = self.state == State::Playing;
        if a {
            match self.state {
                State::Ready => {
                    self.launch_ball();
                    self.state = State::Playing;
                }
                // Preserve score across a fresh round on win.
                State::Win => self.reset_game(&mut rng, false),
                State::Lose => self.reset_game(&mut rng, true),
                State::Playing => {}
            }
        }

        // Paddle accel runs in every state for responsive feel on Ready.
        if buttons.is_pressed(Button::Left) {
            self.paddle_vx =
                (self.paddle_vx - PADDLE_ACCELERATION * dt).max(-PADDLE_MAX_SPEED);
        }
        if buttons.is_pressed(Button::Right) {
            self.paddle_vx =
                (self.paddle_vx + PADDLE_ACCELERATION * dt).min(PADDLE_MAX_SPEED);
        }

        if self.state == State::Playing {
            self.tick_timers(dt);
        }
        self.update_paddle(dt);

        if self.state == State::Ready {
            self.position_ball_on_paddle();
        } else if self.state == State::Playing {
            // A launches held balls first; otherwise it swipes the claws.
            let launch = a && was_playing;
            let held = self.update_held(launch, dt);
            if launch && !held && self.claws_timer > 0.0 && self.claw_cooldown <= 0.0 {
                self.fire_claws();
            }
            self.update_balls(&mut rng, dt);
            if self.state == State::Playing {
                self.update_shots(&mut rng, dt);
            }
            if self.state == State::Playing {
                self.update_drops(dt);
            }
        }

        ctx.rng = rng;
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        renderer.draw_sprite(
            &CAT_AVATAR1,
            Point::new(CAT_X, CAT_Y),
            SpriteOpts { transparent: true, ..Default::default() },
        );

        self.draw_bricks(renderer);

        renderer.draw_rect(
            Point::new(self.paddle_x as i32, PADDLE_Y),
            Size::new(self.paddle_width() as u32, PADDLE_HEIGHT as u32),
            true,
        );

        if self.barrier {
            for x in (0..DISPLAY_W).step_by(2) {
                renderer.draw_pixel(Point::new(x, BARRIER_Y), true);
            }
        }

        for b in &self.balls {
            renderer.draw_rect(
                Point::new(b.x as i32, b.y as i32),
                Size::new(BALL_SIZE as u32, BALL_SIZE as u32),
                true,
            );
            if b.fire {
                // A short trail behind a fireball.
                let cx = b.x + BALL_SIZE as f32 / 2.0;
                let cy = b.y + BALL_SIZE as f32 / 2.0;
                for k in [3.0, 5.0, 8.0] {
                    let tx = cx - b.vx / BALL_SPEED * k;
                    let ty = cy - b.vy / BALL_SPEED * k;
                    renderer.draw_pixel(Point::new(tx as i32, ty as i32), true);
                }
            }
        }

        for s in &self.shots {
            renderer.draw_rect(Point::new(s.x, s.y as i32), Size::new(1, SHOT_H as u32), true);
        }

        for d in &self.drops {
            let pos = Point::new(d.x as i32, d.y as i32);
            match d.kind {
                Some(kind) => {
                    renderer.draw_rect(pos, Size::new(DROP_SIZE as u32, DROP_SIZE as u32), false);
                    draw_glyph(renderer, kind.glyph(), pos.x + 1, pos.y + 1);
                }
                None => renderer.draw_sprite(
                    &PAW_SMALL1,
                    pos,
                    SpriteOpts { transparent: true, ..Default::default() },
                ),
            }
        }

        self.draw_hud(renderer);

        if self.score > 0 {
            let mut s: String<8> = String::new();
            let _ = write!(s, "{}", self.score);
            let score_x = CAT_X + (CAT_AVATAR1.width as i32 - s.len() as i32 * CHAR_W) / 2;
            let score_y = CAT_Y - 10;
            renderer.draw_text(s.as_str(), Point::new(score_x, score_y));
        }

        match self.state {
            State::Ready => renderer.draw_text(t!("A: Start"), Point::new(32, 30)),
            State::Win => renderer.draw_text(t!("WIN!"), Point::new(50, 30)),
            State::Lose => renderer.draw_text(t!("GAME OVER"), Point::new(34, 30)),
            State::Playing => {}
        }
    }
}
