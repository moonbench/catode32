use embedded_graphics::prelude::{Point, Size};

use crate::{
    assets::{furniture::draw_cat_bed_rim, nature::COBWEB},
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
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 0, x_max: 0 },
    PlantSurface { y_snap: 59, layer: PlantLayer::Midground,  x_min: 0, x_max: 0 },
];

const WORLD_WIDTH: i32 = 256;
const CHAR_WORLD_X: i32 = 64;
const CHAR_WORLD_Y: i32 = 64;
const CAT_BED_WORLD_X: i32 = 130;
const COBWEB_WORLD_X: i32 = 156;

pub struct TreehouseScene {
    base: LocationScene,
}

impl TreehouseScene {
    pub fn new() -> Self {
        Self {
            base: LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y)),
        }
    }

    fn draw_platform_fg(&self, renderer: &mut Renderer) {
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        let sx = (0 - fg_offset).max(0);
        let ex = (WORLD_WIDTH - fg_offset).min(128);
        if sx >= ex {
            return;
        }
        for py in [59, 61] {
            renderer.draw_line(Point::new(sx, py), Point::new(ex, py));
        }
        for &world_x in &[10, 80, 160, 245] {
            let px = world_x - fg_offset;
            if sx <= px && px <= ex {
                renderer.draw_line(Point::new(px, 59), Point::new(px, 63));
            }
        }
    }

    fn draw_platform_mid(&self, renderer: &mut Renderer) {
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        let sx = (0 - mg_offset).max(0);
        let ex = (300 - mg_offset).min(128);
        if sx >= ex {
            return;
        }
        renderer.draw_line(Point::new(sx, 57), Point::new(ex, 57));
        for &world_x in &[20, 90, 160, 230] {
            let px = world_x - mg_offset;
            if sx <= px && px <= ex {
                renderer.draw_line(Point::new(px, 57), Point::new(px, 59));
            }
        }
        // Tree trunks framing the platform
        renderer.fill_rect_off(Point::new(0 - mg_offset - 1, -1), Size::new(10, 61));
        renderer.fill_rect_off(Point::new(180 - mg_offset, -1), Size::new(10, 61));
        renderer.draw_rect(Point::new(0 - mg_offset - 1, -1), Size::new(10, 61), false);
        renderer.draw_rect(Point::new(180 - mg_offset, -1), Size::new(10, 61), false);
        // Cobweb tucked in the top-right corner of the platform.
        renderer.draw_sprite(
            &COBWEB,
            Point::new(COBWEB_WORLD_X - mg_offset, 0),
            SpriteOpts::default(),
        );
    }

}

// Foreground world-x of the cat bed interior; sleep/nap behaviors read this
// so the cat can walk to the bed and get the in-bed comfort bonus.
const CAT_BED_X: i32 = 150;

impl Scene for TreehouseScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Treehouse, PLANT_SURFACES);
        // Walkable strip is inset from the tree-trunk frames.
        ctx.scene_x_min = 20;
        ctx.scene_x_max = 236;
        ctx.cat_bed_x = Some(CAT_BED_X);
        // Acquire the radio for passive ESP-NOW listening. Phase 4
        // will gate this on "not currently visiting" once visits exist.
        crate::espnow_manager::start_session(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        ctx.cat_bed_x = None;
        crate::espnow_manager::stop_session(ctx);
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
        None
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
        // Open-air, sky visible.
        self.base.draw_sky(renderer, ctx);
        self.base.environment.draw_layer(renderer, Layer::Background);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.draw_platform_mid(renderer);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.draw_platform_fg(renderer);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        draw_cat_bed_rim(renderer, CAT_BED_WORLD_X - fg_offset);
        self.base.draw_overlay(ctx, renderer);
    }
}
