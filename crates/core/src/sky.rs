use embedded_graphics::prelude::Point;
use crate::platform::time::Instant;
use heapless::Vec;
use micromath::F32Ext;

use crate::{
    assets::nature::{CLOUD1, CLOUD2, CLOUD3, HOT_AIR_BALLOON, MOON, PLANE_TINY, SUN, SUN_HOT},
    context::GameContext,
    rand::xorshift32,
    render::{Renderer, Sprite, SpriteOpts},
    time_system::{Season, Weather},
};

const SUN_RISE: f32 = 5.0;
const SUN_SET: f32 = 19.0;
const SUN_X_START: f32 = -20.0;
const SUN_X_END: f32 = 180.0;
const SUN_Y_HORIZON: f32 = 14.0;
const SUN_Y_PEAK: f32 = 2.0;

const MOON_RISE: f32 = 17.0;
const MOON_SET: f32 = 29.0;
const MOON_X_START: f32 = -20.0;
const MOON_X_END: f32 = 180.0;
const MOON_Y_HORIZON: f32 = 14.0;
const MOON_Y_PEAK: f32 = 3.0;

const STAR_SEED: u32 = 42;
const STAR_COUNT: usize = 50;
const STAR_FIELD_WIDTH: i32 = 256;
const STAR_FIELD_HEIGHT: i32 = 50;
const TWINKLE_RATIO_PERCENT: u32 = 20;
const TWINKLE_CYCLE_LENGTH: u8 = 12;
const TWINKLE_LARGE_PHASE: u8 = 10;
const TWINKLE_SMALL_PHASES: [u8; 3] = [8, 9, 11];

const SUN_FRAME_PERIOD: f32 = 0.5;
const TWINKLE_PERIOD: f32 = 0.3;

const DISPLAY_WIDTH: i32 = 128;

const MAX_CLOUDS: usize = 11;
const CLOUD_WRAP_BUFFER: i32 = 65;
const CLOUD_RESPAWN_X: f32 = -60.0;

const MAX_PARTICLES: usize = 16;
const SS_MAX_LIFETIME: f32 = 3.1;
const SS_GROW_DURATION: f32 = 0.5;
const SS_SHRINK_START: f32 = SS_MAX_LIFETIME - 0.7;
const SS_PARTICLE_INTERVAL: f32 = 0.12;
const SS_PARTICLE_LIFETIME: f32 = 0.7;

const METEOR_BASE_RATE: f32 = 0.0005;
const METEOR_SHOWER_MULT: f32 = 16.0;
const SKY_EVENT_SPAWN_RATE: f32 = 0.001;

const RAIN_PARTICLE_COUNT: usize = 20;
const SNOW_PARTICLE_COUNT: usize = 15;
const PRECIP_MAX: usize = 20;
const RAIN_SPEED_Y: f32 = 100.0;
const RAIN_SPEED_X: f32 = 10.0;
const RAIN_STREAK_LENGTH: i32 = 3;
const SNOW_SPEED_Y: f32 = 15.0;
const SNOW_DRIFT_SPEED: f32 = 8.0;
const PRECIP_PARALLAX: f32 = 0.3;
const DISPLAY_HEIGHT: i32 = 64;
const SKY_PARALLAX_BG: f32 = 0.3;

const LIGHTNING_FLASH_INTERVAL: f32 = 0.05;
const LIGHTNING_SPAWN_RATE: f32 = 0.003;

const MOON_PHASE_FRAMES: [Option<u8>; 8] = [
    None,    // New
    Some(0), // Wax Cres
    Some(1), // 1st Qtr
    Some(2), // Wax Gib
    Some(3), // Full
    Some(4), // Wan Gib
    Some(5), // 3rd Qtr
    Some(6), // Wan Cres
];

struct CloudTemplate {
    sprite: &'static Sprite,
    y: i16,
    base_speed: f32,
}

const CLOUD_TEMPLATES: &[CloudTemplate] = &[
    CloudTemplate { sprite: &CLOUD1, y: -7,  base_speed: 2.5 },
    CloudTemplate { sprite: &CLOUD1, y: -17, base_speed: 8.0 },
    CloudTemplate { sprite: &CLOUD2, y: 0,   base_speed: 4.0 },
    CloudTemplate { sprite: &CLOUD2, y: -10, base_speed: 3.0 },
    CloudTemplate { sprite: &CLOUD1, y: -5,  base_speed: 5.0 },
    CloudTemplate { sprite: &CLOUD2, y: -15, base_speed: 6.0 },
    CloudTemplate { sprite: &CLOUD1, y: -12, base_speed: 3.5 },
    CloudTemplate { sprite: &CLOUD3, y: -3,  base_speed: 3.0 },
    CloudTemplate { sprite: &CLOUD3, y: -14, base_speed: 5.5 },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeCategory {
    LateNight,
    Dawn,
    Morning,
    Noon,
    Afternoon,
    Dusk,
    Evening,
    Night,
}

impl TimeCategory {
    pub fn from_hour(h: u8) -> Self {
        if h >= 21 {
            Self::Night
        } else if h >= 19 {
            Self::Evening
        } else if h >= 17 {
            Self::Dusk
        } else if h >= 14 {
            Self::Afternoon
        } else if h >= 12 {
            Self::Noon
        } else if h >= 7 {
            Self::Morning
        } else if h >= 5 {
            Self::Dawn
        } else {
            Self::LateNight
        }
    }

    pub fn show_stars(self) -> bool {
        matches!(
            self,
            Self::Night | Self::LateNight | Self::Evening | Self::Dawn
        )
    }
}


// (min_count, max_count, speed_multiplier) by weather
fn weather_cloud_config(weather: Weather) -> (u8, u8, f32) {
    match weather {
        Weather::Clear => (1, 2, 0.7),
        Weather::Cloudy => (3, 5, 1.0),
        Weather::Overcast => (5, 7, 0.8),
        Weather::Rain => (5, 8, 1.0),
        Weather::Storm => (8, 11, 1.3),
        Weather::Snow => (5, 8, 0.6),
        Weather::Windy => (3, 4, 1.5),
    }
}

fn meteor_weather_ok(weather: Weather) -> bool {
    matches!(weather, Weather::Clear | Weather::Cloudy | Weather::Windy)
}

fn meteor_season_mult(season: Season) -> f32 {
    match season {
        Season::Summer => 1.0,
        Season::Spring => 0.6,
        Season::Fall => 0.4,
        Season::Winter => 0.15,
    }
}

struct SkyEventType {
    sprite: &'static Sprite,
    speed: f32,
    mirror_when_right: bool,
}

const SKY_EVENT_TYPES: &[SkyEventType] = &[
    SkyEventType { sprite: &HOT_AIR_BALLOON, speed: 4.0,  mirror_when_right: false },
    SkyEventType { sprite: &PLANE_TINY,      speed: 12.0, mirror_when_right: true },
];

#[derive(Clone, Copy)]
struct ShootingParticle {
    x: f32,
    y: f32,
    life: f32,
}

struct ShootingStarEvent {
    x: f32,
    y: f32,
    max_length: f32,
    speed_x: f32,
    speed_y: f32,
    lifetime: f32,
    particles: Vec<ShootingParticle, MAX_PARTICLES>,
    particle_timer: f32,
    active: bool,
}

impl ShootingStarEvent {
    fn new(x: f32, y: f32, max_length: f32, speed_x: f32, speed_y: f32) -> Self {
        Self {
            x,
            y,
            max_length,
            speed_x,
            speed_y,
            lifetime: 0.0,
            particles: Vec::new(),
            particle_timer: 0.0,
            active: true,
        }
    }

    fn current_length(&self) -> f32 {
        if self.lifetime < SS_GROW_DURATION {
            self.max_length * (self.lifetime / SS_GROW_DURATION)
        } else if self.lifetime > SS_SHRINK_START {
            let remaining = SS_MAX_LIFETIME - self.lifetime;
            let shrink_duration = SS_MAX_LIFETIME - SS_SHRINK_START;
            self.max_length * (remaining / shrink_duration)
        } else {
            self.max_length
        }
    }

    fn get_points(&self) -> (i32, i32, i32, i32) {
        let cl = self.current_length();
        let trail_x = self.x - cl * self.speed_x / self.max_length;
        let trail_y = self.y - cl * self.speed_y / self.max_length;
        (trail_x as i32, trail_y as i32, self.x as i32, self.y as i32)
    }

    fn update(&mut self, dt: f32) {
        self.x += self.speed_x * dt;
        self.y += self.speed_y * dt;
        self.lifetime += dt;

        self.particle_timer += dt;
        if self.particle_timer > SS_PARTICLE_INTERVAL && self.lifetime > SS_GROW_DURATION {
            self.particle_timer = 0.0;
            let (tx, ty, _, _) = self.get_points();
            let _ = self.particles.push(ShootingParticle {
                x: tx as f32,
                y: ty as f32,
                life: 0.0,
            });
        }

        for p in self.particles.iter_mut() {
            p.x += self.speed_x * 0.15 * dt;
            p.y += self.speed_y * 0.1 * dt;
            p.life += dt;
        }
        self.particles.retain(|p| p.life < SS_PARTICLE_LIFETIME);

        if self.lifetime >= SS_MAX_LIFETIME {
            self.active = false;
        }
    }

    fn draw(&self, renderer: &mut Renderer) {
        for p in &self.particles {
            let px = p.x as i32;
            let py = p.y as i32;
            if px >= 0 && px < DISPLAY_WIDTH && py >= 0 && py < STAR_FIELD_HEIGHT {
                renderer.draw_pixel(Point::new(px, py), true);
            }
        }
        let (x1, y1, x2, y2) = self.get_points();
        if x1 < DISPLAY_WIDTH && x2 >= 0 && y1 < STAR_FIELD_HEIGHT + 10 {
            renderer.draw_line(Point::new(x1, y1), Point::new(x2, y2));
        }
    }
}

#[derive(Clone, Copy)]
enum PrecipType {
    Rain,
    Snow,
}

#[derive(Clone, Copy)]
struct RainParticle {
    x: f32,
    y: f32,
    speed_var: f32,
    x_var: f32,
}

#[derive(Clone, Copy)]
struct SnowParticle {
    x: f32,
    y: f32,
    drift_offset: f32,
    drift_speed: f32,
    speed_var: f32,
}

fn spawn_rain_particle(rng: &mut u32, world_width: i32, random_y: bool) -> RainParticle {
    RainParticle {
        x: (xorshift32(rng) % world_width as u32) as f32,
        y: if random_y {
            (xorshift32(rng) % DISPLAY_HEIGHT as u32) as f32
        } else {
            -((xorshift32(rng) % 16) as f32)
        },
        speed_var: 0.7 + (xorshift32(rng) % 60) as f32 / 100.0,
        x_var: 0.8 + (xorshift32(rng) % 40) as f32 / 100.0,
    }
}

fn spawn_snow_particle(rng: &mut u32, world_width: i32, random_y: bool) -> SnowParticle {
    SnowParticle {
        x: (xorshift32(rng) % world_width as u32) as f32,
        y: if random_y {
            (xorshift32(rng) % DISPLAY_HEIGHT as u32) as f32
        } else {
            -((xorshift32(rng) % 16) as f32)
        },
        drift_offset: (xorshift32(rng) % 628) as f32 / 100.0,
        drift_speed: 1.5 + (xorshift32(rng) % 200) as f32 / 100.0,
        speed_var: 0.6 + (xorshift32(rng) % 80) as f32 / 100.0,
    }
}

struct SkyEventEntity {
    sprite: &'static Sprite,
    x: f32,
    y: i16,
    speed: f32,
    going_right: bool,
    mirror_when_right: bool,
    world_width: i32,
    active: bool,
}

impl SkyEventEntity {
    fn update(&mut self, dt: f32) {
        if self.going_right {
            self.x += self.speed * dt;
            if self.x > (self.world_width + self.sprite.width as i32 + 20) as f32 {
                self.active = false;
            }
        } else {
            self.x -= self.speed * dt;
            if self.x < -(self.sprite.width as f32) - 20.0 {
                self.active = false;
            }
        }
    }

    fn mirror(&self) -> bool {
        self.mirror_when_right && self.going_right
    }

    fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        let screen_x = self.x as i32 - camera_offset;
        let w = self.sprite.width as i32;
        if screen_x > -w && screen_x < DISPLAY_WIDTH + w {
            renderer.draw_sprite(
                self.sprite,
                Point::new(screen_x, self.y as i32),
                SpriteOpts {
                    mirror_h: self.mirror(),
                    ..Default::default()
                },
            );
        }
    }
}

#[derive(Clone, Copy)]
struct Star {
    x: i16,
    y: i16,
    twinkles: bool,
    phase_offset: u8,
}

fn generate_stars(seed: u32) -> [Star; STAR_COUNT] {
    let mut state = seed.max(1);
    let mut stars = [Star { x: 0, y: 0, twinkles: false, phase_offset: 0 }; STAR_COUNT];
    for slot in &mut stars {
        let x = (xorshift32(&mut state) % STAR_FIELD_WIDTH as u32) as i16;
        let y = (xorshift32(&mut state) % STAR_FIELD_HEIGHT as u32) as i16;
        let twinkles = (xorshift32(&mut state) % 100) < TWINKLE_RATIO_PERCENT;
        let phase_offset = (xorshift32(&mut state) % TWINKLE_CYCLE_LENGTH as u32) as u8;
        *slot = Star { x, y, twinkles, phase_offset };
    }
    stars
}

#[derive(Clone, Copy)]
struct Cloud {
    sprite: &'static Sprite,
    y: i16,
    base_speed: f32,
    x: f32,
}

pub struct SkyRenderer {
    world_width: i32,
    elapsed_time: f32,
    stars: [Star; STAR_COUNT],
    twinkle_timer: f32,
    twinkle_phase: u8,
    sun_anim_timer: f32,
    sun_anim_frame: usize,
    clouds: Vec<Cloud, MAX_CLOUDS>,
    cloud_speed_mult: f32,
    last_weather: Option<Weather>,
    shooting_star: Option<ShootingStarEvent>,
    sky_event: Option<SkyEventEntity>,
    precip_type: Option<PrecipType>,
    rain_particles: Vec<RainParticle, PRECIP_MAX>,
    snow_particles: Vec<SnowParticle, PRECIP_MAX>,
    lightning_active: bool,
    lightning_flashes_remaining: u8,
    lightning_timer: f32,
    lightning_invert: bool,
    rng: u32,
    star_seed: u32,
    /// Parameters of the most recently spawned shooting star, if it
    /// has not yet been read by `take_just_spawned_shooting_star`.
    /// Used by `LocationScene` during a visit to broadcast a `vss `
    /// frame on the inviter side. Locally-spawned and packet-spawned
    /// stars both set this; the caller is responsible for not
    /// echoing packet-driven spawns back across the wire.
    just_spawned_ss: Option<(f32, f32, f32, f32, f32)>,
    /// Same as `just_spawned_ss` but for balloon / plane spawns.
    /// Tuple is `(event_idx, going_right, y, speed)`.
    just_spawned_se: Option<(u8, bool, i16, f32)>,
}

impl SkyRenderer {
    pub fn new(world_width: i32) -> Self {
        let mut seed = Instant::now().duration_since_epoch().as_micros() as u32;
        seed ^= 0xdead_beef;
        if seed == 0 {
            seed = 1;
        }
        Self {
            world_width,
            elapsed_time: 0.0,
            stars: generate_stars(STAR_SEED),
            twinkle_timer: 0.0,
            twinkle_phase: 0,
            sun_anim_timer: 0.0,
            sun_anim_frame: 0,
            clouds: Vec::new(),
            cloud_speed_mult: 1.0,
            last_weather: None,
            shooting_star: None,
            sky_event: None,
            precip_type: None,
            rain_particles: Vec::new(),
            snow_particles: Vec::new(),
            lightning_active: false,
            lightning_flashes_remaining: 0,
            lightning_timer: 0.0,
            lightning_invert: false,
            rng: seed,
            star_seed: STAR_SEED,
            just_spawned_ss: None,
            just_spawned_se: None,
        }
    }

    /// Regenerate the star field from a pet seed so each pet has a unique but
    /// stable starscape.
    pub fn reseed_stars(&mut self, pet_seed: u64) {
        let folded = ((pet_seed ^ (pet_seed >> 32)) & 0xFFFF_FFFF) as u32;
        let s = if folded == 0 { STAR_SEED } else { folded };
        if s == self.star_seed {
            return;
        }
        self.star_seed = s;
        self.stars = generate_stars(s);
    }

    pub fn lightning_invert(&self) -> bool {
        self.lightning_invert
    }

    pub fn force_spawn_random(&mut self) {
        if xorshift32(&mut self.rng) % 2 == 0 {
            self.spawn_shooting_star();
        } else {
            self.spawn_sky_event();
        }
    }

    fn spawn_shooting_star(&mut self) {
        let start_x = (xorshift32(&mut self.rng) % 61 + 10) as f32; // 10..=70
        let start_y = (xorshift32(&mut self.rng) % 21 + 2) as f32;  // 2..=22
        let max_length = (xorshift32(&mut self.rng) % 13 + 16) as f32; // 16..=28
        let speed_x = (xorshift32(&mut self.rng) % 17 + 20) as f32;    // 20..=36
        let speed_y = (xorshift32(&mut self.rng) % 11 + 3) as f32;     // 3..=13
        self.shooting_star = Some(ShootingStarEvent::new(
            start_x, start_y, max_length, speed_x, speed_y,
        ));
        self.just_spawned_ss = Some((start_x, start_y, max_length, speed_x, speed_y));
    }

    /// Spawn a shooting star using parameters provided by an inbound
    /// `vss ` packet from the playdate inviter. Skips the
    /// "already-have-one" check so a packet-driven star can ride on
    /// top of any local timing. Does **not** flag
    /// `just_spawned_ss`, since this came from the wire and we don't
    /// want to echo it back.
    pub fn spawn_shooting_star_from_packet(
        &mut self,
        x: f32,
        y: f32,
        max_length: f32,
        sx: f32,
        sy: f32,
    ) {
        self.shooting_star = Some(ShootingStarEvent::new(x, y, max_length, sx, sy));
    }

    /// Read-and-clear the most recent locally-spawned shooting star's
    /// params. `LocationScene` polls this on the inviter side so it
    /// can broadcast a `vss ` to the invitee.
    pub fn take_just_spawned_shooting_star(&mut self) -> Option<(f32, f32, f32, f32, f32)> {
        self.just_spawned_ss.take()
    }

    fn spawn_sky_event(&mut self) {
        let evt_idx = (xorshift32(&mut self.rng) as usize) % SKY_EVENT_TYPES.len();
        let event_type = &SKY_EVENT_TYPES[evt_idx];
        let going_right = xorshift32(&mut self.rng) % 2 == 0;
        let start_x = if going_right {
            -(event_type.sprite.width as f32) - 10.0
        } else {
            self.world_width as f32 + 10.0
        };
        let y = (xorshift32(&mut self.rng) % 21 + 2) as i16; // 2..=22
        let speed_var = (xorshift32(&mut self.rng) % 41) as f32 / 100.0; // 0.0..0.4
        let speed = event_type.speed * (0.8 + speed_var);
        self.sky_event = Some(SkyEventEntity {
            sprite: event_type.sprite,
            x: start_x,
            y,
            speed,
            going_right,
            mirror_when_right: event_type.mirror_when_right,
            world_width: self.world_width,
            active: true,
        });
        self.just_spawned_se = Some((evt_idx as u8, going_right, y, speed));
    }

    /// Spawn a sky event (balloon / plane) using parameters from an
    /// inbound `vse ` packet. Index out-of-range wraps via modulo. Does
    /// not flag `just_spawned_se`, so a packet-driven spawn isn't echoed
    /// back.
    pub fn spawn_sky_event_from_packet(
        &mut self,
        event_idx: u8,
        going_right: bool,
        y: i16,
        speed: f32,
    ) {
        if SKY_EVENT_TYPES.is_empty() {
            return;
        }
        let idx = (event_idx as usize) % SKY_EVENT_TYPES.len();
        let event_type = &SKY_EVENT_TYPES[idx];
        let start_x = if going_right {
            -(event_type.sprite.width as f32) - 10.0
        } else {
            self.world_width as f32 + 10.0
        };
        self.sky_event = Some(SkyEventEntity {
            sprite: event_type.sprite,
            x: start_x,
            y,
            speed,
            going_right,
            mirror_when_right: event_type.mirror_when_right,
            world_width: self.world_width,
            active: true,
        });
    }

    /// Read-and-clear the most recent locally-spawned sky-event's
    /// params. Counterpart to [`Self::take_just_spawned_shooting_star`].
    pub fn take_just_spawned_sky_event(&mut self) -> Option<(u8, bool, i16, f32)> {
        self.just_spawned_se.take()
    }

    fn meteor_rate(&self, ctx: &GameContext) -> f32 {
        if !meteor_weather_ok(ctx.weather) {
            return 0.0;
        }
        let mut rate = METEOR_BASE_RATE * meteor_season_mult(ctx.season);
        if ctx.meteor_shower_timer > 0.0 {
            rate *= METEOR_SHOWER_MULT;
        }
        rate
    }

    fn maybe_spawn_shooting_star(&mut self, ctx: &GameContext) {
        if self.shooting_star.is_some() {
            return;
        }
        if !TimeCategory::from_hour(ctx.time_hours).show_stars() {
            return;
        }
        let rate = self.meteor_rate(ctx);
        let roll = (xorshift32(&mut self.rng) % 1_000_000) as f32 / 1_000_000.0;
        if roll < rate {
            self.spawn_shooting_star();
        }
    }

    fn maybe_spawn_sky_event(&mut self, ctx: &GameContext) {
        if self.sky_event.is_some() {
            return;
        }
        let cat = TimeCategory::from_hour(ctx.time_hours);
        if !matches!(
            cat,
            TimeCategory::Morning | TimeCategory::Noon | TimeCategory::Afternoon
        ) {
            return;
        }
        let roll = (xorshift32(&mut self.rng) % 1_000_000) as f32 / 1_000_000.0;
        if roll < SKY_EVENT_SPAWN_RATE {
            self.spawn_sky_event();
        }
    }

    pub fn update(&mut self, ctx: &GameContext, dt: f32) {
        self.elapsed_time += dt;
        self.twinkle_timer += dt;
        if self.twinkle_timer > TWINKLE_PERIOD {
            self.twinkle_timer = 0.0;
            self.twinkle_phase = (self.twinkle_phase + 1) % TWINKLE_CYCLE_LENGTH;
        }

        self.sun_anim_timer += dt;
        if self.sun_anim_timer > SUN_FRAME_PERIOD {
            self.sun_anim_timer = 0.0;
            self.sun_anim_frame = (self.sun_anim_frame + 1) % SUN.frames.len();
        }

        if self.last_weather != Some(ctx.weather) {
            self.configure_clouds(ctx.weather);
            self.configure_precipitation(ctx.weather);
        }

        let wrap_point = (self.world_width + CLOUD_WRAP_BUFFER) as f32;
        for cloud in self.clouds.iter_mut() {
            cloud.x += dt * cloud.base_speed * self.cloud_speed_mult;
            if cloud.x > wrap_point {
                cloud.x = CLOUD_RESPAWN_X;
            }
        }

        if let Some(ss) = self.shooting_star.as_mut() {
            ss.update(dt);
            if !ss.active {
                self.shooting_star = None;
            }
        }
        if let Some(se) = self.sky_event.as_mut() {
            se.update(dt);
            if !se.active {
                self.sky_event = None;
            }
        }

        self.maybe_spawn_shooting_star(ctx);
        self.maybe_spawn_sky_event(ctx);
        self.update_precipitation(dt);
        self.update_lightning(ctx, dt);

        // TODO: implement get_star_offset (time-of-night + seasonal drift), a
        // small slow shift of the star field over time.
    }

    fn configure_precipitation(&mut self, weather: Weather) {
        self.rain_particles.clear();
        self.snow_particles.clear();
        let (kind, intensity) = match weather {
            Weather::Rain => (Some(PrecipType::Rain), 0.3),
            Weather::Storm => (Some(PrecipType::Rain), 1.0),
            Weather::Snow => (Some(PrecipType::Snow), 0.7),
            _ => (None, 0.0),
        };
        self.precip_type = kind;
        match kind {
            Some(PrecipType::Rain) => {
                let count = (RAIN_PARTICLE_COUNT as f32 * intensity) as usize;
                for _ in 0..count {
                    let p = spawn_rain_particle(&mut self.rng, self.world_width, true);
                    let _ = self.rain_particles.push(p);
                }
            }
            Some(PrecipType::Snow) => {
                let count = (SNOW_PARTICLE_COUNT as f32 * intensity) as usize;
                for _ in 0..count {
                    let p = spawn_snow_particle(&mut self.rng, self.world_width, true);
                    let _ = self.snow_particles.push(p);
                }
            }
            None => {}
        }
    }

    fn update_precipitation(&mut self, dt: f32) {
        let screen_bottom = (DISPLAY_HEIGHT + 5) as f32;
        let world_width_f = self.world_width as f32;
        match self.precip_type {
            Some(PrecipType::Rain) => {
                for p in self.rain_particles.iter_mut() {
                    p.y += RAIN_SPEED_Y * p.speed_var * dt;
                    p.x += RAIN_SPEED_X * p.x_var * dt;
                    if p.y > screen_bottom {
                        p.y = -((xorshift32(&mut self.rng) % 16) as f32);
                        p.x = (xorshift32(&mut self.rng) % self.world_width as u32) as f32;
                        p.speed_var = 0.7 + (xorshift32(&mut self.rng) % 60) as f32 / 100.0;
                        p.x_var = 0.8 + (xorshift32(&mut self.rng) % 40) as f32 / 100.0;
                    }
                    if p.x > world_width_f {
                        p.x -= world_width_f;
                    }
                }
            }
            Some(PrecipType::Snow) => {
                for p in self.snow_particles.iter_mut() {
                    p.y += SNOW_SPEED_Y * p.speed_var * dt;
                    let drift = (self.elapsed_time * p.drift_speed + p.drift_offset).sin()
                        * SNOW_DRIFT_SPEED
                        * dt;
                    p.x += drift;
                    if p.y > screen_bottom {
                        p.y = -((xorshift32(&mut self.rng) % 16) as f32);
                        p.x = (xorshift32(&mut self.rng) % self.world_width as u32) as f32;
                        p.drift_offset = (xorshift32(&mut self.rng) % 628) as f32 / 100.0;
                        p.drift_speed = 1.5 + (xorshift32(&mut self.rng) % 200) as f32 / 100.0;
                        p.speed_var = 0.6 + (xorshift32(&mut self.rng) % 80) as f32 / 100.0;
                    }
                    if p.x < 0.0 {
                        p.x += world_width_f;
                    } else if p.x > world_width_f {
                        p.x -= world_width_f;
                    }
                }
            }
            None => {}
        }
    }

    fn update_lightning(&mut self, ctx: &GameContext, dt: f32) {
        if self.lightning_active {
            self.lightning_timer -= dt;
            if self.lightning_timer <= 0.0 {
                self.lightning_invert = !self.lightning_invert;
                self.lightning_flashes_remaining = self.lightning_flashes_remaining.saturating_sub(1);
                if self.lightning_flashes_remaining == 0 {
                    self.lightning_active = false;
                    self.lightning_invert = false;
                } else {
                    self.lightning_timer = LIGHTNING_FLASH_INTERVAL;
                }
            }
        } else if ctx.weather == Weather::Storm {
            let roll = (xorshift32(&mut self.rng) % 100_000) as f32 / 100_000.0;
            if roll < LIGHTNING_SPAWN_RATE {
                self.lightning_active = true;
                let num_inversions = (xorshift32(&mut self.rng) % 4 + 2) as u8;
                self.lightning_flashes_remaining = num_inversions * 2;
                self.lightning_timer = LIGHTNING_FLASH_INTERVAL;
                self.lightning_invert = true;
            }
        }
    }

    fn configure_clouds(&mut self, weather: Weather) {
        let (min_count, max_count, speed_mult) = weather_cloud_config(weather);
        let rolled = crate::rand::rand_range_u32(&mut self.rng, min_count as u32, max_count as u32);
        let count = (rolled as usize).min(CLOUD_TEMPLATES.len());
        self.cloud_speed_mult = speed_mult;
        self.clouds.clear();
        let spacing = self.world_width / count.max(1) as i32;
        for i in 0..count {
            let template = &CLOUD_TEMPLATES[i % CLOUD_TEMPLATES.len()];
            let cloud = Cloud {
                sprite: template.sprite,
                y: template.y,
                base_speed: template.base_speed,
                x: (i as i32 * spacing - 30) as f32,
            };
            let _ = self.clouds.push(cloud);
        }
        self.last_weather = Some(weather);
    }

    pub fn draw(&self, renderer: &mut Renderer, ctx: &GameContext, camera_x: i32) {
        let hours_f = ctx.time_hours as f32 + ctx.time_minutes as f32 / 60.0;
        let time_cat = TimeCategory::from_hour(ctx.time_hours);
        let bg_offset = (camera_x as f32 * SKY_PARALLAX_BG) as i32;

        if time_cat.show_stars() {
            self.draw_stars(renderer, ctx, bg_offset);
        }
        self.draw_moon(renderer, ctx.moon_phase, hours_f, bg_offset);
        self.draw_sun(renderer, hours_f, bg_offset, ctx.temperature);
        if let Some(ss) = &self.shooting_star {
            ss.draw(renderer);
        }
        if let Some(se) = &self.sky_event {
            se.draw(renderer, bg_offset);
        }
        // Clouds sit in front of celestial bodies so they pass over the sun/moon.
        self.draw_clouds(renderer, bg_offset);
        // Precipitation falls in front of clouds.
        let precip_offset = (camera_x as f32 * PRECIP_PARALLAX) as i32;
        self.draw_precipitation(renderer, precip_offset);
    }

    fn draw_precipitation(&self, renderer: &mut Renderer, camera_offset: i32) {
        match self.precip_type {
            Some(PrecipType::Rain) => {
                for p in &self.rain_particles {
                    let mut sx = p.x as i32 - camera_offset;
                    while sx < 0 { sx += DISPLAY_WIDTH; }
                    while sx >= DISPLAY_WIDTH { sx -= DISPLAY_WIDTH; }
                    let sy = p.y as i32;
                    if sy < -RAIN_STREAK_LENGTH || sy > DISPLAY_HEIGHT {
                        continue;
                    }
                    renderer.draw_line(
                        Point::new(sx - 1, sy - RAIN_STREAK_LENGTH),
                        Point::new(sx, sy),
                    );
                }
            }
            Some(PrecipType::Snow) => {
                for p in &self.snow_particles {
                    let mut sx = p.x as i32 - camera_offset;
                    while sx < 0 { sx += DISPLAY_WIDTH; }
                    while sx >= DISPLAY_WIDTH { sx -= DISPLAY_WIDTH; }
                    let sy = p.y as i32;
                    if sy >= 0 && sy < DISPLAY_HEIGHT {
                        renderer.draw_pixel(Point::new(sx, sy), true);
                    }
                }
            }
            None => {}
        }
    }

    fn star_offset(&self, ctx: &GameContext) -> i32 {
        // Drift 20px per game-hour and 60px per game-year.
        let time_offset = ((self.elapsed_time % 3600.0) / 3600.0 * 20.0) as i32;
        let season_offset = ((ctx.day_number % 365) as f32 / 365.0 * 60.0) as i32;
        time_offset + season_offset
    }

    fn draw_stars(&self, renderer: &mut Renderer, ctx: &GameContext, camera_offset: i32) {
        let offset_x = self.star_offset(ctx);
        for star in &self.stars {
            let world_x = (star.x as i32 + offset_x).rem_euclid(STAR_FIELD_WIDTH);
            let screen_x = world_x - camera_offset;
            if screen_x < 0 || screen_x >= DISPLAY_WIDTH {
                continue;
            }
            let screen_y = star.y as i32;
            if screen_y < 0 || screen_y >= STAR_FIELD_HEIGHT {
                continue;
            }

            if star.twinkles {
                let phase = (self.twinkle_phase + star.phase_offset) % TWINKLE_CYCLE_LENGTH;
                if phase == TWINKLE_LARGE_PHASE {
                    renderer.draw_pixel(Point::new(screen_x, screen_y), true);
                    renderer.draw_pixel(Point::new(screen_x - 1, screen_y), true);
                    renderer.draw_pixel(Point::new(screen_x + 1, screen_y), true);
                    renderer.draw_pixel(Point::new(screen_x, screen_y - 1), true);
                    renderer.draw_pixel(Point::new(screen_x, screen_y + 1), true);
                } else if TWINKLE_SMALL_PHASES.contains(&phase) {
                    renderer.draw_pixel(Point::new(screen_x, screen_y), true);
                    renderer.draw_pixel(Point::new(screen_x - 1, screen_y), true);
                    renderer.draw_pixel(Point::new(screen_x + 1, screen_y), true);
                } else {
                    renderer.draw_pixel(Point::new(screen_x, screen_y), true);
                }
            } else {
                renderer.draw_pixel(Point::new(screen_x, screen_y), true);
            }
        }
    }

    fn draw_moon(
        &self,
        renderer: &mut Renderer,
        moon_phase: u8,
        hours_f: f32,
        camera_offset: i32,
    ) {
        let frame = MOON_PHASE_FRAMES
            .get(moon_phase as usize)
            .copied()
            .flatten();

        let mh = if hours_f >= MOON_RISE { hours_f } else { hours_f + 24.0 };
        let t = (mh - MOON_RISE) / (MOON_SET - MOON_RISE);
        let x = (MOON_X_START + t * (MOON_X_END - MOON_X_START)) as i32;
        let y = (MOON_Y_HORIZON - (MOON_Y_HORIZON - MOON_Y_PEAK) * 4.0 * t * (1.0 - t)) as i32;

        let screen_x = x - camera_offset;
        if screen_x + MOON.width as i32 <= 0 || screen_x >= DISPLAY_WIDTH {
            return;
        }

        let pos = Point::new(screen_x, y);
        match frame {
            Some(frame_idx) => renderer.draw_sprite(
                &MOON,
                pos,
                SpriteOpts {
                    frame: frame_idx as usize,
                    ..Default::default()
                },
            ),
            None => {
                // New phase: the disc is invisible, but its silhouette still
                // occludes the stars behind it. Draw the fill mask only.
                if let Some(fill_frames) = MOON.fill_frames {
                    renderer.draw_sprite_raw(
                        fill_frames[0],
                        MOON.width,
                        MOON.height,
                        pos,
                        SpriteOpts {
                            transparent: true,
                            transparent_color: true,
                            invert: true,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    fn draw_sun(&self, renderer: &mut Renderer, hours_f: f32, camera_offset: i32, temperature: f32) {
        let t = (hours_f - SUN_RISE) / (SUN_SET - SUN_RISE);
        let x = (SUN_X_START + t * (SUN_X_END - SUN_X_START)) as i32;
        let y = (SUN_Y_HORIZON - (SUN_Y_HORIZON - SUN_Y_PEAK) * 4.0 * t * (1.0 - t)) as i32;

        let sprite: &Sprite = if temperature > 30.0 { &SUN_HOT } else { &SUN };
        let screen_x = x - camera_offset;
        if screen_x + sprite.width as i32 <= 0 || screen_x >= DISPLAY_WIDTH {
            return;
        }

        renderer.draw_sprite(
            sprite,
            Point::new(screen_x, y),
            SpriteOpts {
                frame: self.sun_anim_frame,
                ..Default::default()
            },
        );
    }

    fn draw_clouds(&self, renderer: &mut Renderer, camera_offset: i32) {
        for cloud in &self.clouds {
            let screen_x = cloud.x as i32 - camera_offset;
            if screen_x + cloud.sprite.width as i32 <= 0 || screen_x >= DISPLAY_WIDTH {
                continue;
            }
            renderer.draw_sprite(
                cloud.sprite,
                Point::new(screen_x, cloud.y as i32),
                SpriteOpts::default(),
            );
        }
    }
}
