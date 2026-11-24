use embedded_graphics::prelude::{Point, Size};

use crate::{
    assets::furniture::{draw_fridge, draw_trashcan, FridgeLayout, FAUCET, FRIDGE_H, FRIDGE_W},
    clock::ClockWidget,
    context::GameContext,
    environment::Layer,
    gardening_ui::PlantSurface,
    input::Buttons,
    location_scene::LocationScene,
    plant_system::PlantLayer,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
};

const PLANT_SURFACES: &[PlantSurface] = &[
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 0,  x_max: WORLD_WIDTH - 12 },
    PlantSurface { y_snap: 24, layer: PlantLayer::Midground,  x_min: COUNTER_X_START, x_max: 160 },
];

/// Wide enough that the bin past the end of the counter scrolls into view.
const WORLD_WIDTH: i32 = 256;
const CHAR_WORLD_X: i32 = 64;
const CHAR_WORLD_Y: i32 = 64;

/// The fridge stands at the far left of the midground, feet on the floor.
const FRIDGE_X: i32 = 4;
const FRIDGE_FLOOR_Y: i32 = 60;
const COUNTER_X_START: i32 = FRIDGE_X + FRIDGE_W + 6;
const COUNTER_X_END: i32 = 175;
const COUNTER_TOP_Y: i32 = 24;
const FAUCET_WORLD_X: i32 = 104;
/// The bin stands on the floor just past the end of the counter.
const TRASH_X: i32 = COUNTER_X_END + 5;

pub struct KitchenScene {
    base: LocationScene,
    clock: ClockWidget,
    fridge: FridgeLayout,
}

impl KitchenScene {
    pub fn new() -> Self {
        Self {
            base: LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y)),
            clock: ClockWidget::new(122, 0),
            fridge: FridgeLayout::from_seed(0),
        }
    }

    fn draw_counter(&self, renderer: &mut Renderer) {
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        draw_fridge(renderer, FRIDGE_X - mg_offset, FRIDGE_FLOOR_Y - FRIDGE_H, &self.fridge);
        let sx = (COUNTER_X_START - mg_offset).max(0);
        let ex = (COUNTER_X_END - mg_offset).min(128);
        if sx >= ex {
            return;
        }

        // Counter top
        renderer.draw_rect(
            Point::new(sx - 3, COUNTER_TOP_Y),
            Size::new((ex - sx + 6) as u32, 4),
            true,
        );
        // Cabinet front outline
        renderer.draw_rect(
            Point::new(sx, COUNTER_TOP_Y + 4),
            Size::new((ex - sx) as u32, 30),
            false,
        );
        // Cabinet door dividers
        let mut door_wx = COUNTER_X_START + 15;
        while door_wx < COUNTER_X_END {
            let dx = door_wx - mg_offset;
            if sx < dx && dx < ex {
                renderer.draw_line(Point::new(dx, COUNTER_TOP_Y + 4), Point::new(dx, 57));
            }
            door_wx += 30;
        }

        // Faucet sits on the counter (sprite y = top_y - faucet_h)
        let fx = FAUCET_WORLD_X - mg_offset;
        let fy = COUNTER_TOP_Y - FAUCET.height as i32;
        renderer.draw_sprite(&FAUCET, Point::new(fx, fy), SpriteOpts::default());

        draw_trashcan(renderer, TRASH_X - mg_offset, FRIDGE_FLOOR_Y);
    }
}

impl Scene for KitchenScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.fridge = FridgeLayout::from_seed(ctx.pet_seed);
        self.base.enter(ctx, SceneId::Kitchen, PLANT_SURFACES);
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
        // Closed room, no sky.
        self.base.environment.draw_layer(renderer, Layer::Background);
        self.base.draw_plants(ctx, renderer, PlantLayer::Background);
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        self.clock.draw(renderer, mg_offset);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.draw_counter(renderer);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        self.base.draw_overlay(ctx, renderer);
    }
}
