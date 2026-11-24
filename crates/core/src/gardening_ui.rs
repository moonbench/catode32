//! Interactive cursor modes for the plant system.
//!
//! `PlacementMode` drives the cursor used to place a new pot, plant a seed in
//! the ground, or relocate an existing plant. `PlantSelectionMode` cycles the
//! cursor across plants in the current scene so the player can pick one to
//! water / fertilize / tend.

use embedded_graphics::prelude::Point;

use crate::{
    assets::{
        icons,
        plants::{pot_sprite, plant_sprite, PotKind},
    },
    context::GameContext,
    environment::Environment,
    input::{Button, Buttons},
    plant_system::{display_stage, Plant, PlantLayer},
    render::{Renderer, SpriteOpts},
    scene::SceneId,
};

const DISPLAY_WIDTH: i32 = 128;
const STEP: i32 = 8;
const BOUNCE_PERIOD: f32 = 0.4;

#[derive(Clone, Copy)]
pub struct PlantSurface {
    pub y_snap: i32,
    pub layer: PlantLayer,
    pub x_min: i32,
    pub x_max: i32,
}

const fn layer_order(l: PlantLayer) -> u8 {
    match l {
        PlantLayer::Background => 0,
        PlantLayer::Midground => 1,
        PlantLayer::Foreground => 2,
    }
}

fn surface_x_max(surf: &PlantSurface, world_width: i32) -> i32 {
    if surf.x_max != 0 {
        return surf.x_max;
    }
    let p = surf.layer.parallax();
    (DISPLAY_WIDTH as f32 * (1.0 - p) + world_width as f32 * p) as i32
}

/// Whether `x` on `layer` at `y_snap` is somewhere `surfaces` allows a plant.
fn on_a_surface(surfaces: &[PlantSurface], world_width: i32, layer: PlantLayer, x: i32, y_snap: i32) -> bool {
    surfaces.iter().any(|s| {
        s.layer == layer && s.y_snap == y_snap && x >= s.x_min && x <= surface_x_max(s, world_width)
    })
}

/// Move any plant in `scene` that is no longer on one of the scene's
/// surfaces down to the foreground floor. Lets a scene's layout change (a
/// counter moved, a shelf removed) without a save migration: plants left
/// floating in old spots land on the ground the next time the scene opens.
///
/// The plant keeps roughly the same place on screen. If another plant
/// already stands at exactly that spot on the floor, it shuffles over two
/// pixels at a time, right then left, until it finds a free one.
pub fn rehome_stranded_plants(
    ctx: &mut GameContext,
    scene: SceneId,
    surfaces: &[PlantSurface],
    world_width: i32,
) {
    let Some(floor) = surfaces
        .iter()
        .filter(|s| s.layer == PlantLayer::Foreground)
        .max_by_key(|s| s.y_snap)
        .copied()
    else {
        return;
    };
    let (lo, hi) = (floor.x_min, surface_x_max(&floor, world_width));
    let max_cam = (world_width - DISPLAY_WIDTH).max(0);
    for i in 0..ctx.plants.len() {
        let p = ctx.plants[i];
        if p.scene != scene || on_a_surface(surfaces, world_width, p.layer, p.x, p.y_snap) {
            continue;
        }
        // Where the plant shows on screen with the camera centred on it,
        // then the floor x that shows in the same place.
        let par = p.layer.parallax();
        let cam = ((p.x - DISPLAY_WIDTH / 2) as f32 / par).clamp(0.0, max_cam as f32);
        let want = (p.x as f32 + cam * (1.0 - par)) as i32;
        let want = want.clamp(lo, hi);
        let taken = |x: i32| {
            ctx.plants.iter().any(|q| {
                q.id != p.id
                    && q.scene == scene
                    && q.layer == PlantLayer::Foreground
                    && q.y_snap == floor.y_snap
                    && q.x == x
            })
        };
        let mut x = want;
        for step in 1..=(hi - lo + 2) {
            if !taken(x) {
                break;
            }
            let off = 2 * ((step + 1) / 2);
            let cand = if step % 2 == 1 { want + off } else { want - off };
            if cand >= lo && cand <= hi {
                x = cand;
            }
        }
        let q = &mut ctx.plants[i];
        q.layer = PlantLayer::Foreground;
        q.x = x;
        q.y_snap = floor.y_snap;
    }
}

/// Sort surfaces y_snap asc, then by layer (foreground last), so the starting
/// cursor lands on the foreground floor.
fn sort_surfaces(src: &[PlantSurface], out: &mut heapless::Vec<PlantSurface, 8>) {
    out.clear();
    for s in src {
        let _ = out.push(*s);
    }
    out.sort_unstable_by(|a, b| {
        a.y_snap
            .cmp(&b.y_snap)
            .then(layer_order(a.layer).cmp(&layer_order(b.layer)))
    });
}

// ---------------------------------------------------------------------------
// PlacementMode
// ---------------------------------------------------------------------------

/// What the player is placing. The `Ground` variant uses no pot sprite and
/// goes straight to a seedling.
#[derive(Clone, Copy)]
pub enum PlacementKind {
    /// A new empty pot of the given size, taken from inventory on confirm.
    Pot(PotKind),
    /// Direct ground planting (outdoor only). Inventory consumed by the
    /// confirm handler.
    Ground,
    /// Re-positioning an existing plant. Confirm just calls `move_plant`.
    Move { pot: PotKind, plant_id: u32 },
    /// Planting a seed directly in the ground. The seed kind is held by the
    /// scene's confirm dispatcher so we don't need to thread it here.
    GroundSeed,
}

pub struct PlacementMode {
    active: bool,
    kind: PlacementKind,
    surfaces: heapless::Vec<PlantSurface, 8>,
    surface_idx: usize,
    cursor_x: i32,
    bounce_t: f32,
    world_width: i32,
}

impl PlacementMode {
    pub fn new() -> Self {
        Self {
            active: false,
            kind: PlacementKind::Ground,
            surfaces: heapless::Vec::new(),
            surface_idx: 0,
            cursor_x: 64,
            bounce_t: 0.0,
            world_width: DISPLAY_WIDTH,
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn enter(
        &mut self,
        kind: PlacementKind,
        surfaces: &[PlantSurface],
        env: &Environment,
    ) {
        sort_surfaces(surfaces, &mut self.surfaces);
        if self.surfaces.is_empty() {
            return;
        }
        let start_idx = self.surfaces.len() - 1; // foreground floor
        let surf = self.surfaces[start_idx];
        let x_min = surf.x_min;
        let x_max = surface_x_max(&surf, env.world_width);
        let cursor = (env.camera_x + DISPLAY_WIDTH / 2).clamp(x_min, x_max);

        self.active = true;
        self.kind = kind;
        self.surface_idx = start_idx;
        self.cursor_x = cursor;
        self.bounce_t = 0.0;
        self.world_width = env.world_width;
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.bounce_t = 0.0;
    }

    pub fn current_kind(&self) -> PlacementKind {
        self.kind
    }

    pub fn update(&mut self, dt: f32) {
        self.bounce_t = (self.bounce_t + dt) % (BOUNCE_PERIOD * 2.0);
    }

    /// Returns `Some((layer, x, y_snap))` on A-press, signalling the scene to
    /// commit the placement. None for cancel/no-op.
    pub fn handle_input(
        &mut self,
        buttons: &mut Buttons,
        env: &mut Environment,
    ) -> PlacementResult {
        // Menu1 / Menu2 act as a global escape hatch so the cursor never
        // soft-locks the game even if A/B somehow get swallowed elsewhere.
        if buttons.was_just_pressed(Button::B)
            || buttons.was_just_pressed(Button::Menu1)
            || buttons.was_just_pressed(Button::Menu2)
        {
            self.cancel();
            return PlacementResult::Cancelled;
        }
        if buttons.was_just_pressed(Button::A) {
            let surf = self.surfaces[self.surface_idx];
            let result = PlacementResult::Confirm {
                layer: surf.layer,
                x: self.cursor_x,
                y_snap: surf.y_snap,
            };
            self.cancel();
            return result;
        }

        let dx = if buttons.was_just_pressed(Button::Right) {
            STEP
        } else if buttons.was_just_pressed(Button::Left) {
            -STEP
        } else {
            0
        };
        if dx != 0 {
            let surf = self.surfaces[self.surface_idx];
            let x_min = surf.x_min;
            let x_max = surface_x_max(&surf, self.world_width);
            self.cursor_x = (self.cursor_x + dx).clamp(x_min, x_max);
            self.follow_camera(env);
        }

        // Up / down switch surfaces, keeping screen-x stable across parallax.
        let n = self.surfaces.len();
        if buttons.was_just_pressed(Button::Up) && self.surface_idx > 0 {
            self.switch_surface(self.surface_idx - 1, env.camera_x);
        } else if buttons.was_just_pressed(Button::Down) && self.surface_idx + 1 < n {
            self.switch_surface(self.surface_idx + 1, env.camera_x);
        }

        PlacementResult::Continue
    }

    fn follow_camera(&self, env: &mut Environment) {
        let surf = self.surfaces[self.surface_idx];
        let parallax = surf.layer.parallax();
        let margin = 32;
        let sx = self.cursor_x - (env.camera_x as f32 * parallax) as i32;
        if sx < margin {
            env.set_camera(((self.cursor_x - margin) as f32 / parallax) as i32);
        } else if sx > DISPLAY_WIDTH - margin {
            env.set_camera(
                ((self.cursor_x - (DISPLAY_WIDTH - margin)) as f32 / parallax) as i32,
            );
        }
    }

    fn switch_surface(&mut self, new_idx: usize, camera_x: i32) {
        let old = self.surfaces[self.surface_idx];
        let old_p = old.layer.parallax();
        self.surface_idx = new_idx;
        let new_surf = self.surfaces[new_idx];
        let new_p = new_surf.layer.parallax();
        self.cursor_x = self.cursor_x + (camera_x as f32 * (new_p - old_p)) as i32;
        let x_min = new_surf.x_min;
        let x_max = surface_x_max(&new_surf, self.world_width);
        self.cursor_x = self.cursor_x.clamp(x_min, x_max);
    }

    pub fn draw(&self, renderer: &mut Renderer, env: &Environment) {
        if !self.active {
            return;
        }
        let surf = self.surfaces[self.surface_idx];
        let parallax = surf.layer.parallax();
        let sx = self.cursor_x - (env.camera_x as f32 * parallax) as i32;
        let bounce_offset = if self.bounce_t < BOUNCE_PERIOD { 0 } else { 2 };

        let (sy, icon_x) = match self.kind {
            PlacementKind::Pot(pot) | PlacementKind::Move { pot, .. } => {
                if let Some(spr) = pot_sprite(pot) {
                    let sy = surf.y_snap - spr.height as i32;
                    renderer.draw_sprite(
                        spr,
                        Point::new(sx, sy),
                        SpriteOpts::default(),
                    );
                    let icon_x = sx + (spr.width as i32) / 2
                        - (icons::PLACE_DOWN_W as i32) / 2;
                    (sy, icon_x)
                } else {
                    (surf.y_snap, sx - (icons::PLACE_DOWN_W as i32) / 2)
                }
            }
            PlacementKind::Ground | PlacementKind::GroundSeed => {
                let sy = surf.y_snap;
                let icon_x = sx - (icons::PLACE_DOWN_W as i32) / 2;
                (sy, icon_x)
            }
        };

        let icon_y = sy - icons::PLACE_DOWN_H as i32 - 2 + bounce_offset;
        renderer.draw_sprite_raw(
            icons::PLACE_DOWN_FILL,
            icons::PLACE_DOWN_W,
            icons::PLACE_DOWN_H,
            Point::new(icon_x, icon_y),
            SpriteOpts {
                transparent: true,
                transparent_color: true,
                invert: true,
                ..Default::default()
            },
        );
        renderer.draw_sprite_raw(
            icons::PLACE_DOWN_FRAME,
            icons::PLACE_DOWN_W,
            icons::PLACE_DOWN_H,
            Point::new(icon_x, icon_y),
            SpriteOpts {
                transparent: true,
                ..Default::default()
            },
        );
    }
}

pub enum PlacementResult {
    Continue,
    Cancelled,
    Confirm {
        layer: PlantLayer,
        x: i32,
        y_snap: i32,
    },
}

// ---------------------------------------------------------------------------
// PlantSelectionMode
// ---------------------------------------------------------------------------

/// Filter applied when cycling plants.
#[derive(Clone, Copy)]
pub enum SelectionFilter {
    /// Any plant in the scene.
    All,
    /// Only empty pots (for "Plant Seed in Pot").
    EmptyPot,
    /// Only alive, waterable plants (skip dead + empty_pot).
    Alive,
}

impl SelectionFilter {
    fn matches(self, p: &Plant) -> bool {
        use crate::assets::plants::PlantStage;
        match self {
            SelectionFilter::All => true,
            SelectionFilter::EmptyPot => p.stage == PlantStage::EmptyPot,
            SelectionFilter::Alive => p.stage != PlantStage::EmptyPot && !p.stage.is_dead(),
        }
    }
}

const MAX_SELECTABLE: usize = 16;

pub struct PlantSelectionMode {
    active: bool,
    ids: heapless::Vec<u32, MAX_SELECTABLE>,
    idx: usize,
    bounce_t: f32,
}

impl PlantSelectionMode {
    pub fn new() -> Self {
        Self {
            active: false,
            ids: heapless::Vec::new(),
            idx: 0,
            bounce_t: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    /// Returns true if at least one plant matched. Pre-populates the cursor
    /// at `start_plant_id` if it's still selectable, otherwise index 0, and
    /// pans the camera to it if it's off-screen.
    pub fn enter(
        &mut self,
        ctx: &GameContext,
        env: &mut Environment,
        scene: SceneId,
        filter: SelectionFilter,
        start_plant_id: Option<u32>,
    ) -> bool {
        // Build a list sorted by x.
        let mut entries: heapless::Vec<(u32, i32), MAX_SELECTABLE> = heapless::Vec::new();
        for p in ctx.plants.iter() {
            if p.scene != scene || !filter.matches(p) {
                continue;
            }
            if entries.push((p.id, p.x)).is_err() {
                break;
            }
        }
        if entries.is_empty() {
            self.active = false;
            return false;
        }
        entries.sort_unstable_by_key(|(_, x)| *x);
        self.ids.clear();
        for (id, _) in entries.iter() {
            let _ = self.ids.push(*id);
        }
        self.idx = start_plant_id
            .and_then(|id| self.ids.iter().position(|i| *i == id))
            .unwrap_or(0);
        self.active = true;
        self.bounce_t = 0.0;
        if !self.selected_on_screen(ctx, env) {
            self.follow_camera(ctx, env);
        }
        true
    }

    fn selected_on_screen(&self, ctx: &GameContext, env: &Environment) -> bool {
        let plant = match self
            .current_id()
            .and_then(|id| ctx.plants.iter().find(|p| p.id == id))
        {
            Some(p) => p,
            None => return true,
        };
        let sx = plant.x - (env.camera_x as f32 * plant.layer.parallax()) as i32;
        let pot_w = pot_sprite(plant.pot).map(|s| s.width as i32).unwrap_or(0);
        sx >= 0 && sx + pot_w <= DISPLAY_WIDTH
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }

    pub fn current_id(&self) -> Option<u32> {
        if self.active {
            self.ids.get(self.idx).copied()
        } else {
            None
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.bounce_t = (self.bounce_t + dt) % (BOUNCE_PERIOD * 2.0);
    }

    /// Returns the selected plant id on A-press, or None for cancel/no-op.
    pub fn handle_input(
        &mut self,
        ctx: &GameContext,
        buttons: &mut Buttons,
        env: &mut Environment,
    ) -> SelectionResult {
        if buttons.was_just_pressed(Button::B)
            || buttons.was_just_pressed(Button::Menu1)
            || buttons.was_just_pressed(Button::Menu2)
        {
            self.cancel();
            return SelectionResult::Cancelled;
        }
        if buttons.was_just_pressed(Button::A) {
            if let Some(id) = self.current_id() {
                self.cancel();
                return SelectionResult::Confirm(id);
            }
            self.cancel();
            return SelectionResult::Cancelled;
        }
        let n = self.ids.len();
        if n > 1 {
            if buttons.was_just_pressed(Button::Right) {
                self.idx = (self.idx + 1) % n;
                self.follow_camera(ctx, env);
            } else if buttons.was_just_pressed(Button::Left) {
                self.idx = (self.idx + n - 1) % n;
                self.follow_camera(ctx, env);
            }
        }
        SelectionResult::Continue
    }

    fn follow_camera(&self, ctx: &GameContext, env: &mut Environment) {
        let id = match self.ids.get(self.idx) {
            Some(i) => *i,
            None => return,
        };
        let plant = match ctx.plants.iter().find(|p| p.id == id) {
            Some(p) => p,
            None => return,
        };
        let parallax = plant.layer.parallax();
        let target = ((plant.x - DISPLAY_WIDTH / 2) as f32 / parallax) as i32;
        env.set_camera(target);
    }

    pub fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, env: &Environment) {
        if !self.active {
            return;
        }
        let id = match self.current_id() {
            Some(i) => i,
            None => return,
        };
        let plant = match ctx.plants.iter().find(|p| p.id == id) {
            Some(p) => p,
            None => return,
        };

        let parallax = plant.layer.parallax();
        let sx_base = plant.x - (env.camera_x as f32 * parallax) as i32;

        let (pot_w, pot_h) = if let Some(spr) = pot_sprite(plant.pot) {
            (spr.width as i32, spr.height as i32)
        } else {
            // Ground plant uses the plant sprite height as the icon offset.
            let h = plant
                .seed
                .and_then(|s| plant_sprite(s, display_stage(plant)))
                .map(|s| s.height as i32)
                .unwrap_or(0);
            (0, h)
        };

        let sy = plant.y_snap - pot_h;
        let bounce_offset = if self.bounce_t < BOUNCE_PERIOD { 0 } else { 2 };
        let icon_x = sx_base + pot_w / 2 - (icons::PLACE_DOWN_W as i32) / 2;
        let icon_y = sy - icons::PLACE_DOWN_H as i32 - 2 + bounce_offset;

        renderer.draw_sprite_raw(
            icons::PLACE_DOWN_FILL,
            icons::PLACE_DOWN_W,
            icons::PLACE_DOWN_H,
            Point::new(icon_x, icon_y),
            SpriteOpts {
                transparent: true,
                transparent_color: true,
                invert: true,
                ..Default::default()
            },
        );
        renderer.draw_sprite_raw(
            icons::PLACE_DOWN_FRAME,
            icons::PLACE_DOWN_W,
            icons::PLACE_DOWN_H,
            Point::new(icon_x, icon_y),
            SpriteOpts {
                transparent: true,
                ..Default::default()
            },
        );
    }
}

pub enum SelectionResult {
    Continue,
    Cancelled,
    Confirm(u32),
}

// ---------------------------------------------------------------------------
// PlantBursts: small map of {plant_id: BurstEffect} for watering/fertilizing
// ---------------------------------------------------------------------------

use crate::ui::burst::BurstEffect;

const MAX_PLANT_BURSTS: usize = 4;

pub struct PlantBursts {
    slots: heapless::Vec<(u32, BurstEffect), MAX_PLANT_BURSTS>,
}

impl PlantBursts {
    pub fn new() -> Self {
        Self {
            slots: heapless::Vec::new(),
        }
    }

    pub fn trigger(&mut self, plant_id: u32, rng: &mut u32, count: usize) {
        if let Some(slot) = self.slots.iter_mut().find(|(id, _)| *id == plant_id) {
            slot.1.trigger_plant(rng, count);
            return;
        }
        if self.slots.is_full() {
            // Evict whichever burst is no longer active; if all are active, drop
            // the request rather than disturb an in-flight animation.
            if let Some(pos) = self.slots.iter().position(|(_, e)| !e.active()) {
                self.slots.swap_remove(pos);
            } else {
                return;
            }
        }
        let mut eff = BurstEffect::new();
        eff.trigger_plant(rng, count);
        let _ = self.slots.push((plant_id, eff));
    }

    pub fn update(&mut self, dt: f32) {
        for (_, e) in self.slots.iter_mut() {
            e.update(dt);
        }
        self.slots.retain(|(_, e)| e.active());
    }

    pub fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, env: &Environment) {
        for (id, eff) in self.slots.iter() {
            let plant = match ctx.plants.iter().find(|p| p.id == *id) {
                Some(p) => p,
                None => continue,
            };
            let parallax = plant.layer.parallax();
            let sx = plant.x - (env.camera_x as f32 * parallax) as i32;
            let pot_h = pot_sprite(plant.pot).map(|s| s.height as i32).unwrap_or(0);
            let plant_h = plant
                .seed
                .and_then(|s| plant_sprite(s, display_stage(plant)))
                .map(|s| s.height as i32)
                .unwrap_or(0);
            let pot_w = pot_sprite(plant.pot).map(|s| s.width as i32).unwrap_or(0);
            let cx = sx + pot_w / 2;
            let mid_y = plant.y_snap - pot_h - plant_h / 2;
            eff.draw(renderer, Point::new(cx, mid_y));
        }
    }
}
