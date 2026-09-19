use embedded_graphics::prelude::{Point, Size};

use crate::{
    assets::{furniture::BOOKSHELF, items::BOX_SMALL_1},
    behavior::NextBehavior,
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

const PLANT_SURFACES: &[PlantSurface] = &[
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 26, x_max: 182 },
    PlantSurface { y_snap: 60, layer: PlantLayer::Midground,  x_min: 26, x_max: 150 },
    PlantSurface { y_snap: 29, layer: PlantLayer::Midground,  x_min: 95, x_max: 150 },
];

const WORLD_WIDTH: i32 = 192;
const CHAR_WORLD_X: i32 = 64;
const CHAR_WORLD_Y: i32 = 64;

const DISPLAY_WIDTH: i32 = 128;
const DISPLAY_HEIGHT: i32 = 64;

// Window cutout.
const WINDOW_WORLD_X: i32 = 100;
const WINDOW_Y: i32 = -10;
const WINDOW_W: i32 = 56;
const WINDOW_H: i32 = 36;

pub struct InsideScene {
    base: LocationScene,
    clock: ClockWidget,
}

impl InsideScene {
    pub fn new() -> Self {
        Self {
            base: LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y)),
            clock: ClockWidget::new(36, 0),
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
    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Inside, PLANT_SURFACES);
        let bookshelf_y = 63 - BOOKSHELF.height as i32;
        self.base.environment.add_object(
            Layer::Foreground,
            &BOOKSHELF,
            0,
            bookshelf_y,
            false,
        );
        let box_y = bookshelf_y - BOX_SMALL_1.height as i32;
        self.base.environment.add_object(
            Layer::Foreground,
            &BOX_SMALL_1,
            2,
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
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        self.base.draw_overlay(ctx, renderer);
    }
}
