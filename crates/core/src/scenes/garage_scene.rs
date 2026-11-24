use embedded_graphics::prelude::{Point, Size};

use crate::{
    assets::{furniture::draw_trashcan, nature::COBWEB},
    context::GameContext,
    entities::dust::DustBeam,
    environment::Layer,
    gardening_ui::PlantSurface,
    input::Buttons,
    location_scene::{draw_window_cutout, LocationScene},
    plant_system::PlantLayer,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    sky::sky_light_now,
};

const PLANT_SURFACES: &[PlantSurface] = &[
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 0, x_max: WORLD_WIDTH - 12 },
    // Window sill, as in the living room.
    PlantSurface { y_snap: 29, layer: PlantLayer::Midground, x_min: WINDOW_WORLD_X - 5, x_max: WINDOW_WORLD_X + 50 },
    // Workbench top.
    PlantSurface { y_snap: BENCH_TOP_Y, layer: PlantLayer::Midground, x_min: BENCH_X_START + 2, x_max: BENCH_X_END - 14 },
];

const WORLD_WIDTH: i32 = 256;
const CHAR_WORLD_X: i32 = 64;
const CHAR_WORLD_Y: i32 = 64;

/// Everything below is on the midground (the back wall), whose floor line
/// sits a little above the cat's.
const MIDGROUND_FLOOR: i32 = 60;

// Window opening above the bench, the same size and height as the living
// room's: it runs off the top of the screen, sill just above the bench.
const WINDOW_WORLD_X: i32 = 40;
const WINDOW_Y: i32 = -10;
const WINDOW_W: i32 = 56;
const WINDOW_H: i32 = 36;

/// The workbench: lower than the kitchen counter (top at 24), so it sits just
/// under the window sill.
const BENCH_X_START: i32 = 18;
const BENCH_X_END: i32 = 118;
const BENCH_TOP_Y: i32 = 36;
const BENCH_TOP_H: i32 = 3;
const BENCH_SHELF_Y: i32 = 50;
const BENCH_LEG_W: i32 = 2;
const BENCH_LEGS: [i32; 3] = [BENCH_X_START + 3, (BENCH_X_START + BENCH_X_END) / 2, BENCH_X_END - 3 - BENCH_LEG_W];

/// Two bins side by side, right of the bench.
const TRASH_XS: [i32; 2] = [138, 161];
/// Cobweb in the top-right corner of the back wall, which only pans into
/// view at the far right of the room.
const COBWEB_WORLD_X: i32 = 180;

pub struct GarageScene {
    base: LocationScene,
    dust: DustBeam,
}

impl GarageScene {
    pub fn new() -> Self {
        Self {
            base: LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y)),
            dust: Self::new_dust(0),
        }
    }

    fn new_dust(seed: u32) -> DustBeam {
        DustBeam::new(WINDOW_WORLD_X, WINDOW_Y, WINDOW_W, WINDOW_H, MIDGROUND_FLOOR, seed)
    }

    fn tick_dust(&mut self, ctx: &GameContext, dt: f32) {
        self.dust.set_light(sky_light_now(ctx));
        self.dust.update(dt);
    }

    fn draw_window(&self, renderer: &mut Renderer) {
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        draw_window_cutout(renderer, WINDOW_WORLD_X - mg_offset, WINDOW_Y, WINDOW_W, WINDOW_H);
    }

    fn draw_bench(&self, renderer: &mut Renderer) {
        let mg = self.base.environment.camera_offset(Layer::Midground);
        let sx = BENCH_X_START - mg;
        let ex = BENCH_X_END - mg;
        if ex < 0 || sx >= 128 {
            return;
        }
        let w = (BENCH_X_END - BENCH_X_START) as u32;

        // Top slab.
        renderer.draw_rect(Point::new(sx, BENCH_TOP_Y), Size::new(w, BENCH_TOP_H as u32), true);
        // Apron board just under the top.
        renderer.draw_line(
            Point::new(sx + 3, BENCH_TOP_Y + BENCH_TOP_H + 1),
            Point::new(ex - 4, BENCH_TOP_Y + BENCH_TOP_H + 1),
        );
        // Legs down to the floor.
        let leg_top = BENCH_TOP_Y + BENCH_TOP_H;
        for &lx in &BENCH_LEGS {
            renderer.draw_rect(
                Point::new(lx - mg, leg_top),
                Size::new(BENCH_LEG_W as u32, (MIDGROUND_FLOOR - leg_top) as u32),
                true,
            );
        }
        // Lower shelf between the outer legs.
        renderer.draw_line(
            Point::new(BENCH_LEGS[0] - mg, BENCH_SHELF_Y),
            Point::new(BENCH_LEGS[2] + BENCH_LEG_W - 1 - mg, BENCH_SHELF_Y),
        );

        // Odds and ends on the shelf: a box and a paint can.
        let box_x = BENCH_LEGS[0] + BENCH_LEG_W + 4 - mg;
        renderer.draw_rect(Point::new(box_x, BENCH_SHELF_Y - 7), Size::new(12, 7), false);
        renderer.draw_line(
            Point::new(box_x + 4, BENCH_SHELF_Y - 7),
            Point::new(box_x + 7, BENCH_SHELF_Y - 7),
        );
        let can_x = BENCH_LEGS[1] + BENCH_LEG_W + 10 - mg;
        renderer.draw_rect(Point::new(can_x, BENCH_SHELF_Y - 8), Size::new(8, 8), false);
        renderer.draw_line(
            Point::new(can_x, BENCH_SHELF_Y - 6),
            Point::new(can_x + 7, BENCH_SHELF_Y - 6),
        );
        // Can handle.
        renderer.draw_line(
            Point::new(can_x + 2, BENCH_SHELF_Y - 10),
            Point::new(can_x + 5, BENCH_SHELF_Y - 10),
        );
        renderer.draw_pixel(Point::new(can_x + 1, BENCH_SHELF_Y - 9), true);
        renderer.draw_pixel(Point::new(can_x + 6, BENCH_SHELF_Y - 9), true);
    }

    fn draw_back_wall(&self, renderer: &mut Renderer) {
        let mg = self.base.environment.camera_offset(Layer::Midground);
        for &tx in &TRASH_XS {
            draw_trashcan(renderer, tx - mg, MIDGROUND_FLOOR);
        }
        renderer.draw_sprite(
            &COBWEB,
            Point::new(COBWEB_WORLD_X - mg, 0),
            SpriteOpts::default(),
        );
    }
}

impl Scene for GarageScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Garage, PLANT_SURFACES);
        self.dust = Self::new_dust(ctx.rng ^ 0x6A5_D057);
        self.dust.set_light(sky_light_now(ctx));
        self.dust.fill();
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        if let Some(id) = self.base.update(ctx, buttons, dt) {
            return Some(id);
        }
        self.tick_dust(ctx, dt);
        None
    }

    fn tick_background(&mut self, ctx: &mut GameContext, dt: f32) {
        self.base.tick_background(ctx, dt);
        self.tick_dust(ctx, dt);
    }

    fn mark_behavior_almost_done(&mut self, ctx: &mut GameContext) {
        self.base.mark_behavior_almost_done(ctx);
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.base.menu_active() {
            self.base.draw_menu(renderer);
            return;
        }
        self.base.draw_sky(renderer, ctx);
        self.draw_window(renderer);
        self.base.environment.draw_layer(renderer, Layer::Background);
        self.base.draw_plants(ctx, renderer, PlantLayer::Background);
        self.draw_back_wall(renderer);
        self.draw_bench(renderer);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        self.dust.draw(renderer, mg_offset);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        self.base.draw_overlay(ctx, renderer);
    }
}
