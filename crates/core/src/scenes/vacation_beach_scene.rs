use embedded_graphics::prelude::Point;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::{
    assets::nature::{
        BEACH_HILL, BEACH_MOUNTAIN, BEACH_SHIMMERS_BG, BEACH_SHIMMERS_MG, BEACH_TREES,
        BEACH_WAVE_BG, BEACH_WAVE_FG, BEACH_WAVE_MG_LG, BEACH_WAVE_MG_SM, WAVE_CHUNKS,
        WAVE_CHUNKS2,
    },
    context::{GameContext, StatId},
    environment::Layer,
    location_scene::LocationScene,
    render::{Renderer, Sprite, SpriteOpts},
    scene::SceneId,
    scenes::vacation_base::{VacationConfig, VacationScene, VacationWorld},
};

const WORLD_WIDTH: i32 = 234;
const GROUND_Y: i32 = 58;
const CHAR_WORLD_X: i32 = 110;

// Hill anchor + derived horizons.
const HILL_WORLD_X: i32 = 54;
const HILL_Y: i32 = 38;
const HILL_W: i32 = 36;
const HILL_H: i32 = 4;
const WATER_HORIZON_Y: i32 = HILL_Y + HILL_H; // 1px below hill bottom
const SAND_HORIZON_Y: i32 = HILL_Y - 1;       // 1px above hill top

const MTN_W: i32 = 48;
const MTN_H: i32 = 16;
const MTN_WORLD_X: i32 = 128 - MTN_W + 31;
const MTN_Y: i32 = HILL_Y - MTN_H - 1;

const TREE_W: i32 = 16;
const TREE_H: i32 = 11;
const TREES_WORLD_X: i32 = HILL_WORLD_X + 14;
const TREES_Y: i32 = HILL_Y - TREE_H + 1;

const SHORE_WORLD_X: i32 = HILL_WORLD_X;

// Packed sand cluster bytes: pairs of (x, y).
const SAND_MG_CLUSTERS: &[u8] = &[
    98, 44, 101, 46,
    115, 41, 118, 43, 120, 41,
    128, 48, 131, 50,
    140, 43, 143, 45,
    152, 50, 155, 48, 158, 51,
    163, 45, 167, 40,
    174, 39, 177, 42, 180, 40,
    185, 47, 180, 43,
];
const SAND_FG_CLUSTERS: &[u8] = &[
    80, 55, 88, 57,
    99, 60, 102, 62, 106, 60,
    118, 53, 121, 55,
    135, 58, 139, 60,
    146, 62, 150, 60, 153, 63,
    158, 55, 162, 57, 165, 55,
    168, 60, 172, 58,
    199, 54, 201, 56,
    210, 58, 215, 60,
];

const TIMER_LENGTH: f32 = 3.0;
const WAVE_CREST_DURATION: f32 = 0.6;
const WAVE_RECEDE_DURATION: f32 = 1.2;
const WAVE_INCOMING_DURATION: f32 = TIMER_LENGTH - WAVE_CREST_DURATION - WAVE_RECEDE_DURATION;

const SHIMMER_STEP_DURATION: f32 = 0.18;

const WAVE_TRAVEL_BG: i32 = 4;
const WAVE_TRAVEL_MG_SM: i32 = 6;
const WAVE_TRAVEL_MG_LG: i32 = 8;
const WAVE_TRAVEL_FG: i32 = 12;

const TIMER_SCALE: f32 = TIMER_LENGTH / 256.0;
const SHORE_PAR_SCALE: f32 = 1.0 / 128.0;

#[derive(Default)]
pub struct BeachWorld {
    wave_timer: f32,
}

pub type VacationBeachScene = VacationScene<BeachWorld>;

impl BeachWorld {
    fn draw_background(&self, renderer: &mut Renderer, base: &LocationScene) {
        let offset = base.environment.camera_offset(Layer::Background);
        // Shimmers use the layer parallax (already baked in to `shimmer_cam`);
        // waves use the per-y `shore_par_128` baked into the chunks, so they
        // need the raw camera_x. Applying layer parallax on top would slow
        // the waves below the hill's drift and make them appear to track right.
        let raw_cam = base.environment.camera_x as f32;
        let shimmer_cam = raw_cam * 0.3;
        let sw = 128;
        let hill_sx = HILL_WORLD_X - offset;

        // Water horizon left of the hill.
        let water_right = hill_sx.clamp(0, sw);
        if water_right > 0 {
            renderer.draw_line(
                Point::new(0, WATER_HORIZON_Y),
                Point::new(water_right - 1, WATER_HORIZON_Y),
            );
        }
        // Sand horizon right of the hill.
        let sand_left = (hill_sx + HILL_W).clamp(0, sw);
        if sand_left < sw {
            renderer.draw_line(
                Point::new(sand_left, SAND_HORIZON_Y),
                Point::new(sw - 1, SAND_HORIZON_Y),
            );
        }

        if hill_sx + HILL_W >= 0 && hill_sx < sw {
            renderer.draw_sprite(&BEACH_HILL, Point::new(hill_sx, HILL_Y), SpriteOpts::default());
        }
        let mtn_sx = MTN_WORLD_X - offset;
        if mtn_sx + MTN_W >= 0 && mtn_sx < sw {
            renderer.draw_sprite(
                &BEACH_MOUNTAIN,
                Point::new(mtn_sx, MTN_Y),
                SpriteOpts::default(),
            );
        }
        let tree_sx = TREES_WORLD_X - offset;
        if tree_sx + TREE_W >= 0 && tree_sx < sw {
            renderer.draw_sprite(
                &BEACH_TREES,
                Point::new(tree_sx, TREES_Y),
                SpriteOpts::default(),
            );
        }

        draw_shimmers(renderer, shimmer_cam, self.wave_timer, BEACH_SHIMMERS_BG);
        draw_wave_group(
            renderer,
            raw_cam,
            self.wave_timer,
            BEACH_WAVE_BG,
            &WAVE_CHUNKS2,
            4,
            WAVE_TRAVEL_BG,
        );
    }

    fn draw_midground(&self, renderer: &mut Renderer, base: &LocationScene) {
        let offset = base.environment.camera_offset(Layer::Midground);
        let raw_cam = base.environment.camera_x as f32;
        let shimmer_cam = raw_cam * 0.6;
        let sw = 128;
        let mut i = 0;
        while i + 1 < SAND_MG_CLUSTERS.len() {
            let sx = SAND_MG_CLUSTERS[i] as i32 - offset;
            let sy = SAND_MG_CLUSTERS[i + 1] as i32;
            if (0..sw).contains(&sx) {
                renderer.draw_pixel(Point::new(sx, sy), true);
            }
            i += 2;
        }

        draw_shimmers(renderer, shimmer_cam, self.wave_timer, BEACH_SHIMMERS_MG);
        draw_wave_group(
            renderer,
            raw_cam,
            self.wave_timer,
            BEACH_WAVE_MG_SM,
            &WAVE_CHUNKS2,
            4,
            WAVE_TRAVEL_MG_SM,
        );
        draw_wave_group(
            renderer,
            raw_cam,
            self.wave_timer,
            BEACH_WAVE_MG_LG,
            &WAVE_CHUNKS,
            8,
            WAVE_TRAVEL_MG_LG,
        );
    }

    fn draw_foreground(&self, renderer: &mut Renderer, base: &LocationScene) {
        let offset = base.environment.camera_offset(Layer::Foreground);
        let raw_cam = base.environment.camera_x as f32;
        let sw = 128;
        let mut i = 0;
        while i + 1 < SAND_FG_CLUSTERS.len() {
            let sx = SAND_FG_CLUSTERS[i] as i32 - offset;
            let sy = SAND_FG_CLUSTERS[i + 1] as i32;
            if (0..sw).contains(&sx) {
                renderer.draw_pixel(Point::new(sx, sy), true);
            }
            i += 2;
        }
        draw_wave_group(
            renderer,
            raw_cam,
            self.wave_timer,
            BEACH_WAVE_FG,
            &WAVE_CHUNKS,
            8,
            WAVE_TRAVEL_FG,
        );
    }
}

/// Wave groups travel from offshore to the shoreline, hold a crest, then
/// recede. `chunks` packs (y, phase_256, shore_par_128) per entry so each
/// wave has its own vertical anchor, time offset, and per-y shore parallax.
fn draw_wave_group(
    renderer: &mut Renderer,
    camera_x: f32,
    timer: f32,
    chunks: &[u8],
    sprite: &'static Sprite,
    num_frames: usize,
    travel_px: i32,
) {
    let sprite_w = sprite.width as i32;
    let half_w = sprite_w / 2;
    let last_frame = num_frames - 1;
    let frame_interval = WAVE_INCOMING_DURATION / num_frames as f32;
    let mut i = 0;
    while i + 2 < chunks.len() {
        let y = chunks[i] as i32;
        let phase = chunks[i + 1] as f32 * TIMER_SCALE;
        let shore_par_128 = chunks[i + 2] as f32;
        let t = rem_euclid_f32(timer + phase, TIMER_LENGTH);
        let shore_x = (SHORE_WORLD_X as f32 - camera_x * shore_par_128 * SHORE_PAR_SCALE) as i32
            - half_w;
        let (frame, travel);
        if t < WAVE_INCOMING_DURATION {
            let mut f = (t / frame_interval) as usize;
            if f > last_frame {
                f = last_frame;
            }
            frame = f;
            travel = travel_px * f as i32 / last_frame as i32;
        } else if t < WAVE_INCOMING_DURATION + WAVE_CREST_DURATION {
            frame = last_frame;
            travel = travel_px;
        } else {
            frame = last_frame;
            let recede_elapsed = t - (WAVE_INCOMING_DURATION + WAVE_CREST_DURATION);
            let progress = (1.0 - recede_elapsed / WAVE_RECEDE_DURATION).max(0.0);
            travel = (travel_px as f32 * progress) as i32;
        }
        renderer.draw_sprite(
            sprite,
            Point::new(shore_x - (travel_px - travel), y),
            SpriteOpts {
                frame,
                ..Default::default()
            },
        );
        i += 3;
    }
}

/// Shimmer pixels: each entry holds (x, y, phase_256, dir_bit, max_drift) so
/// each shimmer drifts a couple of pixels left or right then disappears.
fn draw_shimmers(renderer: &mut Renderer, camera_x: f32, timer: f32, shimmers: &[u8]) {
    let sw = 128;
    let offset = (camera_x) as i32; // parallax already baked into camera_x
    let mut i = 0;
    while i + 4 < shimmers.len() {
        let x = shimmers[i] as i32;
        let y = shimmers[i + 1] as i32;
        let phase_256 = shimmers[i + 2] as f32;
        let dir_bit = shimmers[i + 3];
        let max_drift = shimmers[i + 4] as i32;
        let dark_dur = TIMER_LENGTH - max_drift as f32 * SHIMMER_STEP_DURATION;
        let t = rem_euclid_f32(timer + phase_256 * TIMER_SCALE, TIMER_LENGTH);
        if t < dark_dur {
            i += 5;
            continue;
        }
        let step = ((t - dark_dur) / SHIMMER_STEP_DURATION) as i32;
        if step >= max_drift {
            i += 5;
            continue;
        }
        let direction = if dir_bit != 0 { 1 } else { -1 };
        let sx = x + step * direction - offset;
        if (0..sw).contains(&sx) {
            renderer.draw_pixel(Point::new(sx, y), true);
        }
        i += 5;
    }
}

fn rem_euclid_f32(a: f32, m: f32) -> f32 {
    let r = a - (a / m).floor() * m;
    if r < 0.0 {
        r + m
    } else {
        r
    }
}

impl VacationWorld for BeachWorld {
    const SCENE_ID: SceneId = SceneId::VacationBeach;
    const WORLD_WIDTH: i32 = WORLD_WIDTH;
    const CHAR_WORLD_X: i32 = CHAR_WORLD_X;
    const GROUND_Y: i32 = GROUND_Y;
    // Beach walkable strip is shifted right so the cat stays on sand, not water.
    const X_MIN: i32 = 100;
    const X_MAX: i32 = WORLD_WIDTH - 10;
    const CONFIG: VacationConfig = VacationConfig::standard(&[
        (StatId::Serenity, 8.0),
        (StatId::Fulfillment, 8.0),
    ]);
    const HAS_SKY: bool = true;

    fn enter(&mut self, _ctx: &mut GameContext, _base: &mut LocationScene) {
        self.wave_timer = 0.0;
    }

    fn tick(&mut self, _ctx: &mut GameContext, dt: f32) {
        self.wave_timer += dt;
        if self.wave_timer >= TIMER_LENGTH {
            self.wave_timer -= TIMER_LENGTH;
        }
    }

    fn draw_world(&self, _ctx: &GameContext, renderer: &mut Renderer, base: &LocationScene) {
        base.environment.draw_layer(renderer, Layer::Background);
        self.draw_background(renderer, base);
        base.environment.draw_layer(renderer, Layer::Midground);
        self.draw_midground(renderer, base);
        base.environment.draw_layer(renderer, Layer::Foreground);
        self.draw_foreground(renderer, base);
    }
}
