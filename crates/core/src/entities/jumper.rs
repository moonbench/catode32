use core::f32::consts::PI;

use embedded_graphics::prelude::Point;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::{
    assets::nature::{
        FROG_IDLE, FROG_LEAP, FROG_MID, GRASSHOPPER_IDLE, GRASSHOPPER_LEAP, GRASSHOPPER_MID,
    },
    rand::{rand_bool, rand_range_f32, rand_range_u32, xorshift32},
    render::{Renderer, Sprite, SpriteOpts},
};

pub const GROUND_Y: i32 = 64;
const HOP_DURATION: f32 = 0.45;
const HOP_DISTANCE: f32 = 22.0;
const HOPS_MIN: u32 = 1;
const HOPS_MAX: u32 = 3;
const IDLE_MIN: f32 = 2.0;
const IDLE_MAX: f32 = 7.0;
const WORLD_LEFT: f32 = -30.0;
const WORLD_RIGHT: f32 = 286.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JumperKind {
    Frog,
    Grasshopper,
}

fn sprites_for(kind: JumperKind) -> [&'static Sprite; 3] {
    match kind {
        JumperKind::Frog => [&FROG_IDLE, &FROG_MID, &FROG_LEAP],
        JumperKind::Grasshopper => [&GRASSHOPPER_IDLE, &GRASSHOPPER_MID, &GRASSHOPPER_LEAP],
    }
}

fn arc_height(kind: JumperKind) -> f32 {
    match kind {
        JumperKind::Frog => 7.0,
        JumperKind::Grasshopper => 4.0,
    }
}

pub struct JumperEntity {
    pub kind: JumperKind,
    pub x: f32,
    pub direction: i8,
    pub despawned: bool,
    sprites: [&'static Sprite; 3],
    arc_height: f32,
    hopping: bool,
    hop_progress: f32,
    hops_remaining: u32,
    idle_timer: f32,
    arc_offset: f32,
    pose: u8,
    rng: u32,
}

impl JumperEntity {
    pub fn new(kind: JumperKind, x: f32, direction: i8, parent_rng: &mut u32) -> Self {
        let mut local_rng = xorshift32(parent_rng);
        Self {
            kind,
            x,
            direction,
            despawned: false,
            sprites: sprites_for(kind),
            arc_height: arc_height(kind),
            hopping: false,
            hop_progress: 0.0,
            hops_remaining: 0,
            idle_timer: rand_range_f32(&mut local_rng, 0.2, 1.2),
            arc_offset: 0.0,
            pose: 0,
            rng: local_rng,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.hopping {
            self.hop_progress += dt / HOP_DURATION;
            if self.hop_progress >= 1.0 {
                self.hop_progress = 0.0;
                self.arc_offset = 0.0;
                self.pose = 0;
                self.hops_remaining = self.hops_remaining.saturating_sub(1);
                if self.hops_remaining == 0 {
                    self.hopping = false;
                    self.idle_timer = rand_range_f32(&mut self.rng, IDLE_MIN, IDLE_MAX);
                    if rand_bool(&mut self.rng, 0.25) {
                        self.direction = -self.direction;
                    }
                }
            } else {
                let p = self.hop_progress;
                self.pose = if p < 0.2 {
                    0
                } else if p < 0.45 {
                    1
                } else if p < 0.75 {
                    2
                } else if p < 0.9 {
                    1
                } else {
                    0
                };
                self.arc_offset = if (0.2..=0.9).contains(&p) {
                    let arc_p = (p - 0.2) / 0.7;
                    (arc_p * PI).sin() * self.arc_height
                } else {
                    0.0
                };
                self.x += (self.direction as f32) * (HOP_DISTANCE / HOP_DURATION) * dt;
            }
        } else {
            self.idle_timer -= dt;
            if self.idle_timer <= 0.0 {
                self.hopping = true;
                self.hop_progress = 0.0;
                self.hops_remaining = rand_range_u32(&mut self.rng, HOPS_MIN, HOPS_MAX);
            }
        }

        if self.x < WORLD_LEFT || self.x > WORLD_RIGHT {
            self.despawned = true;
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        if self.despawned {
            return;
        }
        let sprite = self.sprites[self.pose as usize];
        let draw_x = self.x as i32 - sprite.width as i32 / 2 - camera_offset;
        let draw_y = GROUND_Y - sprite.height as i32 - self.arc_offset as i32 - 2;
        renderer.draw_sprite(
            sprite,
            Point::new(draw_x, draw_y),
            SpriteOpts {
                mirror_h: self.direction > 0,
                ..Default::default()
            },
        );
    }
}
