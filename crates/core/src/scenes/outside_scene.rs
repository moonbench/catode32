use embedded_graphics::prelude::Point;
use crate::platform::time::Instant;
use heapless::Vec;

use crate::{
    context::GameContext,
    entities::{
        flyer::{FlyerEntity, FlyerKind},
        jumper::{JumperEntity, JumperKind},
    },
    environment::Layer,
    gardening_ui::PlantSurface,
    input::Buttons,
    location_scene::LocationScene,
    plant_system::PlantLayer,
    rand::{rand_bool, rand_range_f32, rand_range_u32},
    render::Renderer,
    scene::{Scene, SceneId},
    time_system::{Season, Weather},
};

const MAX_RESPAWN_TIMERS: usize = 4;
const JUMPER_RESPAWN_CHANCE: f32 = 0.6;
const JUMPER_RESPAWN_MIN_S: f32 = 5.0;
const JUMPER_RESPAWN_MAX_S: f32 = 90.0;

const PLANT_SURFACES: &[PlantSurface] = &[
    PlantSurface { y_snap: 63, layer: PlantLayer::Foreground, x_min: 0, x_max: 0 },
    PlantSurface { y_snap: 61, layer: PlantLayer::Midground,  x_min: 0, x_max: 0 },
    PlantSurface { y_snap: 56, layer: PlantLayer::Background, x_min: 0, x_max: 0 },
];

const WORLD_WIDTH: i32 = 256;
const CHAR_WORLD_X: i32 = 64;
const CHAR_WORLD_Y: i32 = 64;

const GRASS_WORLD_XS: &[i32] = &[10, 35, 80, 110, 150, 190, 230];
const MAX_CRITTERS: usize = 12;

enum Critter {
    Flyer(FlyerEntity),
    Jumper(JumperEntity),
}

#[derive(Clone, Copy)]
enum CritterKind {
    Flyer(FlyerKind),
    Jumper(JumperKind),
}

struct CritterSpec {
    kind: CritterKind,
    seasons: &'static [Season],
    spawn_weight: f32,
    max_spawn: u32,
    night_only: bool,
}

const CRITTER_SPECS: &[CritterSpec] = &[
    CritterSpec {
        kind: CritterKind::Flyer(FlyerKind::Butterfly),
        seasons: &[Season::Spring, Season::Summer],
        spawn_weight: 0.65,
        max_spawn: 3,
        night_only: false,
    },
    CritterSpec {
        kind: CritterKind::Flyer(FlyerKind::Moth),
        seasons: &[Season::Spring, Season::Summer, Season::Fall],
        spawn_weight: 0.55,
        max_spawn: 2,
        night_only: true,
    },
    CritterSpec {
        kind: CritterKind::Flyer(FlyerKind::Firefly),
        seasons: &[Season::Spring, Season::Summer, Season::Fall],
        spawn_weight: 0.50,
        max_spawn: 3,
        night_only: true,
    },
    CritterSpec {
        kind: CritterKind::Jumper(JumperKind::Frog),
        seasons: &[Season::Spring, Season::Summer],
        spawn_weight: 0.30,
        max_spawn: 1,
        night_only: false,
    },
    CritterSpec {
        kind: CritterKind::Jumper(JumperKind::Grasshopper),
        seasons: &[Season::Summer, Season::Fall],
        spawn_weight: 0.25,
        max_spawn: 1,
        night_only: false,
    },
];

pub struct OutsideScene {
    base: LocationScene,
    critters: Vec<Critter, MAX_CRITTERS>,
    respawn_timers: Vec<(f32, &'static CritterSpec), MAX_RESPAWN_TIMERS>,
    rng: u32,
    last_weather: Option<Weather>,
}

impl OutsideScene {
    pub fn new() -> Self {
        Self {
            base: LocationScene::new(WORLD_WIDTH, Point::new(CHAR_WORLD_X, CHAR_WORLD_Y)),
            critters: Vec::new(),
            respawn_timers: Vec::new(),
            rng: 1,
            last_weather: None,
        }
    }

    fn find_jumper_spec(kind: JumperKind) -> Option<&'static CritterSpec> {
        CRITTER_SPECS
            .iter()
            .find(|s| matches!(s.kind, CritterKind::Jumper(k) if k == kind))
    }

    fn draw_grass(&self, renderer: &mut Renderer) {
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        for &world_x in GRASS_WORLD_XS {
            let sx = world_x - fg_offset;
            if sx < -5 || sx > 128 + 5 {
                continue;
            }
            renderer.draw_line(Point::new(sx, 64), Point::new(sx - 2, 60));
            renderer.draw_line(Point::new(sx, 64), Point::new(sx, 60));
            renderer.draw_line(Point::new(sx, 64), Point::new(sx + 2, 60));
        }
    }

    fn spawn_critters(&mut self, ctx: &GameContext) {
        self.critters.clear();
        let day = ctx.is_daytime();
        for spec in CRITTER_SPECS {
            if !spec.seasons.contains(&ctx.season) {
                continue;
            }
            // night_only=true requires night, night_only=false requires day.
            if spec.night_only == day {
                continue;
            }
            if !rand_bool(&mut self.rng, spec.spawn_weight) {
                continue;
            }
            let count = rand_range_u32(&mut self.rng, 1, spec.max_spawn);
            for _ in 0..count {
                if self.critters.is_full() {
                    return;
                }
                let critter = make_critter(spec.kind, WORLD_WIDTH, &mut self.rng);
                let _ = self.critters.push(critter);
            }
        }
    }

    fn update_critters(&mut self, ctx: &GameContext, dt: f32) {
        for c in self.critters.iter_mut() {
            match c {
                Critter::Flyer(f) => f.update(dt),
                Critter::Jumper(j) => j.update(dt),
            }
        }

        // Roll for a replacement on each despawned jumper before removing it.
        let day = ctx.is_daytime();
        let season = ctx.season;
        for c in self.critters.iter() {
            if let Critter::Jumper(j) = c {
                if !j.despawned {
                    continue;
                }
                let Some(spec) = Self::find_jumper_spec(j.kind) else {
                    continue;
                };
                if !spec.seasons.contains(&season) {
                    continue;
                }
                if spec.night_only == day {
                    continue;
                }
                if !rand_bool(&mut self.rng, JUMPER_RESPAWN_CHANCE) {
                    continue;
                }
                let delay = rand_range_f32(
                    &mut self.rng,
                    JUMPER_RESPAWN_MIN_S,
                    JUMPER_RESPAWN_MAX_S,
                );
                let _ = self.respawn_timers.push((delay, spec));
            }
        }
        self.critters.retain(|c| match c {
            Critter::Jumper(j) => !j.despawned,
            _ => true,
        });

        // Tick respawn timers and spawn replacements as they fire.
        let mut i = 0;
        while i < self.respawn_timers.len() {
            self.respawn_timers[i].0 -= dt;
            if self.respawn_timers[i].0 <= 0.0 {
                let (_, spec) = self.respawn_timers.swap_remove(i);
                if !ctx.is_daytime() == spec.night_only
                    && spec.seasons.contains(&ctx.season)
                    && !self.critters.is_full()
                {
                    let critter = make_critter(spec.kind, WORLD_WIDTH, &mut self.rng);
                    let _ = self.critters.push(critter);
                }
            } else {
                i += 1;
            }
        }
    }

    fn draw_critters(&self, renderer: &mut Renderer) {
        let fg_offset = self.base.environment.camera_offset(Layer::Foreground);
        for c in &self.critters {
            match c {
                Critter::Flyer(f) => f.draw(renderer, fg_offset),
                Critter::Jumper(j) => j.draw(renderer, fg_offset),
            }
        }
    }
}

fn make_critter(kind: CritterKind, world_width: i32, rng: &mut u32) -> Critter {
    match kind {
        CritterKind::Flyer(variant) => {
            let x = rand_range_f32(rng, 20.0, (world_width - 20) as f32);
            let y = rand_range_f32(rng, 10.0, 35.0);
            Critter::Flyer(FlyerEntity::new(variant, x, y, world_width, rng))
        }
        CritterKind::Jumper(variant) => {
            // Spawn just off a world edge, facing inward.
            let direction: i8 = if rand_bool(rng, 0.5) { 1 } else { -1 };
            let x = if direction == 1 {
                -12.0
            } else {
                (world_width + 12) as f32
            };
            Critter::Jumper(JumperEntity::new(variant, x, direction, rng))
        }
    }
}

impl Scene for OutsideScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.base.enter(ctx, SceneId::Outside, PLANT_SURFACES);
        // Seed the scene's RNG from the system clock so each entry rolls a fresh world.
        self.rng = (Instant::now().duration_since_epoch().as_micros() as u32).max(1);
        self.respawn_timers.clear();
        self.spawn_critters(ctx);
        self.last_weather = Some(ctx.weather);
        // Acquire the radio for passive ESP-NOW listening. Phase 4
        // will gate this on "not currently visiting" once visits exist.
        crate::espnow_manager::start_session(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
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
        if self.last_weather != Some(ctx.weather) {
            self.respawn_timers.clear();
            self.spawn_critters(ctx);
            self.last_weather = Some(ctx.weather);
        }
        self.update_critters(ctx, dt);
        None
    }

    fn tick_background(&mut self, ctx: &mut GameContext, dt: f32) {
        self.base.tick_background(ctx, dt);
        self.update_critters(ctx, dt);
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
        self.base.environment.draw_layer(renderer, Layer::Background);
        self.base.draw_plants(ctx, renderer, PlantLayer::Background);
        self.base.environment.draw_layer(renderer, Layer::Midground);
        self.base.draw_plants(ctx, renderer, PlantLayer::Midground);
        self.base.environment.draw_layer(renderer, Layer::Foreground);
        self.draw_grass(renderer);
        self.draw_critters(renderer);
        self.base.draw_plants(ctx, renderer, PlantLayer::Foreground);
        self.base.draw_character(renderer, ctx);
        self.base.draw_overlay(ctx, renderer);
    }
}
