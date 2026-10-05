//! Snake minigame. Guide the cat to eat spots on a 32x16 grid.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::minigame_assets::{CAT_THIN_DOWN, CAT_THIN_RIGHT, SPOT},
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::xorshift32,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
};

const CELL: i32 = 4;
const GRID_W: i32 = 32; // 128 / CELL
const GRID_H: i32 = 16; // 64 / CELL
const TOTAL_CELLS: usize = (GRID_W * GRID_H) as usize; // 512

const SPEED: f32 = 6.0; // cells per second

const MAX_DIRT: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Playing,
    Win,
    Lose,
}

pub struct SnakeScene {
    state: State,
    snake: Vec<(u8, u8), TOTAL_CELLS>,
    occupied: [u8; TOTAL_CELLS],
    direction: (i8, i8),
    next_dir: (i8, i8),
    move_progress: f32,
    food: Option<(u8, u8)>,
    dirt: Vec<(u8, u8), MAX_DIRT>,
    score: i32,
    session_score: i32,
}

impl SnakeScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            snake: Vec::new(),
            occupied: [0; TOTAL_CELLS],
            direction: (1, 0),
            next_dir: (1, 0),
            move_progress: 0.0,
            food: None,
            dirt: Vec::new(),
            score: 0,
            session_score: 0,
        }
    }

    fn generate_dirt(&mut self, rng: &mut u32) {
        const W: i32 = 128;
        const H: i32 = 64;
        const EDGE: i32 = 13;

        self.dirt.clear();
        let zones = 4 + (xorshift32(rng) % 4) as i32; // 4..=7
        for _ in 0..zones {
            let edge = xorshift32(rng) % 4;
            let (cx, cy) = match edge {
                0 => (
                    4 + (xorshift32(rng) % (W - 8) as u32) as i32, // 4..=W-5
                    1 + (xorshift32(rng) % EDGE as u32) as i32,    // 1..=EDGE
                ),
                1 => (
                    4 + (xorshift32(rng) % (W - 8) as u32) as i32,
                    (H - EDGE - 1) + (xorshift32(rng) % EDGE as u32) as i32, // H-EDGE-1..=H-2
                ),
                2 => (
                    1 + (xorshift32(rng) % EDGE as u32) as i32,
                    4 + (xorshift32(rng) % (H - 8) as u32) as i32,
                ),
                _ => (
                    (W - EDGE - 1) + (xorshift32(rng) % EDGE as u32) as i32,
                    4 + (xorshift32(rng) % (H - 8) as u32) as i32,
                ),
            };
            let mut placed = 0;
            let mut attempts = 0;
            // Re-roll the target each loop iteration.
            while placed < 2 + (xorshift32(rng) % 5) as i32 && attempts < 20 {
                attempts += 1;
                let dx = (xorshift32(rng) % 9) as i32 - 4;
                let dy = (xorshift32(rng) % 9) as i32 - 4;
                let px = (cx + dx).clamp(0, W - 1) as u8;
                let py = (cy + dy).clamp(0, H - 1) as u8;
                let too_close = self.dirt.iter().any(|&(ex, ey)| {
                    (px as i32 - ex as i32).abs() <= 2 && (py as i32 - ey as i32).abs() <= 2
                });
                if !too_close {
                    if self.dirt.push((px, py)).is_err() {
                        return;
                    }
                    placed += 1;
                }
            }
        }
    }

    fn init_game(&mut self, rng: &mut u32) {
        // Accumulate score from the run that just ended before resetting.
        self.session_score += self.score;
        self.generate_dirt(rng);

        let mid_x = (GRID_W / 2) as u8;
        let mid_y = (GRID_H / 2) as u8;
        self.snake.clear();
        let _ = self.snake.push((mid_x, mid_y));
        let _ = self.snake.push((mid_x - 1, mid_y));
        let _ = self.snake.push((mid_x - 2, mid_y));

        for cell in self.occupied.iter_mut() {
            *cell = 0;
        }
        for &(x, y) in &self.snake {
            self.occupied[y as usize * GRID_W as usize + x as usize] = 1;
        }

        self.direction = (1, 0);
        self.next_dir = (1, 0);
        self.move_progress = 0.0;
        self.food = None;
        self.place_food(rng);
        self.score = 0;
        self.state = State::Ready;
    }

    fn place_food(&mut self, rng: &mut u32) {
        let empty_count = TOTAL_CELLS - self.snake.len();
        if empty_count == 0 {
            self.food = None;
            return;
        }
        let mut k = xorshift32(rng) % empty_count as u32;
        for i in 0..TOTAL_CELLS {
            if self.occupied[i] == 0 {
                if k == 0 {
                    let gx = (i as i32 % GRID_W) as u8;
                    let gy = (i as i32 / GRID_W) as u8;
                    self.food = Some((gx, gy));
                    return;
                }
                k -= 1;
            }
        }
    }

    fn step(&mut self, rng: &mut u32) {
        self.direction = self.next_dir;
        let (dx, dy) = self.direction;
        let (hx, hy) = self.snake[0];
        let nx = (hx as i32 + dx as i32).rem_euclid(GRID_W) as u8;
        let ny = (hy as i32 + dy as i32).rem_euclid(GRID_H) as u8;
        let idx = ny as usize * GRID_W as usize + nx as usize;

        if self.occupied[idx] != 0 {
            self.state = State::Lose;
            return;
        }

        if self.snake.insert(0, (nx, ny)).is_err() {
            return;
        }
        self.occupied[idx] = 1;

        if matches!(self.food, Some((fx, fy)) if fx == nx && fy == ny) {
            self.score += 1;
            if self.snake.len() == TOTAL_CELLS {
                self.state = State::Win;
                self.food = None;
                return;
            }
            self.place_food(rng);
        } else if let Some(&(tx, ty)) = self.snake.last() {
            self.occupied[ty as usize * GRID_W as usize + tx as usize] = 0;
            self.snake.pop();
        }
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        let session = self.session_score + self.score;
        if session <= 0 {
            return;
        }
        let progress = (session as f32 / 50.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Focus,       5.0 * progress),
            (StatId::Playfulness, 4.0 * progress),
            (StatId::Fitness,     5.0 * progress),
            (StatId::Sociability, 3.0 * progress + 0.5),
            (StatId::Loyalty,     0.5 * progress),
        ]);
        let coins = (5.0 * progress) as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }

    fn draw_head(&self, renderer: &mut Renderer) {
        let (hx, hy) = self.snake[0];
        let (dx, dy) = self.direction;
        let prog = (self.move_progress * CELL as f32) as i32;

        // Per-direction head draw params.
        // (data, w, h, mirror_h, mirror_v, ox, oy)
        let (data, w, h, mh, mv, ox, oy): (&[u8], u16, u16, bool, bool, i32, i32) = match (dx, dy) {
            (0, 1)  => (CAT_THIN_DOWN,  6, 8, false, false, -1, -2), // down
            (0, -1) => (CAT_THIN_DOWN,  6, 8, false, true,  -1, -2), // up
            (1, 0)  => (CAT_THIN_RIGHT, 8, 6, false, false, -2, -1), // right
            _       => (CAT_THIN_RIGHT, 8, 6, true,  false, -2, -1), // left
        };

        let px = hx as i32 * CELL + dx as i32 * prog + ox;
        let py = hy as i32 * CELL + dy as i32 * prog + oy;

        renderer.draw_sprite_raw(
            data,
            w,
            h,
            Point::new(px, py),
            SpriteOpts {
                transparent: true,
                mirror_h: mh,
                mirror_v: mv,
                ..Default::default()
            },
        );
    }
}

impl Scene for SnakeScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_score = 0;
        self.score = 0;
        let mut rng = ctx.rng;
        self.init_game(&mut rng);
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
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }

        let (dx, dy) = self.direction;
        if buttons.was_just_pressed(Button::Up) && dy == 0 {
            self.next_dir = (0, -1);
        } else if buttons.was_just_pressed(Button::Down) && dy == 0 {
            self.next_dir = (0, 1);
        } else if buttons.was_just_pressed(Button::Left) && dx == 0 {
            self.next_dir = (-1, 0);
        } else if buttons.was_just_pressed(Button::Right) && dx == 0 {
            self.next_dir = (1, 0);
        }

        if buttons.was_just_pressed(Button::A) {
            match self.state {
                State::Ready => self.state = State::Playing,
                State::Win | State::Lose => {
                    let mut rng = ctx.rng;
                    self.init_game(&mut rng);
                    ctx.rng = rng;
                    self.state = State::Playing;
                }
                State::Playing => {}
            }
        }

        if self.state != State::Playing {
            return None;
        }

        self.move_progress += SPEED * dt;
        let mut rng = ctx.rng;
        while self.move_progress >= 1.0 {
            self.move_progress -= 1.0;
            self.step(&mut rng);
            if self.score > ctx.snake_high_score {
                ctx.snake_high_score = self.score;
            }
            if self.state != State::Playing {
                break;
            }
        }
        ctx.rng = rng;

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        match self.state {
            State::Ready => {
                renderer.draw_text(t!("SNAKE"), Point::new(44, 12));
                renderer.draw_text(t!("A: START"), Point::new(32, 28));
                return;
            }
            State::Win => {
                renderer.draw_text(t!("YOU WIN!"), Point::new(32, 16));
                let mut s: String<16> = String::new();
                let _ = write!(s, "SCORE:{}", self.score);
                renderer.draw_text(&s, Point::new(28, 30));
                renderer.draw_text(t!("A: RETRY"), Point::new(32, 44));
                return;
            }
            State::Lose => {
                renderer.draw_text(t!("GAME OVER"), Point::new(28, 16));
                let mut s: String<16> = String::new();
                let _ = write!(s, "SCORE:{}", self.score);
                renderer.draw_text(&s, Point::new(28, 30));
                renderer.draw_text(t!("A: RETRY"), Point::new(32, 44));
                return;
            }
            State::Playing => {}
        }

        // Dirt pixels.
        for &(px, py) in &self.dirt {
            renderer.draw_rect(
                Point::new(px as i32, py as i32),
                Size::new(1, 1),
                true,
            );
        }

        // Food.
        if let Some((fx, fy)) = self.food {
            renderer.draw_sprite(
                &SPOT,
                Point::new(fx as i32 * CELL, fy as i32 * CELL),
                SpriteOpts { transparent: true, ..Default::default() },
            );
        }

        // Body segments (between head and tail).
        let slen = self.snake.len();
        if slen >= 2 {
            for i in 1..slen - 1 {
                let (gx, gy) = self.snake[i];
                renderer.draw_rect(
                    Point::new(gx as i32 * CELL, gy as i32 * CELL),
                    Size::new(CELL as u32, CELL as u32),
                    true,
                );
            }
        }

        // Tail: 4x2 (horizontal) or 2x4 (vertical), oriented toward prev segment.
        if slen > 1 {
            let (tx, ty) = self.snake[slen - 1];
            let (px, py) = self.snake[slen - 2];
            let mut tdx = px as i32 - tx as i32;
            let mut tdy = py as i32 - ty as i32;
            if tdx > 1 {
                tdx = -1;
            } else if tdx < -1 {
                tdx = 1;
            }
            if tdy > 1 {
                tdy = -1;
            } else if tdy < -1 {
                tdy = 1;
            }
            if tdx != 0 {
                renderer.draw_rect(
                    Point::new(tx as i32 * CELL, ty as i32 * CELL + 1),
                    Size::new(CELL as u32, 2),
                    true,
                );
            } else {
                let _ = tdy;
                renderer.draw_rect(
                    Point::new(tx as i32 * CELL + 1, ty as i32 * CELL),
                    Size::new(2, CELL as u32),
                    true,
                );
            }
        }

        // Head with sub-cell interpolation.
        if !self.snake.is_empty() {
            self.draw_head(renderer);
        }
    }
}
