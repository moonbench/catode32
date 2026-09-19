use embedded_graphics::prelude::{Point, Size};

use crate::{
    assets::{
        furniture::{draw_cat_bed_rim, BOOKSHELF, PILLOW},
        items::YARN_BALL,
    },
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
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 34,  x_max: 182 },
    PlantSurface { y_snap: 15, layer: PlantLayer::Foreground, x_min: 0,   x_max: 33 },
    PlantSurface { y_snap: 60, layer: PlantLayer::Midground,  x_min: 34,  x_max: 90 },
    PlantSurface { y_snap: 16, layer: PlantLayer::Midground,  x_min: 184, x_max: 188 },
    PlantSurface { y_snap: 56, layer: PlantLayer::Background, x_min: 34,  x_max: 80 },
];

const WORLD_WIDTH: i32 = 256;
const CHAR_WORLD_X: i32 = 64;
const CHAR_WORLD_Y: i32 = 64;
const BED_WORLD_X: i32 = 108;
const LAMP_WORLD_X: i32 = 146;
const CAT_BED_WORLD_X: i32 = 125;

pub struct BedroomScene {
    base: LocationScene,
}

impl BedroomScene {
    pub fn new() -> Self {
        Self {
            base: LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y)),
        }
    }

    fn draw_lamp(&self, renderer: &mut Renderer) {
        let bg_offset = self.base.environment.camera_offset(Layer::Background);
        let lamp_x = LAMP_WORLD_X - bg_offset;
        // Base
        renderer.draw_line(Point::new(lamp_x - 5, 63), Point::new(lamp_x + 5, 63));
        renderer.draw_line(Point::new(lamp_x - 3, 62), Point::new(lamp_x + 3, 62));
        // Stem
        renderer.draw_rect(Point::new(lamp_x - 1, 20), Size::new(3, 48), true);
        // Shade
        renderer.draw_line(Point::new(lamp_x - 12, 20), Point::new(lamp_x + 12, 20));
        renderer.draw_line(Point::new(lamp_x - 5, 8), Point::new(lamp_x + 5, 8));
        renderer.draw_line(Point::new(lamp_x - 12, 20), Point::new(lamp_x - 5, 8));
        renderer.draw_line(Point::new(lamp_x + 12, 20), Point::new(lamp_x + 5, 8));
        // Cord
        renderer.draw_line(Point::new(lamp_x + 4, 20), Point::new(lamp_x + 4, 28));
    }

    fn draw_bed(&self, renderer: &mut Renderer) {
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        let bed_x = BED_WORLD_X - mg_offset;
        // Mask
        renderer.fill_rect_off(Point::new(bed_x, 32), Size::new(82, 20));
        renderer.fill_rect_off(Point::new(bed_x + 50, 23), Size::new(23, 10));
        renderer.fill_rect_off(Point::new(bed_x + 73, 25), Size::new(2, 8));
        // Frame
        renderer.draw_rect(Point::new(bed_x, 50), Size::new(80, 5), true);
        renderer.draw_rect(Point::new(bed_x, 55), Size::new(8, 9), true);
        renderer.draw_rect(Point::new(bed_x + 80, 16), Size::new(8, 48), true);
        // Mattress outline
        renderer.draw_rect(Point::new(bed_x, 32), Size::new(79, 16), false);
    }

}

// Approx foreground world-x of the bed interior; sleep/nap behaviors read
// this so the cat can walk to the bed and get the in-bed comfort bonus.
const CAT_BED_X: i32 = 154;

impl Scene for BedroomScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Bedroom, PLANT_SURFACES);
        // Walkable strip stops well short of the world_width because the
        // bed occupies the right side of the room.
        ctx.scene_x_min = 10;
        ctx.scene_x_max = 182;
        ctx.cat_bed_x = Some(CAT_BED_X);
        let bookshelf_y = 63 - BOOKSHELF.height as i32;
        self.base.environment.add_object(
            Layer::Foreground,
            &BOOKSHELF,
            0,
            bookshelf_y,
            false,
        );
        self.base.environment.add_object(Layer::Midground, &PILLOW, 158, 23, false);
        let yarn_y = 63 - YARN_BALL.height as i32;
        self.base.environment.add_object(Layer::Midground, &YARN_BALL, 82, yarn_y, false);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        ctx.cat_bed_x = None;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        self.base.update(ctx, buttons, dt)
    }

    fn tick_background(&mut self, ctx: &mut GameContext, dt: f32) {
        self.base.tick_background(ctx, dt);
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
        self.draw_lamp(renderer);
        self.base.draw_plants(ctx, renderer, PlantLayer::Background);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.draw_bed(renderer);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        // Cat bed rim drawn AFTER the character so the cat appears nestled inside.
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        draw_cat_bed_rim(renderer, CAT_BED_WORLD_X - fg_offset);
        self.base.draw_overlay(ctx, renderer);
    }
}
