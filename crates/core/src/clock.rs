use core::f32::consts::PI;

use embedded_graphics::prelude::Point;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::{
    assets::items::CLOCKFACE,
    render::{Renderer, SpriteOpts},
};

const HOUR_LEN: f32 = 4.0;
const MINUTE_LEN: f32 = 6.0;
const CENTER_OX: i32 = 8;
const CENTER_OY: i32 = 8;

pub struct ClockWidget {
    pub world_x: i32,
    pub world_y: i32,
    hour_angle: f32,
    minute_angle: f32,
}

impl ClockWidget {
    pub fn new(world_x: i32, world_y: i32) -> Self {
        Self {
            world_x,
            world_y,
            hour_angle: 0.0,
            minute_angle: 0.0,
        }
    }

    pub fn set_time(&mut self, hours: u8, minutes: u8) {
        let h = (hours % 12) as f32 + minutes as f32 / 60.0;
        self.hour_angle = (h / 12.0) * 2.0 * PI;
        self.minute_angle = (minutes as f32 / 60.0) * 2.0 * PI;
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        let sx = self.world_x - camera_offset;
        let sy = self.world_y;
        renderer.draw_sprite(&CLOCKFACE, Point::new(sx, sy), SpriteOpts::default());

        let cx = sx + CENTER_OX;
        let cy = sy + CENTER_OY;
        draw_hand(renderer, cx, cy, self.hour_angle, HOUR_LEN);
        draw_hand(renderer, cx, cy, self.minute_angle, MINUTE_LEN);
    }
}

fn draw_hand(renderer: &mut Renderer, cx: i32, cy: i32, angle: f32, length: f32) {
    let ex = cx + (length * angle.sin() + 0.5) as i32;
    let ey = cy - (length * angle.cos() + 0.5) as i32;
    renderer.draw_line(Point::new(cx, cy), Point::new(ex, ey));
}
