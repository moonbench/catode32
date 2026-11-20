//! Cat Tree, an endless climber. The cat bounces on its own every time it
//! lands on a perch; Left and Right steer it, and it wraps around the sides
//! of the screen. Fall off the bottom and the run is over.
//!
//! Perches come in a few kinds: plain shelves, ones that slide back and
//! forth, cracked ones that break just after a bounce, and up in the clouds,
//! cloud puffs that vanish after one bounce. Now and then a wide rest perch
//! stops the bouncing: the cat can stroll about on it, and A jumps on. Springs give a big bounce, and a
//! propeller hat or jetpack carries the cat up for a while. Goo monsters from
//! Hunter stand on some perches and knock the cat off if it touches them.
//!
//! The sky changes with height as in Balancing Act: clouds drift past, then
//! birds, then stars come out, the moon rises, and shooting stars streak by.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{
        effects::{POOF, POOF_FRAME_DUR},
        items::FISH1,
        minigame_assets::{CATTREE_JETPACK, CATTREE_PROPELLER_HAT, CATTREE_SPRING, CATTREE_SPRING_SPRUNG},
        minigame_character::{PLATFORMER_CAT_JUMP, PLATFORMER_CAT_RUN, PLATFORMER_CAT_SIT, PLATFORMER_SLIME_IDLE, SMALL_BIRD1},
        nature::{CLOUD1, CLOUD2, CLOUD3, MOON},
        plants::{FREESIA_GROWING, FREESIA_YOUNG, GRASS_GROWING, GRASS_MATURE, GRASS_YOUNG, ROSE_YOUNG, SUNFLOWER_YOUNG},
    },
    board::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_range_f32, xorshift32},
    render::{Renderer, Sprite, SpriteOpts},
    scene::{Scene, SceneId},
    sky::ShootingStarEvent,
    ui::burst::{BurstEffect, DEFAULT_STYLE},
    ui::popup::Popup,
};

const DISPLAY_W: i32 = DISPLAY_WIDTH as i32;
const DISPLAY_H: i32 = DISPLAY_HEIGHT as i32;
const WF: f32 = DISPLAY_W as f32;

// World space: y grows downward like the screen, and the ground's top is at
// y = 0, so everything above it is negative. `cam` is the world y of the
// top row of the screen.
const CAM_START: f32 = -((DISPLAY_H - 1) as f32);
/// The camera only moves up, keeping the cat's feet at least this far down
/// the screen.
const FOLLOW_Y: f32 = 34.0;

// Physics, in px and seconds. A normal bounce peaks about 32px up, a spring
// about 88px.
const GRAVITY: f32 = 300.0;
const BOUNCE_VEL: f32 = -140.0;
const SPRING_VEL: f32 = -230.0;
const MAX_FALL: f32 = 220.0;
const MAX_VX: f32 = 80.0;
const ACCEL: f32 = 420.0;
const FRICTION: f32 = 300.0;
/// Top speed strolling about on a rest perch.
const WALK_VX: f32 = 40.0;
const RUN_FPS: f32 = 12.0;
/// How long the cat sits on a rest perch before the A hint shows, and how
/// fast it blinks.
const REST_HINT_DELAY: f32 = 1.0;
const REST_HINT_BLINK: f32 = 0.8;
/// Below this speed either way the cat shows its peak frame, as in Hunter.
const JUMP_PEAK_RANGE: f32 = 50.0;
const DT_MAX: f32 = 0.1;

// Cat hitboxes, centred on `x`. The feet decide landings, the body decides
// pickups and goo.
const FEET_HALF_W: f32 = 5.0;
const BODY_HALF_W: f32 = 6.0;
const BODY_H: f32 = 12.0;
const CAT_WAG_FPS: f32 = 4.0;
const CAT_RISE_FRAME: usize = 0;
const CAT_PEAK_FRAME: usize = 1;
const CAT_FALL_FRAME: usize = 2;
/// Hop and spin-out speed when goo knocks the cat off.
const KNOCK_VEL: f32 = -80.0;

// Flights. The cat ignores perches and goo while flying.
const HAT_TIME: f32 = 1.5;
const HAT_VEL: f32 = -90.0;
const JET_TIME: f32 = 2.5;
const JET_VEL: f32 = -160.0;
/// Left over upward speed as a flight ends.
const FLIGHT_EXIT_VEL: f32 = -60.0;
const PROP_FPS: f32 = 12.0;
/// Where worn items sit on the rising cat frame (facing right), from its
/// top-left: the hat's brim on the ears, the jetpack on the back.
const HAT_ANCHOR: (i32, i32) = (12, -4);
const JET_ANCHOR: (i32, i32) = (6, 4);
const JET_FLAME_FRAMES: [usize; 2] = [1, 2];

// Perches.
const PERCH_H: i32 = 4;
const REST_H: i32 = 6;
const REST_W: (u8, u8) = (36, 44);
const MAX_PERCHES: usize = 28;
/// Highest a chain perch can be above the last one; a bounce peaks at 32px.
const MAX_REACH: f32 = 26.0;
/// Furthest the next chain perch's centre moves sideways, before wrapping.
const MAX_SHIFT: f32 = 40.0;
/// Gaps at least this tall may get an extra perch halfway up.
const EXTRA_MIN_GAP: f32 = 14.0;
const CRUMBLE_DELAY: f32 = 0.25;
const CRUMBLE_SHAKE_PERIOD: f32 = 0.05;
const GOO_PERCH_W: u8 = 30;
/// Room kept clear between a goo perch and the chain perches either side,
/// so there is always a way past it.
const GOO_CLEARANCE: f32 = BODY_HALF_W + 4.0;
const GOO_SPEED: f32 = 6.0;
const GOO_ANIM_SPF: f32 = 0.5;
/// Goo hitbox inside its 18x10 sprite.
const GOO_INSET_X: f32 = 3.0;
const GOO_INSET_TOP: f32 = 2.0;
/// Cloud perches show up at these heights (px above the ground), and this
/// share of chain perches there are clouds.
const CLOUD_PERCHES_FROM: f32 = 150.0;
const CLOUD_PERCHES_TO: f32 = 650.0;
const CLOUD_PERCH_PERCENT: u32 = 35;
const CLOUD_BUMP_R: i32 = 3;
const CLOUD_BUMP_STEP: i32 = 5;

// Score.
const PX_PER_POINT: f32 = 4.0;
const FISH_POINTS: u32 = 10;
const FISH_BOB_PERIOD: f32 = 1.2;
const FISH_SPARKLES: usize = 4;
const FISH_SPARKLE_STAGGER: f32 = 0.06;

const MAX_POOFS: usize = 3;
/// Pause after the poof before the result popup.
const RESULT_PAUSE: f32 = 0.3;

/// How the climb changes with height. Each stage applies from `from` px
/// above the ground. Percentages are per chain perch, except `goo`, which is
/// the share of extra perches that carry goo.
struct Stage {
    from: f32,
    gap: (f32, f32),
    width: (u8, u8),
    slide: u32,
    slide_speed: f32,
    extra: u32,
    goo: u32,
    rest: u32,
    spring: u32,
    hat: u32,
    jet: u32,
    fish: u32,
}

const STAGES: [Stage; 5] = [
    Stage { from: 0.0,    gap: (10.0, 15.0), width: (22, 28), slide: 0,  slide_speed: 0.0,  extra: 30, goo: 0,  rest: 0, spring: 6, hat: 2, jet: 0, fish: 10 },
    Stage { from: 200.0,  gap: (12.0, 18.0), width: (20, 26), slide: 15, slide_speed: 16.0, extra: 40, goo: 35, rest: 5, spring: 6, hat: 4, jet: 3, fish: 10 },
    Stage { from: 600.0,  gap: (14.0, 21.0), width: (18, 24), slide: 25, slide_speed: 22.0, extra: 45, goo: 45, rest: 5, spring: 5, hat: 4, jet: 4, fish: 10 },
    Stage { from: 1200.0, gap: (16.0, 23.0), width: (16, 22), slide: 35, slide_speed: 28.0, extra: 50, goo: 50, rest: 6, spring: 5, hat: 3, jet: 4, fish: 10 },
    Stage { from: 2000.0, gap: (18.0, 26.0), width: (14, 20), slide: 45, slide_speed: 34.0, extra: 55, goo: 55, rest: 6, spring: 5, hat: 3, jet: 4, fish: 10 },
];

// Sky, borrowed from Balancing Act. Heights are how far the camera has
// risen, in px; scenery scrolls at a fraction of that so it reads as far
// away.
const SKY_PARALLAX: f32 = 0.5;
const MAX_CLOUDS: usize = 5;
const CLOUD_FIRST: f32 = 100.0;
const CLOUD_LAST: f32 = 600.0;
const CLOUD_DRIFT_MAX: f32 = 5.0;
const GRASS_SPRITES: [&Sprite; 3] = [&GRASS_YOUNG, &GRASS_GROWING, &GRASS_MATURE];
const FLOWER_SPRITES: [&Sprite; 4] = [&FREESIA_YOUNG, &FREESIA_GROWING, &ROSE_YOUNG, &SUNFLOWER_YOUNG];
const PLANT_MAX_H: i32 = 17;
const MAX_PLANTS: usize = 10;
const PLANT_GAP_MIN: u32 = 3;
const PLANT_GAP_MAX: u32 = 14;
const MAX_BIRDS: usize = 1;
const BIRDS_FROM: f32 = 300.0;
const BIRD_WAIT_MIN: f32 = 6.0;
const BIRD_WAIT_MAX: f32 = 15.0;
const BIRD_CHANCE: f32 = 0.6;
const BIRD_SPEED_MIN: f32 = 32.0;
const BIRD_SPEED_MAX: f32 = 48.0;
const BIRD_Y_MIN: f32 = 10.0;
const BIRD_Y_MAX: f32 = 34.0;
const BIRD_ANIM_FPS: f32 = 8.0;
const SHOOTING_STARS_FROM: f32 = 1800.0;
const SHOOTING_STARS_MORE_FROM: f32 = 3000.0;
const SHOOTING_STAR_ODDS: (f32, f32, f32) = (12.0, 30.0, 0.4);
const SHOOTING_STAR_MORE_ODDS: (f32, f32, f32) = (6.0, 18.0, 0.65);
const MAX_STARS: usize = 48;
/// Stars fade in between these heights, and scroll slower than clouds.
const STARS_FROM: f32 = 700.0;
const STARS_FULL: f32 = 1200.0;
const STAR_PARALLAX: f32 = 0.1;
const TWINKLE_RATIO_PERCENT: u32 = 20;
const TWINKLE_CYCLE_LENGTH: u8 = 12;
const TWINKLE_LARGE_PHASE: u8 = 10;
const TWINKLE_SMALL_PHASES: [u8; 3] = [8, 9, 11];
const TWINKLE_PERIOD: f32 = 0.3;
/// The moon drifts down into view around this height, then stays put.
const MOON_HEIGHT: f32 = 1400.0;
const MOON_X: i32 = 100;
const MOON_PARK_Y: i32 = 9;
const MOON_FULL_FRAME: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Playing,
    /// The cat has dropped off the bottom of the screen; the poof plays.
    Dying,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Solid,
    Sliding,
    Crumbly,
    Cloud,
    /// Wide and thick; the cat stops bouncing here until A is pressed.
    Rest,
}

impl Kind {
    fn height(self) -> i32 {
        if self == Kind::Rest { REST_H } else { PERCH_H }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Life {
    Intact,
    /// Bounced on; breaks in a poof once the timer runs out.
    Cracking(f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemKind {
    Spring,
    Hat,
    Jetpack,
    Fish,
}

impl ItemKind {
    fn sprite(self) -> &'static Sprite {
        match self {
            ItemKind::Spring => &CATTREE_SPRING,
            ItemKind::Hat => &CATTREE_PROPELLER_HAT,
            ItemKind::Jetpack => &CATTREE_JETPACK,
            ItemKind::Fish => &FISH1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Item {
    kind: ItemKind,
    /// Left edge, from the perch's left edge.
    dx: u8,
    sprung: bool,
}

#[derive(Clone, Copy, Debug)]
struct Goo {
    /// Left edge of the sprite, from the perch's left edge.
    dx: f32,
    dir: f32,
}

#[derive(Clone, Copy, Debug)]
struct Perch {
    x: f32,
    /// World y of the top surface.
    y: f32,
    w: u8,
    kind: Kind,
    life: Life,
    vx: f32,
    item: Option<Item>,
    goo: Option<Goo>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlightKind {
    Hat,
    Jetpack,
}

#[derive(Clone, Copy)]
struct Flight {
    kind: FlightKind,
    left: f32,
}

/// A hat or jetpack tumbling away after its flight.
struct Dropped {
    kind: ItemKind,
    x: f32,
    y: f32,
    vy: f32,
}

struct Cloud {
    sprite: &'static Sprite,
    x: f32,
    y: f32,
    vx: f32,
}

struct Bird {
    x: f32,
    y: f32,
    speed: f32,
    anim: f32,
}

struct Plant {
    sprite: &'static Sprite,
    x: i16,
    mirror: bool,
}

#[derive(Clone, Copy)]
struct Star {
    x: u8,
    y: u8,
    twinkles: bool,
    phase_offset: u8,
}

fn stage_at(height: f32) -> &'static Stage {
    STAGES.iter().rev().find(|s| height >= s.from).unwrap_or(&STAGES[0])
}

fn wrap_x(x: f32) -> f32 {
    if x < 0.0 {
        x + WF
    } else if x >= WF {
        x - WF
    } else {
        x
    }
}

/// Whether spans `a` and `b` overlap, with `a` allowed to wrap around the
/// sides of the screen.
fn overlaps_wrapped(a0: f32, a1: f32, b0: f32, b1: f32) -> bool {
    [-WF, 0.0, WF].iter().any(|o| a0 + o < b1 && b0 < a1 + o)
}

/// Whether feet centred on `x`, moving from `prev_feet` down to `feet`,
/// crossed the top of a perch spanning `px..px + pw` at `top`. Swept, so a
/// fast fall can't skip through a thin perch between frames.
fn lands_on(prev_feet: f32, feet: f32, x: f32, px: f32, pw: f32, top: f32) -> bool {
    feet > prev_feet
        && prev_feet <= top
        && feet >= top
        && overlaps_wrapped(x - FEET_HALF_W, x + FEET_HALF_W, px, px + pw)
}

fn percent(rng: &mut u32) -> u32 {
    xorshift32(rng) % 100
}

fn rand_u8(rng: &mut u32, lo: u8, hi: u8) -> u8 {
    lo + (xorshift32(rng) % (hi - lo + 1) as u32) as u8
}

pub struct CatTreeScene {
    state: State,

    // The cat. `x` is its centre, `feet` the world y it stands on.
    x: f32,
    feet: f32,
    vx: f32,
    vy: f32,
    facing_right: bool,
    flight: Option<Flight>,
    knocked: bool,
    /// Standing on a rest perch, and for how long.
    resting: bool,
    rest_time: f32,
    cat_frame: usize,
    cat_timer: f32,

    cam: f32,
    perches: Vec<Perch, MAX_PERCHES>,
    /// Top and centre of the newest chain perch; the next one goes above.
    next_y: f32,
    chain_cx: f32,
    chain_w: u8,
    /// Whether the newest chain perch is a rest perch; two never come in a row.
    chain_rest: bool,
    dropped: Option<Dropped>,
    poofs: Vec<(Point, f32), MAX_POOFS>,
    /// Where the cat poofed after dropping off the screen, and when.
    death_poof: Option<(Point, f32)>,
    sparkles: BurstEffect,
    sparkle_at: Point,
    /// Drives the propeller, flame, goo and fish animations.
    anim: f32,

    best_height: f32,
    fish: u32,
    fish_total: u32,
    session_best: u32,
    /// Points scored over all finished runs this visit.
    session_total: u32,
    result_popup: Popup,

    clouds: Vec<Cloud, MAX_CLOUDS>,
    plants: Vec<Plant, MAX_PLANTS>,
    birds: Vec<Bird, MAX_BIRDS>,
    bird_wait: f32,
    shooting_star: Option<ShootingStarEvent>,
    shooting_star_wait: f32,
    stars: [Star; MAX_STARS],
    twinkle_timer: f32,
    twinkle_phase: u8,
}

impl CatTreeScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,

            x: WF / 2.0,
            feet: 0.0,
            vx: 0.0,
            vy: 0.0,
            facing_right: true,
            flight: None,
            knocked: false,
            resting: false,
            rest_time: 0.0,
            cat_frame: 0,
            cat_timer: 0.0,

            cam: CAM_START,
            perches: Vec::new(),
            next_y: 0.0,
            chain_cx: WF / 2.0,
            chain_w: DISPLAY_W as u8,
            chain_rest: false,
            dropped: None,
            poofs: Vec::new(),
            death_poof: None,
            sparkles: BurstEffect::new(),
            sparkle_at: Point::zero(),
            anim: 0.0,

            best_height: 0.0,
            fish: 0,
            fish_total: 0,
            session_best: 0,
            session_total: 0,
            result_popup: Popup::new(14, 13, 100, 38),

            clouds: Vec::new(),
            plants: Vec::new(),
            birds: Vec::new(),
            bird_wait: BIRD_WAIT_MIN,
            shooting_star: None,
            shooting_star_wait: SHOOTING_STAR_ODDS.0,
            stars: [Star { x: 0, y: 0, twinkles: false, phase_offset: 0 }; MAX_STARS],
            twinkle_timer: 0.0,
            twinkle_phase: 0,
        }
    }

    fn reset_round(&mut self, rng: &mut u32) {
        self.x = WF / 2.0;
        self.feet = 0.0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.facing_right = true;
        self.flight = None;
        self.knocked = false;
        self.resting = false;
        self.cam = CAM_START;
        self.perches.clear();
        self.next_y = 0.0;
        self.chain_cx = WF / 2.0;
        // The ground counts as the first chain perch.
        self.chain_w = DISPLAY_W as u8;
        self.chain_rest = false;
        self.dropped = None;
        self.poofs.clear();
        self.death_poof = None;
        self.sparkles = BurstEffect::new();
        self.best_height = 0.0;
        self.fish = 0;
        self.spawn_ahead(rng);
        self.generate_sky(rng);
        self.generate_plants(rng);
        self.birds.clear();
        self.bird_wait = rand_range_f32(rng, BIRD_WAIT_MIN, BIRD_WAIT_MAX);
        self.shooting_star = None;
        self.shooting_star_wait = rand_range_f32(rng, SHOOTING_STAR_ODDS.0, SHOOTING_STAR_ODDS.1);
    }

    /// How far the camera has risen from the ground.
    fn lift(&self) -> f32 {
        CAM_START - self.cam
    }

    fn score(&self) -> u32 {
        (self.best_height / PX_PER_POINT) as u32 + self.fish * FISH_POINTS
    }

    // -----------------------------------------------------------------------
    // Perch generation
    // -----------------------------------------------------------------------

    /// Keep perches generated up to a screen above the top of the view.
    fn spawn_ahead(&mut self, rng: &mut u32) {
        while self.next_y > self.cam - DISPLAY_H as f32 && self.perches.len() + 2 <= MAX_PERCHES {
            self.spawn_chain(rng);
        }
    }

    /// The next perch on the path up, always within a bounce of the last,
    /// and maybe an extra one halfway between them.
    fn spawn_chain(&mut self, rng: &mut u32) {
        let height = -self.next_y;
        let st = stage_at(height);
        let gap = rand_range_f32(rng, st.gap.0, st.gap.1).min(MAX_REACH).round();
        let y = self.next_y - gap;

        let in_clouds = (CLOUD_PERCHES_FROM..CLOUD_PERCHES_TO).contains(&-y);
        let kind = if in_clouds && percent(rng) < CLOUD_PERCH_PERCENT {
            Kind::Cloud
        } else if !self.chain_rest && percent(rng) < st.rest {
            Kind::Rest
        } else if percent(rng) < st.slide {
            Kind::Sliding
        } else {
            Kind::Solid
        };
        let w = if kind == Kind::Rest {
            rand_u8(rng, REST_W.0, REST_W.1)
        } else {
            rand_u8(rng, st.width.0, st.width.1)
        };

        let cx = wrap_x(self.chain_cx + rand_range_f32(rng, -MAX_SHIFT, MAX_SHIFT));
        let x = (cx - w as f32 / 2.0).clamp(0.0, WF - w as f32).round();
        let vx = if kind == Kind::Sliding {
            if xorshift32(rng) % 2 == 0 { st.slide_speed } else { -st.slide_speed }
        } else {
            0.0
        };

        let roll = percent(rng);
        let mut cut = 0;
        let mut pick = |chance: u32| {
            cut += chance;
            roll < cut
        };
        // Clouds and rest perches only ever hold fish.
        let gear = matches!(kind, Kind::Solid | Kind::Sliding);
        let item_kind = if gear && pick(st.spring) {
            Some(ItemKind::Spring)
        } else if gear && pick(st.hat) {
            Some(ItemKind::Hat)
        } else if gear && pick(st.jet) {
            Some(ItemKind::Jetpack)
        } else if pick(st.fish) {
            Some(ItemKind::Fish)
        } else {
            None
        };
        let item = item_kind.map(|kind| {
            let room = w.saturating_sub(kind.sprite().width as u8);
            Item { kind, dx: rand_u8(rng, 0, room), sprung: false }
        });

        let chain = Perch { x, y, w, kind, life: Life::Intact, vx, item, goo: None };

        // Nothing squeezed in under a rest perch's thick underside.
        if kind != Kind::Rest && gap >= EXTRA_MIN_GAP && percent(rng) < st.extra {
            let prev = (self.chain_cx, self.chain_w);
            self.spawn_extra(rng, st, y + (gap / 2.0).round(), prev, (x + w as f32 / 2.0, w));
        }

        let _ = self.perches.push(chain);
        self.next_y = y;
        self.chain_cx = x + w as f32 / 2.0;
        self.chain_w = w;
        self.chain_rest = kind == Kind::Rest;
    }

    /// An optional perch between two chain perches: crumbly, or carrying goo.
    /// Goo perches keep clear of the chain perches above and below so the
    /// cat can always climb straight past.
    fn spawn_extra(&mut self, rng: &mut u32, st: &Stage, y: f32, below: (f32, u8), above: (f32, u8)) {
        if percent(rng) < st.goo {
            let w = GOO_PERCH_W;
            let clear = |x: f32| {
                [below, above].iter().all(|&(cx, cw)| {
                    let half = cw as f32 / 2.0 + GOO_CLEARANCE;
                    !overlaps_wrapped(cx - half, cx + half, x, x + w as f32)
                })
            };
            for _ in 0..4 {
                let x = rand_range_f32(rng, 0.0, WF - w as f32).round();
                if clear(x) {
                    let room = (w - PLATFORMER_SLIME_IDLE.width as u8) as f32;
                    let goo = Goo {
                        dx: rand_range_f32(rng, 0.0, room),
                        dir: if xorshift32(rng) % 2 == 0 { 1.0 } else { -1.0 },
                    };
                    let _ = self.perches.push(Perch {
                        x, y, w, kind: Kind::Solid, life: Life::Intact, vx: 0.0,
                        item: None, goo: Some(goo),
                    });
                    return;
                }
            }
        } else {
            let w = rand_u8(rng, st.width.0, st.width.1);
            let x = rand_range_f32(rng, 0.0, WF - w as f32).round();
            let _ = self.perches.push(Perch {
                x, y, w, kind: Kind::Crumbly, life: Life::Intact, vx: 0.0,
                item: None, goo: None,
            });
        }
    }

    fn cull_perches(&mut self) {
        let bottom = self.cam + DISPLAY_H as f32;
        self.perches.retain(|p| p.y < bottom + REST_H as f32);
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    fn update_perches(&mut self, dt: f32) {
        for p in self.perches.iter_mut() {
            if p.vx != 0.0 {
                let max_x = WF - p.w as f32;
                p.x += p.vx * dt;
                if p.x < 0.0 {
                    p.x = -p.x;
                    p.vx = -p.vx;
                } else if p.x > max_x {
                    p.x = 2.0 * max_x - p.x;
                    p.vx = -p.vx;
                }
            }
            if let Some(g) = &mut p.goo {
                let room = (p.w - PLATFORMER_SLIME_IDLE.width as u8) as f32;
                g.dx += g.dir * GOO_SPEED * dt;
                if g.dx < 0.0 {
                    g.dx = 0.0;
                    g.dir = 1.0;
                } else if g.dx > room {
                    g.dx = room;
                    g.dir = -1.0;
                }
            }
            if let Life::Cracking(t) = &mut p.life {
                *t += dt;
            }
        }
        let mut i = 0;
        while i < self.perches.len() {
            let p = &self.perches[i];
            if matches!(p.life, Life::Cracking(t) if t >= CRUMBLE_DELAY) {
                let at = Point::new(p.x as i32 + p.w as i32 / 2, p.y as i32);
                self.perches.remove(i);
                push_poof(&mut self.poofs, at);
            } else {
                i += 1;
            }
        }

        if let Some(d) = &mut self.dropped {
            d.vy += GRAVITY * dt;
            d.y += d.vy * dt;
            if d.y - self.cam > DISPLAY_H as f32 {
                self.dropped = None;
            }
        }
        for (_, t) in self.poofs.iter_mut() {
            *t += dt;
        }
        self.poofs.retain(|(_, t)| *t < POOF_FRAME_DUR * POOF.frames.len() as f32);
    }

    fn steer(&mut self, buttons: &Buttons, dt: f32, max_vx: f32) {
        let left = buttons.is_pressed(Button::Left);
        let right = buttons.is_pressed(Button::Right);
        if left && !right {
            self.vx = (self.vx - ACCEL * dt).max(-max_vx);
            self.facing_right = false;
        } else if right && !left {
            self.vx = (self.vx + ACCEL * dt).min(max_vx);
            self.facing_right = true;
        } else if self.vx > 0.0 {
            self.vx = (self.vx - FRICTION * dt).max(0.0);
        } else {
            self.vx = (self.vx + FRICTION * dt).min(0.0);
        }
        self.x = wrap_x(self.x + self.vx * dt);
    }

    fn update_play(&mut self, buttons: &mut Buttons, rng: &mut u32, dt: f32) {
        self.steer(buttons, dt, if self.resting { WALK_VX } else { MAX_VX });
        if self.resting {
            if buttons.was_just_pressed(Button::A) {
                self.resting = false;
                self.vy = BOUNCE_VEL;
            } else if !self.on_rest_perch() {
                // Strolled off the end.
                self.resting = false;
                self.vy = 0.0;
            }
        }

        let prev = self.feet;
        if self.resting {
            self.rest_time += dt;
        } else if let Some(f) = &mut self.flight {
            f.left -= dt;
            self.vy = match f.kind {
                FlightKind::Hat => HAT_VEL,
                FlightKind::Jetpack => JET_VEL,
            };
            if f.left <= 0.0 {
                self.end_flight();
            }
            self.feet += self.vy * dt;
        } else {
            // Exact for constant gravity, so a bounce peaks at the same
            // height whatever the frame rate.
            let vy = (self.vy + GRAVITY * dt).min(MAX_FALL);
            self.feet += (self.vy + vy) / 2.0 * dt;
            self.vy = vy;
        }

        if self.flight.is_none() && !self.knocked {
            if !self.resting {
                self.check_landing(prev);
            }
            self.check_touches(rng);
        }

        self.cam = self.cam.min(self.feet - FOLLOW_Y);
        self.best_height = self.best_height.max(-self.feet);
        self.session_best = self.session_best.max(self.score());

        // Whole body off the bottom of the screen: poof just inside it. The
        // ground or a perch just out of sight can still catch it before then.
        if self.feet - BODY_H - self.cam > DISPLAY_H as f32 {
            self.state = State::Dying;
            self.session_total += self.score();
            let at = Point::new(self.x as i32, (self.cam as i32) + DISPLAY_H - POOF.height as i32 / 2);
            self.death_poof = Some((at, 0.0));
        }
    }

    /// Whether the feet are still over the rest perch the cat landed on.
    fn on_rest_perch(&self) -> bool {
        self.perches.iter().any(|p| {
            p.kind == Kind::Rest
                && p.y == self.feet
                && overlaps_wrapped(self.x - FEET_HALF_W, self.x + FEET_HALF_W, p.x, p.x + p.w as f32)
        })
    }

    /// Bounce off the first surface the feet crossed on the way down.
    fn check_landing(&mut self, prev: f32) {
        let mut hit: Option<usize> = None;
        let mut hit_y = f32::MAX;
        for (i, p) in self.perches.iter().enumerate() {
            if p.y < hit_y && lands_on(prev, self.feet, self.x, p.x, p.w as f32, p.y) {
                hit = Some(i);
                hit_y = p.y;
            }
        }
        // Perches are all above the ground, so any perch crossed comes first.
        let Some(i) = hit else {
            if lands_on(prev, self.feet, self.x, 0.0, WF, 0.0) {
                self.feet = 0.0;
                self.vy = BOUNCE_VEL;
            }
            return;
        };

        self.feet = hit_y;
        self.vy = BOUNCE_VEL;
        let (x, w) = (self.x, self.perches[i].w);
        let p = &mut self.perches[i];
        if let Some(item) = &mut p.item {
            let ix = p.x + item.dx as f32;
            let iw = CATTREE_SPRING.width as f32;
            if item.kind == ItemKind::Spring && overlaps_wrapped(x - FEET_HALF_W, x + FEET_HALF_W, ix, ix + iw) {
                item.sprung = true;
                self.vy = SPRING_VEL;
            }
        }
        match p.kind {
            Kind::Crumbly if p.life == Life::Intact => p.life = Life::Cracking(0.0),
            Kind::Cloud => {
                let at = Point::new(p.x as i32 + w as i32 / 2, p.y as i32);
                self.perches.remove(i);
                push_poof(&mut self.poofs, at);
            }
            Kind::Rest => {
                self.vy = 0.0;
                self.vx = 0.0;
                self.resting = true;
                self.rest_time = 0.0;
            }
            _ => {}
        }
    }

    /// Pickups and goo, by overlap with the cat's body.
    fn check_touches(&mut self, rng: &mut u32) {
        let (b0, b1) = (self.x - BODY_HALF_W, self.x + BODY_HALF_W);
        let (top, bottom) = (self.feet - BODY_H, self.feet);
        let mut knocked = false;
        let mut flight = None;
        let mut fish_at = None;
        for p in self.perches.iter_mut() {
            if let Some(g) = p.goo {
                let gx = p.x + g.dx;
                let gw = PLATFORMER_SLIME_IDLE.width as f32;
                let gtop = p.y - PLATFORMER_SLIME_IDLE.height as f32 + GOO_INSET_TOP;
                if top < p.y && gtop < bottom
                    && overlaps_wrapped(b0, b1, gx + GOO_INSET_X, gx + gw - GOO_INSET_X)
                {
                    knocked = true;
                }
            }
            let Some(item) = p.item else { continue };
            let sprite = item.kind.sprite();
            let ix = p.x + item.dx as f32;
            let itop = p.y - sprite.height as f32;
            if item.kind == ItemKind::Spring || !(top < p.y && itop < bottom) {
                continue;
            }
            if !overlaps_wrapped(b0, b1, ix, ix + sprite.width as f32) {
                continue;
            }
            p.item = None;
            match item.kind {
                ItemKind::Hat => flight = Some(Flight { kind: FlightKind::Hat, left: HAT_TIME }),
                ItemKind::Jetpack => flight = Some(Flight { kind: FlightKind::Jetpack, left: JET_TIME }),
                ItemKind::Fish => fish_at = Some(Point::new((ix + sprite.width as f32 / 2.0) as i32, itop as i32)),
                ItemKind::Spring => {}
            }
        }

        if let Some(at) = fish_at {
            self.fish += 1;
            self.fish_total += 1;
            self.sparkle_at = at;
            self.sparkles.trigger_with(rng, FISH_SPARKLES, DEFAULT_STYLE, -8.0, 8.0, -8.0, 2.0, FISH_SPARKLE_STAGGER);
        }
        if flight.is_some() {
            self.flight = flight;
            self.resting = false;
        } else if knocked {
            self.resting = false;
            self.knocked = true;
            self.vy = KNOCK_VEL;
            self.vx *= 0.5;
        }
    }

    fn end_flight(&mut self) {
        if let Some(f) = self.flight.take() {
            let kind = match f.kind {
                FlightKind::Hat => ItemKind::Hat,
                FlightKind::Jetpack => ItemKind::Jetpack,
            };
            let (dx, dy) = self.worn_offset(f.kind);
            let (x, _) = self.cat_origin();
            let y = self.feet - PLATFORMER_CAT_JUMP.height as f32 + dy as f32;
            self.dropped = Some(Dropped { kind, x: (x + dx) as f32, y, vy: 0.0 });
        }
        self.vy = FLIGHT_EXIT_VEL;
    }

    fn update_dying(&mut self, dt: f32) {
        if let Some((_, t)) = &mut self.death_poof {
            *t += dt;
            if *t >= POOF_FRAME_DUR * POOF.frames.len() as f32 + RESULT_PAUSE {
                self.state = State::GameOver;
                self.set_result_text();
            }
        }
    }

    fn set_result_text(&mut self) {
        let mut text: String<64> = String::new();
        let mut n: String<8> = String::new();
        let _ = write!(n, "{}", self.score());
        crate::i18n::substitute(&mut text, t!("\n\nScore: {n}"), &[("n", n.as_str())]);
        let _ = text.push('\n');
        n.clear();
        let _ = write!(n, "{}", self.session_best);
        crate::i18n::substitute(&mut text, t!("Best: {n}"), &[("n", n.as_str())]);
        let _ = text.push('\n');
        let _ = text.push_str(t!("A: RETRY"));
        self.result_popup.set_text(text.trim_start_matches('\n'), false, true);
    }

    fn start_run(&mut self) {
        self.state = State::Playing;
        self.vy = BOUNCE_VEL;
    }

    // -----------------------------------------------------------------------
    // Sky
    // -----------------------------------------------------------------------

    fn generate_plants(&mut self, rng: &mut u32) {
        self.plants.clear();
        let mut x = (xorshift32(rng) % PLANT_GAP_MAX) as i32;
        let mut flower = xorshift32(rng) % 2 == 0;
        while x < DISPLAY_W && !self.plants.is_full() {
            let pool: &[&'static Sprite] = if flower { &FLOWER_SPRITES } else { &GRASS_SPRITES };
            let sprite = pool[xorshift32(rng) as usize % pool.len()];
            if x + sprite.width as i32 > DISPLAY_W {
                break;
            }
            let mirror = xorshift32(rng) % 2 == 0;
            let _ = self.plants.push(Plant { sprite, x: x as i16, mirror });
            flower = !flower;
            x += sprite.width as i32 + (PLANT_GAP_MIN + xorshift32(rng) % (PLANT_GAP_MAX - PLANT_GAP_MIN + 1)) as i32;
        }
    }

    fn generate_sky(&mut self, rng: &mut u32) {
        self.clouds.clear();
        let band = (CLOUD_LAST - CLOUD_FIRST) / MAX_CLOUDS as f32;
        for i in 0..MAX_CLOUDS {
            let sprite: &'static Sprite = match xorshift32(rng) % 6 {
                0 => &CLOUD1,
                1 | 2 => &CLOUD2,
                _ => &CLOUD3,
            };
            // Just above the top of the screen at this height.
            let h = CLOUD_FIRST + band * (i as f32 + rand_range_f32(rng, 0.0, 1.0));
            let y = -(sprite.height as f32) - h * SKY_PARALLAX;
            let x = rand_range_f32(rng, -(sprite.width as f32) / 2.0, WF - sprite.width as f32 / 2.0);
            let mut vx = rand_range_f32(rng, 1.0, CLOUD_DRIFT_MAX);
            if xorshift32(rng) % 2 == 0 {
                vx = -vx;
            }
            let _ = self.clouds.push(Cloud { sprite, x, y, vx });
        }

        for star in self.stars.iter_mut() {
            *star = Star {
                x: (xorshift32(rng) % DISPLAY_W as u32) as u8,
                y: (xorshift32(rng) % DISPLAY_H as u32) as u8,
                twinkles: xorshift32(rng) % 100 < TWINKLE_RATIO_PERCENT,
                phase_offset: (xorshift32(rng) % TWINKLE_CYCLE_LENGTH as u32) as u8,
            };
        }
    }

    fn sky_y(&self, y: f32) -> i32 {
        (y + self.lift() * SKY_PARALLAX) as i32
    }

    fn update_birds(&mut self, rng: &mut u32, dt: f32) {
        for b in self.birds.iter_mut() {
            b.x -= b.speed * dt;
            b.anim += BIRD_ANIM_FPS * dt;
        }
        self.birds.retain(|b| b.x > -(SMALL_BIRD1.width as f32));

        if !(BIRDS_FROM..STARS_FROM).contains(&self.lift()) || self.birds.is_full() {
            return;
        }
        self.bird_wait -= dt;
        if self.bird_wait > 0.0 {
            return;
        }
        self.bird_wait = rand_range_f32(rng, BIRD_WAIT_MIN, BIRD_WAIT_MAX);
        if rand_range_f32(rng, 0.0, 1.0) < BIRD_CHANCE {
            let screen_y = rand_range_f32(rng, BIRD_Y_MIN, BIRD_Y_MAX);
            let _ = self.birds.push(Bird {
                x: WF,
                y: screen_y - self.lift() * SKY_PARALLAX,
                speed: rand_range_f32(rng, BIRD_SPEED_MIN, BIRD_SPEED_MAX),
                anim: 0.0,
            });
        }
    }

    fn update_shooting_star(&mut self, rng: &mut u32, dt: f32) {
        if let Some(ss) = &mut self.shooting_star {
            ss.update(dt);
            if !ss.is_active() {
                self.shooting_star = None;
            }
            return;
        }
        let lift = self.lift();
        if lift < SHOOTING_STARS_FROM {
            return;
        }
        self.shooting_star_wait -= dt;
        if self.shooting_star_wait > 0.0 {
            return;
        }
        let (wait_min, wait_max, chance) = if lift >= SHOOTING_STARS_MORE_FROM {
            SHOOTING_STAR_MORE_ODDS
        } else {
            SHOOTING_STAR_ODDS
        };
        self.shooting_star_wait = rand_range_f32(rng, wait_min, wait_max);
        if rand_range_f32(rng, 0.0, 1.0) < chance {
            self.shooting_star = Some(ShootingStarEvent::random(rng));
        }
    }

    fn update_ambient(&mut self, dt: f32) {
        self.anim += dt;
        self.cat_timer += dt;
        if self.cat_timer >= 1.0 / CAT_WAG_FPS {
            self.cat_timer -= 1.0 / CAT_WAG_FPS;
            self.cat_frame = (self.cat_frame + 1) % PLATFORMER_CAT_SIT.frames.len();
        }

        for c in self.clouds.iter_mut() {
            let w = c.sprite.width as f32;
            c.x += c.vx * dt;
            if c.x > WF {
                c.x = -w;
            } else if c.x < -w {
                c.x = WF;
            }
        }

        self.twinkle_timer += dt;
        if self.twinkle_timer > TWINKLE_PERIOD {
            self.twinkle_timer = 0.0;
            self.twinkle_phase = (self.twinkle_phase + 1) % TWINKLE_CYCLE_LENGTH;
        }

        self.sparkles.update(dt);
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        let mut total = self.session_total;
        if matches!(self.state, State::Ready | State::Playing) {
            total += self.score();
        }
        if total == 0 {
            return;
        }
        // A decent run reaches the clouds, around 100-150 points.
        let progress = (total as f32 / 150.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Fitness,     5.0 * progress),
            (StatId::Playfulness, 4.0 * progress),
            (StatId::Courage,     2.0 * progress),
            (StatId::Focus,       1.0 * progress),
        ]);
        let coins = (4.0 * progress) as i32 + (self.fish_total / 3) as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }

    // -----------------------------------------------------------------------
    // Drawing
    // -----------------------------------------------------------------------

    fn screen_y(&self, world_y: f32) -> i32 {
        (world_y - self.cam).round() as i32
    }

    fn draw_sky(&self, r: &mut Renderer) {
        let lift = self.lift();
        let visible = (((lift - STARS_FROM) / (STARS_FULL - STARS_FROM)).clamp(0.0, 1.0) * MAX_STARS as f32) as usize;
        let drift = (lift * STAR_PARALLAX) as i32;
        for star in &self.stars[..visible] {
            let x = star.x as i32;
            let y = (star.y as i32 + drift).rem_euclid(DISPLAY_H);
            r.draw_pixel(Point::new(x, y), true);
            if !star.twinkles {
                continue;
            }
            let phase = (self.twinkle_phase + star.phase_offset) % TWINKLE_CYCLE_LENGTH;
            if phase == TWINKLE_LARGE_PHASE || TWINKLE_SMALL_PHASES.contains(&phase) {
                r.draw_pixel(Point::new(x - 1, y), true);
                r.draw_pixel(Point::new(x + 1, y), true);
            }
            if phase == TWINKLE_LARGE_PHASE {
                r.draw_pixel(Point::new(x, y - 1), true);
                r.draw_pixel(Point::new(x, y + 1), true);
            }
        }

        if let Some(ss) = &self.shooting_star {
            ss.draw(r);
        }

        let moon_y = (MOON_PARK_Y - ((MOON_HEIGHT - lift) * SKY_PARALLAX) as i32).min(MOON_PARK_Y);
        if moon_y > -(MOON.height as i32) {
            let opts = SpriteOpts { frame: MOON_FULL_FRAME, ..SpriteOpts::default() };
            r.draw_sprite(&MOON, Point::new(MOON_X, moon_y), opts);
        }

        for c in &self.clouds {
            let y = self.sky_y(c.y);
            if y < DISPLAY_H && y + c.sprite.height as i32 > 0 {
                r.draw_sprite(c.sprite, Point::new(c.x as i32, y), SpriteOpts::default());
            }
        }

        for b in &self.birds {
            let frame = b.anim as usize % SMALL_BIRD1.frames.len();
            let pos = Point::new(b.x as i32, self.sky_y(b.y));
            r.draw_sprite(&SMALL_BIRD1, pos, SpriteOpts { frame, ..SpriteOpts::default() });
        }
    }

    fn draw_ground(&self, r: &mut Renderer) {
        let ground = self.screen_y(0.0);
        if ground - PLANT_MAX_H >= DISPLAY_H {
            return;
        }
        for p in &self.plants {
            let pos = Point::new(p.x as i32, ground + 1 - p.sprite.height as i32);
            r.draw_sprite(p.sprite, pos, SpriteOpts { mirror_h: p.mirror, ..SpriteOpts::default() });
        }
        if ground < DISPLAY_H {
            r.draw_line(Point::new(0, ground), Point::new(DISPLAY_W - 1, ground));
        }
    }

    fn draw_perches(&self, r: &mut Renderer) {
        for p in &self.perches {
            let y = self.screen_y(p.y);
            if y >= DISPLAY_H || y + p.kind.height() + PLANT_MAX_H < 0 {
                continue;
            }
            let mut x = p.x.round() as i32;
            if let Life::Cracking(t) = p.life {
                x += if (t / CRUMBLE_SHAKE_PERIOD) as i32 % 2 == 0 { 1 } else { -1 };
            }
            draw_perch(r, p.kind, x, y, p.w as i32);

            if let Some(item) = p.item {
                let s = if item.sprung { &CATTREE_SPRING_SPRUNG } else { item.kind.sprite() };
                let mut iy = y - s.height as i32;
                if item.kind == ItemKind::Fish {
                    iy -= 1 + (self.anim % FISH_BOB_PERIOD > FISH_BOB_PERIOD / 2.0) as i32;
                }
                r.draw_sprite(s, Point::new(x + item.dx as i32, iy), SpriteOpts::default());
            }
            if let Some(g) = p.goo {
                let s = &PLATFORMER_SLIME_IDLE;
                let frame = (self.anim / GOO_ANIM_SPF) as usize % s.frames.len();
                let pos = Point::new(x + g.dx as i32, y - s.height as i32);
                r.draw_sprite(s, pos, SpriteOpts { frame, mirror_h: g.dir < 0.0, ..SpriteOpts::default() });
            }
        }

        for (at, t) in &self.poofs {
            draw_poof(r, Point::new(at.x, self.screen_y(at.y as f32)), *t);
        }

        if let Some(d) = &self.dropped {
            let pos = Point::new(d.x as i32, self.screen_y(d.y));
            r.draw_sprite(d.kind.sprite(), pos, SpriteOpts { mirror_v: true, ..SpriteOpts::default() });
        }
    }

    /// Top-left of the cat sprite in screen space, for the jump sprite.
    fn cat_origin(&self) -> (i32, i32) {
        let s = &PLATFORMER_CAT_JUMP;
        (self.x as i32 - s.width as i32 / 2, self.screen_y(self.feet) - s.height as i32)
    }

    /// Where a worn item goes, from the cat sprite's top-left.
    fn worn_offset(&self, kind: FlightKind) -> (i32, i32) {
        let (sprite, (ax, ay)) = match kind {
            FlightKind::Hat => (&CATTREE_PROPELLER_HAT, HAT_ANCHOR),
            FlightKind::Jetpack => (&CATTREE_JETPACK, JET_ANCHOR),
        };
        let ax = if self.facing_right {
            ax
        } else {
            PLATFORMER_CAT_JUMP.width as i32 - ax - sprite.width as i32
        };
        (ax, ay)
    }

    fn jump_frame(&self) -> usize {
        if self.knocked {
            CAT_FALL_FRAME
        } else if self.flight.is_some() || self.vy < -JUMP_PEAK_RANGE {
            CAT_RISE_FRAME
        } else if self.vy <= JUMP_PEAK_RANGE {
            CAT_PEAK_FRAME
        } else {
            CAT_FALL_FRAME
        }
    }

    fn draw_cat(&self, r: &mut Renderer) {
        match self.state {
            State::Ready => self.draw_cat_standing(r),
            State::Playing if self.resting => {
                self.draw_cat_standing(r);
                let blink = self.rest_time % REST_HINT_BLINK < REST_HINT_BLINK * 0.6;
                if self.rest_time > REST_HINT_DELAY && blink {
                    let y = self.screen_y(self.feet) - PLATFORMER_CAT_SIT.height as i32 - 7;
                    draw_hint(r, Point::new(self.x as i32, y));
                }
            }
            State::Playing => {
                let (x, y) = self.cat_origin();
                let opts = SpriteOpts {
                    frame: self.jump_frame(),
                    mirror_h: !self.facing_right,
                    mirror_v: self.knocked,
                    ..SpriteOpts::default()
                };
                draw_wrapped(r, &PLATFORMER_CAT_JUMP, Point::new(x, y), opts);

                if let Some(f) = self.flight {
                    let (dx, dy) = self.worn_offset(f.kind);
                    let pos = Point::new(x + dx, y + dy);
                    let spin = (self.anim * PROP_FPS) as usize % 2;
                    let (sprite, frame) = match f.kind {
                        FlightKind::Hat => (&CATTREE_PROPELLER_HAT, spin),
                        FlightKind::Jetpack => (&CATTREE_JETPACK, JET_FLAME_FRAMES[spin]),
                    };
                    let opts = SpriteOpts { frame, mirror_h: !self.facing_right, ..SpriteOpts::default() };
                    draw_wrapped(r, sprite, pos, opts);
                }
            }
            State::Dying => {
                if let Some((at, t)) = self.death_poof {
                    draw_poof(r, Point::new(at.x, self.screen_y(at.y as f32)), t);
                }
            }
            State::GameOver => {}
        }
    }

    /// Sitting, or strolling about while on a rest perch.
    fn draw_cat_standing(&self, r: &mut Renderer) {
        let (s, frame) = if self.vx.abs() > 1.0 {
            let s = &PLATFORMER_CAT_RUN;
            (s, (self.anim * RUN_FPS) as usize % s.frames.len())
        } else {
            (&PLATFORMER_CAT_SIT, self.cat_frame)
        };
        let pos = Point::new(self.x as i32 - s.width as i32 / 2, self.screen_y(self.feet) - s.height as i32);
        let opts = SpriteOpts { frame, mirror_h: !self.facing_right, ..SpriteOpts::default() };
        draw_wrapped(r, s, pos, opts);
    }

    fn draw_sparkles(&self, r: &mut Renderer) {
        let at = Point::new(self.sparkle_at.x, self.screen_y(self.sparkle_at.y as f32));
        self.sparkles.draw(r, at);
    }

    fn draw_hud(&self, r: &mut Renderer) {
        let mut n: String<8> = String::new();
        let _ = write!(n, "{}", self.score());
        r.draw_text_small(n.as_str(), Point::new(2, 1));

        if self.session_best > 0 {
            n.clear();
            let _ = write!(n, "{}", self.session_best);
            let mut best: String<24> = String::new();
            crate::i18n::substitute(&mut best, t!("Best: {n}"), &[("n", n.as_str())]);
            let x = DISPLAY_W - 1 - 4 * best.chars().count() as i32;
            r.draw_text_small(best.as_str(), Point::new(x, 1));
        }
    }
}

/// Draw a sprite, and again on the far side when it hangs off an edge, so
/// the cat wraps smoothly.
fn draw_wrapped(r: &mut Renderer, sprite: &Sprite, pos: Point, opts: SpriteOpts) {
    r.draw_sprite(sprite, pos, opts);
    if pos.x < 0 {
        r.draw_sprite(sprite, Point::new(pos.x + DISPLAY_W, pos.y), opts);
    } else if pos.x + sprite.width as i32 > DISPLAY_W {
        r.draw_sprite(sprite, Point::new(pos.x - DISPLAY_W, pos.y), opts);
    }
}

fn push_poof(poofs: &mut Vec<(Point, f32), MAX_POOFS>, at: Point) {
    if poofs.is_full() {
        poofs.remove(0);
    }
    let _ = poofs.push((at, 0.0));
}

/// A little boxed "A" centred on `at`, nudging the player to jump on.
fn draw_hint(r: &mut Renderer, at: Point) {
    let pos = Point::new(at.x - 4, at.y - 3);
    r.fill_rect_off(pos, Size::new(9, 9));
    draw_rounded(r, pos.x, pos.y, 9, 9, false);
    r.draw_text_small("A", Point::new(pos.x + 3, pos.y + 2));
}

fn draw_poof(r: &mut Renderer, centre: Point, t: f32) {
    let frame = (t / POOF_FRAME_DUR) as usize;
    if frame < POOF.frames.len() {
        let pos = Point::new(centre.x - POOF.width as i32 / 2, centre.y - POOF.height as i32 / 2);
        r.draw_sprite(&POOF, pos, SpriteOpts { frame, ..SpriteOpts::default() });
    }
}

/// A box with its corner pixels left off, either outlined over a cleared
/// inside (corners untouched, so the sky shows there) or filled.
fn draw_rounded(r: &mut Renderer, x: i32, y: i32, w: i32, h: i32, filled: bool) {
    let across = Size::new((w - 2) as u32, h as u32);
    let down = Size::new(w as u32, (h - 2) as u32);
    if filled {
        r.draw_rect(Point::new(x + 1, y), across, true);
        r.draw_rect(Point::new(x, y + 1), down, true);
        return;
    }
    r.fill_rect_off(Point::new(x + 1, y), across);
    r.fill_rect_off(Point::new(x, y + 1), down);
    r.draw_line(Point::new(x + 1, y), Point::new(x + w - 2, y));
    r.draw_line(Point::new(x + 1, y + h - 1), Point::new(x + w - 2, y + h - 1));
    r.draw_line(Point::new(x, y + 1), Point::new(x, y + h - 2));
    r.draw_line(Point::new(x + w - 1, y + 1), Point::new(x + w - 1, y + h - 2));
}

/// Shelves are rounded boxes: plain ones outlined, sliding ones solid,
/// crumbly ones cracked through the middle, and rest perches thick and
/// carpeted. Clouds are a row of puffs.
fn draw_perch(r: &mut Renderer, kind: Kind, x: i32, y: i32, w: i32) {
    match kind {
        Kind::Solid => draw_rounded(r, x, y, w, PERCH_H, false),
        Kind::Sliding => draw_rounded(r, x, y, w, PERCH_H, true),
        Kind::Crumbly => {
            draw_rounded(r, x, y, w, PERCH_H, false);
            // A crack stepping down and left through the middle.
            let c = x + w / 2;
            r.draw_pixel(Point::new(c, y), false);
            r.draw_pixel(Point::new(c, y + 1), true);
            r.draw_pixel(Point::new(c - 1, y + 2), true);
            r.draw_pixel(Point::new(c - 1, y + PERCH_H - 1), false);
        }
        Kind::Rest => {
            draw_rounded(r, x, y, w, REST_H, false);
            for i in 2..w - 2 {
                if i % 4 == 0 {
                    r.draw_pixel(Point::new(x + i, y + 2), true);
                } else if i % 4 == 2 {
                    r.draw_pixel(Point::new(x + i, y + 3), true);
                }
            }
        }
        Kind::Cloud => {
            let h = 2 * CLOUD_BUMP_R;
            let bumps = (w - 1 - 2 * CLOUD_BUMP_R) / CLOUD_BUMP_STEP + 1;
            let right = x + (bumps - 1) * CLOUD_BUMP_STEP + 2 * CLOUD_BUMP_R;
            r.fill_rect_off(Point::new(x, y), Size::new((right - x + 1) as u32, (h + 1) as u32));
            for i in 0..bumps {
                let cx = x + CLOUD_BUMP_R + i * CLOUD_BUMP_STEP;
                r.draw_circle_outline(Point::new(cx, y + CLOUD_BUMP_R), CLOUD_BUMP_R);
            }
            // Clear the arcs inside the puff, then close off its bottom.
            r.fill_rect_off(Point::new(x + 1, y + 2), Size::new((right - x - 1) as u32, (h - 2) as u32));
            r.draw_line(Point::new(x + 1, y + h), Point::new(right - 1, y + h));
        }
    }
}

impl Scene for CatTreeScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_best = 0;
        self.session_total = 0;
        self.fish_total = 0;
        let mut rng = ctx.rng;
        self.reset_round(&mut rng);
        ctx.rng = rng;
        // Only the first round shows the controls popup.
        self.state = State::Ready;
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
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        let dt = dt.min(DT_MAX);

        let mut rng = ctx.rng;
        self.update_perches(dt);
        match self.state {
            State::Ready => {
                let go = [Button::A, Button::Left, Button::Right]
                    .iter()
                    .fold(false, |any, &b| buttons.was_just_pressed(b) | any);
                if go {
                    self.start_run();
                }
            }
            State::Playing => self.update_play(buttons, &mut rng, dt),
            State::Dying => self.update_dying(dt),
            State::GameOver => {
                if buttons.was_just_pressed(Button::A) {
                    self.reset_round(&mut rng);
                    self.start_run();
                }
            }
        }
        self.cull_perches();
        self.spawn_ahead(&mut rng);
        self.update_birds(&mut rng, dt);
        self.update_shooting_star(&mut rng, dt);
        ctx.rng = rng;

        self.update_ambient(dt);
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.draw_sky(renderer);
        self.draw_ground(renderer);
        self.draw_perches(renderer);
        self.draw_cat(renderer);
        self.draw_sparkles(renderer);
        self.draw_hud(renderer);

        match self.state {
            State::Ready => {
                let mut popup = Popup::new(10, 8, 108, 18);
                popup.set_text(t!("Left/Right: move"), false, true);
                popup.draw(renderer, false);
            }
            State::GameOver => self.result_popup.draw(renderer, false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lands_only_when_falling_onto_the_top() {
        let (px, pw, top) = (40.0, 20.0, -30.0);

        // Falling through the top, centred.
        assert!(lands_on(-32.0, -29.0, 50.0, px, pw, top));
        // A fast frame that passes right through the perch still lands.
        assert!(lands_on(-40.0, -15.0, 50.0, px, pw, top));
        // Rising through it doesn't.
        assert!(!lands_on(-29.0, -32.0, 50.0, px, pw, top));
        // Already below the top doesn't.
        assert!(!lands_on(-28.0, -25.0, 50.0, px, pw, top));
        // Feet just clipping the edge land; clear of it don't.
        assert!(lands_on(-32.0, -29.0, 36.0, px, pw, top));
        assert!(!lands_on(-32.0, -29.0, 34.0, px, pw, top));
        assert!(!lands_on(-32.0, -29.0, 66.0, px, pw, top));
    }

    #[test]
    fn landing_wraps_around_the_sides() {
        // Cat at the far right, feet hanging past the edge onto a perch at
        // the left.
        assert!(lands_on(-1.0, 1.0, 126.0, 0.0, 10.0, 0.0));
        // And the other way.
        assert!(lands_on(-1.0, 1.0, 2.0, 118.0, 10.0, 0.0));
    }

    #[test]
    fn x_wraps() {
        assert_eq!(wrap_x(-3.0), WF - 3.0);
        assert_eq!(wrap_x(WF + 2.0), 2.0);
        assert_eq!(wrap_x(64.0), 64.0);
    }

    fn idle_buttons() -> Buttons {
        Buttons::new(core::array::from_fn(crate::input::ButtonPin::new))
    }

    fn rest_perch(x: f32, y: f32, w: u8) -> Perch {
        Perch { x, y, w, kind: Kind::Rest, life: Life::Intact, vx: 0.0, item: None, goo: None }
    }

    #[test]
    fn rest_perch_stops_the_bounce_until_the_cat_walks_off() {
        let mut s = CatTreeScene::new();
        let mut rng = 0x1234_5678;
        s.reset_round(&mut rng);
        s.perches.clear();
        let _ = s.perches.push(rest_perch(40.0, -20.0, 40));
        s.state = State::Playing;
        s.x = 60.0;
        s.feet = -24.0;
        s.vy = 60.0;

        let mut buttons = idle_buttons();
        for _ in 0..30 {
            s.update_play(&mut buttons, &mut rng, 1.0 / 12.0);
        }
        assert!(s.resting);
        assert_eq!(s.feet, -20.0);
        assert_eq!(s.vy, 0.0);

        // Off the end, it drops.
        s.x = 90.0;
        s.update_play(&mut buttons, &mut rng, 1.0 / 12.0);
        assert!(!s.resting);
        assert!(s.feet > -20.0);
    }

    #[test]
    fn chain_stays_in_reach_and_hazards_stay_off_it() {
        let mut s = CatTreeScene::new();
        let mut rng = 0x1234_5678;
        s.reset_round(&mut rng);
        s.perches.clear();
        s.next_y = 0.0;

        let mut saw_extra = false;
        while s.next_y > -4000.0 {
            let prev_y = s.next_y;
            s.perches.clear();
            s.spawn_chain(&mut rng);

            // The chain perch goes in last, after any extra below it.
            let (c, extras) = s.perches.split_last().unwrap();
            let gap = prev_y - c.y;
            assert!(gap > 0.0 && gap <= MAX_REACH, "gap {gap} at {prev_y}");
            assert!(c.x >= 0.0 && c.x + c.w as f32 <= WF);
            assert!(!matches!(c.kind, Kind::Crumbly));
            assert!(c.goo.is_none());

            for e in extras {
                saw_extra = true;
                assert!(e.y < prev_y && e.y > c.y);
                assert!(e.goo.is_some() || e.kind == Kind::Crumbly);
                if e.goo.is_some() {
                    let half = c.w as f32 / 2.0 + GOO_CLEARANCE;
                    let cx = c.x + c.w as f32 / 2.0;
                    assert!(!overlaps_wrapped(cx - half, cx + half, e.x, e.x + e.w as f32));
                }
            }
        }
        assert!(saw_extra);
    }
}
