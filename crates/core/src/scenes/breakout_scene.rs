//! Breakout / brick-breaker minigame.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{minigame_assets::PAW_SMALL1, minigame_character::CAT_AVATAR1},
    board::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::xorshift32,
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

// Brick grid layout
const BRICK_ROWS: usize = 5;
const BRICK_COLS: usize = 18;
const BRICK_COUNT: usize = BRICK_ROWS * BRICK_COLS;
const BRICK_START_X: i32 = 2;
const BRICK_START_Y: i32 = 2;
const SPECIAL_BRICK_COUNT: usize = 12;

// Falling paw
const PAW_FALL_SPEED: f32 = 40.0;
/// Cap on simultaneous falling paws. At most `SPECIAL_BRICK_COUNT` can be
/// spawned across a game; doubled for safety headroom.
const MAX_PAWS: usize = 32;

// Cat avatar
const CAT_X: i32 = 0;
const CAT_Y: i32 = DISPLAY_H - 18; // CAT_AVATAR1 height = 18

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
struct Paw {
    x: f32,
    y: f32,
}

pub struct BreakoutScene {
    state: State,
    paddle_x: f32,
    paddle_vx: f32,
    ball_x: f32,
    ball_y: f32,
    ball_vx: f32,
    ball_vy: f32,
    bricks: [u8; BRICK_COUNT],
    bricks_remaining: u16,
    falling_paws: Vec<Paw, MAX_PAWS>,
    score: i32,

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
            ball_x: 0.0,
            ball_y: 0.0,
            ball_vx: 0.0,
            ball_vy: 0.0,
            bricks: [BRICK_EMPTY; BRICK_COUNT],
            bricks_remaining: 0,
            falling_paws: Vec::new(),
            score: 0,
            session_bricks: 0,
            session_paws: 0,
            session_won: false,
        }
    }

    fn reset_game(&mut self, rng: &mut u32, reset_score: bool) {
        self.state = State::Ready;
        self.paddle_x = ((DISPLAY_W - PADDLE_WIDTH) / 2) as f32;
        self.paddle_vx = 0.0;
        self.position_ball_on_paddle();
        self.create_bricks(rng);
        self.falling_paws.clear();
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

    fn position_ball_on_paddle(&mut self) {
        self.ball_x = self.paddle_x + (PADDLE_WIDTH / 2) as f32 - (BALL_SIZE / 2) as f32;
        self.ball_y = (PADDLE_Y - BALL_SIZE) as f32;
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
        self.ball_vx = BALL_SPEED * angle.sin();
        self.ball_vy = -BALL_SPEED * angle.cos();
    }

    fn update_paddle(&mut self, dt: f32) {
        if self.paddle_vx > 0.0 {
            self.paddle_vx = (self.paddle_vx - PADDLE_FRICTION * dt).max(0.0);
        } else if self.paddle_vx < 0.0 {
            self.paddle_vx = (self.paddle_vx + PADDLE_FRICTION * dt).min(0.0);
        }
        self.paddle_x += self.paddle_vx * dt;

        // Don't overlap the cat avatar on the left.
        let min_x = CAT_AVATAR1.width as f32 + 1.0;
        let max_x = (DISPLAY_W - PADDLE_WIDTH) as f32;
        if self.paddle_x < min_x {
            self.paddle_x = min_x;
            self.paddle_vx = 0.0;
        } else if self.paddle_x > max_x {
            self.paddle_x = max_x;
            self.paddle_vx = 0.0;
        }
    }

    fn update_falling_paws(&mut self, dt: f32) {
        let paw_w = PAW_SMALL1.width as f32;
        let paw_h = PAW_SMALL1.height as f32;
        let fall = PAW_FALL_SPEED * dt;
        let paddle_left = self.paddle_x;
        let paddle_right = paddle_left + PADDLE_WIDTH as f32;
        let paddle_top = PADDLE_Y as f32;
        let paddle_bottom = paddle_top + PADDLE_HEIGHT as f32;
        let cat_left = CAT_X as f32;
        let cat_top = CAT_Y as f32;
        let cat_right = cat_left + CAT_AVATAR1.width as f32;
        let cat_bottom = cat_top + CAT_AVATAR1.height as f32;
        let screen_h = DISPLAY_H as f32;

        let mut score = self.score;
        let mut paws_caught: u32 = 0;

        let mut i = 0;
        while i < self.falling_paws.len() {
            self.falling_paws[i].y += fall;
            let p = self.falling_paws[i];
            let pr = p.x + paw_w;
            let pb = p.y + paw_h;

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
                score += 1;
                paws_caught += 1;
                self.falling_paws.swap_remove(i);
            } else if p.y > screen_h {
                self.falling_paws.swap_remove(i);
            } else {
                i += 1;
            }
        }

        self.score = score;
        self.session_paws += paws_caught;
    }

    fn handle_wall_collisions(&mut self) {
        if self.ball_x <= 0.0 {
            self.ball_x = 0.0;
            self.ball_vx = self.ball_vx.abs();
        }
        if self.ball_x + BALL_SIZE as f32 >= DISPLAY_W as f32 {
            self.ball_x = (DISPLAY_W - BALL_SIZE) as f32;
            self.ball_vx = -self.ball_vx.abs();
        }
        if self.ball_y <= 0.0 {
            self.ball_y = 0.0;
            self.ball_vy = self.ball_vy.abs();
        }
    }

    fn handle_cat_collision(&mut self) {
        let cat_x = CAT_X as f32;
        let cat_y = CAT_Y as f32;
        let cat_right = cat_x + CAT_AVATAR1.width as f32;
        let cat_bottom = cat_y + CAT_AVATAR1.height as f32;
        let ball_size = BALL_SIZE as f32;
        let ball_right = self.ball_x + ball_size;
        let ball_bottom = self.ball_y + ball_size;

        if self.ball_x < cat_right
            && ball_right > cat_x
            && self.ball_y < cat_bottom
            && ball_bottom > cat_y
        {
            let overlap_left = ball_right - cat_x;
            let overlap_right = cat_right - self.ball_x;
            let overlap_top = ball_bottom - cat_y;
            let overlap_bottom = cat_bottom - self.ball_y;
            let min_overlap = overlap_left
                .min(overlap_right)
                .min(overlap_top)
                .min(overlap_bottom);

            if min_overlap == overlap_left {
                self.ball_x = cat_x - ball_size;
                self.ball_vx = -self.ball_vx.abs();
            } else if min_overlap == overlap_right {
                self.ball_x = cat_right;
                self.ball_vx = self.ball_vx.abs();
            } else if min_overlap == overlap_top {
                self.ball_y = cat_y - ball_size;
                self.ball_vy = -self.ball_vy.abs();
            } else {
                self.ball_y = cat_bottom;
                self.ball_vy = self.ball_vy.abs();
            }
        }
    }

    fn handle_paddle_collision(&mut self) {
        let paddle_top = PADDLE_Y as f32;
        let paddle_left = self.paddle_x;
        let paddle_right = self.paddle_x + PADDLE_WIDTH as f32;
        let ball_bottom = self.ball_y + BALL_SIZE as f32;
        let ball_center_x = self.ball_x + BALL_SIZE as f32 / 2.0;

        if self.ball_vy > 0.0
            && ball_bottom >= paddle_top
            && self.ball_y < paddle_top + PADDLE_HEIGHT as f32
            && ball_center_x >= paddle_left
            && ball_center_x <= paddle_right
        {
            self.ball_y = paddle_top - BALL_SIZE as f32;

            // -1 left edge, 0 center, 1 right edge -> +/-60 degrees.
            let mut hit_pos = (ball_center_x - paddle_left) / PADDLE_WIDTH as f32;
            hit_pos = (hit_pos - 0.5) * 2.0;
            let angle = hit_pos * (core::f32::consts::PI / 3.0);
            self.ball_vx = BALL_SPEED * angle.sin();
            self.ball_vy = -BALL_SPEED * angle.cos();
        }
    }

    fn handle_brick_collisions(&mut self) {
        let ball_size = BALL_SIZE as f32;
        let ball_right = self.ball_x + ball_size;
        let ball_bottom = self.ball_y + ball_size;
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

            if self.ball_x < bx_right
                && ball_right > bx
                && self.ball_y < by_bottom
                && ball_bottom > by
            {
                if self.bricks[idx] == BRICK_SPECIAL {
                    let paw_x = bx + bw / 2.0 - PAW_SMALL1.width as f32 / 2.0;
                    let _ = self.falling_paws.push(Paw {
                        x: paw_x,
                        y: by_bottom,
                    });
                }

                self.bricks[idx] = BRICK_EMPTY;
                self.bricks_remaining -= 1;
                self.session_bricks += 1;
                if self.bricks_remaining == 0 {
                    self.state = State::Win;
                    self.session_won = true;
                    return;
                }

                let dx_left = ball_right - bx;
                let dx_right = bx_right - self.ball_x;
                let dy_top = ball_bottom - by;
                let dy_bottom = by_bottom - self.ball_y;
                let min_d = dx_left.min(dx_right).min(dy_top).min(dy_bottom);
                if min_d == dy_top || min_d == dy_bottom {
                    self.ball_vy = -self.ball_vy;
                } else {
                    self.ball_vx = -self.ball_vx;
                }
                return;
            }
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

        // State transitions on A.
        if buttons.was_just_pressed(Button::A) {
            match self.state {
                State::Ready => {
                    self.launch_ball();
                    self.state = State::Playing;
                }
                State::Win => {
                    // Preserve score across a fresh round on win.
                    let mut rng = ctx.rng;
                    self.reset_game(&mut rng, false);
                    ctx.rng = rng;
                }
                State::Lose => {
                    let mut rng = ctx.rng;
                    self.reset_game(&mut rng, true);
                    ctx.rng = rng;
                }
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

        self.update_paddle(dt);

        if self.state == State::Ready {
            self.position_ball_on_paddle();
            return None;
        }
        if self.state != State::Playing {
            return None;
        }

        // Sub-step ball physics so it can't tunnel through bricks at low FPS.
        let max_step = (BALL_SIZE - 1) as f32; // 2 px
        let steps = ((BALL_SPEED * dt / max_step) as i32 + 1).max(1);
        let sub_dt = dt / steps as f32;
        for _ in 0..steps {
            self.ball_x += self.ball_vx * sub_dt;
            self.ball_y += self.ball_vy * sub_dt;
            self.handle_wall_collisions();
            self.handle_cat_collision();
            self.handle_paddle_collision();
            self.handle_brick_collisions();
            if self.state != State::Playing {
                break;
            }
        }

        self.update_falling_paws(dt);

        if self.ball_y > DISPLAY_H as f32 {
            self.state = State::Lose;
        }

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
            Size::new(PADDLE_WIDTH as u32, PADDLE_HEIGHT as u32),
            true,
        );

        renderer.draw_rect(
            Point::new(self.ball_x as i32, self.ball_y as i32),
            Size::new(BALL_SIZE as u32, BALL_SIZE as u32),
            true,
        );

        for paw in &self.falling_paws {
            renderer.draw_sprite(
                &PAW_SMALL1,
                Point::new(paw.x as i32, paw.y as i32),
                SpriteOpts { transparent: true, ..Default::default() },
            );
        }

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
