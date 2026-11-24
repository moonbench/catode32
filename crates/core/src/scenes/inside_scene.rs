use embedded_graphics::prelude::{Point, Size};

use crate::{
    assets::{
        furniture::{draw_bookshelf, draw_couch, draw_floor_lamp, CABINET_H, CABINET_W, COUCH_H, COUCH_SEAT_Y},
        items::BOX_SMALL_1,
    },
    behavior::NextBehavior,
    behaviors::climbing::Perch,
    clock::ClockWidget,
    context::GameContext,
    environment::Layer,
    gardening_ui::PlantSurface,
    input::Buttons,
    location_scene::LocationScene,
    plant_system::PlantLayer,
    render::Renderer,
    scene::{Scene, SceneId},
};

// Everything here sits 36px further right on the floor than it used to (and
// 22px / 11px on the slower layers) to fit the whole bookshelf on the left.
// The spawn moved with it, so the room opens on the same view as before.
const PLANT_SURFACES: &[PlantSurface] = &[
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 62,  x_max: WORLD_WIDTH - 10 },
    // Top of the bookshelf, right of the box.
    PlantSurface { y_snap: BOOKSHELF_Y, layer: PlantLayer::Foreground, x_min: BOX_X + 13, x_max: 52 },
    // Back wall, up to the couch.
    PlantSurface { y_snap: MIDGROUND_FLOOR, layer: PlantLayer::Midground, x_min: 48, x_max: COUCH_WORLD_X - 18 },
    // Window sill.
    PlantSurface { y_snap: 29, layer: PlantLayer::Midground,  x_min: 93, x_max: 148 },
];

/// Wide enough to pan the couch fully into view at the far right.
const WORLD_WIDTH: i32 = 388;
const CHAR_WORLD_X: i32 = 100;
const CHAR_WORLD_Y: i32 = 64;

const DISPLAY_WIDTH: i32 = 128;
const DISPLAY_HEIGHT: i32 = 64;

// Window cutout.
const WINDOW_WORLD_X: i32 = 98;
const WINDOW_Y: i32 = -10;
const WINDOW_W: i32 = 56;
const WINDOW_H: i32 = 36;

const BOOKSHELF_X: i32 = 0;
const BOOKSHELF_Y: i32 = 63 - CABINET_H;
/// The cardboard box at the left end of the bookshelf's top.
const BOX_X: i32 = BOOKSHELF_X + 2;
const CLOCK_WORLD_X: i32 = 58;

/// The midground's floor line: higher up than the cat's, so the back wall
/// furniture reads as further away. Plants there snap to the same line.
const MIDGROUND_FLOOR: i32 = 60;

/// Couch against the wall, just right of the window sill.
const COUCH_WORLD_X: i32 = 170;
const COUCH_Y: i32 = MIDGROUND_FLOOR - COUCH_H;
/// Floor lamp beside the couch, at the right end of the room.
const LAMP_WORLD_X: i32 = 268;
/// The middle of the seat cushions.
const PERCHES: &[Perch] = &[Perch {
    layer: Layer::Midground,
    x_min: COUCH_WORLD_X + 24,
    x_max: COUCH_WORLD_X + 60,
    top_y: COUCH_Y + COUCH_SEAT_Y + 1,
}];

pub struct InsideScene {
    base: LocationScene,
    clock: ClockWidget,
}

impl InsideScene {
    pub fn new() -> Self {
        let mut base = LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y));
        base.visit_anchor_x = CHAR_WORLD_X;
        Self {
            base,
            clock: ClockWidget::new(CLOCK_WORLD_X, 0),
        }
    }

    fn draw_window(&self, renderer: &mut Renderer) {
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        let win_sx = WINDOW_WORLD_X - mg_offset;
        let wall_bottom = WINDOW_Y + WINDOW_H;
        let screen_left = win_sx.max(0);
        let screen_right = (win_sx + WINDOW_W).min(DISPLAY_WIDTH);

        // Mask everything outside the window opening with black. Tall sprites
        // (balloon, plane) that extend below the window get covered by the
        // side and bottom rects.
        if screen_left > 0 {
            renderer.fill_rect_off(
                Point::new(0, 0),
                Size::new(screen_left as u32, DISPLAY_HEIGHT as u32),
            );
        }
        if screen_right < DISPLAY_WIDTH {
            renderer.fill_rect_off(
                Point::new(screen_right, 0),
                Size::new(
                    (DISPLAY_WIDTH - screen_right) as u32,
                    DISPLAY_HEIGHT as u32,
                ),
            );
        }
        if WINDOW_Y > 0 {
            renderer.fill_rect_off(
                Point::new(screen_left, 0),
                Size::new((screen_right - screen_left) as u32, WINDOW_Y as u32),
            );
        }
        if wall_bottom < DISPLAY_HEIGHT && screen_right > screen_left {
            renderer.fill_rect_off(
                Point::new(screen_left, wall_bottom),
                Size::new(
                    (screen_right - screen_left) as u32,
                    (DISPLAY_HEIGHT - wall_bottom) as u32,
                ),
            );
        }

        // Window frame: inner outline, outer outline, and a filled sill.
        renderer.draw_rect(
            Point::new(win_sx, WINDOW_Y),
            Size::new(WINDOW_W as u32, WINDOW_H as u32),
            false,
        );
        renderer.draw_rect(
            Point::new(win_sx - 4, WINDOW_Y - 4),
            Size::new((WINDOW_W + 8) as u32, (WINDOW_H + 8) as u32),
            false,
        );
        renderer.draw_rect(
            Point::new(win_sx - 6, WINDOW_Y + WINDOW_H + 4),
            Size::new((WINDOW_W + 12) as u32, 3),
            true,
        );
    }
}

impl Scene for InsideScene {
    fn exit(&mut self, ctx: &mut GameContext) {
        ctx.bookshelf_right_x = None;
    }

    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Inside, PLANT_SURFACES);
        ctx.bookshelf_right_x = Some(BOOKSHELF_X + CABINET_W);
        ctx.perches = PERCHES;
        let box_y = BOOKSHELF_Y - BOX_SMALL_1.height as i32;
        self.base.environment.add_object(
            Layer::Foreground,
            &BOX_SMALL_1,
            BOX_X,
            box_y,
            false,
        );
        if ctx.first_impressions {
            ctx.first_impressions = false;
            let mis = ctx.mischievousness - 50.0;
            let cour = ctx.courage - 50.0;
            let cur = ctx.curiosity - 50.0;
            let next = if mis >= 5.0 && mis > cur && mis > cour {
                NextBehavior::Zoomies
            } else if cour <= -5.0 && cour <= mis && cour <= cur {
                NextBehavior::Sulking
            } else {
                NextBehavior::Investigating
            };
            self.base
                .behaviors
                .trigger(next, ctx, &mut self.base.character);
        }
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
        self.clock.set_time(ctx.time_hours, ctx.time_minutes);
        None
    }

    fn tick_background(&mut self, ctx: &mut GameContext, dt: f32) {
        self.base.tick_background(ctx, dt);
        self.clock.set_time(ctx.time_hours, ctx.time_minutes);
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
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        self.clock.draw(renderer, mg_offset);
        draw_couch(renderer, COUCH_WORLD_X - mg_offset, COUCH_Y);
        draw_floor_lamp(renderer, LAMP_WORLD_X - mg_offset, MIDGROUND_FLOOR - 1);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        draw_bookshelf(renderer, BOOKSHELF_X - fg_offset, BOOKSHELF_Y);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        self.base.draw_overlay(ctx, renderer);
    }
}
