//! Zoomies minigame: endless runner inspired by Chrome dino.

use core::fmt::Write as _;

use embedded_graphics::prelude::Point;
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{
        minigame_character::{RUNCAT1, SITCAT1, SMALL_BIRD1},
        nature::{MOON, SMALLTREE1, SUN},
        plants::{
            FREESIA_GROWING, FREESIA_MATURE, FREESIA_THRIVING, PLANT1, PLANT2, PLANT6,
            ROSE_GROWING, ROSE_MATURE, SUNFLOWER_YOUNG,
        },
    },
    board::DISPLAY_WIDTH,
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_f32, rand_range_f32, rand_range_u32, xorshift32},
    render::{Renderer, Sprite, SpriteOpts},
    scene::{Scene, SceneId},
};

const DISPLAY_W: i32 = DISPLAY_WIDTH as i32;
const CHAR_W: i32 = 6; // FONT_6X10

const GROUND_Y: i32 = 54;
const PLAYER_X_MIN: f32 = 2.0;
const PLAYER_X_MAX: f32 = 89.0;
const PLAYER_MOVE_SPEED: f32 = 60.0;
const GRAVITY: f32 = 207.0;
const JUMP_VELOCITY: f32 = -136.0;
const BASE_SPEED: f32 = 48.0;
const MAX_SPEED: f32 = 144.0;
const SPEED_INCREASE_INTERVAL: i32 = 7;
const SPAWN_MIN: f32 = 0.75;
const SPAWN_MAX: f32 = 2.25;
const CLOUD_SPEED_RATIO: f32 = 0.3;
const BIRD_CHANCE: f32 = 0.22;
const BIRD_Y_LOW: f32 = 40.0;
const BIRD_Y_HIGH: f32 = 25.0;
const RUN_ANIM_FPS: f32 = 12.0;
const BIRD_ANIM_FPS: f32 = 8.0;

const MAX_OBSTACLES: usize = 8;
const MAX_GROUND_DECOR: usize = 16;
const MAX_GROUND_BUMPS: usize = 8;
const MAX_CLOUDS: usize = 6;
const MAX_BONUS_TEXTS: usize = 4;
const STAR_COUNT: usize = 20;

/// Obstacle sprite pool. First 6 are the base set; the last 4 are
/// added once `current_speed > 110`.
static OBSTACLE_SPRITES: &[&Sprite] = &[
    &SMALLTREE1,
    &PLANT1,
    &PLANT2,
    &SUNFLOWER_YOUNG,
    &ROSE_GROWING,
    &FREESIA_GROWING,
    &PLANT6,
    &ROSE_MATURE,
    &FREESIA_MATURE,
    &FREESIA_THRIVING,
];
const OBSTACLE_BASE_COUNT: u32 = 6;
const OBSTACLE_FAST_COUNT: u32 = 10;

static CLOUD_SPRITES: &[&Sprite] = &[
    &crate::assets::nature::CLOUD1,
    &crate::assets::nature::CLOUD2,
    &crate::assets::nature::CLOUD3,
];

struct Obstacle {
    sprite: &'static Sprite,
    x: f32,
    /// -1.0 = sit on ground; otherwise absolute y for birds.
    y: f32,
    /// -1.0 = static obstacle; otherwise animation timer for birds.
    anim: f32,
    /// Birds only: marked when the player overlapped this bird while in the
    /// air, so we can award the jump bonus once the bird leaves screen.
    jumped: bool,
}

struct Decor {
    x: f32,
    kind: u8,
}

struct Cloud {
    sprite: &'static Sprite,
    x: f32,
    y: i32,
}

struct BonusText {
    x: f32,
    y: f32,
    timer: f32,
}

// ---------------------------------------------------------------------------
// Sky
// ---------------------------------------------------------------------------

const ARC_DURATION: f32 = 90.0;
const STAR_SPEED: f32 = 4.0;
const SKY_X_START: f32 = -17.0;
const SKY_X_END: f32 = 128.0;
const SKY_Y_HORIZON: f32 = 12.0;
const SKY_Y_PEAK: f32 = 2.0;
const TWINKLE_PERIOD: f32 = 0.15;

/// `None` = new moon (not drawn); `Some(n)` = MOON frame index.
const MOON_PHASES: [Option<usize>; 8] =
    [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)];

const STAR_DATA: [(f32, i32, u8); STAR_COUNT] = [
    (6.0, 4, 0), (14.0, 31, 3), (23.0, 14, 6), (33.0, 42, 2), (44.0, 8, 9),
    (52.0, 26, 5), (61.0, 47, 1), (70.0, 18, 8), (79.0, 36, 4), (90.0, 3, 7),
    (98.0, 50, 2), (107.0, 22, 10), (118.0, 11, 3), (127.0, 40, 8), (9.0, 48, 1),
    (29.0, 34, 5), (48.0, 6, 7), (68.0, 44, 0), (88.0, 17, 9), (122.0, 29, 6),
];

#[derive(Clone, Copy)]
struct Star {
    x: f32,
    y: i32,
    twinkle_offset: u8,
}

struct ZoomiesSky {
    sky_progress: f32,
    moon_phase_idx: usize,
    sun_anim_timer: f32,
    sun_anim_frame: usize,
    twinkle_timer: f32,
    twinkle_phase: u8,
    stars: [Star; STAR_COUNT],
}

impl ZoomiesSky {
    fn new() -> Self {
        let mut sky = Self {
            sky_progress: 0.45,
            moon_phase_idx: 1,
            sun_anim_timer: 0.0,
            sun_anim_frame: 0,
            twinkle_timer: 0.0,
            twinkle_phase: 0,
            stars: [Star { x: 0.0, y: 0, twinkle_offset: 0 }; STAR_COUNT],
        };
        sky.load_star_data();
        sky
    }

    fn load_star_data(&mut self) {
        for (slot, (x, y, off)) in self.stars.iter_mut().zip(STAR_DATA.iter()) {
            slot.x = *x;
            slot.y = *y;
            slot.twinkle_offset = *off;
        }
    }

    fn reset(&mut self, rng: &mut u32) {
        self.sky_progress = 0.45;
        self.moon_phase_idx = 1;
        self.sun_anim_timer = 0.0;
        self.sun_anim_frame = 0;
        self.twinkle_timer = 0.0;
        self.twinkle_phase = 0;
        self.load_star_data();
        self.shuffle_stars(rng);
    }

    fn shuffle_stars(&mut self, rng: &mut u32) {
        // Fisher-Yates.
        for i in (1..self.stars.len()).rev() {
            let j = (xorshift32(rng) as usize) % (i + 1);
            self.stars.swap(i, j);
        }
    }

    fn update(&mut self, dt: f32) {
        let prev = self.sky_progress;
        self.sky_progress += dt / ARC_DURATION;

        if prev < 2.0 && self.sky_progress >= 2.0 {
            self.moon_phase_idx = (self.moon_phase_idx + 1) % MOON_PHASES.len();
        }
        self.sky_progress %= 2.0;

        self.sun_anim_timer += dt;
        if self.sun_anim_timer >= 0.5 {
            self.sun_anim_timer -= 0.5;
            self.sun_anim_frame = (self.sun_anim_frame + 1) % SUN.frames.len();
        }

        let scroll = STAR_SPEED * dt;
        let w = DISPLAY_W as f32;
        for star in &mut self.stars {
            star.x -= scroll;
            if star.x < -5.0 {
                star.x += w + 10.0;
            }
        }

        self.twinkle_timer += dt;
        if self.twinkle_timer >= TWINKLE_PERIOD {
            self.twinkle_timer -= TWINKLE_PERIOD;
            self.twinkle_phase = (self.twinkle_phase + 1) % 12;
        }
    }

    fn draw(&self, renderer: &mut Renderer) {
        if self.sky_progress >= 1.0 {
            self.draw_stars(renderer);
            self.draw_moon(renderer);
        } else {
            self.draw_sun(renderer);
        }
    }

    fn arc_xy(t: f32) -> (i32, i32) {
        let x = SKY_X_START + t * (SKY_X_END - SKY_X_START);
        let y = SKY_Y_HORIZON - (SKY_Y_HORIZON - SKY_Y_PEAK) * 4.0 * t * (1.0 - t);
        (x as i32, y as i32)
    }

    fn draw_sun(&self, renderer: &mut Renderer) {
        let (x, y) = Self::arc_xy(self.sky_progress);
        renderer.draw_sprite(
            &SUN,
            Point::new(x, y),
            SpriteOpts { frame: self.sun_anim_frame, transparent: true, ..Default::default() },
        );
    }

    fn draw_moon(&self, renderer: &mut Renderer) {
        let t = self.sky_progress - 1.0;
        let (x, y) = Self::arc_xy(t);
        if let Some(frame) = MOON_PHASES[self.moon_phase_idx] {
            renderer.draw_sprite(
                &MOON,
                Point::new(x, y),
                SpriteOpts { frame, transparent: true, ..Default::default() },
            );
        }
    }

    fn draw_stars(&self, renderer: &mut Renderer) {
        let t = self.sky_progress - 1.0;
        let n = self.stars.len();
        let fade = 0.15_f32;
        let visible = if t < fade {
            (t / fade * n as f32) as usize
        } else if t > 1.0 - fade {
            ((1.0 - t) / fade * n as f32) as usize
        } else {
            n
        };

        let phase = self.twinkle_phase;
        for star in &self.stars[..visible.min(n)] {
            let sx = star.x as i32;
            if sx < 0 || sx >= DISPLAY_W {
                continue;
            }
            let sy = star.y;
            let p = (phase + star.twinkle_offset) % 12;
            renderer.draw_pixel(Point::new(sx, sy), true);
            if p == 10 {
                renderer.draw_pixel(Point::new(sx - 1, sy), true);
                renderer.draw_pixel(Point::new(sx + 1, sy), true);
                renderer.draw_pixel(Point::new(sx, sy - 1), true);
                renderer.draw_pixel(Point::new(sx, sy + 1), true);
            } else if p == 8 || p == 9 || p == 11 {
                renderer.draw_pixel(Point::new(sx - 1, sy), true);
                renderer.draw_pixel(Point::new(sx + 1, sy), true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

pub struct ZoomiesScene {
    sky: ZoomiesSky,
    player_x: f32,
    player_y: f32,
    player_vy: f32,
    is_jumping: bool,
    is_hit: bool,
    is_new_best: bool,
    run_anim: f32,
    obstacles: Vec<Obstacle, MAX_OBSTACLES>,
    spawn_timer: f32,
    ground_decor: Vec<Decor, MAX_GROUND_DECOR>,
    ground_bumps: Vec<f32, MAX_GROUND_BUMPS>,
    clouds: Vec<Cloud, MAX_CLOUDS>,
    bonus_texts: Vec<BonusText, MAX_BONUS_TEXTS>,
    score: i32,
    score_timer: f32,
    current_speed: f32,
    game_started: bool,
    /// Accumulated score across all runs since entering the scene; used to
    /// scale stat rewards on exit.
    session_score: i32,
}

impl ZoomiesScene {
    pub fn new() -> Self {
        Self {
            sky: ZoomiesSky::new(),
            player_x: PLAYER_X_MIN,
            player_y: GROUND_Y as f32 - RUNCAT1.height as f32,
            player_vy: 0.0,
            is_jumping: false,
            is_hit: false,
            is_new_best: false,
            run_anim: 0.0,
            obstacles: Vec::new(),
            spawn_timer: 1.0,
            ground_decor: Vec::new(),
            ground_bumps: Vec::new(),
            clouds: Vec::new(),
            bonus_texts: Vec::new(),
            score: 0,
            score_timer: 0.0,
            current_speed: BASE_SPEED,
            game_started: false,
            session_score: 0,
        }
    }

    fn reset_game(&mut self, rng: &mut u32) {
        self.player_x = PLAYER_X_MIN;
        self.player_y = GROUND_Y as f32 - RUNCAT1.height as f32;
        self.player_vy = 0.0;
        self.is_jumping = false;
        self.is_hit = false;
        self.is_new_best = false;
        self.run_anim = 0.0;
        self.obstacles.clear();
        self.spawn_timer = 1.0;

        self.ground_decor.clear();
        let mut x = 0;
        while x < DISPLAY_W + 20 {
            let kind = rand_range_u32(rng, 1, 4) as u8;
            let _ = self.ground_decor.push(Decor { x: x as f32, kind });
            x += 15;
        }

        self.ground_bumps.clear();
        let mut x = 16;
        while x < DISPLAY_W + 40 {
            let _ = self.ground_bumps.push(x as f32);
            x += 32;
        }

        self.clouds.clear();
        for &cx in &[20i32, 90, 160] {
            self.spawn_cloud_at(rng, cx as f32);
        }

        self.bonus_texts.clear();
        self.score = 0;
        self.score_timer = 0.0;
        self.current_speed = BASE_SPEED;
        self.game_started = false;
        self.sky.reset(rng);
    }

    fn spawn_cloud_at(&mut self, rng: &mut u32, x: f32) {
        let idx = rand_range_u32(rng, 0, CLOUD_SPRITES.len() as u32 - 1) as usize;
        let sprite = CLOUD_SPRITES[idx];
        let y = rand_range_u32(rng, 0, 20) as i32 - 10;
        let _ = self.clouds.push(Cloud { sprite, x, y });
    }

    fn spawn_obstacle(&mut self, rng: &mut u32) {
        if rand_f32(rng) < BIRD_CHANCE {
            let y = if rand_range_u32(rng, 0, 1) == 0 { BIRD_Y_LOW } else { BIRD_Y_HIGH };
            let _ = self.obstacles.push(Obstacle {
                sprite: &SMALL_BIRD1,
                x: DISPLAY_W as f32 + 5.0,
                y,
                anim: 0.0,
                jumped: false,
            });
        } else {
            let count = if self.current_speed > 110.0 {
                OBSTACLE_FAST_COUNT
            } else {
                OBSTACLE_BASE_COUNT
            };
            let idx = rand_range_u32(rng, 0, count - 1) as usize;
            let _ = self.obstacles.push(Obstacle {
                sprite: OBSTACLE_SPRITES[idx],
                x: DISPLAY_W as f32 + 5.0,
                y: -1.0,
                anim: -1.0,
                jumped: false,
            });
        }
    }

    fn check_collisions(&mut self, ctx: &mut GameContext) {
        let player_left = self.player_x as i32 + 4;
        let player_right = self.player_x as i32 + RUNCAT1.width as i32 - 4;
        let player_top = self.player_y as i32 + 2;
        let player_bottom = self.player_y as i32 + RUNCAT1.height as i32;

        for obs in &self.obstacles {
            let obs_x = obs.x as i32;
            let obs_y = if obs.y < 0.0 {
                GROUND_Y - obs.sprite.height as i32
            } else {
                obs.y as i32
            };
            let obs_left = obs_x + 2;
            let obs_right = obs_x + obs.sprite.width as i32 - 2;
            let obs_top = obs_y + 2;
            let obs_bottom = obs_y + obs.sprite.height as i32;

            if player_right > obs_left
                && player_left < obs_right
                && player_bottom > obs_top
                && player_top < obs_bottom
            {
                self.is_hit = true;
                self.session_score += self.score;
                if self.score > ctx.zoomies_high_score {
                    ctx.zoomies_high_score = self.score;
                    self.is_new_best = true;
                } else {
                    self.is_new_best = false;
                }
                return;
            }
        }
    }

    fn award_bird_jump_bonus(&mut self) {
        self.score += 100;
        // Center "+100" (4 chars * CHAR_W) over the player.
        let text_x = self.player_x + RUNCAT1.width as f32 / 2.0 - (4 * CHAR_W / 2) as f32;
        let _ = self.bonus_texts.push(BonusText {
            x: text_x,
            y: (self.player_y as i32 - 2) as f32,
            timer: 0.8,
        });
    }

    fn draw_ground(&self, renderer: &mut Renderer) {
        let y = GROUND_Y;
        renderer.draw_line(Point::new(0, y), Point::new(DISPLAY_W, y));
        for &bump_x in &self.ground_bumps {
            let x = bump_x as i32;
            if x >= 0 && x < DISPLAY_W - 4 {
                renderer.draw_line_color(Point::new(x, y), Point::new(x + 4, y), false);
                renderer.draw_line(Point::new(x, y), Point::new(x + 2, y - 2));
                renderer.draw_line(Point::new(x + 2, y - 2), Point::new(x + 4, y));
            }
        }
    }

    fn draw_ground_decor(&self, renderer: &mut Renderer) {
        let ground_y = GROUND_Y;
        for decor in &self.ground_decor {
            let x = decor.x as i32;
            if x < 0 || x >= DISPLAY_W {
                continue;
            }
            // Note: rand_range_u32(1, 4) means kind is in {1,2,3,4}, so the
            // `DECOR_DOT == 0` branch is dead and 75% of decor (2,3,4) all
            // render as bumps. Preserved here for parity with the existing
            // visual.
            match decor.kind {
                0 => renderer.draw_pixel(Point::new(x, ground_y + 3), true),
                1 => renderer.draw_line(Point::new(x, ground_y + 4), Point::new(x + 3, ground_y + 4)),
                _ => {
                    renderer.draw_pixel(Point::new(x, ground_y + 2), true);
                    renderer.draw_pixel(Point::new(x + 1, ground_y + 3), true);
                }
            }
        }
    }

    fn draw_clouds(&self, renderer: &mut Renderer) {
        for cloud in &self.clouds {
            renderer.draw_sprite(
                cloud.sprite,
                Point::new(cloud.x as i32, cloud.y),
                SpriteOpts { transparent: true, ..Default::default() },
            );
        }
    }

    fn draw_player(&self, renderer: &mut Renderer) {
        let x = self.player_x as i32;
        let y = self.player_y as i32;
        if self.is_hit {
            let sit_y = GROUND_Y - SITCAT1.height as i32;
            renderer.draw_sprite(
                &SITCAT1,
                Point::new(x, sit_y),
                SpriteOpts { transparent: true, ..Default::default() },
            );
        } else if self.is_jumping {
            renderer.draw_sprite(
                &RUNCAT1,
                Point::new(x, y),
                SpriteOpts { frame: 0, transparent: true, ..Default::default() },
            );
        } else {
            let frame = (self.run_anim as usize) % RUNCAT1.frames.len();
            renderer.draw_sprite(
                &RUNCAT1,
                Point::new(x, y),
                SpriteOpts { frame, transparent: true, ..Default::default() },
            );
        }
    }

    fn draw_score(&self, renderer: &mut Renderer) {
        let mut s: String<8> = String::new();
        let _ = write!(s, "{}", self.score);
        let x = DISPLAY_W - (s.len() as i32 * CHAR_W) - 2;
        renderer.draw_text(s.as_str(), Point::new(x, 2));
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        let session = self.session_score + self.score;
        if session <= 0 {
            return;
        }
        // Asymptotic: 1000pts ~= 1x, 5000pts ~= 2.2x, 10000pts ~= 3.2x.
        let progress = (session as f32 / 1000.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Energy,      -5.0 * progress),
            (StatId::Fitness,      8.0 * progress),
            (StatId::Fulfillment,  3.0 * progress),
            (StatId::Fullness,    -3.0 * progress),
            (StatId::Playfulness,  3.0 * progress),
            (StatId::Sociability,  3.0 * progress),
            (StatId::Loyalty,      1.0 * progress),
        ]);
        let coins = (5.0 * progress) as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }
}

impl Scene for ZoomiesScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.reset_game(&mut rng);
        self.session_score = 0;
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        self.apply_rewards(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        // Allow exit back to the main menu at any time.
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }

        // Jump / start / restart.
        if buttons.was_just_pressed(Button::A) || buttons.was_just_pressed(Button::Up) {
            if !self.game_started {
                self.game_started = true;
            } else if self.is_hit {
                let mut rng = ctx.rng;
                self.reset_game(&mut rng);
                ctx.rng = rng;
                self.game_started = true;
            } else if !self.is_jumping {
                self.is_jumping = true;
                self.player_vy = JUMP_VELOCITY;
            }
        }

        if !self.is_hit {
            self.sky.update(dt);
        }

        if !self.game_started || self.is_hit {
            return None;
        }

        // Score timer ticks at 10 Hz.
        self.score_timer += dt;
        if self.score_timer >= 0.1 {
            self.score += 1;
            self.score_timer -= 0.1;
            if self.score % SPEED_INCREASE_INTERVAL == 0 {
                self.current_speed = (self.current_speed + 1.0).min(MAX_SPEED);
            }
        }

        let cloud_speed = self.current_speed * CLOUD_SPEED_RATIO;

        // Player physics.
        if self.is_jumping {
            let gravity_mult = if buttons.is_pressed(Button::A) || buttons.is_pressed(Button::Up) {
                1.0
            } else if buttons.is_pressed(Button::Down) {
                1.5
            } else {
                2.0
            };
            self.player_vy += GRAVITY * gravity_mult * dt;
            self.player_y += self.player_vy * dt;
            let ground_level = GROUND_Y as f32 - RUNCAT1.height as f32;
            if self.player_y >= ground_level {
                self.player_y = ground_level;
                self.player_vy = 0.0;
                self.is_jumping = false;
            }
        } else {
            if buttons.is_pressed(Button::Left) {
                self.player_x = (self.player_x - PLAYER_MOVE_SPEED * dt).max(PLAYER_X_MIN);
            } else if buttons.is_pressed(Button::Right) {
                self.player_x = (self.player_x + PLAYER_MOVE_SPEED * dt).min(PLAYER_X_MAX);
            }
        }

        if !self.is_jumping {
            self.run_anim += dt * RUN_ANIM_FPS;
            let n = RUNCAT1.frames.len() as f32;
            if self.run_anim >= n {
                self.run_anim -= n;
            }
        }

        // Update obstacles: move, animate birds, detect jumped birds, cull.
        let speed_dt = self.current_speed * dt;
        let anim_dt = dt * BIRD_ANIM_FPS;
        let player_x = self.player_x;
        let is_jumping = self.is_jumping;
        let mut bird_bonuses: u8 = 0;
        let mut i = 0;
        while i < self.obstacles.len() {
            let remove;
            {
                let obs = &mut self.obstacles[i];
                obs.x -= speed_dt;
                if obs.anim >= 0.0 {
                    obs.anim += anim_dt;
                    if !obs.jumped && is_jumping {
                        let bird_right = obs.x + obs.sprite.width as f32;
                        if obs.x < player_x + RUNCAT1.width as f32 && bird_right > player_x {
                            obs.jumped = true;
                        }
                    }
                }
                if obs.x > -20.0 {
                    remove = false;
                } else {
                    if obs.anim >= 0.0 && obs.jumped {
                        bird_bonuses += 1;
                    }
                    remove = true;
                }
            }
            if remove {
                self.obstacles.swap_remove(i);
            } else {
                i += 1;
            }
        }
        for _ in 0..bird_bonuses {
            self.award_bird_jump_bonus();
        }

        // Spawn new obstacles.
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            let mut rng = ctx.rng;
            self.spawn_obstacle(&mut rng);
            self.spawn_timer = if self.current_speed < BASE_SPEED + 40.0 {
                rand_range_f32(&mut rng, SPAWN_MIN * 1.5, SPAWN_MAX * 1.25)
            } else if self.current_speed > MAX_SPEED - 20.0 {
                rand_range_f32(&mut rng, SPAWN_MIN, SPAWN_MAX * 0.75)
            } else {
                rand_range_f32(&mut rng, SPAWN_MIN, SPAWN_MAX)
            };
            ctx.rng = rng;
        }

        // Ground decor: move and cull, refill from the right.
        let mut i = 0;
        while i < self.ground_decor.len() {
            self.ground_decor[i].x -= speed_dt;
            if self.ground_decor[i].x <= -10.0 {
                self.ground_decor.swap_remove(i);
            } else {
                i += 1;
            }
        }
        let rightmost = self
            .ground_decor
            .iter()
            .map(|d| d.x)
            .fold(f32::MIN, f32::max);
        let rightmost = if rightmost == f32::MIN { 0.0 } else { rightmost };
        if rightmost < DISPLAY_W as f32 {
            let mut rng = ctx.rng;
            let gap = rand_range_u32(&mut rng, 12, 20) as f32;
            let kind = rand_range_u32(&mut rng, 1, 4) as u8;
            ctx.rng = rng;
            let _ = self.ground_decor.push(Decor {
                x: rightmost + gap,
                kind,
            });
        }

        // Ground bumps: move and cull, refill at fixed 32px spacing.
        let mut i = 0;
        while i < self.ground_bumps.len() {
            self.ground_bumps[i] -= speed_dt;
            if self.ground_bumps[i] <= -10.0 {
                self.ground_bumps.swap_remove(i);
            } else {
                i += 1;
            }
        }
        let rightmost = self.ground_bumps.iter().copied().fold(f32::MIN, f32::max);
        let rightmost = if rightmost == f32::MIN { 0.0 } else { rightmost };
        if rightmost < DISPLAY_W as f32 {
            let _ = self.ground_bumps.push(rightmost + 32.0);
        }

        // Clouds: drift left, cull, refill.
        let cloud_speed_dt = cloud_speed * dt;
        let mut i = 0;
        while i < self.clouds.len() {
            self.clouds[i].x -= cloud_speed_dt;
            if self.clouds[i].x <= -70.0 {
                self.clouds.swap_remove(i);
            } else {
                i += 1;
            }
        }
        let rightmost = self.clouds.iter().map(|c| c.x).fold(f32::MIN, f32::max);
        let rightmost = if rightmost == f32::MIN { 0.0 } else { rightmost };
        if rightmost < DISPLAY_W as f32 {
            let mut rng = ctx.rng;
            let gap = rand_range_u32(&mut rng, 60, 100) as f32;
            let x = rightmost + gap;
            self.spawn_cloud_at(&mut rng, x);
            ctx.rng = rng;
        }

        // Bonus texts: rise and expire.
        let mut i = 0;
        while i < self.bonus_texts.len() {
            self.bonus_texts[i].y -= 18.0 * dt;
            self.bonus_texts[i].timer -= dt;
            if self.bonus_texts[i].timer <= 0.0 {
                self.bonus_texts.swap_remove(i);
            } else {
                i += 1;
            }
        }

        self.check_collisions(ctx);
        None
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.sky.draw(renderer);
        self.draw_clouds(renderer);
        self.draw_ground(renderer);
        self.draw_ground_decor(renderer);

        for obs in &self.obstacles {
            let x = obs.x as i32;
            let y = if obs.y < 0.0 {
                GROUND_Y - obs.sprite.height as i32
            } else {
                obs.y as i32
            };
            let frame = if obs.anim >= 0.0 {
                (obs.anim as usize) % obs.sprite.frames.len()
            } else {
                0
            };
            renderer.draw_sprite(
                obs.sprite,
                Point::new(x, y),
                SpriteOpts { frame, transparent: true, ..Default::default() },
            );
        }

        self.draw_player(renderer);

        for bt in &self.bonus_texts {
            renderer.draw_text(t!("+100"), Point::new(bt.x as i32, bt.y as i32));
        }

        self.draw_score(renderer);

        if !self.game_started {
            // Start prompt: "A to jump\n\nHold for\nhigh jumps!"
            use crate::ui::popup::Popup;
            let mut popup = Popup::new(14, 10, 100, 48);
            popup.set_text(t!("A to jump\n\nHold for\nhigh jumps!"), false, true);
            popup.draw(renderer, false);
        } else if self.is_hit {
            use crate::ui::popup::Popup;
            let mut popup = Popup::new(14, 10, 100, 28);
            let mut buf: String<32> = String::new();
            if self.is_new_best {
                let _ = buf.push_str(t!("NEW BEST!"));
                let _ = buf.push('\n');
                let _ = write!(buf, "{}", self.score);
            } else {
                let _ = buf.push_str(t!("Ooof!"));
                let _ = buf.push('\n');
                let mut n: String<8> = String::new();
                let _ = write!(n, "{}", ctx.zoomies_high_score);
                crate::i18n::substitute(&mut buf, t!("Best: {n}"), &[("n", n.as_str())]);
            }
            popup.set_text(buf.as_str(), false, true);
            popup.draw(renderer, false);
        }
    }
}
