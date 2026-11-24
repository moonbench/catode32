use embedded_graphics::prelude::Point;

use crate::{
    assets::{
        furniture::{
            draw_bed, draw_cat_bed_rim, draw_dresser, BASKET, BED_H, BED_MATTRESS_Y, CABINET_H, CABINET_W,
            CAT_BED_RIM_WIDTH,
        },
        items::YARN_BALL,
    },
    behaviors::climbing::Perch,
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
// 22px / 11px on the slower layers) to fit the whole dresser on the left.
// The spawn moved with it, so the room opens on the same view as before.
const PLANT_SURFACES: &[PlantSurface] = &[
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 70,  x_max: 218 },
    // Top of the dresser.
    PlantSurface { y_snap: DRESSER_Y, layer: PlantLayer::Foreground, x_min: 2, x_max: 52 },
    PlantSurface { y_snap: 60, layer: PlantLayer::Midground,  x_min: 56,  x_max: 112 },
    // Top of the headboard.
    PlantSurface { y_snap: BED_Y, layer: PlantLayer::Midground,  x_min: 206, x_max: 210 },
    // Right of the basket.
    PlantSurface { y_snap: 56, layer: PlantLayer::Background, x_min: 72,  x_max: 126 },
];

const WORLD_WIDTH: i32 = 292;
const CHAR_WORLD_X: i32 = 100;
const CHAR_WORLD_Y: i32 = 64;
const BED_WORLD_X: i32 = 130;
// Each layer stands on its own floor line, higher up the further back it
// is (the same lines plants snap to), so the depth reads as you pan.
const MIDGROUND_FLOOR: i32 = 60;
const BACKGROUND_FLOOR: i32 = 56;
const BED_Y: i32 = MIDGROUND_FLOOR - BED_H;
const CAT_BED_WORLD_X: i32 = 161;
const YARN_WORLD_X: i32 = 104;
/// Laundry basket at the back of the room. With the camera all the way left
/// the dresser's right edge just overlaps it; panning right uncovers it.
const BASKET_WORLD_X: i32 = 42;
const DRESSER_X: i32 = 0;
const DRESSER_Y: i32 = 63 - CABINET_H;
/// The mattress's top edge, between the footboard and the pillow.
const PERCHES: &[Perch] = &[Perch {
    layer: Layer::Midground,
    x_min: BED_WORLD_X + 26,
    x_max: BED_WORLD_X + 42,
    top_y: BED_Y + BED_MATTRESS_Y + 1,
}];

pub struct BedroomScene {
    base: LocationScene,
}

impl BedroomScene {
    pub fn new() -> Self {
        let mut base = LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y));
        base.visit_anchor_x = CHAR_WORLD_X;
        Self { base }
    }

    fn draw_bed(&self, renderer: &mut Renderer) {
        let mg_offset = self.base.environment.camera_offset(Layer::Midground);
        draw_bed(renderer, BED_WORLD_X - mg_offset, BED_Y, false);
    }
}

// Approx foreground world-x of the bed interior; sleep/nap behaviors read
// this so the cat can walk to the bed and get the in-bed comfort bonus.
const CAT_BED_X: i32 = 190;

impl Scene for BedroomScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Bedroom, PLANT_SURFACES);
        // Walkable strip stops well short of the world_width because the
        // bed occupies the right side of the room.
        ctx.scene_x_min = 10;
        ctx.scene_x_max = 218;
        ctx.cat_bed_x = Some(CAT_BED_X);
        ctx.cat_bed_span = Some((CAT_BED_WORLD_X, CAT_BED_WORLD_X + CAT_BED_RIM_WIDTH));
        // Pawing scratches at the dresser's edge the same as a bookshelf's.
        ctx.bookshelf_right_x = Some(DRESSER_X + CABINET_W);
        ctx.perches = PERCHES;
        self.base.environment.add_object(
            Layer::Background,
            &BASKET,
            BASKET_WORLD_X,
            BACKGROUND_FLOOR - BASKET.height as i32,
            false,
        );
        let yarn_y = MIDGROUND_FLOOR - YARN_BALL.height as i32;
        self.base.environment.add_object(Layer::Midground, &YARN_BALL, YARN_WORLD_X, yarn_y, false);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        ctx.cat_bed_x = None;
        ctx.cat_bed_span = None;
        ctx.bookshelf_right_x = None;
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
        self.base.draw_plants(ctx, renderer, PlantLayer::Background);
        // Bed before the midground layer so its mask doesn't wipe the yarn.
        self.draw_bed(renderer);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        draw_dresser(renderer, DRESSER_X - fg_offset, DRESSER_Y);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        // Cat bed rim drawn AFTER the character so the cat appears nestled inside.
        draw_cat_bed_rim(renderer, CAT_BED_WORLD_X - fg_offset);
        self.base.draw_overlay(ctx, renderer);
    }
}
