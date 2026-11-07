//! Pest Control, a cat's take on Space Invaders. A formation of pests marches
//! side to side and creeps down the screen. The cat shoots them from below and
//! ducks under thin bunkers that wear away as shots hit them. Waves keep
//! coming, each one starting lower and moving faster, until the cat is hit.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::effects::{POOF, POOF_FRAME_DUR},
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

// 5x5 cat, one byte per row.
const CAT: &[u8] = b"\x88\xf8\xa8\xa8\x70";
const CAT_W: i32 = 5;
const CAT_H: i32 = 5;
const CAT_Y: i32 = DISPLAY_H - CAT_H - 1;

// 6x5 pests, one type per formation row, two walking frames each.
const PEST_W: i32 = 6;
const PEST_H: i32 = 5;
const PESTS: [[&[u8]; 2]; PEST_ROWS] = [
    [b"\x78\xb4\xfc\x84\x48", b"\x78\xb4\xfc\x84\x84"],
    [b"\x78\xb4\xb4\x78\x84", b"\x78\xfc\xb4\x78\x48"],
    [b"\x84\x48\xb4\xb4\x78", b"\x48\x48\xfc\xb4\x78"],
];

// Formation
const PEST_ROWS: usize = 3;
const PEST_COLS: usize = 8;
const PEST_COUNT: usize = PEST_ROWS * PEST_COLS;
const PITCH_X: i32 = 10;
const PITCH_Y: i32 = 7;
const FORMATION_W: i32 = (PEST_COLS as i32 - 1) * PITCH_X + PEST_W;
const FORMATION_Y: i32 = 8;
const STEP_X: i32 = 2;
const DROP_Y: i32 = 3;
/// Seconds between formation steps: the minimum plus a share that shrinks
/// as pests die, so the last few scurry.
const STEP_MIN: f32 = 0.04;
const STEP_RANGE: f32 = 0.5;

// UFO, a skill shot that now and then crosses the top of the screen.
const UFO: &[u8] = b"\x08\x00\x7f\x00\xff\x80\xaa\x80\x7f\x00";
const UFO_W: i32 = 9;
const UFO_H: i32 = 5;
const UFO_Y: i32 = 0;
const UFO_SPEED: f32 = 30.0;
/// Seconds between UFOs: the minimum plus a random share of the range.
const UFO_MIN: f32 = 8.0;
const UFO_RANGE: f32 = 10.0;
const UFO_POINTS: [i32; 3] = [5, 10, 15];

// Waves
/// Each wave's step and fire timings are this share of the last wave's.
const PACE_PER_WAVE: f32 = 0.85;
const MIN_PACE: f32 = 0.35;
/// Each wave starts one drop lower than the last, up to this many.
const MAX_START_DROPS: u32 = 4;
/// How long the "Level n" banner shows before a wave starts.
const INTRO_TIME: f32 = 1.2;
/// Pause after the poof before GAME OVER.
const POOF_PAUSE: f32 = 0.4;

// Player
const PLAYER_MAX_SPEED: f32 = 70.0;
const PLAYER_ACCELERATION: f32 = 600.0;
const PLAYER_FRICTION: f32 = 400.0;

// Bullets
const BULLET_H: i32 = 3;
const PLAYER_BULLET_SPEED: f32 = 100.0;
const PEST_BULLET_SPEED: f32 = 40.0;
const MAX_PEST_BULLETS: usize = 3;
/// Seconds between pest shots: the minimum plus a random share of the range.
const FIRE_MIN: f32 = 0.5;
const FIRE_RANGE: f32 = 1.0;

// Bunkers, one bit per pixel, bit 0 on the left.
const BUNKERS: usize = 4;
const BUNKER_W: i32 = 14;
const BUNKER_H: usize = 3;
const BUNKER_GAP: i32 = 16;
const BUNKER_X: i32 = (DISPLAY_W - (BUNKERS as i32 * BUNKER_W + (BUNKERS as i32 - 1) * BUNKER_GAP)) / 2;
const BUNKER_Y: i32 = 48;
const BUNKER_FULL: u16 = (1 << BUNKER_W) - 1;
/// Rounded off top corners.
const BUNKER_SHAPE: [u16; BUNKER_H] = [BUNKER_FULL & !(1 | 1 << (BUNKER_W - 1)), BUNKER_FULL, BUNKER_FULL];

const MESSAGE_Y: i32 = 37;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    /// "Level n" over a fresh wave.
    Intro,
    Playing,
    /// The cat was hit and is going up in a poof.
    Poof,
    Lose,
}

#[derive(Clone, Copy)]
struct Bullet {
    x: i32,
    y: f32,
}

pub struct PestControlScene {
    state: State,
    player_x: f32,
    player_vx: f32,
    player_bullet: Option<Bullet>,
    pest_bullets: Vec<Bullet, MAX_PEST_BULLETS>,
    alive: [bool; PEST_COUNT],
    pests_remaining: u8,
    formation_x: i32,
    formation_y: i32,
    formation_dir: i32,
    frame: usize,
    step_timer: f32,
    fire_timer: f32,
    bunkers: [[u16; BUNKER_H]; BUNKERS],
    score: i32,
    wave: u32,
    /// Scales the step and fire timings, shrinking each wave.
    pace: f32,
    /// Time in the current Intro or Poof state.
    timer: f32,
    /// The UFO's x and direction while it's crossing.
    ufo: Option<(f32, i32)>,
    ufo_timer: f32,
    /// Where the shot-down UFO is going up in a poof, and how long it's played.
    ufo_poof: Option<(i32, f32)>,

    session_pests: u32,
    session_waves: u32,
}

impl PestControlScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            player_x: 0.0,
            player_vx: 0.0,
            player_bullet: None,
            pest_bullets: Vec::new(),
            alive: [false; PEST_COUNT],
            pests_remaining: 0,
            formation_x: 0,
            formation_y: 0,
            formation_dir: 1,
            frame: 0,
            step_timer: 0.0,
            fire_timer: 0.0,
            bunkers: [BUNKER_SHAPE; BUNKERS],
            score: 0,
            wave: 1,
            pace: 1.0,
            timer: 0.0,
            ufo: None,
            ufo_timer: 0.0,
            ufo_poof: None,
            session_pests: 0,
            session_waves: 0,
        }
    }

    fn new_game(&mut self, rng: &mut u32) {
        self.player_x = ((DISPLAY_W - CAT_W) / 2) as f32;
        self.player_vx = 0.0;
        self.score = 0;
        self.wave = 1;
        self.start_wave(rng);
        self.state = State::Ready;
    }

    fn start_wave(&mut self, rng: &mut u32) {
        self.state = State::Intro;
        self.timer = 0.0;
        self.pace = 1.0;
        for _ in 1..self.wave.min(20) {
            self.pace *= PACE_PER_WAVE;
        }
        self.pace = self.pace.max(MIN_PACE);
        self.player_bullet = None;
        self.pest_bullets.clear();
        self.alive = [true; PEST_COUNT];
        self.pests_remaining = PEST_COUNT as u8;
        self.formation_x = (DISPLAY_W - FORMATION_W) / 2;
        self.formation_y = FORMATION_Y + (self.wave - 1).min(MAX_START_DROPS) as i32 * DROP_Y;
        self.formation_dir = 1;
        self.frame = 0;
        self.step_timer = 0.0;
        self.reset_fire_timer(rng);
        self.bunkers = [BUNKER_SHAPE; BUNKERS];
        self.ufo = None;
        self.ufo_poof = None;
        self.reset_ufo_timer(rng);
    }

    fn reset_ufo_timer(&mut self, rng: &mut u32) {
        let share = (xorshift32(rng) % 1000) as f32 / 1000.0;
        self.ufo_timer = UFO_MIN + UFO_RANGE * share;
    }

    fn update_ufo(&mut self, rng: &mut u32, dt: f32) {
        if let Some((_, t)) = &mut self.ufo_poof {
            *t += dt;
            if *t >= POOF_FRAME_DUR * POOF.frames.len() as f32 {
                self.ufo_poof = None;
            }
        }

        if let Some((x, dir)) = &mut self.ufo {
            *x += *dir as f32 * UFO_SPEED * dt;
            if *x < -UFO_W as f32 || *x > DISPLAY_W as f32 {
                self.ufo = None;
            }
            return;
        }

        self.ufo_timer -= dt;
        if self.ufo_timer > 0.0 {
            return;
        }
        self.reset_ufo_timer(rng);
        self.ufo = if xorshift32(rng) % 2 == 0 {
            Some((-UFO_W as f32, 1))
        } else {
            Some((DISPLAY_W as f32, -1))
        };
    }

    fn hit_ufo(&mut self, rng: &mut u32) {
        let Some((x, _)) = self.ufo.take() else {
            return;
        };
        let points = UFO_POINTS[(xorshift32(rng) % UFO_POINTS.len() as u32) as usize];
        self.score += points;
        self.ufo_poof = Some((x as i32 + UFO_W / 2, 0.0));
    }

    fn reset_fire_timer(&mut self, rng: &mut u32) {
        let share = (xorshift32(rng) % 1000) as f32 / 1000.0;
        self.fire_timer = (FIRE_MIN + FIRE_RANGE * share) * self.pace;
    }

    fn pest_xy(&self, idx: usize) -> (i32, i32) {
        let row = (idx / PEST_COLS) as i32;
        let col = (idx % PEST_COLS) as i32;
        (
            self.formation_x + col * PITCH_X,
            self.formation_y + row * PITCH_Y,
        )
    }

    fn bunker_x(b: usize) -> i32 {
        BUNKER_X + b as i32 * (BUNKER_W + BUNKER_GAP)
    }

    fn step_interval(&self) -> f32 {
        (STEP_MIN + STEP_RANGE * self.pests_remaining as f32 / PEST_COUNT as f32) * self.pace
    }

    fn update_player(&mut self, buttons: &Buttons, dt: f32) {
        if buttons.is_pressed(Button::Left) {
            self.player_vx = (self.player_vx - PLAYER_ACCELERATION * dt).max(-PLAYER_MAX_SPEED);
        }
        if buttons.is_pressed(Button::Right) {
            self.player_vx = (self.player_vx + PLAYER_ACCELERATION * dt).min(PLAYER_MAX_SPEED);
        }

        if self.player_vx > 0.0 {
            self.player_vx = (self.player_vx - PLAYER_FRICTION * dt).max(0.0);
        } else if self.player_vx < 0.0 {
            self.player_vx = (self.player_vx + PLAYER_FRICTION * dt).min(0.0);
        }
        self.player_x += self.player_vx * dt;

        let max_x = (DISPLAY_W - CAT_W) as f32;
        if self.player_x < 0.0 {
            self.player_x = 0.0;
            self.player_vx = 0.0;
        } else if self.player_x > max_x {
            self.player_x = max_x;
            self.player_vx = 0.0;
        }
    }

    /// Chips the bunker pixel at (x, y) if there is one, along with its
    /// neighbours. Returns whether anything was there to hit.
    fn hit_bunker(&mut self, x: i32, y: i32) -> bool {
        let r = y - BUNKER_Y;
        if r < 0 || r >= BUNKER_H as i32 {
            return false;
        }
        let r = r as usize;
        for b in 0..BUNKERS {
            let c = x - Self::bunker_x(b);
            if c < 0 || c >= BUNKER_W {
                continue;
            }
            let rows = &mut self.bunkers[b];
            if rows[r] & 1 << c == 0 {
                return false;
            }
            rows[r] &= !(((0b111u32 << c) >> 1) as u16);
            if r > 0 {
                rows[r - 1] &= !(1 << c);
            }
            if r + 1 < BUNKER_H {
                rows[r + 1] &= !(1 << c);
            }
            return true;
        }
        false
    }

    /// Pests marching through a bunker eat whatever they overlap.
    fn erase_bunkers_under_pests(&mut self) {
        for idx in 0..PEST_COUNT {
            if !self.alive[idx] {
                continue;
            }
            let (px, py) = self.pest_xy(idx);
            for r in 0..BUNKER_H {
                let y = BUNKER_Y + r as i32;
                if y < py || y >= py + PEST_H {
                    continue;
                }
                for b in 0..BUNKERS {
                    let bx = Self::bunker_x(b);
                    for c in (px - bx).max(0)..(px + PEST_W - bx).min(BUNKER_W) {
                        self.bunkers[b][r] &= !(1 << c);
                    }
                }
            }
        }
    }

    fn update_player_bullet(&mut self, rng: &mut u32, dt: f32) {
        let Some(mut bullet) = self.player_bullet else {
            return;
        };
        // One pixel per sub-step so it can't skip over a bunker row or a pest.
        let steps = (PLAYER_BULLET_SPEED * dt) as i32 + 1;
        let sub = PLAYER_BULLET_SPEED * dt / steps as f32;
        for _ in 0..steps {
            bullet.y -= sub;
            let tip = bullet.y as i32;
            if tip < 0 || self.hit_bunker(bullet.x, tip) {
                self.player_bullet = None;
                return;
            }
            if let Some((ux, _)) = self.ufo {
                let ux = ux as i32;
                if bullet.x >= ux && bullet.x < ux + UFO_W && tip >= UFO_Y && tip < UFO_Y + UFO_H {
                    self.hit_ufo(rng);
                    self.player_bullet = None;
                    return;
                }
            }
            for idx in 0..PEST_COUNT {
                if !self.alive[idx] {
                    continue;
                }
                let (px, py) = self.pest_xy(idx);
                if bullet.x >= px && bullet.x < px + PEST_W && tip >= py && tip < py + PEST_H {
                    self.alive[idx] = false;
                    self.pests_remaining -= 1;
                    // Higher rows are worth more.
                    self.score += (PEST_ROWS - idx / PEST_COLS) as i32;
                    self.session_pests += 1;
                    self.player_bullet = None;
                    return;
                }
            }
        }
        self.player_bullet = Some(bullet);
    }

    fn update_pest_bullets(&mut self, dt: f32) {
        let steps = (PEST_BULLET_SPEED * dt) as i32 + 1;
        let sub = PEST_BULLET_SPEED * dt / steps as f32;
        let cat_x = self.player_x as i32;

        let mut i = 0;
        'bullets: while i < self.pest_bullets.len() {
            let mut bullet = self.pest_bullets[i];
            for _ in 0..steps {
                bullet.y += sub;
                let tip = bullet.y as i32 + BULLET_H - 1;
                if tip >= DISPLAY_H || self.hit_bunker(bullet.x, tip) {
                    self.pest_bullets.swap_remove(i);
                    continue 'bullets;
                }
                if bullet.x >= cat_x && bullet.x < cat_x + CAT_W && tip >= CAT_Y && tip < CAT_Y + CAT_H {
                    self.pest_bullets.swap_remove(i);
                    self.kill_player();
                    return;
                }
            }
            self.pest_bullets[i] = bullet;
            i += 1;
        }
    }

    fn step_formation(&mut self) {
        let mut min_col = PEST_COLS;
        let mut max_col = 0;
        let mut max_row = 0;
        for idx in 0..PEST_COUNT {
            if self.alive[idx] {
                min_col = min_col.min(idx % PEST_COLS);
                max_col = max_col.max(idx % PEST_COLS);
                max_row = max_row.max(idx / PEST_COLS);
            }
        }
        let left = self.formation_x + min_col as i32 * PITCH_X;
        let right = self.formation_x + max_col as i32 * PITCH_X + PEST_W;

        let next_left = left + self.formation_dir * STEP_X;
        let next_right = right + self.formation_dir * STEP_X;
        if next_left < 0 || next_right > DISPLAY_W {
            self.formation_y += DROP_Y;
            self.formation_dir = -self.formation_dir;
        } else {
            self.formation_x += self.formation_dir * STEP_X;
        }
        self.frame ^= 1;

        self.erase_bunkers_under_pests();

        let bottom = self.formation_y + max_row as i32 * PITCH_Y + PEST_H;
        if bottom >= CAT_Y {
            self.kill_player();
        }
    }

    fn kill_player(&mut self) {
        self.state = State::Poof;
        self.timer = 0.0;
        self.player_bullet = None;
    }

    /// The lowest pest in a random column that still has any fires a shot.
    fn pest_fire(&mut self, rng: &mut u32) {
        if self.pest_bullets.is_full() {
            return;
        }
        let mut shooters: Vec<usize, PEST_COLS> = Vec::new();
        for col in 0..PEST_COLS {
            if let Some(row) = (0..PEST_ROWS).rev().find(|&row| self.alive[row * PEST_COLS + col]) {
                let _ = shooters.push(row * PEST_COLS + col);
            }
        }
        if shooters.is_empty() {
            return;
        }
        let idx = shooters[(xorshift32(rng) % shooters.len() as u32) as usize];
        let (px, py) = self.pest_xy(idx);
        let _ = self.pest_bullets.push(Bullet {
            x: px + PEST_W / 2 - 1,
            y: (py + PEST_H) as f32,
        });
    }

    fn draw_message(renderer: &mut Renderer, text: &str) {
        let w = text.chars().count() as i32 * CHAR_W;
        let x = (DISPLAY_W - w) / 2;
        // Clear behind the text so pests don't run through it.
        renderer.fill_rect_off(Point::new(x - 1, MESSAGE_Y - 1), Size::new((w + 2) as u32, 12));
        renderer.draw_text(text, Point::new(x, MESSAGE_Y));
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        if self.session_pests == 0 {
            return;
        }
        let reward = (self.session_pests as f32 / PEST_COUNT as f32).sqrt();
        let fulfillment_mult = if self.session_waves > 0 { 4.0 } else { 2.0 };
        ctx.apply_stat_changes(&[
            (StatId::Playfulness, 5.0 * reward),
            (StatId::Focus,       5.0 * reward),
            (StatId::Fitness,     3.0 * reward),
            (StatId::Loyalty,     1.0 * reward),
            (StatId::Fulfillment, fulfillment_mult * reward),
        ]);
        let coins = (3.0 * reward) as i32 + self.session_waves as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }
}

impl Scene for PestControlScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.session_pests = 0;
        self.session_waves = 0;
        self.new_game(&mut rng);
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

        let mut rng = ctx.rng;
        let mut fire = false;
        if buttons.was_just_pressed(Button::A) {
            match self.state {
                State::Ready => self.state = State::Playing,
                State::Lose => self.new_game(&mut rng),
                State::Playing => fire = true,
                State::Intro | State::Poof => {}
            }
        }

        match self.state {
            State::Ready | State::Intro | State::Playing => self.update_player(buttons, dt),
            State::Poof | State::Lose => {}
        }

        match self.state {
            State::Intro => {
                self.timer += dt;
                if self.timer >= INTRO_TIME {
                    self.state = State::Playing;
                }
            }
            State::Poof => {
                self.timer += dt;
                if self.timer >= POOF_FRAME_DUR * POOF.frames.len() as f32 + POOF_PAUSE {
                    self.state = State::Lose;
                }
            }
            _ => {}
        }

        if self.state == State::Playing {
            self.update_player_bullet(&mut rng, dt);
            if self.pests_remaining == 0 {
                self.session_waves += 1;
                self.wave += 1;
                self.start_wave(&mut rng);
            }
        }
        // Fire after moving bullets so a new shot is first drawn where it
        // spawns, between the cat's ears.
        if self.state == State::Playing && fire && self.player_bullet.is_none() {
            self.player_bullet = Some(Bullet {
                x: self.player_x as i32 + CAT_W / 2,
                y: (CAT_Y - BULLET_H + 1) as f32,
            });
        }
        if self.state == State::Playing {
            self.update_ufo(&mut rng, dt);
            self.update_pest_bullets(dt);
        }
        if self.state == State::Playing {
            self.step_timer += dt;
            let interval = self.step_interval();
            if self.step_timer >= interval {
                self.step_timer -= interval;
                self.step_formation();
            }
        }
        if self.state == State::Playing {
            self.fire_timer -= dt;
            if self.fire_timer <= 0.0 {
                self.pest_fire(&mut rng);
                self.reset_fire_timer(&mut rng);
            }
        }
        ctx.rng = rng;

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        let opts = SpriteOpts { transparent: true, ..Default::default() };

        for idx in 0..PEST_COUNT {
            if self.alive[idx] {
                let (x, y) = self.pest_xy(idx);
                let sprite = PESTS[idx / PEST_COLS][self.frame];
                renderer.draw_sprite_raw(sprite, PEST_W as u16, PEST_H as u16, Point::new(x, y), opts);
            }
        }

        if let Some((x, _)) = self.ufo {
            renderer.draw_sprite_raw(UFO, UFO_W as u16, UFO_H as u16, Point::new(x as i32, UFO_Y), opts);
        }
        if let Some((x, t)) = self.ufo_poof {
            let frame = (t / POOF_FRAME_DUR) as usize;
            if frame < POOF.frames.len() {
                let pos = Point::new(
                    x - POOF.width as i32 / 2,
                    UFO_Y + UFO_H / 2 - POOF.height as i32 / 2,
                );
                renderer.draw_sprite(&POOF, pos, SpriteOpts { frame, ..opts });
            }
        }

        for b in 0..BUNKERS {
            let bx = Self::bunker_x(b);
            for (r, &bits) in self.bunkers[b].iter().enumerate() {
                for c in 0..BUNKER_W {
                    if bits & 1 << c != 0 {
                        renderer.draw_pixel(Point::new(bx + c, BUNKER_Y + r as i32), true);
                    }
                }
            }
        }

        let cat_x = self.player_x as i32;
        match self.state {
            State::Poof => {
                let frame = (self.timer / POOF_FRAME_DUR) as usize;
                if frame < POOF.frames.len() {
                    let pos = Point::new(
                        cat_x + CAT_W / 2 - POOF.width as i32 / 2,
                        CAT_Y + CAT_H / 2 - POOF.height as i32 / 2,
                    );
                    renderer.draw_sprite(&POOF, pos, SpriteOpts { frame, ..opts });
                }
            }
            State::Lose => {}
            _ => renderer.draw_sprite_raw(CAT, CAT_W as u16, CAT_H as u16, Point::new(cat_x, CAT_Y), opts),
        }

        let bullets = self.player_bullet.iter().chain(self.pest_bullets.iter());
        for bullet in bullets {
            renderer.draw_rect(
                Point::new(bullet.x, bullet.y as i32),
                Size::new(1, BULLET_H as u32),
                true,
            );
        }

        let mut s: String<8> = String::new();
        let _ = write!(s, "{}", self.score);
        renderer.draw_text_small(s.as_str(), Point::new(0, 0));

        match self.state {
            State::Ready => Self::draw_message(renderer, t!("A: Start")),
            State::Intro => {
                let mut n: String<8> = String::new();
                let _ = write!(n, "{}", self.wave);
                let mut text: String<24> = String::new();
                crate::i18n::substitute(&mut text, t!("Level {n}"), &[("n", n.as_str())]);
                Self::draw_message(renderer, &text);
            }
            State::Lose => Self::draw_message(renderer, t!("GAME OVER")),
            State::Playing | State::Poof => {}
        }
    }
}
