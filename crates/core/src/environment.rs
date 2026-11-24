use embedded_graphics::prelude::Point;
use heapless::Vec;

use crate::render::{Renderer, Sprite, SpriteOpts};

const DISPLAY_WIDTH: i32 = 128;
const MAX_OBJECTS_PER_LAYER: usize = 16;

#[derive(Clone, Copy)]
pub enum Layer {
    Background = 0,
    Midground = 1,
    Foreground = 2,
}

pub(crate) const PARALLAX: [f32; 3] = [0.3, 0.6, 1.0];

/// How far `layer` has scrolled when the camera is at `camera_x`.
pub fn parallax_offset(camera_x: i32, layer: Layer) -> i32 {
    (camera_x as f32 * PARALLAX[layer as usize]) as i32
}

#[derive(Clone, Copy)]
pub struct EnvObject {
    pub sprite: &'static Sprite,
    pub x: i32,
    pub y: i32,
    pub mirror_h: bool,
}

pub struct Environment {
    pub world_width: i32,
    pub camera_x: i32,
    layers: [Vec<EnvObject, MAX_OBJECTS_PER_LAYER>; 3],
}

impl Environment {
    pub fn new(world_width: i32) -> Self {
        Self {
            world_width,
            camera_x: 0,
            layers: [Vec::new(), Vec::new(), Vec::new()],
        }
    }

    pub fn add_object(
        &mut self,
        layer: Layer,
        sprite: &'static Sprite,
        x: i32,
        y: i32,
        mirror_h: bool,
    ) {
        let _ = self.layers[layer as usize].push(EnvObject { sprite, x, y, mirror_h });
    }

    pub fn pan(&mut self, dx: i32) {
        let max_camera = (self.world_width - DISPLAY_WIDTH).max(0);
        self.camera_x = (self.camera_x + dx).clamp(0, max_camera);
    }

    pub fn set_camera(&mut self, x: i32) {
        let max_camera = (self.world_width - DISPLAY_WIDTH).max(0);
        self.camera_x = x.clamp(0, max_camera);
    }

    pub fn camera_offset(&self, layer: Layer) -> i32 {
        parallax_offset(self.camera_x, layer)
    }

    pub fn draw_layer(&self, renderer: &mut Renderer, layer: Layer) {
        let camera_offset = self.camera_offset(layer);
        for obj in &self.layers[layer as usize] {
            let screen_x = obj.x - camera_offset;
            let w = obj.sprite.width as i32;
            if screen_x + w <= 0 || screen_x >= DISPLAY_WIDTH {
                continue;
            }
            renderer.draw_sprite(
                obj.sprite,
                Point::new(screen_x, obj.y),
                SpriteOpts {
                    mirror_h: obj.mirror_h,
                    ..Default::default()
                },
            );
        }
    }

    pub fn draw_all_layers(&self, renderer: &mut Renderer) {
        self.draw_layer(renderer, Layer::Background);
        self.draw_layer(renderer, Layer::Midground);
        self.draw_layer(renderer, Layer::Foreground);
    }
}
