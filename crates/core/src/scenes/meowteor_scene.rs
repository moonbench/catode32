//! Meowteor, a cat's take on Asteroids. The cat's ship drifts around a
//! wrap-around field of space rocks, turning with Left/Right, thrusting with
//! Up/Down and shooting with A. Rocks break into smaller rocks when shot, and
//! each cleared wave brings one more rock, until the last life is lost.

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
const FIELD_W: f32 = DISPLAY_W as f32;
const FIELD_H: f32 = DISPLAY_H as f32;
const CHAR_W: i32 = 6; // FONT_6X10
const SMALL_CHAR_W: i32 = 4; // FONT_4X6

// Ship
const TURN_SPEED: f32 = 4.0;
const THRUST: f32 = 70.0;
const MAX_SPEED: f32 = 55.0;
/// Share of the ship's speed lost per second.
const DRAG: f32 = 0.6;
/// Nose and tail distances from the centre, and the tail's half-width.
const NOSE: f32 = 4.0;
const TAIL: f32 = 3.0;
const TAIL_HALF_W: f32 = 2.5;
const SHIP_R: f32 = 2.5;
/// Blinking spell after a respawn during which rocks pass through the ship.
const INVULN_TIME: f32 = 2.0;

// 5x5 ship, one byte per row, one per life left.
const LIFE_GLYPH: &[u8] = b"\x20\x70\x70\xf8\xd8";
const LIFE_W: i32 = 5;
const LIFE_H: i32 = 5;
const LIVES: u8 = 3;

// Bullets
const MAX_BULLETS: usize = 4;
const BULLET_SPEED: f32 = 90.0;
const BULLET_LIFE: f32 = 0.7;
/// Pixels the streak trails behind a bullet's tip.
const BULLET_LEN: f32 = 2.0;

// Rocks, indexed by size: large, medium, small.
const ROCK_SIZES: usize = 3;
const ROCK_R: [f32; ROCK_SIZES] = [8.0, 5.0, 3.0];
const ROCK_SPEED: [f32; ROCK_SIZES] = [4.0, 6.0, 8.0];
const ROCK_SPEED_RANGE: [f32; ROCK_SIZES] = [3.0, 4.0, 5.0];
/// Share of the broken rock's speed its pieces keep.
const PIECE_KEEP: f32 = 0.3;
const ROCK_POINTS: [i32; ROCK_SIZES] = [1, 2, 3];
/// Rocks broken in a full wave of one large rock: itself, 2 medium, 4 small.
const ROCKS_PER_LARGE: u32 = 7;
const MAX_SPIN: f32 = 1.5;
/// Each vertex sits between this share of the radius and the full radius.
const ROCK_MIN_SHAPE: f32 = 0.7;
const ROCK_VERTS: usize = 8;
const RT: f32 = core::f32::consts::FRAC_1_SQRT_2;
const ROCK_DIRS: [(f32, f32); ROCK_VERTS] = [
    (1.0, 0.0), (RT, RT), (0.0, 1.0), (-RT, RT),
    (-1.0, 0.0), (-RT, -RT), (0.0, -1.0), (RT, -RT),
];
const MAX_ROCKS: usize = 32;

// Waves
const START_ROCKS: u32 = 3;
const MAX_WAVE_ROCKS: u32 = 7;
/// Rock speeds grow by this share each wave, up to the cap.
const SPEED_PER_WAVE: f32 = 0.15;
const MAX_SPEED_SCALE: f32 = 2.5;
/// New rocks spawn at least this far from the ship.
const SAFE_DIST: f32 = 30.0;
/// How long the "Level n" banner shows before a wave starts.
const INTRO_TIME: f32 = 1.2;
/// Pause after the poof before respawning or GAME OVER.
const POOF_PAUSE: f32 = 0.4;

// Particles
const MAX_PARTICLES: usize = 48;
const PARTICLES_MIN: u32 = 6;
const PARTICLES_RANGE: u32 = 5;
const PARTICLE_SPEED: f32 = 15.0;
const PARTICLE_SPEED_RANGE: f32 = 25.0;
const PARTICLE_LIFE: f32 = 0.5;
const PARTICLE_LIFE_RANGE: f32 = 0.4;

const MESSAGE_Y: i32 = 27;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    /// "Level n" over a fresh wave.
    Intro,
    Playing,
    /// The ship hit a rock and is going up in a poof.
    Poof,
    Lose,
}

#[derive(Clone, Copy)]
struct Bullet {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

#[derive(Clone, Copy)]
struct Rock {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    size: usize,
    angle: f32,
    spin: f32,
    /// Per-vertex distance from the centre, 0 = `ROCK_MIN_SHAPE`, 255 = full.
    shape: [u8; ROCK_VERTS],
}

#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    seed: u8,
}

fn rand01(rng: &mut u32) -> f32 {
    (xorshift32(rng) % 1000) as f32 / 1000.0
}

/// -1.0..1.0
fn rand_signed(rng: &mut u32) -> f32 {
    rand01(rng) * 2.0 - 1.0
}

fn wrap(v: f32, size: f32) -> f32 {
    if v < 0.0 {
        v + size
    } else if v >= size {
        v - size
    } else {
        v
    }
}

/// Shortest way from a to b around the field.
fn wrap_delta(a: f32, b: f32, size: f32) -> f32 {
    let d = b - a;
    if d > size / 2.0 {
        d - size
    } else if d < -size / 2.0 {
        d + size
    } else {
        d
    }
}

fn wrapped_dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = wrap_delta(ax, bx, FIELD_W);
    let dy = wrap_delta(ay, by, FIELD_H);
    dx * dx + dy * dy
}

/// Where to draw something of radius r centred at (x, y) so it shows on
/// both sides of any edge it straddles.
fn wrap_offsets(x: f32, y: f32, r: f32) -> Vec<(f32, f32), 4> {
    let mut xs: Vec<f32, 2> = Vec::new();
    let mut ys: Vec<f32, 2> = Vec::new();
    let _ = xs.push(0.0);
    let _ = ys.push(0.0);
    if x + r >= FIELD_W {
        let _ = xs.push(-FIELD_W);
    } else if x - r < 0.0 {
        let _ = xs.push(FIELD_W);
    }
    if y + r >= FIELD_H {
        let _ = ys.push(-FIELD_H);
    } else if y - r < 0.0 {
        let _ = ys.push(FIELD_H);
    }
    let mut out = Vec::new();
    for &ox in &xs {
        for &oy in &ys {
            let _ = out.push((ox, oy));
        }
    }
    out
}

/// Scanline fill of a polygon that's star-shaped around its centre, sampling
/// each row and column at pixel centres.
fn fill_polygon(renderer: &mut Renderer, pts: &[(f32, f32)]) {
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for &(_, y) in pts {
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let top = (min_y.floor() as i32).max(0);
    let bottom = (max_y.ceil() as i32).min(DISPLAY_H - 1);
    for row in top..=bottom {
        let yc = row as f32 + 0.5;
        let mut xs: Vec<f32, ROCK_VERTS> = Vec::new();
        for i in 0..pts.len() {
            let (ax, ay) = pts[i];
            let (bx, by) = pts[(i + 1) % pts.len()];
            if (ay <= yc && yc < by) || (by <= yc && yc < ay) {
                let _ = xs.push(ax + (yc - ay) * (bx - ax) / (by - ay));
            }
        }
        xs.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        for span in xs.chunks_exact(2) {
            let start = ((span[0] - 0.5).ceil() as i32).max(0);
            let end = ((span[1] - 0.5).ceil() as i32 - 1).min(DISPLAY_W - 1);
            if start <= end {
                renderer.draw_line(Point::new(start, row), Point::new(end, row));
            }
        }
    }
}

pub struct MeowteorScene {
    state: State,
    ship_x: f32,
    ship_y: f32,
    ship_vx: f32,
    ship_vy: f32,
    /// Radians clockwise from pointing up.
    ship_angle: f32,
    thrusting: bool,
    invuln: f32,
    lives: u8,
    bullets: Vec<Bullet, MAX_BULLETS>,
    rocks: Vec<Rock, MAX_ROCKS>,
    particles: Vec<Particle, MAX_PARTICLES>,
    score: i32,
    wave: u32,
    /// Time in the current Intro or Poof state.
    timer: f32,
    /// Counts updates, to flicker fading particles.
    tick: u32,

    session_rocks: u32,
    session_waves: u32,
}

impl MeowteorScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            ship_x: 0.0,
            ship_y: 0.0,
            ship_vx: 0.0,
            ship_vy: 0.0,
            ship_angle: 0.0,
            thrusting: false,
            invuln: 0.0,
            lives: LIVES,
            bullets: Vec::new(),
            rocks: Vec::new(),
            particles: Vec::new(),
            score: 0,
            wave: 1,
            timer: 0.0,
            tick: 0,
            session_rocks: 0,
            session_waves: 0,
        }
    }

    fn new_game(&mut self, rng: &mut u32) {
        self.reset_ship();
        self.invuln = 0.0;
        self.lives = LIVES;
        self.score = 0;
        self.wave = 1;
        self.particles.clear();
        self.start_wave(rng);
        self.state = State::Ready;
    }

    fn reset_ship(&mut self) {
        self.ship_x = FIELD_W / 2.0;
        self.ship_y = FIELD_H / 2.0;
        self.ship_vx = 0.0;
        self.ship_vy = 0.0;
        self.ship_angle = 0.0;
        self.thrusting = false;
    }

    fn start_wave(&mut self, rng: &mut u32) {
        self.state = State::Intro;
        self.timer = 0.0;
        self.bullets.clear();
        self.rocks.clear();
        let count = (START_ROCKS + self.wave - 1).min(MAX_WAVE_ROCKS);
        for _ in 0..count {
            // A few tries for a spot clear of the ship.
            let (mut x, mut y) = (0.0, 0.0);
            for _ in 0..10 {
                x = rand01(rng) * FIELD_W;
                y = rand01(rng) * FIELD_H;
                if wrapped_dist2(x, y, self.ship_x, self.ship_y) >= SAFE_DIST * SAFE_DIST {
                    break;
                }
            }
            let a = rand01(rng) * core::f32::consts::TAU;
            let speed = self.rock_speed(0, rng);
            self.push_rock(rng, x, y, a.cos() * speed, a.sin() * speed, 0);
        }
    }

    fn rock_speed(&self, size: usize, rng: &mut u32) -> f32 {
        let scale = (1.0 + SPEED_PER_WAVE * (self.wave - 1) as f32).min(MAX_SPEED_SCALE);
        (ROCK_SPEED[size] + ROCK_SPEED_RANGE[size] * rand01(rng)) * scale
    }

    fn push_rock(&mut self, rng: &mut u32, x: f32, y: f32, vx: f32, vy: f32, size: usize) {
        let mut shape = [0u8; ROCK_VERTS];
        for v in shape.iter_mut() {
            *v = xorshift32(rng) as u8;
        }
        let _ = self.rocks.push(Rock {
            x,
            y,
            vx,
            vy,
            size,
            angle: rand01(rng) * core::f32::consts::TAU,
            spin: rand_signed(rng) * MAX_SPIN,
            shape,
        });
    }

    /// Breaks rock i into two smaller ones (if it isn't already the
    /// smallest), kicked out sideways from (hit_vx, hit_vy), in a spray of
    /// debris.
    fn break_rock(&mut self, rng: &mut u32, i: usize, hit_vx: f32, hit_vy: f32) {
        let rock = self.rocks.swap_remove(i);
        self.score += ROCK_POINTS[rock.size];
        self.session_rocks += 1;

        let n = PARTICLES_MIN + xorshift32(rng) % PARTICLES_RANGE;
        for _ in 0..n {
            let speed = PARTICLE_SPEED + PARTICLE_SPEED_RANGE * rand01(rng);
            let _ = self.particles.push(Particle {
                x: rock.x,
                y: rock.y,
                vx: rand_signed(rng) * speed + rock.vx * 0.3,
                vy: rand_signed(rng) * speed + rock.vy * 0.3,
                age: 0.0,
                life: PARTICLE_LIFE + PARTICLE_LIFE_RANGE * rand01(rng),
                seed: xorshift32(rng) as u8,
            });
        }

        let size = rock.size + 1;
        if size >= ROCK_SIZES {
            return;
        }
        let len = (hit_vx * hit_vx + hit_vy * hit_vy).sqrt().max(0.001);
        let (px, py) = (-hit_vy / len, hit_vx / len);
        for side in [-1.0, 1.0] {
            let speed = self.rock_speed(size, rng);
            let vx = rock.vx * PIECE_KEEP + (px * side + rand_signed(rng) * 0.4) * speed;
            let vy = rock.vy * PIECE_KEEP + (py * side + rand_signed(rng) * 0.4) * speed;
            self.push_rock(rng, rock.x, rock.y, vx, vy, size);
        }
    }

    fn heading(&self) -> (f32, f32) {
        (self.ship_angle.sin(), -self.ship_angle.cos())
    }

    fn update_ship(&mut self, buttons: &Buttons, dt: f32) {
        if buttons.is_pressed(Button::Left) {
            self.ship_angle -= TURN_SPEED * dt;
        }
        if buttons.is_pressed(Button::Right) {
            self.ship_angle += TURN_SPEED * dt;
        }
        self.thrusting = buttons.is_pressed(Button::Up) || buttons.is_pressed(Button::Down);
        if self.thrusting {
            let (hx, hy) = self.heading();
            self.ship_vx += hx * THRUST * dt;
            self.ship_vy += hy * THRUST * dt;
        }

        let keep = (1.0 - DRAG * dt).max(0.0);
        self.ship_vx *= keep;
        self.ship_vy *= keep;
        let speed = (self.ship_vx * self.ship_vx + self.ship_vy * self.ship_vy).sqrt();
        if speed > MAX_SPEED {
            self.ship_vx *= MAX_SPEED / speed;
            self.ship_vy *= MAX_SPEED / speed;
        }
        self.ship_x = wrap(self.ship_x + self.ship_vx * dt, FIELD_W);
        self.ship_y = wrap(self.ship_y + self.ship_vy * dt, FIELD_H);
    }

    fn fire(&mut self) {
        if self.bullets.is_full() {
            return;
        }
        let (hx, hy) = self.heading();
        let _ = self.bullets.push(Bullet {
            x: wrap(self.ship_x + hx * NOSE, FIELD_W),
            y: wrap(self.ship_y + hy * NOSE, FIELD_H),
            vx: self.ship_vx + hx * BULLET_SPEED,
            vy: self.ship_vy + hy * BULLET_SPEED,
            life: BULLET_LIFE,
        });
    }

    fn update_rocks(&mut self, dt: f32) {
        for rock in self.rocks.iter_mut() {
            rock.x = wrap(rock.x + rock.vx * dt, FIELD_W);
            rock.y = wrap(rock.y + rock.vy * dt, FIELD_H);
            rock.angle += rock.spin * dt;
        }
    }

    fn update_bullets(&mut self, rng: &mut u32, dt: f32) {
        let mut i = 0;
        'bullets: while i < self.bullets.len() {
            let b = &mut self.bullets[i];
            b.life -= dt;
            if b.life <= 0.0 {
                self.bullets.swap_remove(i);
                continue;
            }
            b.x = wrap(b.x + b.vx * dt, FIELD_W);
            b.y = wrap(b.y + b.vy * dt, FIELD_H);
            let b = *b;
            for r in 0..self.rocks.len() {
                let rad = ROCK_R[self.rocks[r].size];
                if wrapped_dist2(b.x, b.y, self.rocks[r].x, self.rocks[r].y) < rad * rad {
                    self.bullets.swap_remove(i);
                    self.break_rock(rng, r, b.vx, b.vy);
                    continue 'bullets;
                }
            }
            i += 1;
        }
    }

    fn update_particles(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.particles.len() {
            let p = &mut self.particles[i];
            p.age += dt;
            if p.age >= p.life {
                self.particles.swap_remove(i);
                continue;
            }
            p.x = wrap(p.x + p.vx * dt, FIELD_W);
            p.y = wrap(p.y + p.vy * dt, FIELD_H);
            i += 1;
        }
    }

    /// The ship hitting a rock breaks the rock and costs a life.
    fn check_ship_hit(&mut self, rng: &mut u32) {
        if self.invuln > 0.0 {
            return;
        }
        for r in 0..self.rocks.len() {
            let reach = ROCK_R[self.rocks[r].size] * 0.9 + SHIP_R;
            if wrapped_dist2(self.ship_x, self.ship_y, self.rocks[r].x, self.rocks[r].y) < reach * reach {
                let (vx, vy) = (self.ship_vx, self.ship_vy);
                self.break_rock(rng, r, vx, vy + 1.0);
                self.lives -= 1;
                self.state = State::Poof;
                self.timer = 0.0;
                self.thrusting = false;
                return;
            }
        }
    }

    fn particle_visible(&self, p: &Particle) -> bool {
        // Fades out by showing on fewer and fewer frames as it ages.
        let mut h = p.seed as u32 ^ self.tick.wrapping_mul(0x9e37_79b9);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        (h % 100) as f32 >= p.age / p.life * 100.0
    }

    fn draw_ship(&self, renderer: &mut Renderer) {
        let (hx, hy) = self.heading();
        // Perpendicular, to the ship's right.
        let (px, py) = (-hy, hx);
        let (x, y) = (self.ship_x, self.ship_y);
        let tail_x = x - hx * TAIL;
        let tail_y = y - hy * TAIL;
        let pts = [
            (x + hx * NOSE, y + hy * NOSE),
            (tail_x + px * TAIL_HALF_W, tail_y + py * TAIL_HALF_W),
            (tail_x - px * TAIL_HALF_W, tail_y - py * TAIL_HALF_W),
        ];
        let flame = self.thrusting && self.tick % 2 == 0;
        for (ox, oy) in wrap_offsets(x, y, NOSE + 2.0) {
            // Hollow, so it can't be mistaken for a small rock.
            for i in 0..pts.len() {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % pts.len()];
                renderer.draw_line(
                    Point::new((ax + ox) as i32, (ay + oy) as i32),
                    Point::new((bx + ox) as i32, (by + oy) as i32),
                );
            }
            if flame {
                let fx = (x - hx * (TAIL + 1.5) + ox) as i32;
                let fy = (y - hy * (TAIL + 1.5) + oy) as i32;
                renderer.draw_pixel(Point::new(fx, fy), true);
            }
        }
    }

    fn draw_rock(renderer: &mut Renderer, rock: &Rock) {
        let r = ROCK_R[rock.size];
        let (s, c) = (rock.angle.sin(), rock.angle.cos());
        let mut pts = [(0.0, 0.0); ROCK_VERTS];
        for (k, pt) in pts.iter_mut().enumerate() {
            let len = r * (ROCK_MIN_SHAPE + (1.0 - ROCK_MIN_SHAPE) * rock.shape[k] as f32 / 255.0);
            let (dx, dy) = ROCK_DIRS[k];
            *pt = (rock.x + (dx * c - dy * s) * len, rock.y + (dx * s + dy * c) * len);
        }
        for (ox, oy) in wrap_offsets(rock.x, rock.y, r) {
            let moved = pts.map(|(px, py)| (px + ox, py + oy));
            fill_polygon(renderer, &moved);
        }
    }

    fn draw_message(renderer: &mut Renderer, text: &str) {
        let w = text.chars().count() as i32 * CHAR_W;
        let x = (DISPLAY_W - w) / 2;
        // Clear behind the text so rocks don't run through it.
        renderer.fill_rect_off(Point::new(x - 1, MESSAGE_Y - 1), Size::new((w + 2) as u32, 12));
        renderer.draw_text(text, Point::new(x, MESSAGE_Y));
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        if self.session_rocks == 0 {
            return;
        }
        let reward = (self.session_rocks as f32 / (START_ROCKS * ROCKS_PER_LARGE) as f32).sqrt();
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

impl Scene for MeowteorScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.session_rocks = 0;
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
        self.tick = self.tick.wrapping_add(1);
        if buttons.was_just_pressed(Button::A) {
            match self.state {
                State::Ready => self.state = State::Playing,
                State::Lose => self.new_game(&mut rng),
                State::Playing => self.fire(),
                State::Intro | State::Poof => {}
            }
        }

        match self.state {
            State::Ready | State::Intro | State::Playing => self.update_ship(buttons, dt),
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
                    if self.lives > 0 {
                        self.reset_ship();
                        self.invuln = INVULN_TIME;
                        self.state = State::Playing;
                    } else {
                        self.state = State::Lose;
                    }
                }
            }
            _ => {}
        }

        // Rocks hold still behind the Ready and Level banners.
        if matches!(self.state, State::Playing | State::Poof | State::Lose) {
            self.update_rocks(dt);
            self.update_bullets(&mut rng, dt);
        }
        self.update_particles(dt);

        if self.state == State::Playing {
            self.invuln = (self.invuln - dt).max(0.0);
            self.check_ship_hit(&mut rng);
        }
        if self.state == State::Playing && self.rocks.is_empty() {
            self.session_waves += 1;
            self.wave += 1;
            self.start_wave(&mut rng);
        }
        ctx.rng = rng;

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        for rock in &self.rocks {
            Self::draw_rock(renderer, rock);
        }

        for p in &self.particles {
            if self.particle_visible(p) {
                renderer.draw_pixel(Point::new(p.x as i32, p.y as i32), true);
            }
        }

        for b in &self.bullets {
            // A short streak trailing back along its path.
            let speed = (b.vx * b.vx + b.vy * b.vy).sqrt().max(0.001);
            let tx = b.vx / speed * BULLET_LEN;
            let ty = b.vy / speed * BULLET_LEN;
            for (ox, oy) in wrap_offsets(b.x, b.y, BULLET_LEN) {
                renderer.draw_line(
                    Point::new((b.x + ox) as i32, (b.y + oy) as i32),
                    Point::new((b.x - tx + ox) as i32, (b.y - ty + oy) as i32),
                );
            }
        }

        match self.state {
            State::Poof => {
                let frame = (self.timer / POOF_FRAME_DUR) as usize;
                if frame < POOF.frames.len() {
                    let pos = Point::new(
                        self.ship_x as i32 - POOF.width as i32 / 2,
                        self.ship_y as i32 - POOF.height as i32 / 2,
                    );
                    renderer.draw_sprite(&POOF, pos, SpriteOpts { frame, transparent: true, ..Default::default() });
                }
            }
            State::Lose => {}
            _ => {
                // Blink while invulnerable.
                if self.invuln <= 0.0 || (self.invuln * 8.0) as i32 % 2 == 0 {
                    self.draw_ship(renderer);
                }
            }
        }

        let opts = SpriteOpts { transparent: true, ..Default::default() };
        for i in 0..self.lives as i32 {
            let pos = Point::new(i * (LIFE_W + 1), 0);
            renderer.draw_sprite_raw(LIFE_GLYPH, LIFE_W as u16, LIFE_H as u16, pos, opts);
        }

        let mut s: String<8> = String::new();
        let _ = write!(s, "{}", self.score);
        let w = s.len() as i32 * SMALL_CHAR_W;
        renderer.draw_text_small(s.as_str(), Point::new(DISPLAY_W - w, 0));

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
