//! Aquarium tank inhabitants used by `VacationAquariumScene`. These are
//! drawn through midground custom-draw passes (so they share the tanks'
//! 0.6x parallax) rather than going through `Environment::add_entity`.

use core::f32::consts::PI;

use heapless::Vec;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::{
    rand::{rand_bool, rand_range_f32, rand_range_u32, xorshift32},
    render::{Renderer, Sprite, SpriteOpts},
};

// -- Fish --

const TURN_FRAME_TIME: f32 = 0.12;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FishState {
    Left,
    TurnRight,
    Right,
    TurnLeft,
}

pub struct FishEntity {
    pub x: f32,
    pub y: f32,
    sprite: &'static Sprite,
    state: FishState,
    current_frame: usize,
    last_frame: usize,
    turn_idx: u8,
    turn_timer: f32,
    speed: f32,
    swim_timer: f32,
    target_y: f32,
    y_drift_timer: f32,
    y_speed: f32,
    bounds_left: f32,
    bounds_right: f32,
    bounds_top: f32,
    bounds_bottom: f32,
    rng: u32,
}

impl FishEntity {
    pub fn new(
        sprite: &'static Sprite,
        x: f32,
        y: f32,
        speed: f32,
        bounds_left: f32,
        bounds_right: f32,
        bounds_top: f32,
        bounds_bottom: f32,
        parent_rng: &mut u32,
    ) -> Self {
        let mut rng = xorshift32(parent_rng);
        let last_frame = sprite.frames.len() - 1;
        let (state, current_frame) = if rand_bool(&mut rng, 0.5) {
            (FishState::Left, 0)
        } else {
            (FishState::Right, last_frame)
        };
        let speed_jitter = rand_range_f32(&mut rng, -3.0, 3.0);
        let swim_timer = rand_range_f32(&mut rng, 1.5, 6.0);
        let y_drift_timer = rand_range_f32(&mut rng, 4.0, 14.0);
        Self {
            x,
            y,
            sprite,
            state,
            current_frame,
            last_frame,
            turn_idx: 0,
            turn_timer: 0.0,
            speed: speed + speed_jitter,
            swim_timer,
            target_y: y,
            y_drift_timer,
            y_speed: 6.0,
            bounds_left,
            bounds_right,
            bounds_top,
            bounds_bottom,
            rng,
        }
    }

    fn start_turn(&mut self, new_state: FishState) {
        self.state = new_state;
        self.turn_idx = 0;
        self.turn_timer = 0.0;
    }

    fn refresh_swim_timer(&mut self) {
        self.swim_timer = rand_range_f32(&mut self.rng, 1.5, 6.0);
    }

    pub fn update(&mut self, dt: f32) {
        let w = self.sprite.width as f32;

        // Animate intermediate turn frames.
        if matches!(self.state, FishState::TurnRight | FishState::TurnLeft) {
            self.turn_timer += dt;
            let mut completed = false;
            while self.turn_timer >= TURN_FRAME_TIME {
                self.turn_timer -= TURN_FRAME_TIME;
                self.turn_idx += 1;
                if self.turn_idx >= 3 {
                    if self.state == FishState::TurnRight {
                        self.state = FishState::Right;
                        self.current_frame = self.last_frame;
                    } else {
                        self.state = FishState::Left;
                        self.current_frame = 0;
                    }
                    self.refresh_swim_timer();
                    completed = true;
                    break;
                }
            }
            if !completed {
                self.current_frame = match self.state {
                    FishState::TurnRight => (self.turn_idx + 1) as usize,
                    FishState::TurnLeft => (self.last_frame as i32 - 1 - self.turn_idx as i32)
                        .max(0) as usize,
                    _ => self.current_frame,
                };
            }
        }

        // Swim.
        match self.state {
            FishState::Left => {
                self.x -= self.speed * dt;
                self.current_frame = 0;
                self.swim_timer -= dt;
                if self.x <= self.bounds_left {
                    self.x = self.bounds_left;
                    self.start_turn(FishState::TurnRight);
                } else if self.swim_timer <= 0.0 {
                    self.start_turn(FishState::TurnRight);
                }
            }
            FishState::Right => {
                self.x += self.speed * dt;
                self.current_frame = self.last_frame;
                self.swim_timer -= dt;
                if self.x + w >= self.bounds_right {
                    self.x = self.bounds_right - w;
                    self.start_turn(FishState::TurnLeft);
                } else if self.swim_timer <= 0.0 {
                    self.start_turn(FishState::TurnLeft);
                }
            }
            _ => {}
        }

        // Vertical drift.
        self.y_drift_timer -= dt;
        if self.y_drift_timer <= 0.0 {
            self.target_y = rand_range_f32(&mut self.rng, self.bounds_top, self.bounds_bottom);
            self.y_drift_timer = rand_range_f32(&mut self.rng, 4.0, 16.0);
        }
        if self.y < self.target_y {
            self.y = (self.y + self.y_speed * dt).min(self.target_y);
        } else if self.y > self.target_y {
            self.y = (self.y - self.y_speed * dt).max(self.target_y);
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        use embedded_graphics::prelude::Point;
        renderer.draw_sprite(
            self.sprite,
            Point::new(self.x as i32 - camera_offset, self.y as i32),
            SpriteOpts {
                frame: self.current_frame,
                ..Default::default()
            },
        );
    }
}

// -- Octopus --

const OCT_ANIM_TIME: f32 = 0.18;
const OCT_FRAME_COUNT: usize = 8;
const OCT_BOB_AMPLITUDE: f32 = 3.0;
const OCT_BOB_PERIOD: f32 = 4.0;
const OCT_DRIFT_SPEED: f32 = 10.0;
const OCT_DRIFT_MIN: f32 = 2.0;
const OCT_DRIFT_MAX: f32 = 7.0;

pub struct OctopusEntity {
    pub x: f32,
    pub y: f32,
    sprite: &'static Sprite,
    anim_timer: f32,
    current_frame: usize,
    bob_phase: f32,
    vx: f32,
    drift_timer: f32,
    bounds_left: f32,
    bounds_right: f32,
    rng: u32,
}

impl OctopusEntity {
    pub fn new(
        sprite: &'static Sprite,
        x: f32,
        y: f32,
        bounds_left: f32,
        bounds_right: f32,
        parent_rng: &mut u32,
    ) -> Self {
        let mut rng = xorshift32(parent_rng);
        let anim_timer = rand_range_f32(&mut rng, 0.0, OCT_ANIM_TIME);
        let current_frame = rand_range_u32(&mut rng, 0, (OCT_FRAME_COUNT - 1) as u32) as usize;
        let bob_phase = rand_range_f32(&mut rng, 0.0, PI * 2.0);
        let dir: f32 = if rand_bool(&mut rng, 0.5) { -1.0 } else { 1.0 };
        let vx = dir * rand_range_f32(&mut rng, 0.2, 0.5);
        let drift_timer = rand_range_f32(&mut rng, OCT_DRIFT_MIN, OCT_DRIFT_MAX);
        Self {
            x,
            y,
            sprite,
            anim_timer,
            current_frame,
            bob_phase,
            vx,
            drift_timer,
            bounds_left,
            bounds_right,
            rng,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let w = self.sprite.width as f32;
        self.anim_timer += dt;
        if self.anim_timer >= OCT_ANIM_TIME {
            self.anim_timer -= OCT_ANIM_TIME;
            self.current_frame = (self.current_frame + 1) % OCT_FRAME_COUNT;
        }
        self.drift_timer -= dt;
        if self.drift_timer <= 0.0 {
            let dir: f32 = if rand_bool(&mut self.rng, 0.5) { -1.0 } else { 1.0 };
            self.vx = dir * rand_range_f32(&mut self.rng, 0.2, 0.5);
            self.drift_timer = rand_range_f32(&mut self.rng, OCT_DRIFT_MIN, OCT_DRIFT_MAX);
        }
        self.x += self.vx * OCT_DRIFT_SPEED * dt;
        if self.x <= self.bounds_left {
            self.x = self.bounds_left;
            self.vx = self.vx.abs();
        } else if self.x + w >= self.bounds_right {
            self.x = self.bounds_right - w;
            self.vx = -self.vx.abs();
        }
        self.bob_phase = (self.bob_phase + dt * (2.0 * PI / OCT_BOB_PERIOD)) % (PI * 2.0);
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        use embedded_graphics::prelude::Point;
        let bob = (self.bob_phase.sin() * OCT_BOB_AMPLITUDE) as i32;
        renderer.draw_sprite(
            self.sprite,
            Point::new(self.x as i32 - camera_offset, self.y as i32 + bob),
            SpriteOpts {
                frame: self.current_frame,
                ..Default::default()
            },
        );
    }
}

// -- Bubble groups --

const BUBBLE_RISE_SPEED_MIN: f32 = 6.0;
const BUBBLE_RISE_SPEED_MAX: f32 = 10.0;
const BUBBLE_WOBBLE_AMP: f32 = 2.0;
const BUBBLE_WOBBLE_FREQ: f32 = 1.2;
const BUBBLE_RESPAWN_MIN: f32 = 3.0;
const BUBBLE_RESPAWN_MAX: f32 = 8.0;
const BUBBLE_COUNT_MIN: u32 = 3;
const BUBBLE_COUNT_MAX: u32 = 6;
const BUBBLE_Y_SPREAD: f32 = 10.0;
const MAX_BUBBLES: usize = 6;
const MAX_TANK_RANGES: usize = 4;

#[derive(Clone, Copy)]
struct Bubble {
    x: f32,
    y: f32,
    wobble_phase: f32,
    wobble_freq_mult: f32,
    rise_speed: f32,
}

pub struct BubbleGroup {
    sprite: &'static Sprite,
    tank_ranges: Vec<(i32, i32), MAX_TANK_RANGES>,
    tank_top: i32,
    tank_floor: i32,
    active: bool,
    respawn_timer: f32,
    bubbles: Vec<Bubble, MAX_BUBBLES>,
    rng: u32,
}

impl BubbleGroup {
    pub fn new(
        sprite: &'static Sprite,
        tank_ranges: &[(i32, i32)],
        tank_top: i32,
        tank_floor: i32,
        parent_rng: &mut u32,
    ) -> Self {
        let mut rng = xorshift32(parent_rng);
        let mut ranges: Vec<(i32, i32), MAX_TANK_RANGES> = Vec::new();
        for &r in tank_ranges {
            let _ = ranges.push(r);
        }
        let respawn_timer = rand_range_f32(&mut rng, 1.0, 3.0);
        Self {
            sprite,
            tank_ranges: ranges,
            tank_top,
            tank_floor,
            active: false,
            respawn_timer,
            bubbles: Vec::new(),
            rng,
        }
    }

    fn spawn(&mut self) {
        let idx = rand_range_u32(&mut self.rng, 0, (self.tank_ranges.len() - 1) as u32) as usize;
        let (tank_left, tank_right) = self.tank_ranges[idx];
        let margin = 8;
        let lo = (tank_left + margin) as f32;
        let hi = (tank_right - margin) as f32;
        let center_x = rand_range_f32(&mut self.rng, lo, hi);
        let base_y = (self.tank_floor - 2) as f32;
        let n = rand_range_u32(&mut self.rng, BUBBLE_COUNT_MIN, BUBBLE_COUNT_MAX) as usize;
        self.bubbles.clear();
        for _ in 0..n.min(MAX_BUBBLES) {
            let x = center_x + rand_range_f32(&mut self.rng, -4.0, 4.0);
            let y = base_y - rand_range_f32(&mut self.rng, 0.0, BUBBLE_Y_SPREAD);
            let wobble_phase = rand_range_f32(&mut self.rng, 0.0, PI * 2.0);
            let wobble_freq_mult = rand_range_f32(&mut self.rng, 0.8, 1.4);
            let rise_speed =
                rand_range_f32(&mut self.rng, BUBBLE_RISE_SPEED_MIN, BUBBLE_RISE_SPEED_MAX);
            let _ = self.bubbles.push(Bubble {
                x,
                y,
                wobble_phase,
                wobble_freq_mult,
                rise_speed,
            });
        }
        self.active = true;
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active {
            self.respawn_timer -= dt;
            if self.respawn_timer <= 0.0 {
                self.spawn();
            }
            return;
        }
        let top_cutoff = (self.tank_top - 3) as f32;
        let mut i = self.bubbles.len();
        while i > 0 {
            i -= 1;
            let b = &mut self.bubbles[i];
            b.y -= b.rise_speed * dt;
            b.wobble_phase = (b.wobble_phase
                + dt * BUBBLE_WOBBLE_FREQ * b.wobble_freq_mult * PI * 2.0)
                % (PI * 2.0);
            if b.y < top_cutoff {
                self.bubbles.swap_remove(i);
            }
        }
        if self.bubbles.is_empty() {
            self.active = false;
            self.respawn_timer =
                rand_range_f32(&mut self.rng, BUBBLE_RESPAWN_MIN, BUBBLE_RESPAWN_MAX);
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        if !self.active {
            return;
        }
        use embedded_graphics::prelude::Point;
        for b in &self.bubbles {
            let sx = (b.x + b.wobble_phase.sin() * BUBBLE_WOBBLE_AMP) as i32 - camera_offset;
            renderer.draw_sprite(
                self.sprite,
                Point::new(sx, b.y as i32),
                SpriteOpts::default(),
            );
        }
    }
}

// -- Debris --

const DEBRIS_SPEED_MIN: f32 = 3.0;
const DEBRIS_SPEED_MAX: f32 = 6.0;
const DEBRIS_DIR_MIN: f32 = 3.0;
const DEBRIS_DIR_MAX: f32 = 8.0;
const MAX_DEBRIS: usize = 12;

#[derive(Clone, Copy)]
struct DebrisParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    dir_timer: f32,
    tank_left: f32,
    tank_right: f32,
}

pub struct DebrisField {
    tank_top: i32,
    tank_floor: i32,
    particles: Vec<DebrisParticle, MAX_DEBRIS>,
    rng: u32,
}

impl DebrisField {
    pub fn new(
        count: usize,
        tank_ranges: &[(i32, i32)],
        tank_top: i32,
        tank_floor: i32,
        parent_rng: &mut u32,
    ) -> Self {
        let mut rng = xorshift32(parent_rng);
        let mut ranges: Vec<(i32, i32), MAX_TANK_RANGES> = Vec::new();
        for &r in tank_ranges {
            let _ = ranges.push(r);
        }
        let mut particles: Vec<DebrisParticle, MAX_DEBRIS> = Vec::new();
        for _ in 0..count.min(MAX_DEBRIS) {
            particles
                .push(Self::make_particle(&mut rng, &ranges, tank_top, tank_floor))
                .ok();
        }
        Self {
            tank_top,
            tank_floor,
            particles,
            rng,
        }
    }

    fn random_velocity(rng: &mut u32) -> (f32, f32) {
        let spd = rand_range_f32(rng, DEBRIS_SPEED_MIN, DEBRIS_SPEED_MAX);
        let angle = rand_range_f32(rng, 0.0, PI * 2.0);
        (angle.cos() * spd, angle.sin() * spd)
    }

    fn make_particle(
        rng: &mut u32,
        ranges: &Vec<(i32, i32), MAX_TANK_RANGES>,
        tank_top: i32,
        tank_floor: i32,
    ) -> DebrisParticle {
        let idx = rand_range_u32(rng, 0, (ranges.len() - 1) as u32) as usize;
        let (tank_left, tank_right) = ranges[idx];
        let x = rand_range_f32(rng, (tank_left + 2) as f32, (tank_right - 2) as f32);
        let y = rand_range_f32(rng, (tank_top + 2) as f32, (tank_floor - 2) as f32);
        let (vx, vy) = Self::random_velocity(rng);
        let dir_timer = rand_range_f32(rng, DEBRIS_DIR_MIN, DEBRIS_DIR_MAX);
        DebrisParticle {
            x,
            y,
            vx,
            vy,
            dir_timer,
            tank_left: (tank_left + 1) as f32,
            tank_right: (tank_right - 1) as f32,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let top = (self.tank_top + 1) as f32;
        let bot = (self.tank_floor - 1) as f32;
        for p in self.particles.iter_mut() {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            if p.x < p.tank_left {
                p.x = p.tank_left;
                p.vx = p.vx.abs();
            } else if p.x > p.tank_right {
                p.x = p.tank_right;
                p.vx = -p.vx.abs();
            }
            if p.y < top {
                p.y = top;
                p.vy = p.vy.abs();
            } else if p.y > bot {
                p.y = bot;
                p.vy = -p.vy.abs();
            }
            p.dir_timer -= dt;
            if p.dir_timer <= 0.0 {
                let (vx, vy) = Self::random_velocity(&mut self.rng);
                p.vx = vx;
                p.vy = vy;
                p.dir_timer = rand_range_f32(&mut self.rng, DEBRIS_DIR_MIN, DEBRIS_DIR_MAX);
            }
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        use embedded_graphics::prelude::Point;
        for p in &self.particles {
            renderer.draw_pixel(Point::new(p.x as i32 - camera_offset, p.y as i32), true);
        }
    }
}
