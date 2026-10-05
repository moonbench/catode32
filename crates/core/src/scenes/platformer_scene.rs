//! Prowl, a platformer minigame.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{
        effects::{BURST1, BURST1_FRAME_DUR, POOF},
        items::{BANDAGE, KEY, SPIN_COIN},
        minigame_character::{
            PLATFORMER_CAT_JUMP, PLATFORMER_CAT_RUN, PLATFORMER_CAT_RUN_SWIPE,
            PLATFORMER_CAT_SIT, PLATFORMER_CAT_SIT_SWIPE, PLATFORMER_SLIME_BURST,
            PLATFORMER_SLIME_IDLE, PLATFORMER_STRIKE,
        },
        plants::{GRASS_GROWING, GRASS_MATURE, GRASS_SEEDLING, GRASS_THRIVING, GRASS_YOUNG},
        platformer_levels::{
            self as levels, parse_level, BLOCK_H, BLOCK_W, CHUNK_H, CHUNK_W, LevelData,
            MAX_COINS, MAX_KEYS, MAX_SLIMES,
        },
        platformer_terrain::{
            PLATFORMER_BG_TILES, PLATFORMER_CHECKPOINT_DOWN, PLATFORMER_CHECKPOINT_UP,
            PLATFORMER_DOOR, PLATFORMER_DOOR_LOCKED, TERRAIN_TILES,
        },
    },
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_f32, xorshift32},
    render::{Renderer, Sprite, SpriteOpts},
    scene::{Scene, SceneId},
};

// ── Physics ──────────────────────────────────────────────────────────────────
const GRAVITY: f32 = 400.0;
const JUMP_VEL: f32 = -185.0;
const RUN_SPEED: f32 = 84.0;

// Cat logical hitbox (centred on self.x / self.feet_y)
const CAT_HALF_W: i32 = 6;
const CAT_H: i32 = 12;

// Terrain tile sizes (the level grid uses 8x8 blocks; one-way platforms are 4 tall).
const BLOCK_W_I: i32 = BLOCK_W as i32;
const BLOCK_H_I: i32 = BLOCK_H as i32;
const CHUNK_W_I: i32 = CHUNK_W as i32;
const CHUNK_H_I: i32 = CHUNK_H as i32;
const PLAT_H: i32 = 4;

// Camera scroll thresholds (screen pixels)
const LEFT_SCROLL_PX: f32 = 60.0;
const RIGHT_SCROLL_PX: f32 = 68.0;
const TOP_SCROLL_PX: f32 = 30.0;
const BOT_SCROLL_PX: f32 = 42.0;
const CAM_LERP: f32 = 5.0;
const CAM_X_MIN: f32 = 0.0;

const DOUBLE_JUMP_ENABLED: bool = true;

// Animation
const IDLE_FPS: f32 = 4.0;
const RUN_FPS: f32 = 12.0;
const JUMP_PEAK_RANGE: f32 = 70.0;

// Combat
const CAT_START_HP: i32 = 2;
const CAT_BLINK_DUR: f32 = 1.5;
const CAT_BLINK_INT: f32 = 0.1;
const CAT_KNOCKBACK_VX: f32 = 100.0;
const CAT_KNOCKBACK_VY: f32 = -100.0;

const SWIPE_FPS: f32 = 12.0;
const ATTACK_FRAME: i32 = 2;
const SIT_SWIPE_FRAMES: i32 = 5;
const RUN_SWIPE_FRAMES: i32 = 3;
const ATK_REACH: i32 = 16;
const RUN_ATK_REACH: i32 = 28;
const STRIKE_VX: f32 = 55.0;
const RUN_STRIKE_OFFSET: i32 = 10;

// Slime
const SLIME_SPEED: f32 = 18.0;
const SLIME_HALF_W: i32 = 8;
const SLIME_H: i32 = 8;
const SLIME_START_HP: i32 = 2;
const SLIME_HIT_FLASH: f32 = 0.15;
const SLIME_ANIM_SPF: f32 = 0.5;
const SLIME_BURST_SPF: f32 = 1.0 / 12.0;
const SLIME_PATROL_RADIUS: i32 = 84;
const SLIME_INSET: i32 = SLIME_HALF_W + BLOCK_W_I + 4;

// Poof death animation (matches assets::effects::POOF speed=8)
const POOF_SPF: f32 = 1.0 / 8.0;

// Level start banner
const LEVEL_BANNER_DUR: f32 = 2.5;
const LEVEL_BANNER_RISE: f32 = 16.0;

// Collectibles
const KEY_BOB_PERIOD: f32 = 1.2;
const KEY_BOB_AMP: f32 = 6.0;
const COIN_BOB_PERIOD: f32 = 1.2;
const COIN_BOB_AMP: f32 = 4.0;
const COIN_ANIM_SPF: f32 = 0.2;

// Door transition timing
const DOOR_WALK_DELAY: f32 = 0.35;
const DOOR_FADE_DURATION: f32 = 0.25;

// Flawless-clear fireworks on the summary screen. Up to 5 cycles, each
// spawning 2-3 bursts at random screen anchors with 4 staggered BURST1
// sparkles per burst.
const FIREWORK_MAX_CYCLES: u8 = 5;
const FIREWORK_CYCLE_INTERVAL: f32 = 0.5;
const FIREWORK_PARTICLES: usize = 4;
const FIREWORK_GROUPS_CAP: usize = 16; // 5 cycles x max 3 bursts + headroom
const FIREWORK_PARTICLE_STAGGER: f32 = 0.5;
const FIREWORK_PARTICLE_TOTAL: f32 = BURST1_FRAME_DUR * 5.0; // 5 frames
const FIREWORK_SPREAD_X: i32 = 8;
const FIREWORK_SPREAD_Y_MIN: i32 = -10;
const FIREWORK_SPREAD_Y_MAX: i32 = 10;

// Trigger hitboxes (match sprite dims)
const CHECKPOINT_W: i32 = 24;
const CHECKPOINT_H: i32 = 8;
const DOOR_W: i32 = 16;
const DOOR_H: i32 = 19;

// GRASS sprite variants, indexed by Grass.variant (0..=4).
const GRASS_SPRITES: &[&Sprite] = &[
    &GRASS_SEEDLING,
    &GRASS_YOUNG,
    &GRASS_GROWING,
    &GRASS_MATURE,
    &GRASS_THRIVING,
];

// ── Door / transition phase ──────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum DoorPhase {
    None,
    Out,
    Summary,
    In,
}

// ── Fireworks (flawless-clear summary) ───────────────────────────────────────

#[derive(Clone, Copy)]
struct FireworkParticle {
    dx: i8,
    dy: i8,
    delay: f32,
}

#[derive(Clone, Copy)]
struct FireworkGroup {
    ax: i16,
    ay: i16,
    timer: f32,
    particles: [FireworkParticle; FIREWORK_PARTICLES],
}

impl FireworkGroup {
    fn finished(&self) -> bool {
        self.particles
            .iter()
            .all(|p| self.timer - p.delay >= FIREWORK_PARTICLE_TOTAL)
    }
}

fn rand_range_i32(rng: &mut u32, lo: i32, hi_inclusive: i32) -> i32 {
    let span = (hi_inclusive - lo + 1) as u32;
    lo + (xorshift32(rng) % span) as i32
}

// ── Slime ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Slime {
    x: f32,
    feet_y: f32,
    vx: f32,
    hp: i32,
    alive: bool,
    dying: bool,
    anim_frame: u8,
    anim_timer: f32,
    burst_frame: u8,
    burst_timer: f32,
    hit_timer: f32,
    dir_timer: f32,
    patrol_min: f32,
    patrol_max: f32,
    chunk_col: u16,
    ground_row: u16,
    wall_row0: u16,
    wall_row1: u16,
}

impl Slime {
    fn new(x: u16, feet_y: u16) -> Self {
        let ix = x as i32;
        let ify = feet_y as i32;
        let chunk_col = (ix / CHUNK_W_I) as u16;
        let ground_row = (ify / CHUNK_H_I) as u16;
        let wall_row0 = ((ify - SLIME_H - BLOCK_H_I + 1).max(0) / CHUNK_H_I) as u16;
        let wall_row1 = (((ify - 1).max(0)) / CHUNK_H_I) as u16;
        let cx_min = (chunk_col as i32 * CHUNK_W_I + SLIME_INSET) as f32;
        let cx_max = ((chunk_col as i32 + 1) * CHUNK_W_I - SLIME_INSET) as f32;
        let patrol_min = ((ix - SLIME_PATROL_RADIUS) as f32).max(cx_min);
        let patrol_max = ((ix + SLIME_PATROL_RADIUS) as f32).min(cx_max);
        Self {
            x: x as f32,
            feet_y: feet_y as f32,
            vx: SLIME_SPEED,
            hp: SLIME_START_HP,
            alive: true,
            dying: false,
            anim_frame: 0,
            anim_timer: 0.0,
            burst_frame: 0,
            burst_timer: 0.0,
            hit_timer: 0.0,
            dir_timer: 1.0,
            patrol_min,
            patrol_max,
            chunk_col,
            ground_row,
            wall_row0,
            wall_row1,
        }
    }
}

// ── Scene ────────────────────────────────────────────────────────────────────

pub struct PlatformerScene {
    level: LevelData,
    current_level: String<16>,

    // Player state
    x: f32,
    feet_y: f32,
    vx: f32,
    vy: f32,
    on_ground: bool,
    just_landed: bool,
    facing_right: bool,
    on_platform: i32,    // -1 = none
    drop_platform: i32,  // -1 = none
    can_double_jump: bool,
    anim_frame: u8,
    anim_timer: f32,

    // Combat
    cat_hp: i32,
    cat_blink_timer: f32,
    swipe_frame: i32, // -1 = idle
    swipe_timer: f32,
    swipe_is_run: bool,
    strike_active: bool,
    strike_x: f32,
    strike_y: f32,
    strike_vx: f32,
    strike_right: bool,
    strike_frame: u8,
    strike_timer: f32,

    // Poof death
    poof_active: bool,
    poof_x: i32,
    poof_y: i32,
    poof_frame: u8,
    poof_timer: f32,

    // Checkpoint
    checkpoint: (u16, u16),
    checkpoint_activated: Vec<bool, { levels::MAX_CHECKPOINTS }>,

    // Door transition
    door_dest: Option<String<16>>,
    door_walk_timer: f32,
    door_fade_phase: DoorPhase,
    door_fade_prog: f32,

    // Collectibles
    key_active: Vec<bool, MAX_KEYS>,
    has_key: bool,
    keys_remaining: u8,
    key_timer: f32,
    coin_active: Vec<bool, MAX_COINS>,
    coin_anim_frame: u8,
    coin_anim_timer: f32,
    coins_remaining: u8,

    // Slimes (flat; small enough for per-frame chunk filtering)
    slimes: Vec<Slime, MAX_SLIMES>,

    // Per-level stats
    level_num: u8,
    injuries: u32,
    slimes_killed: u32,
    total_slimes: u32,
    total_coins: u32,
    level_coins_collected: u32,
    level_time: f32,
    banner_timer: f32,
    summary_slime_frame: u8,
    summary_slime_timer: f32,
    level_flawless: bool,
    fireworks_count: u8,
    fireworks_timer: f32,
    firework_groups: Vec<FireworkGroup, FIREWORK_GROUPS_CAP>,

    // Camera
    camera_x: f32,
    camera_y: f32,
    target_cam_x: f32,
    target_cam_y: f32,
    cam_x_max: f32,
    cam_y_min: f32,
    cam_y_max: f32,
    kill_y: f32,

    // Session totals (carried across levels until exit())
    coins_collected: u32,
    session_slimes_killed: u32,
    session_levels_completed: u32,
}

impl PlatformerScene {
    pub fn new() -> Self {
        Self {
            level: LevelData::empty(),
            current_level: String::new(),
            x: 0.0,
            feet_y: 0.0,
            vx: 0.0,
            vy: 0.0,
            on_ground: true,
            just_landed: false,
            facing_right: true,
            on_platform: -1,
            drop_platform: -1,
            can_double_jump: false,
            anim_frame: 0,
            anim_timer: 0.0,
            cat_hp: CAT_START_HP,
            cat_blink_timer: 0.0,
            swipe_frame: -1,
            swipe_timer: 0.0,
            swipe_is_run: false,
            strike_active: false,
            strike_x: 0.0,
            strike_y: 0.0,
            strike_vx: 0.0,
            strike_right: true,
            strike_frame: 0,
            strike_timer: 0.0,
            poof_active: false,
            poof_x: 0,
            poof_y: 0,
            poof_frame: 0,
            poof_timer: 0.0,
            checkpoint: (8, 8),
            checkpoint_activated: Vec::new(),
            door_dest: None,
            door_walk_timer: 0.0,
            door_fade_phase: DoorPhase::None,
            door_fade_prog: 0.0,
            key_active: Vec::new(),
            has_key: false,
            keys_remaining: 0,
            key_timer: 0.0,
            coin_active: Vec::new(),
            coin_anim_frame: 0,
            coin_anim_timer: 0.0,
            coins_remaining: 0,
            slimes: Vec::new(),
            level_num: 1,
            injuries: 0,
            slimes_killed: 0,
            total_slimes: 0,
            total_coins: 0,
            level_coins_collected: 0,
            level_time: 0.0,
            banner_timer: 0.0,
            summary_slime_frame: 0,
            summary_slime_timer: 0.0,
            level_flawless: false,
            fireworks_count: 0,
            fireworks_timer: 0.0,
            firework_groups: Vec::new(),
            camera_x: 0.0,
            camera_y: 0.0,
            target_cam_x: 0.0,
            target_cam_y: 0.0,
            cam_x_max: 0.0,
            cam_y_min: 0.0,
            cam_y_max: 0.0,
            kill_y: 0.0,
            coins_collected: 0,
            session_slimes_killed: 0,
            session_levels_completed: 0,
        }
    }

    fn load_level(&mut self, name: &str, rng: &mut u32) -> bool {
        let Some(text) = levels::level_text(name) else {
            return false;
        };
        self.level = parse_level(text, rng);
        self.current_level.clear();
        let _ = self.current_level.push_str(name);
        self.init_level_state();
        true
    }

    /// Advance to a named level after a door summary screen. Banks session
    /// totals from the just-finished level, swaps in the new world, and
    /// restarts per-level state. Returns `false` if the destination isn't a
    /// known level.
    fn transition_to_level(&mut self, name: &str, rng: &mut u32) -> bool {
        self.session_slimes_killed += self.slimes_killed;
        self.session_levels_completed += 1;
        self.load_level(name, rng)
    }

    fn init_level_state(&mut self) {
        // Camera / kill-zone limits derived from world size.
        self.cam_x_max = (self.level.world_w as f32 - 128.0).max(0.0);
        self.cam_y_min = 0.0;
        self.cam_y_max = (self.level.world_h as f32 - 64.0).max(0.0);
        self.kill_y = self.level.world_h as f32 + 24.0;

        let (px, py) = self.level.spawn;
        self.x = px as f32;
        self.feet_y = py as f32;
        self.vx = 0.0;
        self.vy = 0.0;
        self.on_ground = true;
        self.just_landed = false;
        self.facing_right = true;
        self.on_platform = -1;
        self.drop_platform = -1;
        self.can_double_jump = false;

        self.anim_timer = 0.0;
        self.anim_frame = 0;

        // Combat reset
        self.cat_hp = CAT_START_HP;
        self.cat_blink_timer = 0.0;
        self.swipe_frame = -1;
        self.swipe_timer = 0.0;
        self.swipe_is_run = false;
        self.strike_active = false;
        self.strike_x = 0.0;
        self.strike_y = 0.0;
        self.strike_vx = 0.0;
        self.strike_right = true;
        self.strike_frame = 0;
        self.strike_timer = 0.0;

        self.poof_active = false;
        self.poof_x = 0;
        self.poof_y = 0;
        self.poof_frame = 0;
        self.poof_timer = 0.0;

        self.checkpoint = self.level.spawn;
        self.checkpoint_activated.clear();
        for _ in 0..self.level.checkpoints.len() {
            let _ = self.checkpoint_activated.push(false);
        }

        self.door_dest = None;
        self.door_walk_timer = 0.0;
        self.door_fade_phase = DoorPhase::None;
        self.door_fade_prog = 0.0;

        self.key_active.clear();
        for _ in 0..self.level.keys.len() {
            let _ = self.key_active.push(true);
        }
        self.has_key = false;
        self.keys_remaining = self.level.keys.len() as u8;
        self.key_timer = 0.0;

        self.coin_active.clear();
        for _ in 0..self.level.coins.len() {
            let _ = self.coin_active.push(true);
        }
        self.coin_anim_frame = 0;
        self.coin_anim_timer = 0.0;
        self.coins_remaining = self.level.coins.len() as u8;

        // Spawn slimes (flat list, small enough for per-frame chunk filtering).
        self.slimes.clear();
        for s in self.level.slime_spawns.iter() {
            let _ = self.slimes.push(Slime::new(s.x, s.y));
        }

        // Per-level stat counters
        self.level_num = parse_level_num(&self.current_level);
        self.injuries = 0;
        self.slimes_killed = 0;
        self.total_slimes = self.level.slime_spawns.len() as u32;
        self.total_coins = self.level.coins.len() as u32;
        self.level_coins_collected = 0;
        self.level_time = 0.0;
        self.banner_timer = 0.0;
        self.summary_slime_frame = 0;
        self.summary_slime_timer = 0.0;
        self.level_flawless = false;
        self.fireworks_count = 0;
        self.fireworks_timer = 0.0;
        self.firework_groups.clear();

        // Snap camera to player spawn so there's no lerp from (0,0) on entry.
        let init_cam_x = (px as f32 - RIGHT_SCROLL_PX)
            .clamp(CAM_X_MIN, self.cam_x_max);
        let init_cam_y = (py as f32 - BOT_SCROLL_PX)
            .clamp(self.cam_y_min, self.cam_y_max);
        self.camera_x = init_cam_x;
        self.camera_y = init_cam_y;
        self.target_cam_x = init_cam_x;
        self.target_cam_y = init_cam_y;
    }

    // ── Terrain queries ──────────────────────────────────────────────────────

    fn is_supported(&self) -> bool {
        let fy = self.feet_y as i32;
        let cl = self.x as i32 - CAT_HALF_W;
        let cr = self.x as i32 + CAT_HALF_W;

        let row = (fy / CHUNK_H_I).max(0) as u16;
        let col0 = ((cl - BLOCK_W_I + 1).div_euclid(CHUNK_W_I)).max(0) as u16;
        let col1 = ((cr - 1).div_euclid(CHUNK_W_I)).max(0) as u16;

        for col in col0..=col1 {
            for b in self.level.blocks_in_chunk(col, row) {
                let by = b.y as i32;
                let bx = b.x as i32;
                if by == fy && cl < bx + BLOCK_W_I && cr > bx {
                    return true;
                }
            }
        }
        if self.on_platform >= 0 {
            if let Some(p) = self.level.platforms.get(self.on_platform as usize) {
                let py = p.y as i32;
                let px = p.x as i32;
                let pw = p.w as i32;
                if py == fy && cl < px + pw && cr > px {
                    return true;
                }
            }
        }
        false
    }

    fn resolve_x(&mut self) {
        let mut cl = self.x as i32 - CAT_HALF_W;
        let mut cr = self.x as i32 + CAT_HALF_W;
        let ct = self.feet_y as i32 - CAT_H;
        let cb = self.feet_y as i32;

        let col0 = ((cl - BLOCK_W_I + 1).div_euclid(CHUNK_W_I)).max(0) as u16;
        let col1 = ((cr - 1).div_euclid(CHUNK_W_I)).max(0) as u16;
        let row0 = ((ct - BLOCK_H_I + 1).div_euclid(CHUNK_H_I)).max(0) as u16;
        let row1 = ((cb - 1).div_euclid(CHUNK_H_I)).max(0) as u16;

        for col in col0..=col1 {
            for row in row0..=row1 {
                for b in self.level.blocks_in_chunk(col, row) {
                    let bx = b.x as i32;
                    let by = b.y as i32;
                    let br = bx + BLOCK_W_I;
                    let bb = by + BLOCK_H_I;
                    if ct >= bb || cb <= by {
                        continue;
                    }
                    if cl >= br || cr <= bx {
                        continue;
                    }
                    if self.vx > 0.0 {
                        self.x = (bx - CAT_HALF_W) as f32;
                    } else if self.vx < 0.0 {
                        self.x = (br + CAT_HALF_W) as f32;
                    } else if cr - bx < br - cl {
                        self.x = (bx - CAT_HALF_W) as f32;
                    } else {
                        self.x = (br + CAT_HALF_W) as f32;
                    }
                    self.vx = 0.0;
                    cl = self.x as i32 - CAT_HALF_W;
                    cr = self.x as i32 + CAT_HALF_W;
                }
            }
        }
    }

    fn resolve_y(&mut self, prev_feet: f32) {
        let cl = self.x as i32 - CAT_HALF_W;
        let cr = self.x as i32 + CAT_HALF_W;

        let col0 = ((cl - BLOCK_W_I + 1).div_euclid(CHUNK_W_I)).max(0) as u16;
        let col1 = ((cr - 1).div_euclid(CHUNK_W_I)).max(0) as u16;

        if self.vy >= 0.0 {
            // Descending. When the cat overlaps multiple stacked solids in the
            // same chunk (e.g. a 2-tall ledge), snap to the TOPMOST surface
            // (smallest by), since early-returning on the first match could land
            // them inside the ledge since the chunk's block order isn't sorted
            // by y (sort_unstable_by_key isn't stable on ties).
            let row0 = ((prev_feet as i32).max(0) / CHUNK_H_I) as u16;
            let row1 = ((self.feet_y as i32).max(0) / CHUNK_H_I) as u16;
            let mut best: Option<f32> = None;
            for col in col0..=col1 {
                for row in row0..=row1 {
                    for b in self.level.blocks_in_chunk(col, row) {
                        let bx = b.x as i32;
                        let by = b.y as i32;
                        if cl >= bx + BLOCK_W_I || cr <= bx {
                            continue;
                        }
                        let byf = by as f32;
                        if prev_feet <= byf && byf <= self.feet_y {
                            best = Some(match best {
                                Some(cur) if cur < byf => cur,
                                _ => byf,
                            });
                        }
                    }
                }
            }
            if let Some(byf) = best {
                self.feet_y = byf;
                self.vy = 0.0;
                self.on_ground = true;
                self.on_platform = -1;
                self.just_landed = true;
                return;
            }
            // Platforms: same topmost-surface rule.
            let mut best_plat: Option<(f32, i32)> = None;
            for (i, p) in self.level.platforms.iter().enumerate() {
                if i as i32 == self.drop_platform {
                    continue;
                }
                let px = p.x as i32;
                let pw = p.w as i32;
                if cl >= px + pw || cr <= px {
                    continue;
                }
                let pyf = p.y as f32;
                if prev_feet <= pyf && pyf <= self.feet_y {
                    best_plat = Some(match best_plat {
                        Some((cur, _)) if cur < pyf => best_plat.unwrap(),
                        _ => (pyf, i as i32),
                    });
                }
            }
            if let Some((pyf, idx)) = best_plat {
                self.feet_y = pyf;
                self.vy = 0.0;
                self.on_ground = true;
                self.on_platform = idx;
                self.just_landed = true;
            }
        } else {
            // Ascending. Solid block ceilings only. Pick the LOWEST ceiling
            // (largest bb) the cat's head crossed this frame, so stacked
            // blocks above the cat resolve to the nearest one (same chunk
            // ordering caveat as the descending case).
            let prev_head = prev_feet - CAT_H as f32;
            let curr_head = self.feet_y - CAT_H as f32;
            let row0 = (((curr_head as i32) - BLOCK_H_I + 1).max(0) / CHUNK_H_I) as u16;
            let row1 = (((prev_head as i32) - BLOCK_H_I).max(0) / CHUNK_H_I) as u16;
            let mut best: Option<f32> = None;
            for col in col0..=col1 {
                for row in row0..=row1 {
                    for b in self.level.blocks_in_chunk(col, row) {
                        let bx = b.x as i32;
                        let by = b.y as i32;
                        let bb = by + BLOCK_H_I;
                        if cl >= bx + BLOCK_W_I || cr <= bx {
                            continue;
                        }
                        let bbf = bb as f32;
                        if prev_head >= bbf && bbf > curr_head {
                            best = Some(match best {
                                Some(cur) if cur > bbf => cur,
                                _ => bbf,
                            });
                        }
                    }
                }
            }
            if let Some(bbf) = best {
                self.feet_y = bbf + CAT_H as f32;
                self.vy = 0.0;
            }
        }
    }

    // ── Combat ───────────────────────────────────────────────────────────────

    fn apply_cat_attack(&mut self) {
        let reach = if self.swipe_is_run { RUN_ATK_REACH } else { ATK_REACH };
        let (atk_cl, atk_cr) = if self.facing_right {
            let l = self.x as i32 + CAT_H;
            (l, l + reach)
        } else {
            let r = self.x as i32 - CAT_H;
            (r - reach, r)
        };

        let strike_offset = if self.swipe_is_run { RUN_STRIKE_OFFSET } else { 0 };
        self.strike_active = true;
        self.strike_x = if self.facing_right {
            (atk_cl + strike_offset) as f32
        } else {
            (atk_cr - strike_offset) as f32
        };
        self.strike_y = (self.feet_y as i32 - CAT_H / 2) as f32;
        self.strike_vx = if self.facing_right { STRIKE_VX } else { -STRIKE_VX };
        self.strike_right = self.facing_right;
        self.strike_frame = 0;
        self.strike_timer = 0.0;

        let atk_ct = self.feet_y as i32 - CAT_H;
        let atk_cb = self.feet_y as i32;
        let cat_col = self.x as i32 / CHUNK_W_I;

        for slime in self.slimes.iter_mut() {
            if !slime.alive || slime.dying {
                continue;
            }
            let scc = slime.chunk_col as i32;
            if scc < cat_col - 1 || scc > cat_col + 1 {
                continue;
            }
            let scl = slime.x as i32 - SLIME_HALF_W;
            let scr = slime.x as i32 + SLIME_HALF_W;
            let sct = slime.feet_y as i32 - SLIME_H;
            let scb = slime.feet_y as i32;
            if scl >= atk_cr || scr <= atk_cl {
                continue;
            }
            if sct >= atk_cb || scb <= atk_ct {
                continue;
            }
            slime.hp -= if self.swipe_is_run { 2 } else { 1 };
            slime.hit_timer = SLIME_HIT_FLASH;
            if slime.hp <= 0 && !slime.dying {
                slime.dying = true;
                self.slimes_killed += 1;
            }
        }
    }

    fn check_slime_cat_contact(&mut self) {
        if self.cat_blink_timer > 0.0 {
            return;
        }
        let ccl = self.x as i32 - CAT_HALF_W;
        let ccr = self.x as i32 + CAT_HALF_W;
        let cct = self.feet_y as i32 - CAT_H;
        let ccb = self.feet_y as i32;
        let cat_col = self.x as i32 / CHUNK_W_I;
        let mut hit: Option<(f32, bool)> = None; // (slime.x, on_ground)
        for slime in self.slimes.iter() {
            if !slime.alive || slime.dying {
                continue;
            }
            let scc = slime.chunk_col as i32;
            if scc < cat_col - 1 || scc > cat_col + 1 {
                continue;
            }
            let scl = slime.x as i32 - SLIME_HALF_W;
            let scr = slime.x as i32 + SLIME_HALF_W;
            let sct = slime.feet_y as i32 - SLIME_H;
            let scb = slime.feet_y as i32;
            if ccl >= scr || ccr <= scl {
                continue;
            }
            if cct >= scb || ccb <= sct {
                continue;
            }
            hit = Some((slime.x, self.on_ground));
            break;
        }
        if let Some((sx, was_on_ground)) = hit {
            self.cat_hp -= 1;
            self.cat_blink_timer = CAT_BLINK_DUR;
            self.vx = if self.x >= sx { CAT_KNOCKBACK_VX } else { -CAT_KNOCKBACK_VX };
            self.vy = CAT_KNOCKBACK_VY;
            if was_on_ground {
                self.on_ground = false;
                self.on_platform = -1;
            }
            if self.cat_hp <= 0 {
                self.start_poof();
            }
        }
    }

    fn start_poof(&mut self) {
        self.poof_active = true;
        self.poof_x = self.x as i32;
        self.poof_y = self.feet_y as i32;
        self.poof_frame = 0;
        self.poof_timer = 0.0;
        self.vx = 0.0;
        self.vy = 0.0;
    }

    fn respawn_cat(&mut self) {
        self.injuries += 1;
        let (px, py) = self.checkpoint;
        self.x = px as f32;
        self.feet_y = py as f32;
        self.vx = 0.0;
        self.vy = 0.0;
        self.on_ground = true;
        self.on_platform = -1;
        self.drop_platform = -1;
        self.facing_right = true;
        self.target_cam_x = (px as f32 - RIGHT_SCROLL_PX).clamp(CAM_X_MIN, self.cam_x_max);
        self.target_cam_y = (py as f32 - BOT_SCROLL_PX).clamp(self.cam_y_min, self.cam_y_max);
        self.camera_x = self.target_cam_x;
        self.camera_y = self.target_cam_y;
        self.cat_hp = CAT_START_HP;
        self.cat_blink_timer = CAT_BLINK_DUR;
        self.swipe_frame = -1;
        self.anim_frame = 0;
        self.can_double_jump = DOUBLE_JUMP_ENABLED;
    }

    // ── Slime update ─────────────────────────────────────────────────────────

    fn update_slimes(&mut self, dt: f32, rng: &mut u32) {
        let cam_col0 = (self.camera_x as i32 / CHUNK_W_I) as u16;
        let cam_col1 = ((self.camera_x as i32 + 127) / CHUNK_W_I) as u16;
        // Iterate by index; avoid borrowing the whole vec while mutating.
        for i in 0..self.slimes.len() {
            if !self.slimes[i].alive {
                continue;
            }
            if self.slimes[i].chunk_col < cam_col0 || self.slimes[i].chunk_col > cam_col1 {
                continue;
            }
            self.update_slime_at(i, dt, rng);
        }
    }

    fn update_slime_at(&mut self, i: usize, dt: f32, rng: &mut u32) {
        // Burst (death) animation
        if self.slimes[i].dying {
            self.slimes[i].burst_timer += dt;
            if self.slimes[i].burst_timer >= SLIME_BURST_SPF {
                self.slimes[i].burst_timer -= SLIME_BURST_SPF;
                self.slimes[i].burst_frame += 1;
            }
            if (self.slimes[i].burst_frame as usize) >= PLATFORMER_SLIME_BURST.frames.len() {
                self.slimes[i].alive = false;
            }
            return;
        }

        // Random direction change
        self.slimes[i].dir_timer -= dt;
        if self.slimes[i].dir_timer <= 0.0 {
            self.slimes[i].vx = if rand_f32(rng) > 0.5 { SLIME_SPEED } else { -SLIME_SPEED };
            self.slimes[i].dir_timer = 1.0 + rand_f32(rng) * 2.0;
        }

        // Edge detection
        let s = self.slimes[i];
        let look_x = s.x + if s.vx > 0.0 { (SLIME_HALF_W + 2) as f32 } else { -(SLIME_HALF_W + 2) as f32 };
        let ilook = look_x as i32;
        let fy = s.feet_y as i32;
        let mut has_ground = false;
        for b in self.level.blocks_in_chunk(s.chunk_col, s.ground_row) {
            let bx = b.x as i32;
            if (b.y as i32) == fy && ilook - 1 < bx + BLOCK_W_I && ilook + 1 > bx {
                has_ground = true;
                break;
            }
        }
        if !has_ground {
            for p in self.level.platforms.iter() {
                let px = p.x as i32;
                let pw = p.w as i32;
                if (p.y as i32) == fy && ilook - 1 < px + pw && ilook + 1 > px {
                    has_ground = true;
                    break;
                }
            }
        }
        if !has_ground {
            self.slimes[i].vx = -self.slimes[i].vx;
        }

        // Move + wall collision
        let mut next_x = self.slimes[i].x + self.slimes[i].vx * dt;
        let nl = next_x as i32 - SLIME_HALF_W;
        let nr = next_x as i32 + SLIME_HALF_W;
        let st = fy - SLIME_H;
        let sb = fy;
        let mut wall_hit = false;
        for row in s.wall_row0..=s.wall_row1 {
            for b in self.level.blocks_in_chunk(s.chunk_col, row) {
                let bx = b.x as i32;
                let by = b.y as i32;
                if st >= by + BLOCK_H_I || sb <= by {
                    continue;
                }
                if nl >= bx + BLOCK_W_I || nr <= bx {
                    continue;
                }
                next_x = if self.slimes[i].vx > 0.0 {
                    (bx - SLIME_HALF_W) as f32
                } else {
                    (bx + BLOCK_W_I + SLIME_HALF_W) as f32
                };
                self.slimes[i].vx = -self.slimes[i].vx;
                wall_hit = true;
                break;
            }
            if wall_hit {
                break;
            }
        }

        // Clamp to patrol bounds
        if next_x <= self.slimes[i].patrol_min {
            next_x = self.slimes[i].patrol_min;
            self.slimes[i].vx = SLIME_SPEED;
        } else if next_x >= self.slimes[i].patrol_max {
            next_x = self.slimes[i].patrol_max;
            self.slimes[i].vx = -SLIME_SPEED;
        }
        self.slimes[i].x = next_x;

        // 2 fps idle animation
        self.slimes[i].anim_timer += dt;
        if self.slimes[i].anim_timer >= SLIME_ANIM_SPF {
            self.slimes[i].anim_timer -= SLIME_ANIM_SPF;
            self.slimes[i].anim_frame ^= 1;
        }

        if self.slimes[i].hit_timer > 0.0 {
            self.slimes[i].hit_timer -= dt;
        }
    }

    // ── Triggers ─────────────────────────────────────────────────────────────

    fn check_checkpoints(&mut self) {
        let ccl = self.x as i32 - CAT_HALF_W;
        let ccr = self.x as i32 + CAT_HALF_W;
        let cct = self.feet_y as i32 - CAT_H;
        let ccb = self.feet_y as i32;
        for (i, cp) in self.level.checkpoints.iter().enumerate() {
            if self.checkpoint_activated.get(i).copied().unwrap_or(true) {
                continue;
            }
            let cx = cp.x as i32;
            let cy = cp.y as i32;
            if ccl >= cx + CHECKPOINT_W || ccr <= cx {
                continue;
            }
            if cct >= cy || ccb <= cy - CHECKPOINT_H {
                continue;
            }
            self.checkpoint = (cp.x + BLOCK_W / 2, cp.y);
            if let Some(slot) = self.checkpoint_activated.get_mut(i) {
                *slot = true;
            }
        }
    }

    fn check_item_pickups(&mut self) {
        let ccl = self.x as i32 - CAT_HALF_W;
        let ccr = self.x as i32 + CAT_HALF_W;
        let cct = self.feet_y as i32 - CAT_H;
        let ccb = self.feet_y as i32;

        if self.keys_remaining > 0 {
            let kw = KEY.width as i32;
            let kh = KEY.height as i32;
            for (i, k) in self.level.keys.iter().enumerate() {
                if !self.key_active.get(i).copied().unwrap_or(false) {
                    continue;
                }
                let kx = k.x as i32;
                let ky = k.y as i32;
                if ccl >= kx + kw / 2 || ccr <= kx - kw / 2 {
                    continue;
                }
                if cct >= ky || ccb <= ky - kh {
                    continue;
                }
                if let Some(slot) = self.key_active.get_mut(i) {
                    *slot = false;
                }
                self.has_key = true;
                self.keys_remaining = self.keys_remaining.saturating_sub(1);
                break;
            }
        }

        if self.coins_remaining > 0 {
            let cw = SPIN_COIN.width as i32;
            let ch = SPIN_COIN.height as i32;
            for (i, c) in self.level.coins.iter().enumerate() {
                if !self.coin_active.get(i).copied().unwrap_or(false) {
                    continue;
                }
                let cx = c.x as i32;
                let cy = c.y as i32;
                if ccl >= cx + cw / 2 || ccr <= cx - cw / 2 {
                    continue;
                }
                if cct >= cy || ccb <= cy - ch {
                    continue;
                }
                if let Some(slot) = self.coin_active.get_mut(i) {
                    *slot = false;
                }
                self.coins_collected += 1;
                self.level_coins_collected += 1;
                self.coins_remaining = self.coins_remaining.saturating_sub(1);
            }
        }
    }

    fn check_doors(&mut self) {
        if self.door_dest.is_some() {
            return;
        }
        let ccl = self.x as i32 - CAT_HALF_W;
        let ccr = self.x as i32 + CAT_HALF_W;
        let cct = self.feet_y as i32 - CAT_H;
        let ccb = self.feet_y as i32;

        let mut found: Option<&str> = None;
        for d in self.level.doors.iter() {
            let cx = d.x as i32;
            let cy = d.y as i32;
            if ccl >= cx + DOOR_W || ccr <= cx {
                continue;
            }
            if cct >= cy || ccb <= cy - DOOR_H {
                continue;
            }
            found = Some(d.dest.as_str());
            break;
        }
        if found.is_none() && self.has_key {
            for d in self.level.locked_doors.iter() {
                let cx = d.x as i32;
                let cy = d.y as i32;
                if ccl >= cx + DOOR_W || ccr <= cx {
                    continue;
                }
                if cct >= cy || ccb <= cy - DOOR_H {
                    continue;
                }
                found = Some(d.dest.as_str());
                break;
            }
        }
        if let Some(dest) = found {
            let mut s: String<16> = String::new();
            let _ = s.push_str(dest);
            self.door_dest = Some(s);
            self.door_walk_timer = DOOR_WALK_DELAY;
        }
    }

    fn update_camera(&mut self, dt: f32) {
        let cat_sx = self.x - self.camera_x;
        let cat_sy = self.feet_y - self.camera_y;

        if self.facing_right && cat_sx > RIGHT_SCROLL_PX {
            self.target_cam_x = self.x - RIGHT_SCROLL_PX;
        } else if !self.facing_right && cat_sx < LEFT_SCROLL_PX {
            self.target_cam_x = self.x - LEFT_SCROLL_PX;
        }
        if cat_sy < TOP_SCROLL_PX {
            self.target_cam_y = self.feet_y - TOP_SCROLL_PX;
        } else if cat_sy > BOT_SCROLL_PX {
            self.target_cam_y = self.feet_y - BOT_SCROLL_PX;
        }

        self.target_cam_x = self.target_cam_x.clamp(CAM_X_MIN, self.cam_x_max);
        self.target_cam_y = self.target_cam_y.clamp(self.cam_y_min, self.cam_y_max);

        self.camera_x += (self.target_cam_x - self.camera_x) * CAM_LERP * dt;
        self.camera_y += (self.target_cam_y - self.camera_y) * CAM_LERP * dt;
    }

    // ── Animation helpers ────────────────────────────────────────────────────

    fn jump_frame(&self) -> usize {
        if self.vy < -JUMP_PEAK_RANGE {
            0 // rising
        } else if self.vy <= JUMP_PEAK_RANGE {
            1 // near peak
        } else {
            2 // falling
        }
    }

    fn handle_input(&mut self, buttons: &mut Buttons, rng: &mut u32) {
        if self.poof_active {
            return;
        }
        if self.door_fade_phase == DoorPhase::Summary {
            // Any directional/action press advances to the next level. The
            // actual load happens here so the new level renders behind the
            // scanline fade-in below.
            for b in [Button::A, Button::B, Button::Up, Button::Down, Button::Left, Button::Right] {
                if !buttons.was_just_pressed(b) {
                    continue;
                }
                let dest = self.door_dest.clone();
                if let Some(dest) = dest {
                    if self.transition_to_level(dest.as_str(), rng) {
                        // load_level cleared door_dest as part of state reset;
                        // restore it so the fade-in branch can tell it's a
                        // post-transition reveal vs. a fresh fade.
                        let mut s: String<16> = String::new();
                        let _ = s.push_str(dest.as_str());
                        self.door_dest = Some(s);
                        self.door_fade_phase = DoorPhase::In;
                        self.door_fade_prog = 0.0;
                    }
                }
                break;
            }
            return;
        }
        if self.door_fade_phase != DoorPhase::None {
            return;
        }

        // Mid-swipe: chain a new swipe once the hit frame has fired.
        if self.swipe_frame >= 0 {
            if !self.swipe_is_run {
                self.vx = 0.0;
            }
            if self.swipe_frame >= ATTACK_FRAME && buttons.was_just_pressed(Button::B) {
                let moving = self.vx.abs() > 1.0 || !self.on_ground;
                self.swipe_is_run = moving;
                self.swipe_frame = 0;
                self.swipe_timer = 0.0;
                self.anim_frame = 0;
            }
            return;
        }

        let mut moving = false;
        if buttons.is_pressed(Button::Left) {
            self.vx = -RUN_SPEED;
            self.facing_right = false;
            moving = true;
        } else if buttons.is_pressed(Button::Right) {
            self.vx = RUN_SPEED;
            self.facing_right = true;
            moving = true;
        } else {
            self.vx = 0.0;
        }
        if !moving && self.on_ground {
            let sit_n = PLATFORMER_CAT_SIT.frames.len() as u8;
            if self.anim_frame >= sit_n {
                self.anim_frame = 0;
            }
        }

        if buttons.was_just_pressed(Button::A) {
            if self.on_ground && !self.just_landed {
                self.vy = JUMP_VEL;
                self.on_ground = false;
                self.on_platform = -1;
                self.anim_frame = 0;
                self.anim_timer = 0.0;
            } else if self.can_double_jump {
                self.vy = JUMP_VEL;
                self.can_double_jump = false;
                self.anim_frame = 0;
                self.anim_timer = 0.0;
            }
        }

        if buttons.was_just_pressed(Button::Down)
            && self.on_ground
            && self.on_platform >= 0
        {
            self.drop_platform = self.on_platform;
            self.on_platform = -1;
            self.on_ground = false;
            self.vy = 20.0;
            self.anim_frame = 0;
            self.anim_timer = 0.0;
        }

        if buttons.was_just_pressed(Button::B) {
            self.swipe_is_run = moving || !self.on_ground;
            self.swipe_frame = 0;
            self.swipe_timer = 0.0;
            self.anim_frame = 0;
        }
    }

    // ── Draw helpers ─────────────────────────────────────────────────────────

    fn tick_fireworks(&mut self, dt: f32, rng: &mut u32) {
        // Spawn new bursts on a 0.5s cadence until FIREWORK_MAX_CYCLES; only
        // on flawless clears. The first cycle fires immediately on entry.
        if self.level_flawless && self.fireworks_count < FIREWORK_MAX_CYCLES {
            self.fireworks_timer += dt;
            if self.fireworks_count == 0 || self.fireworks_timer >= FIREWORK_CYCLE_INTERVAL {
                self.fireworks_timer = 0.0;
                self.fireworks_count += 1;
                let n = 2 + (xorshift32(rng) % 2) as u8; // 2 or 3
                for _ in 0..n {
                    let ax = rand_range_i32(rng, 15, 113) as i16;
                    let ay = rand_range_i32(rng, 8, 50) as i16;
                    let mut particles = [FireworkParticle { dx: 0, dy: 0, delay: 0.0 };
                        FIREWORK_PARTICLES];
                    for (i, p) in particles.iter_mut().enumerate() {
                        p.dx = rand_range_i32(rng, -FIREWORK_SPREAD_X, FIREWORK_SPREAD_X) as i8;
                        p.dy = rand_range_i32(rng, FIREWORK_SPREAD_Y_MIN, FIREWORK_SPREAD_Y_MAX) as i8;
                        let jitter = rand_f32(rng) * 0.25;
                        p.delay = i as f32 * FIREWORK_PARTICLE_STAGGER + jitter;
                    }
                    let _ = self.firework_groups.push(FireworkGroup {
                        ax,
                        ay,
                        timer: 0.0,
                        particles,
                    });
                }
            }
        }
        for g in self.firework_groups.iter_mut() {
            g.timer += dt;
        }
        // Evict finished groups (in-place; preserves order).
        let mut i = 0;
        while i < self.firework_groups.len() {
            if self.firework_groups[i].finished() {
                self.firework_groups.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn draw_fireworks(&self, renderer: &mut Renderer) {
        let hw = (BURST1.width / 2) as i32;
        let hh = (BURST1.height / 2) as i32;
        let n_frames = BURST1.frames.len();
        for g in self.firework_groups.iter() {
            for p in g.particles.iter() {
                let elapsed = g.timer - p.delay;
                if elapsed < 0.0 || elapsed >= FIREWORK_PARTICLE_TOTAL {
                    continue;
                }
                let fi = ((elapsed / BURST1_FRAME_DUR) as usize).min(n_frames - 1);
                let px = g.ax as i32 + p.dx as i32 - hw;
                let py = g.ay as i32 + p.dy as i32 - hh;
                renderer.draw_sprite(
                    &BURST1,
                    Point::new(px, py),
                    SpriteOpts {
                        frame: fi,
                        transparent: true,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn draw_level_summary(&self, renderer: &mut Renderer) {
        let mins = (self.level_time as u32) / 60;
        let secs = (self.level_time as u32) % 60;
        let mut n_buf: String<8> = String::new();
        let _ = write!(n_buf, "{}", self.level_num);
        let mut time_buf: String<8> = String::new();
        let _ = write!(time_buf, "{}:{:02}", mins, secs);
        let mut header: String<32> = String::new();
        crate::i18n::substitute(
            &mut header,
            t!("Level {n} - {time}"),
            &[("n", n_buf.as_str()), ("time", time_buf.as_str())],
        );
        renderer.draw_text(&header, Point::new(0, 1));

        const ICON_W: i32 = 18;
        const TEXT_X: i32 = 1 + ICON_W + 4;

        let bh = BANDAGE.height as i32;
        renderer.draw_sprite(
            &BANDAGE,
            Point::new(1, 12),
            SpriteOpts { transparent: true, ..Default::default() },
        );
        let text_y = 12 + (bh - 8) / 2;
        let mut s: String<8> = String::new();
        let _ = write!(s, "{}", self.injuries);
        renderer.draw_text(&s, Point::new(TEXT_X, text_y));

        let flawless = self.injuries == 0
            && self.slimes_killed == self.total_slimes
            && self.level_coins_collected == self.total_coins;
        if flawless {
            renderer.draw_text(t!("Flawless!"), Point::new(128 - 9 * 8, text_y));
        }

        let sh = PLATFORMER_SLIME_IDLE.height as i32;
        renderer.draw_sprite(
            &PLATFORMER_SLIME_IDLE,
            Point::new(1, 32),
            SpriteOpts {
                frame: self.summary_slime_frame as usize,
                transparent: true,
                ..Default::default()
            },
        );
        let mut s2: String<16> = String::new();
        let _ = write!(s2, "{}/{}", self.slimes_killed, self.total_slimes);
        renderer.draw_text(&s2, Point::new(TEXT_X, 32 + (sh - 8) / 2));

        let cw = SPIN_COIN.width as i32;
        let coin_x = 1 + (ICON_W - cw) / 2;
        renderer.draw_sprite(
            &SPIN_COIN,
            Point::new(coin_x, 44),
            SpriteOpts {
                frame: self.coin_anim_frame as usize,
                transparent: true,
                ..Default::default()
            },
        );
        let mut s3: String<16> = String::new();
        let _ = write!(s3, "{}/{}", self.level_coins_collected, self.total_coins);
        renderer.draw_text(&s3, Point::new(TEXT_X, 44));
    }

    fn draw_level_banner(&self, renderer: &mut Renderer) {
        let prog = (self.banner_timer / LEVEL_BANNER_DUR).min(1.0);
        let mut n_buf: String<8> = String::new();
        let _ = write!(n_buf, "{}", self.level_num);
        let mut text: String<16> = String::new();
        crate::i18n::substitute(&mut text, t!("Level {n}"), &[("n", n_buf.as_str())]);
        let tw = text.len() as i32 * 8;
        let bx = (128 - tw) / 2;
        let by = 20 - (prog * LEVEL_BANNER_RISE) as i32;
        renderer.draw_text(&text, Point::new(bx, by));
    }
}

fn parse_level_num(name: &str) -> u8 {
    // Names look like "level_01". Return the trailing integer (or 1 if none).
    let bytes = name.as_bytes();
    let mut i = bytes.len();
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    let mut value: u32 = 0;
    for &b in &bytes[i..] {
        value = value * 10 + (b - b'0') as u32;
    }
    if value == 0 { 1 } else { value as u8 }
}

impl Scene for PlatformerScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        // Reset session counters on each fresh entry.
        self.coins_collected = 0;
        self.session_slimes_killed = 0;
        self.session_levels_completed = 0;

        let level_name = if self.current_level.is_empty() {
            "level_01"
        } else {
            // Reuse last level on re-entry.
            self.current_level.as_str()
        };
        let mut name_buf: String<16> = String::new();
        let _ = name_buf.push_str(level_name);
        let mut rng = ctx.rng;
        if !self.load_level(name_buf.as_str(), &mut rng) {
            // Unknown level, fall back to level_01.
            let _ = self.load_level("level_01", &mut rng);
        }
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let coins = self.coins_collected as i32;
        let levels = self.session_levels_completed as f32;
        let slimes = self.session_slimes_killed as f32;
        if coins > 0 {
            ctx.coins += coins;
        }
        if levels > 0.0 || slimes > 0.0 {
            let lp = (levels / 5.0).sqrt();
            let sp = (slimes / 62.0).sqrt();
            ctx.apply_stat_changes(&[
                (StatId::Fitness, 3.0 * lp),
                (StatId::Fulfillment, 2.0 * lp),
                (StatId::Playfulness, 3.0 * lp),
                (StatId::Courage, 2.0 * sp),
            ]);
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        // Menu1 always bails out (matches breakout/snake convention).
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }

        let mut rng = ctx.rng;
        self.handle_input(buttons, &mut rng);
        ctx.rng = rng;

        // Door fade-in: the new level is already loaded; reveal it.
        if self.door_fade_phase == DoorPhase::In {
            self.door_fade_prog += dt / DOOR_FADE_DURATION;
            if self.door_fade_prog >= 1.0 {
                self.door_fade_phase = DoorPhase::None;
                self.door_dest = None;
                self.door_fade_prog = 0.0;
            }
            return None;
        }

        // Door fade-out: freeze game, advance darken progress, then show summary.
        if self.door_fade_phase == DoorPhase::Out {
            self.door_fade_prog += dt / DOOR_FADE_DURATION;
            if self.door_fade_prog >= 1.0 {
                let flawless = self.injuries == 0
                    && self.slimes_killed == self.total_slimes
                    && self.level_coins_collected == self.total_coins;
                if flawless {
                    // Flawless bonus: extra half of the level's coins added
                    // to the session running total.
                    self.coins_collected += self.level_coins_collected / 2;
                }
                self.level_flawless = flawless;
                self.door_fade_phase = DoorPhase::Summary;
                self.door_fade_prog = 0.0;
            }
            return None;
        }

        // Summary frozen state: keep icons animating and tick fireworks.
        if self.door_fade_phase == DoorPhase::Summary {
            self.coin_anim_timer += dt;
            if self.coin_anim_timer >= COIN_ANIM_SPF {
                self.coin_anim_timer -= COIN_ANIM_SPF;
                self.coin_anim_frame =
                    (self.coin_anim_frame + 1) % SPIN_COIN.frames.len() as u8;
            }
            self.summary_slime_timer += dt;
            if self.summary_slime_timer >= SLIME_ANIM_SPF {
                self.summary_slime_timer -= SLIME_ANIM_SPF;
                self.summary_slime_frame ^= 1;
            }
            let mut rng = ctx.rng;
            self.tick_fireworks(dt, &mut rng);
            ctx.rng = rng;
            return None;
        }

        // Walk-into-door delay: physics keeps running, just count down.
        if self.door_dest.is_some() {
            self.door_walk_timer -= dt;
            if self.door_walk_timer <= 0.0 {
                self.door_fade_phase = DoorPhase::Out;
                self.door_fade_prog = 0.0;
            }
        }

        // Poof death freezes everything else and ticks the disintegration.
        if self.poof_active {
            self.poof_timer += dt;
            if self.poof_timer >= POOF_SPF {
                self.poof_timer -= POOF_SPF;
                self.poof_frame += 1;
            }
            if self.poof_frame as usize >= POOF.frames.len() {
                self.poof_active = false;
                self.respawn_cat();
            }
            return None;
        }

        self.level_time += dt;
        self.banner_timer += dt;
        self.just_landed = false;

        // Walk-off-edge detection
        if self.on_ground && !self.is_supported() {
            self.on_ground = false;
            self.on_platform = -1;
        }

        // Vertical physics + collision (do first so feet_y is final before
        // _resolve_x evaluates the cat's vertical bounds).
        if !self.on_ground {
            self.vy += GRAVITY * dt;
            let prev_feet = self.feet_y;
            self.feet_y += self.vy * dt;
            self.resolve_y(prev_feet);
        }

        // Horizontal movement + collision
        self.x += self.vx * dt;
        let half_w = CAT_HALF_W as f32;
        if self.x < half_w {
            self.x = half_w;
        } else if self.x > self.level.world_w as f32 - half_w {
            self.x = self.level.world_w as f32 - half_w;
        }
        self.resolve_x();

        if self.just_landed {
            self.can_double_jump = DOUBLE_JUMP_ENABLED;
        }

        if self.feet_y > self.kill_y {
            self.respawn_cat();
            return None;
        }

        self.key_timer += dt;
        self.coin_anim_timer += dt;
        if self.coin_anim_timer >= COIN_ANIM_SPF {
            self.coin_anim_timer -= COIN_ANIM_SPF;
            self.coin_anim_frame = (self.coin_anim_frame + 1) % SPIN_COIN.frames.len() as u8;
        }
        self.check_checkpoints();
        self.check_item_pickups();
        self.check_doors();

        // Clear drop-through once fully below the platform
        if self.drop_platform >= 0 {
            if let Some(p) = self.level.platforms.get(self.drop_platform as usize) {
                if self.feet_y > (p.y as f32) + PLAT_H as f32 {
                    self.drop_platform = -1;
                }
            }
        }

        // Ground animation (suppressed while swiping)
        if self.on_ground && self.swipe_frame < 0 {
            let fps = if self.vx.abs() > 1.0 { RUN_FPS } else { IDLE_FPS };
            self.anim_timer += dt;
            if self.anim_timer >= 1.0 / fps {
                self.anim_timer -= 1.0 / fps;
                let n = if self.vx.abs() > 1.0 {
                    PLATFORMER_CAT_RUN.frames.len()
                } else {
                    PLATFORMER_CAT_SIT.frames.len()
                };
                self.anim_frame = (self.anim_frame + 1) % n as u8;
            }
        }

        // Swipe animation
        if self.swipe_frame >= 0 {
            let total_frames = if self.swipe_is_run { RUN_SWIPE_FRAMES } else { SIT_SWIPE_FRAMES };
            let old_frame = self.swipe_frame;
            self.swipe_timer += dt;
            if self.swipe_timer >= 1.0 / SWIPE_FPS {
                self.swipe_timer -= 1.0 / SWIPE_FPS;
                self.swipe_frame += 1;
            }
            if old_frame < ATTACK_FRAME && self.swipe_frame >= ATTACK_FRAME {
                self.apply_cat_attack();
            }
            if self.swipe_frame >= total_frames {
                self.swipe_frame = -1;
                self.anim_frame = 0;
            }
        }

        // Strike effect drift
        if self.strike_active {
            self.strike_x += self.strike_vx * dt;
            self.strike_timer += dt;
            if self.strike_timer >= 1.0 / SWIPE_FPS {
                self.strike_timer -= 1.0 / SWIPE_FPS;
                self.strike_frame += 1;
            }
            if self.strike_frame as usize >= PLATFORMER_STRIKE.frames.len() {
                self.strike_active = false;
            }
        }

        if self.cat_blink_timer > 0.0 {
            self.cat_blink_timer -= dt;
        }

        let mut rng = ctx.rng;
        self.update_slimes(dt, &mut rng);
        ctx.rng = rng;

        self.check_slime_cat_contact();
        self.update_camera(dt);
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.door_fade_phase == DoorPhase::Summary {
            self.draw_level_summary(renderer);
            self.draw_fireworks(renderer);
            return;
        }

        let cam_x = self.camera_x as i32;
        let cam_y = self.camera_y as i32;
        let col0 = (cam_x / CHUNK_W_I).max(0) as u16;
        let col1 = ((cam_x + 127) / CHUNK_W_I).max(0) as u16;
        let row0 = (cam_y / CHUNK_H_I).max(0) as u16;
        let row1 = ((cam_y + 63) / CHUNK_H_I).max(0) as u16;

        // Background tiles
        for col in col0..=col1 {
            for row in row0..=row1 {
                for t in self.level.bg_in_chunk(col, row) {
                    let sx = t.x as i32 - cam_x;
                    let sy = t.y as i32 - cam_y;
                    if !(-BLOCK_W_I < sx && sx < 128 && -BLOCK_H_I < sy && sy < 64) {
                        continue;
                    }
                    let group = (t.group as usize).min(PLATFORMER_BG_TILES.len() - 1);
                    let variants = PLATFORMER_BG_TILES[group];
                    let v = (t.variant as usize).min(variants.len() - 1);
                    let sprite = variants[v];
                    renderer.draw_sprite(
                        sprite,
                        Point::new(sx, sy),
                        SpriteOpts { transparent: true, ..Default::default() },
                    );
                }
            }
        }

        // Solid terrain
        for col in col0..=col1 {
            for row in row0..=row1 {
                for b in self.level.blocks_in_chunk(col, row) {
                    let sx = b.x as i32 - cam_x;
                    let sy = b.y as i32 - cam_y;
                    if !(-BLOCK_W_I < sx && sx < 128 && -BLOCK_H_I < sy && sy < 64) {
                        continue;
                    }
                    let tt = (b.tile_type as usize).min(TERRAIN_TILES.len() - 1);
                    let variants = TERRAIN_TILES[tt];
                    let v = (b.variant as usize).min(variants.len() - 1);
                    renderer.draw_sprite(
                        variants[v],
                        Point::new(sx, sy),
                        SpriteOpts { transparent: true, ..Default::default() },
                    );
                }
            }
        }

        // One-way platforms
        for p in self.level.platforms.iter() {
            let sx = p.x as i32 - cam_x;
            let sy = p.y as i32 - cam_y;
            let pw = p.w as i32;
            if -pw < sx && sx < 128 && -PLAT_H < sy && sy < 64 {
                renderer.draw_rect(
                    Point::new(sx, sy),
                    Size::new(pw as u32, PLAT_H as u32),
                    true,
                );
            }
        }

        // Grass decorations
        for col in col0..=col1 {
            for row in row0..=row1 {
                for g in self.level.grass_in_chunk(col, row) {
                    let v = (g.variant as usize).min(GRASS_SPRITES.len() - 1);
                    let sprite = GRASS_SPRITES[v];
                    let sw = sprite.width as i32;
                    let sh = sprite.height as i32;
                    let sx = g.cx as i32 - sw / 2 - cam_x;
                    let sy = g.surface_y as i32 - sh - cam_y;
                    renderer.draw_sprite(
                        sprite,
                        Point::new(sx, sy),
                        SpriteOpts { transparent: true, ..Default::default() },
                    );
                }
            }
        }

        // Checkpoints
        for (i, cp) in self.level.checkpoints.iter().enumerate() {
            let activated = self.checkpoint_activated.get(i).copied().unwrap_or(false);
            let (sprite, sh) = if activated {
                (&PLATFORMER_CHECKPOINT_UP, PLATFORMER_CHECKPOINT_UP.height as i32)
            } else {
                (&PLATFORMER_CHECKPOINT_DOWN, PLATFORMER_CHECKPOINT_DOWN.height as i32)
            };
            let dx = cp.x as i32 - cam_x;
            let dy = cp.y as i32 - sh - cam_y;
            let sw = sprite.width as i32;
            if -sw < dx && dx < 128 && -sh < dy && dy < 64 {
                renderer.draw_sprite(
                    sprite,
                    Point::new(dx, dy),
                    SpriteOpts { transparent: true, ..Default::default() },
                );
            }
        }

        // Doors
        for d in self.level.doors.iter() {
            let dx = d.x as i32 - cam_x;
            let dy = d.y as i32 - DOOR_H - cam_y;
            if -DOOR_W < dx && dx < 128 && -DOOR_H < dy && dy < 64 {
                renderer.draw_sprite(
                    &PLATFORMER_DOOR,
                    Point::new(dx, dy),
                    SpriteOpts { transparent: true, ..Default::default() },
                );
            }
        }
        let locked_sprite: &Sprite = if self.has_key {
            &PLATFORMER_DOOR
        } else {
            &PLATFORMER_DOOR_LOCKED
        };
        for d in self.level.locked_doors.iter() {
            let dx = d.x as i32 - cam_x;
            let dy = d.y as i32 - DOOR_H - cam_y;
            if -DOOR_W < dx && dx < 128 && -DOOR_H < dy && dy < 64 {
                renderer.draw_sprite(
                    locked_sprite,
                    Point::new(dx, dy),
                    SpriteOpts { transparent: true, ..Default::default() },
                );
            }
        }

        // Collectibles: triangle-wave bob
        let half = KEY_BOB_PERIOD * 0.5;
        let t_key = self.key_timer % KEY_BOB_PERIOD;
        let key_bob = if t_key < half {
            (t_key / half * KEY_BOB_AMP) as i32
        } else {
            ((KEY_BOB_PERIOD - t_key) / half * KEY_BOB_AMP) as i32
        };
        let kw = KEY.width as i32;
        let kh = KEY.height as i32;
        for (i, k) in self.level.keys.iter().enumerate() {
            if !self.key_active.get(i).copied().unwrap_or(false) {
                continue;
            }
            let dx = k.x as i32 - kw / 2 - cam_x;
            let dy = k.y as i32 - kh - key_bob - cam_y;
            if -kw < dx && dx < 128 && -kh < dy && dy < 64 {
                renderer.draw_sprite(
                    &KEY,
                    Point::new(dx, dy),
                    SpriteOpts { transparent: true, ..Default::default() },
                );
            }
        }
        let half_c = COIN_BOB_PERIOD * 0.5;
        let t_coin = self.key_timer % COIN_BOB_PERIOD;
        let coin_bob = if t_coin < half_c {
            (t_coin / half_c * COIN_BOB_AMP) as i32
        } else {
            ((COIN_BOB_PERIOD - t_coin) / half_c * COIN_BOB_AMP) as i32
        };
        let cw = SPIN_COIN.width as i32;
        let ch = SPIN_COIN.height as i32;
        for (i, c) in self.level.coins.iter().enumerate() {
            if !self.coin_active.get(i).copied().unwrap_or(false) {
                continue;
            }
            let dx = c.x as i32 - cw / 2 - cam_x;
            let dy = c.y as i32 - ch - coin_bob - cam_y;
            if -cw < dx && dx < 128 && -ch < dy && dy < 64 {
                renderer.draw_sprite(
                    &SPIN_COIN,
                    Point::new(dx, dy),
                    SpriteOpts {
                        frame: self.coin_anim_frame as usize,
                        transparent: true,
                        ..Default::default()
                    },
                );
            }
        }

        // Slimes
        let sw = PLATFORMER_SLIME_IDLE.width as i32;
        let sh = PLATFORMER_SLIME_IDLE.height as i32;
        let bw = PLATFORMER_SLIME_BURST.width as i32;
        let bh = PLATFORMER_SLIME_BURST.height as i32;
        for slime in self.slimes.iter() {
            if !slime.alive {
                continue;
            }
            if (slime.chunk_col as i32) < col0 as i32 - 1
                || (slime.chunk_col as i32) > col1 as i32 + 1
            {
                continue;
            }
            if slime.dying {
                let fi = (slime.burst_frame as usize).min(PLATFORMER_SLIME_BURST.frames.len() - 1);
                let dx = slime.x as i32 - bw / 2 - cam_x;
                let dy = slime.feet_y as i32 - bh - cam_y;
                renderer.draw_sprite(
                    &PLATFORMER_SLIME_BURST,
                    Point::new(dx, dy),
                    SpriteOpts { frame: fi, transparent: true, ..Default::default() },
                );
                continue;
            }
            let facing_right = slime.vx >= 0.0;
            let dx = slime.x as i32 - sw / 2 - cam_x;
            let dy = slime.feet_y as i32 - sh - cam_y;
            let fi = slime.anim_frame as usize;
            if slime.hit_timer > 0.0 {
                if let Some(fill_frames) = PLATFORMER_SLIME_IDLE.fill_frames {
                    let frame = fill_frames[fi.min(fill_frames.len() - 1)];
                    renderer.draw_sprite_raw(
                        frame,
                        PLATFORMER_SLIME_IDLE.width,
                        PLATFORMER_SLIME_IDLE.height,
                        Point::new(dx, dy),
                        SpriteOpts {
                            transparent: true,
                            mirror_h: !facing_right,
                            ..Default::default()
                        },
                    );
                }
            } else {
                renderer.draw_sprite(
                    &PLATFORMER_SLIME_IDLE,
                    Point::new(dx, dy),
                    SpriteOpts {
                        frame: fi,
                        transparent: true,
                        mirror_h: !facing_right,
                        ..Default::default()
                    },
                );
            }
        }

        // Strike effect
        if self.strike_active {
            let stw = PLATFORMER_STRIKE.width as i32;
            let sth = PLATFORMER_STRIKE.height as i32;
            let dx = self.strike_x as i32 - stw / 2 - cam_x;
            let dy = self.strike_y as i32 - sth / 2 - cam_y;
            renderer.draw_sprite(
                &PLATFORMER_STRIKE,
                Point::new(dx, dy),
                SpriteOpts {
                    frame: self.strike_frame as usize,
                    transparent: true,
                    mirror_h: !self.strike_right,
                    ..Default::default()
                },
            );
        }

        // Poof
        if self.poof_active {
            let pw = POOF.width as i32;
            let ph = POOF.height as i32;
            let fi = (self.poof_frame as usize).min(POOF.frames.len() - 1);
            renderer.draw_sprite(
                &POOF,
                Point::new(
                    self.poof_x - pw / 2 - cam_x,
                    self.poof_y - ph - cam_y,
                ),
                SpriteOpts { frame: fi, transparent: true, ..Default::default() },
            );
        }

        // Cat sprite: invisible on even blink intervals while damaged
        let cat_visible = if self.poof_active {
            false
        } else if self.cat_blink_timer > 0.0 {
            ((self.cat_blink_timer / CAT_BLINK_INT) as i32) % 2 == 1
        } else {
            true
        };
        if cat_visible {
            let (sprite, frame): (&Sprite, usize) = if self.swipe_frame >= 0 {
                if self.swipe_is_run {
                    let f = (self.swipe_frame as usize).min(RUN_SWIPE_FRAMES as usize - 1);
                    (&PLATFORMER_CAT_RUN_SWIPE, f)
                } else {
                    let f = (self.swipe_frame as usize).min(SIT_SWIPE_FRAMES as usize - 1);
                    (&PLATFORMER_CAT_SIT_SWIPE, f)
                }
            } else if !self.on_ground || self.just_landed {
                let f = if !self.on_ground { self.jump_frame() } else { 2 };
                (&PLATFORMER_CAT_JUMP, f)
            } else if self.vx.abs() > 1.0 {
                let f = (self.anim_frame as usize) % PLATFORMER_CAT_RUN.frames.len();
                (&PLATFORMER_CAT_RUN, f)
            } else {
                let f = (self.anim_frame as usize) % PLATFORMER_CAT_SIT.frames.len();
                (&PLATFORMER_CAT_SIT, f)
            };
            let dx = self.x as i32 - sprite.width as i32 / 2 - cam_x;
            let dy = self.feet_y as i32 - sprite.height as i32 - cam_y;
            renderer.draw_sprite(
                sprite,
                Point::new(dx, dy),
                SpriteOpts {
                    frame,
                    transparent: true,
                    mirror_h: !self.facing_right,
                    ..Default::default()
                },
            );
        }

        // Level banner
        if self.banner_timer < LEVEL_BANNER_DUR {
            self.draw_level_banner(renderer);
        }

        // Door scanline fade
        if self.door_fade_phase == DoorPhase::Out || self.door_fade_phase == DoorPhase::In {
            let progress = if self.door_fade_phase == DoorPhase::Out {
                self.door_fade_prog
            } else {
                1.0 - self.door_fade_prog
            };
            let passes = ((progress * 8.0) as i32 + 1).min(8);
            if passes >= 8 {
                renderer.fill_rect_off(Point::new(0, 0), Size::new(128, 64));
            } else {
                for offset in 0..passes {
                    let mut y = offset;
                    while y < 64 {
                        renderer.draw_line_color(
                            Point::new(0, y),
                            Point::new(127, y),
                            false,
                        );
                        y += 8;
                    }
                }
            }
        }
    }
}
