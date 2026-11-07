//! Paw Pong, a game of pong against the pet. The player's paddle is on the
//! left and the pet's on the right; first to 11 wins. Hitting the ball with a
//! moving paddle puts spin on it so it curves. Now and then a powerup block
//! drifts down from the top, and whoever last hit the ball that touches it
//! gets the powerup: a longer paddle, multi-ball, a magnet, a barrier, a big
//! slow ball, a hollow decoy, a freeze on the other paddle, a fireball shot
//! or ghost shots that vanish in the middle of the screen. The pet's paddle
//! handles exactly like the player's; its reactions come from its fitness and
//! its reading of the ball from its focus, and it warms up as the match goes on.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    board::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_bool, rand_f32, rand_range_f32, rand_range_u32},
    assets::icons,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::popup::Popup,
};

const DISPLAY_W: i32 = DISPLAY_WIDTH as i32;
const DISPLAY_H: i32 = DISPLAY_HEIGHT as i32;
const CHAR_W: i32 = 6; // FONT_6X10
const SMALL_CHAR_W: i32 = 4; // FONT_4X6

const WIN_SCORE: u8 = 11;

// Good volleys: rallies where both sides keep returning the ball. Each one
// in a session adds `VOLLEY_BONUS` to the stat rewards, up to
// `MAX_VOLLEY_BONUSES` of them. Coins aren't affected.
const GOOD_VOLLEY_HITS: u32 = 12;
const VOLLEY_BONUS: f32 = 0.15;
const MAX_VOLLEY_BONUSES: u32 = 4;

// Paddles
const PADDLE_W: i32 = 2;
const PADDLE_H: f32 = 12.0;
const LONG_PADDLE_H: f32 = 18.0;
const PLAYER_X: i32 = 3;
const PET_X: i32 = DISPLAY_W - 3 - PADDLE_W;

// Player paddle physics
const PADDLE_MAX_SPEED: f32 = 80.0;
const PADDLE_ACCELERATION: f32 = 700.0;
/// Slows a paddle once its button is let go.
const PADDLE_FRICTION: f32 = 500.0;

// Ball
const BALL_SIZE: i32 = 3;
const BALL_START_SPEED: f32 = 40.0;
const BALL_MAX_SPEED: f32 = 90.0;
/// Added to the ball's speed on every paddle hit, until a new serve.
const SPEED_UP: f32 = 3.0;
/// Bounce angle at the very end of a paddle (about 55 degrees).
const MAX_ANGLE: f32 = 0.96;
const MAX_SERVE_ANGLE: f32 = 0.5;
/// Real balls in play at once; decoys can take the total up to `MAX_BALLS`.
const MAX_REAL_BALLS: usize = 3;
const MAX_BALLS: usize = 5;
/// Keeps a curving ball from turning near vertical: |vy| stays under this
/// multiple of |vx|.
const MAX_VY_RATIO: f32 = 1.6;

// Spin. A paddle's vertical speed at contact becomes a sideways pull on the
// ball that fades over the next second or so.
const SPIN_FACTOR: f32 = 1.0;
const SPIN_DECAY: f32 = 1.0;

/// Pause with the ball at centre before each serve.
const SERVE_DELAY: f32 = 0.8;

// Powerups
/// A 5x5 glyph with a pixel of space and a border around it.
const BLOCK_SIZE: i32 = GLYPH_SIZE + 4;
const BLOCK_FALL_SPEED: f32 = 8.0;
const BLOCK_MIN_X: i32 = 30;
const BLOCK_MAX_X: i32 = DISPLAY_W - 30 - BLOCK_SIZE;
const MAX_BLOCKS: usize = 2;
/// Seconds between blocks: the minimum plus a random share of the range.
const BLOCK_MIN_TIME: f32 = 6.0;
const BLOCK_RANGE_TIME: f32 = 6.0;
const LONG_TIME: f32 = 10.0;
const MAGNET_CATCHES: u8 = 3;
/// A held ball launches on its own after this long so play can't stall.
const MAGNET_MAX_HOLD: f32 = 5.0;
/// Angle between the ball that hits a multi-ball block and each new ball.
const MULTI_SPREAD: f32 = 0.4;
/// Big paw: every ball grows and slows down for a while.
const BIG_BALL_SIZE: i32 = 5;
const BIG_TIME: f32 = 8.0;
const BIG_SPEED_SCALE: f32 = 0.65;
/// The opponent's paddle is stuck in place this long.
const FREEZE_TIME: f32 = 1.2;
/// Fireball: the next shot leaves this much faster, up to `FIRE_MAX_SPEED`.
const FIRE_SPEED_MULT: f32 = 1.7;
const FIRE_MAX_SPEED: f32 = 150.0;
/// How long a ghost's shots vanish in the middle third.
const GHOST_TIME: f32 = 10.0;
const GHOST_LEFT: i32 = DISPLAY_W / 3;
const GHOST_RIGHT: i32 = DISPLAY_W * 2 / 3;
/// A ghost shot fades out over `GHOST_LEVELS` steps of this many pixels
/// on the way into the hidden band, and fades back in the same way after it.
const GHOST_FADE_STEP: i32 = 3;
const GHOST_LEVELS: i32 = 4;
/// Ordered dither for the fade: a pixel shows when its threshold is below
/// the ball's level.
const GHOST_DITHER: [[i32; 2]; 2] = [[0, 2], [3, 1]];
/// Rough paddle: shots off it get this much more spin, for a while.
const ROUGH_TIME: f32 = 10.0;
const ROUGH_SPIN_MULT: f32 = 2.0;
/// Chance that a pet with no focus falls for a decoy.
const DECOY_FOOL_CHANCE: f32 = 0.6;

// Pet AI. The pet works its paddle with the same buttons and physics as the
// player, so its skill is all in its decisions: fitness sets how quickly it
// reacts and focus how well it reads the ball. Each skill runs from 0 to 1:
// the pet's stat sets its ceiling and the match warms it up, from
// `PET_WARMUP_START` of that ceiling at 0-0 to all of it once
// `PET_WARMUP_POINTS` points have been played.
const PET_WARMUP_START: f32 = 0.4;
const PET_WARMUP_POINTS: f32 = 14.0;
/// Even a pet with the stat at 0 gets this share of the skill.
const PET_STAT_FLOOR: f32 = 0.3;
/// The pet stops pressing once its paddle is this close to the target.
const PET_DEADBAND: f32 = 1.5;
/// How far off-centre the pet's read of the ball can be.
const PET_ERROR_MIN: f32 = 2.0;
const PET_ERROR_MAX: f32 = 12.0;
const PET_REACTION_MIN: f32 = 0.12;
const PET_REACTION_MAX: f32 = 0.45;
/// How long before the ball arrives the pet starts moving to where it will
/// land. Before that it chases the ball's current height, blending over to
/// the landing spot as the ball comes in. Varies by up to `PET_LOOKAHEAD_JITTER`
/// either way from shot to shot.
const PET_LOOKAHEAD_MIN: f32 = 0.3;
const PET_LOOKAHEAD_MAX: f32 = 1.2;
const PET_LOOKAHEAD_JITTER: f32 = 0.3;
/// Chance of a spin shot for a pet at full focus.
const PET_SPIN_CHANCE: f32 = 0.4;
/// How much of a ball's spin the pet allows for when reading where it will
/// land, from no focus to full. It sees the rest of the curve only as the
/// ball bends, so curved shots catch it out.
const PET_SPIN_READ_MIN: f32 = 0.0;
const PET_SPIN_READ_MAX: f32 = 0.6;
/// Pressure on the pet's read from a fireball, and from more than one ball
/// coming at once. It makes the pet slower, sloppier and keener to chase
/// rather than anticipate, by `pressure * (PET_STRUGGLE_FLOOR + 1 - focus)`:
/// most early in a match, but never nothing.
const PET_FIRE_PRESSURE: f32 = 1.0;
const PET_MULTI_PRESSURE: f32 = 0.7;
const PET_STRUGGLE_FLOOR: f32 = 0.5;
/// A jump this big in the tracked ball's arrival time means the pet has
/// moved on to another ball.
const PET_SWITCH_ETA: f32 = 0.05;
/// For a spin shot the pet swings its target this far just before contact,
/// so its paddle is still moving when the ball arrives.
const PET_SWIPE: f32 = 6.0;
const PET_SWIPE_TIME: f32 = 0.15;
/// The pet holds a magnet ball for a random share of this range, then
/// launches as soon as the shot is heading away from the player's paddle,
/// or after `PET_HOLD_PATIENCE` more seconds regardless.
const PET_HOLD_MIN: f32 = 0.4;
const PET_HOLD_MAX: f32 = 1.2;
const PET_HOLD_PATIENCE: f32 = 0.8;

// Ball path prediction
const PREDICT_DT: f32 = 1.0 / 120.0;
const PREDICT_STEPS: u32 = 480;

const MESSAGE_Y: i32 = 37;
/// Held powerups show in a row under each score.
const HUD_Y: i32 = 7;
const HUD_STEP: i32 = GLYPH_SIZE + 1;

// 5x5 powerup glyphs, one row per byte in the low five bits.
pub(super) const GLYPH_SIZE: i32 = 5;
pub(super) const GLYPH_LONG: [u8; 5] = [0b01110, 0b00100, 0b00100, 0b00100, 0b01110];
pub(super) const GLYPH_MULTI: [u8; 5] = [0b10001, 0b00000, 0b00100, 0b00000, 0b10001];
pub(super) const GLYPH_MAGNET: [u8; 5] = [0b11011, 0b11011, 0b11011, 0b11111, 0b01110];
pub(super) const GLYPH_BARRIER: [u8; 5] = [0b10000, 0b10100, 0b10101, 0b00101, 0b00001];
const GLYPH_BIG: [u8; 5] = [0b01110, 0b11111, 0b11111, 0b11111, 0b01110];
const GLYPH_DECOY: [u8; 5] = [0b01110, 0b10001, 0b10001, 0b10001, 0b01110];
const GLYPH_FREEZE: [u8; 5] = [0b00100, 0b10101, 0b01110, 0b10101, 0b00100];
pub(super) const GLYPH_FIRE: [u8; 5] = [0b00100, 0b01100, 0b01110, 0b11111, 0b01110];
const GLYPH_GHOST: [u8; 5] = [0b01110, 0b11111, 0b10101, 0b11111, 0b10101];
const GLYPH_ROUGH: [u8; 5] = [0b01010, 0b01000, 0b01010, 0b01000, 0b01010];

// Powerup info page (Menu2).
const INFO_ROWS: usize = 8;
const INFO_ROW_H: i32 = 8;
const INFO_TEXT_X: i32 = 10;
/// Characters per line, leaving room for the scroll arrows at the right.
const INFO_CPL: usize = 18;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    /// The ball waits at centre before moving off.
    Serve,
    Playing,
    MatchOver,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Player,
    Pet,
}

impl Side {
    fn idx(self) -> usize {
        self as usize
    }

    fn other(self) -> Side {
        match self {
            Side::Player => Side::Pet,
            Side::Pet => Side::Player,
        }
    }

    /// Which way the ball travels after this side hits it.
    fn dir(self) -> f32 {
        match self {
            Side::Player => 1.0,
            Side::Pet => -1.0,
        }
    }

    fn paddle_x(self) -> i32 {
        match self {
            Side::Player => PLAYER_X,
            Side::Pet => PET_X,
        }
    }

    /// Where a ball of `size` sits when touching this side's paddle.
    fn rest_x(self, size: i32) -> f32 {
        match self {
            Side::Player => (PLAYER_X + PADDLE_W) as f32,
            Side::Pet => (PET_X - size) as f32,
        }
    }
}

#[derive(Clone, Copy)]
enum Powerup {
    Long,
    Multi,
    Magnet,
    Barrier,
    Big,
    Decoy,
    Freeze,
    Fireball,
    Ghost,
    Rough,
}

impl Powerup {
    const ALL: [Powerup; 10] = [
        Powerup::Long,
        Powerup::Multi,
        Powerup::Magnet,
        Powerup::Barrier,
        Powerup::Big,
        Powerup::Decoy,
        Powerup::Freeze,
        Powerup::Fireball,
        Powerup::Ghost,
        Powerup::Rough,
    ];

    fn glyph(self) -> &'static [u8; 5] {
        match self {
            Powerup::Long => &GLYPH_LONG,
            Powerup::Multi => &GLYPH_MULTI,
            Powerup::Magnet => &GLYPH_MAGNET,
            Powerup::Barrier => &GLYPH_BARRIER,
            Powerup::Big => &GLYPH_BIG,
            Powerup::Decoy => &GLYPH_DECOY,
            Powerup::Freeze => &GLYPH_FREEZE,
            Powerup::Fireball => &GLYPH_FIRE,
            Powerup::Ghost => &GLYPH_GHOST,
            Powerup::Rough => &GLYPH_ROUGH,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Powerup::Long => t!("Long Paddle"),
            Powerup::Multi => t!("Multi-ball"),
            Powerup::Magnet => t!("Magnet"),
            Powerup::Barrier => t!("Barrier"),
            Powerup::Big => t!("Big Paw"),
            Powerup::Decoy => t!("Decoy"),
            Powerup::Freeze => t!("Freeze"),
            Powerup::Fireball => t!("Fireball"),
            Powerup::Ghost => t!("Ghost"),
            Powerup::Rough => t!("Rough Paddle"),
        }
    }

    fn description(self) -> &'static str {
        match self {
            Powerup::Long => t!("Your paddle grows for a while."),
            Powerup::Multi => t!("The ball splits into three."),
            Powerup::Magnet => t!("Catch the next 3 balls. A launches."),
            Powerup::Barrier => t!("A wall behind you stops one ball."),
            Powerup::Big => t!("All balls grow and slow down."),
            Powerup::Decoy => t!("A hollow fake ball that can't score."),
            Powerup::Freeze => t!("Freezes the other paddle briefly."),
            Powerup::Fireball => t!("Your next shot flies extra fast."),
            Powerup::Ghost => t!("Your shots vanish mid-court."),
            Powerup::Rough => t!("Your paddle puts double spin on shots."),
        }
    }
}

#[derive(Clone, Copy)]
struct Paddle {
    /// Centre of the paddle.
    y: f32,
    vy: f32,
    long_timer: f32,
    magnet: u8,
    barrier: bool,
    frozen_timer: f32,
    /// The next shot off this paddle is a fireball.
    fireball: bool,
    ghost_timer: f32,
    rough_timer: f32,
}

impl Paddle {
    fn new() -> Self {
        Self {
            y: DISPLAY_H as f32 / 2.0,
            vy: 0.0,
            long_timer: 0.0,
            magnet: 0,
            barrier: false,
            frozen_timer: 0.0,
            fireball: false,
            ghost_timer: 0.0,
            rough_timer: 0.0,
        }
    }

    fn height(&self) -> f32 {
        if self.long_timer > 0.0 { LONG_PADDLE_H } else { PADDLE_H }
    }

    fn top(&self) -> f32 {
        self.y - self.height() / 2.0
    }

    /// Moves the paddle as if Up (-1), Down (1) or neither (0) is held. Both
    /// sides go through here so they handle the same.
    fn drive(&mut self, dir: i32, dt: f32) {
        if self.frozen_timer > 0.0 {
            self.vy = 0.0;
            return;
        }
        if dir != 0 {
            self.vy = (self.vy + dir as f32 * PADDLE_ACCELERATION * dt).clamp(-PADDLE_MAX_SPEED, PADDLE_MAX_SPEED);
        } else if self.vy > 0.0 {
            self.vy = (self.vy - PADDLE_FRICTION * dt).max(0.0);
        } else if self.vy < 0.0 {
            self.vy = (self.vy + PADDLE_FRICTION * dt).min(0.0);
        }
        self.step(dt);
    }

    fn step(&mut self, dt: f32) {
        self.y += self.vy * dt;
        let half = self.height() / 2.0;
        if self.y < half {
            self.y = half;
            self.vy = 0.0;
        } else if self.y > DISPLAY_H as f32 - half {
            self.y = DISPLAY_H as f32 - half;
            self.vy = 0.0;
        }
    }
}

/// A ball stuck to a magnet paddle. The paddle holding it is the ball's
/// `last_hitter`.
#[derive(Clone, Copy)]
struct Held {
    /// Ball centre relative to the paddle centre.
    offset: f32,
    timer: f32,
    /// When the pet first considers launching it.
    wait: f32,
}

#[derive(Clone, Copy)]
struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    spin: f32,
    size: i32,
    last_hitter: Side,
    held: Option<Held>,
    /// A fake that passes through paddles and never scores.
    decoy: bool,
    /// Left a paddle as a fireball and hasn't been hit since.
    fire: bool,
}

impl Ball {
    fn center_x(&self) -> f32 {
        self.x + self.size as f32 / 2.0
    }

    fn center_y(&self) -> f32 {
        self.y + self.size as f32 / 2.0
    }

    /// Moves the ball, applying spin and bouncing off the top and bottom.
    fn integrate(&mut self, dt: f32) {
        self.vy += self.spin * dt;
        self.spin -= self.spin * SPIN_DECAY * dt;
        let max_vy = self.vx.abs() * MAX_VY_RATIO;
        self.vy = self.vy.clamp(-max_vy, max_vy);
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        let max_y = (DISPLAY_H - self.size) as f32;
        // A wall bounce flips the spin too, or it would pull the ball
        // straight back into the wall.
        if self.y < 0.0 {
            self.y = 0.0;
            self.vy = self.vy.abs();
            self.spin = -self.spin * 0.5;
        } else if self.y > max_y {
            self.y = max_y;
            self.vy = -self.vy.abs();
            self.spin = -self.spin * 0.5;
        }
    }

    /// Where the ball's centre will be when it reaches `target_x`.
    fn predict_y(mut self, target_x: f32) -> f32 {
        let rightward = self.vx > 0.0;
        for _ in 0..PREDICT_STEPS {
            let arrived = if rightward {
                self.x + self.size as f32 >= target_x
            } else {
                self.x <= target_x
            };
            if arrived {
                break;
            }
            self.integrate(PREDICT_DT);
        }
        self.center_y()
    }

    /// Sends the ball off `side`'s paddle. The angle depends on where it
    /// meets the paddle and the spin on how the paddle is moving. Uses up the
    /// paddle's fireball if it has one.
    fn hit(&mut self, side: Side, paddle: &mut Paddle) {
        let rel = ((self.center_y() - paddle.y) / (paddle.height() / 2.0)).clamp(-1.0, 1.0);
        let angle = rel * MAX_ANGLE;
        let mut speed = ((self.vx * self.vx + self.vy * self.vy).sqrt() + SPEED_UP)
            .clamp(BALL_START_SPEED, BALL_MAX_SPEED);
        self.fire = core::mem::take(&mut paddle.fireball);
        if self.fire {
            speed = (speed * FIRE_SPEED_MULT).min(FIRE_MAX_SPEED);
        }
        self.vx = side.dir() * speed * angle.cos();
        self.vy = speed * angle.sin();
        let rough = if paddle.rough_timer > 0.0 { ROUGH_SPIN_MULT } else { 1.0 };
        self.spin = paddle.vy * SPIN_FACTOR * rough;
        self.last_hitter = side;
        self.held = None;
        self.x = side.rest_x(self.size);
    }
}

#[derive(Clone, Copy)]
struct Block {
    x: i32,
    y: f32,
    kind: Powerup,
}

pub struct PawPongScene {
    state: State,
    paddles: [Paddle; 2],
    balls: Vec<Ball, MAX_BALLS>,
    blocks: Vec<Block, MAX_BLOCKS>,
    block_timer: f32,
    big_timer: f32,
    scores: [u8; 2],
    /// The side the next serve heads toward.
    serve_to: Side,
    /// Time in the Serve state.
    timer: f32,
    result_popup: Popup,

    /// The pet's fitness and focus as 0 to 1.
    pet_fitness: f32,
    pet_focus: f32,
    /// Counts down after the player hits; the pet keeps its old target until
    /// it reaches zero.
    pet_react_timer: f32,
    pet_target: f32,
    /// How far off the pet's read is for the current shot.
    pet_aim: f32,
    /// Which way the pet swings for a spin shot, or 0 for none.
    pet_swipe: f32,
    /// How far ahead, in seconds, the pet reads the current shot.
    pet_lookahead: f32,
    /// Whether the pet falls for decoys on the current shot.
    pet_fooled: bool,
    /// The pet takes a fresh read at its next update, once the shot that
    /// prompted it is in `balls`.
    pet_needs_read: bool,
    /// Arrival time of the ball the pet tracked last frame.
    pet_last_eta: f32,
    /// Where the pet carries a ball it's holding on a magnet.
    pet_hold_target: f32,

    /// The powerup info page is open over the paused game.
    info_open: bool,
    info_scroll: usize,

    session_points: u32,
    session_wins: u32,
    /// Paddle hits since the last point, and whether this rally has already
    /// counted as a good volley.
    rally_hits: u32,
    rally_counted: bool,
    session_volleys: u32,
}

impl PawPongScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            paddles: [Paddle::new(); 2],
            balls: Vec::new(),
            blocks: Vec::new(),
            block_timer: 0.0,
            big_timer: 0.0,
            scores: [0; 2],
            serve_to: Side::Pet,
            timer: 0.0,
            result_popup: Popup::new(14, 12, 100, 40),
            pet_fitness: 0.0,
            pet_focus: 0.0,
            pet_lookahead: PET_LOOKAHEAD_MIN,
            pet_fooled: false,
            pet_needs_read: false,
            pet_last_eta: f32::MAX,
            pet_react_timer: 0.0,
            pet_target: DISPLAY_H as f32 / 2.0,
            pet_aim: 0.0,
            pet_swipe: 0.0,
            pet_hold_target: DISPLAY_H as f32 / 2.0,
            info_open: false,
            info_scroll: 0,
            session_points: 0,
            session_wins: 0,
            rally_hits: 0,
            rally_counted: false,
            session_volleys: 0,
        }
    }

    fn new_match(&mut self, rng: &mut u32) {
        self.scores = [0; 2];
        self.paddles = [Paddle::new(); 2];
        self.blocks.clear();
        self.reset_block_timer(rng);
        self.big_timer = 0.0;
        self.serve_to = Side::Pet;
        self.serve(rng);
    }

    fn serve(&mut self, rng: &mut u32) {
        self.state = State::Serve;
        self.timer = 0.0;
        self.balls.clear();
        self.end_rally();
        let angle = rand_range_f32(rng, -MAX_SERVE_ANGLE, MAX_SERVE_ANGLE);
        let server = self.serve_to.other();
        let size = self.ball_size();
        let _ = self.balls.push(Ball {
            x: ((DISPLAY_W - size) / 2) as f32,
            y: rand_range_f32(rng, 16.0, (DISPLAY_H - 16) as f32),
            vx: server.dir() * BALL_START_SPEED * angle.cos(),
            vy: BALL_START_SPEED * angle.sin(),
            spin: 0.0,
            size,
            last_hitter: server,
            held: None,
            decoy: false,
            fire: false,
        });
        if self.serve_to == Side::Pet {
            self.pet_needs_read = true;
        }
    }

    fn reset_block_timer(&mut self, rng: &mut u32) {
        self.block_timer = BLOCK_MIN_TIME + BLOCK_RANGE_TIME * rand_f32(rng);
    }

    /// How big a real ball is right now; decoys are one pixel bigger.
    fn ball_size(&self) -> i32 {
        if self.big_timer > 0.0 { BIG_BALL_SIZE } else { BALL_SIZE }
    }

    /// Grows or shrinks every ball about its centre to match `ball_size`.
    fn resize_balls(&mut self) {
        let size = self.ball_size();
        for b in &mut self.balls {
            let want = size + b.decoy as i32;
            if b.size != want {
                let d = (want - b.size) as f32 / 2.0;
                b.x -= d;
                b.y -= d;
                b.size = want;
            }
        }
    }

    /// How much of a ball shows, from 0 (hidden) to `GHOST_LEVELS` (all of
    /// it). Ghost shots fade out approaching the middle third, stay hidden
    /// inside it and fade back in after.
    fn ghost_level(&self, b: &Ball) -> i32 {
        if b.held.is_some() || self.paddles[b.last_hitter.idx()].ghost_timer <= 0.0 {
            return GHOST_LEVELS;
        }
        let x = b.center_x() as i32;
        let dist = if x < GHOST_LEFT {
            GHOST_LEFT - x
        } else if x >= GHOST_RIGHT {
            x - GHOST_RIGHT + 1
        } else {
            0
        };
        (dist / GHOST_FADE_STEP).min(GHOST_LEVELS)
    }

    fn is_hidden(&self, b: &Ball) -> bool {
        self.ghost_level(b) == 0
    }

    fn update_player(&mut self, buttons: &Buttons, dt: f32) {
        let dir = buttons.is_pressed(Button::Down) as i32 - buttons.is_pressed(Button::Up) as i32;
        self.paddles[Side::Player.idx()].drive(dir, dt);
    }

    /// The pet takes a fresh look at the ball after the player sends it over:
    /// a reaction delay, some error in its read, and maybe a spin shot.
    fn new_pet_read(&mut self, rng: &mut u32) {
        let focus = self.pet_skill(self.pet_focus);
        let struggle = self.pet_pressure() * (PET_STRUGGLE_FLOOR + 1.0 - focus);

        let fitness = self.pet_skill(self.pet_fitness);
        let reaction = PET_REACTION_MAX - (PET_REACTION_MAX - PET_REACTION_MIN) * fitness;
        self.pet_react_timer = reaction * (1.0 + 0.5 * struggle);
        let error = PET_ERROR_MIN + (PET_ERROR_MAX - PET_ERROR_MIN) * (1.0 - focus);
        self.pet_aim = rand_range_f32(rng, -1.0, 1.0) * error * (1.0 + struggle);
        let lookahead = PET_LOOKAHEAD_MIN + (PET_LOOKAHEAD_MAX - PET_LOOKAHEAD_MIN) * focus;
        self.pet_lookahead = lookahead / (1.0 + struggle)
            * rand_range_f32(rng, 1.0 - PET_LOOKAHEAD_JITTER, 1.0 + PET_LOOKAHEAD_JITTER);
        self.pet_fooled = rand_bool(rng, DECOY_FOOL_CHANCE * (1.0 - focus));
        self.pet_swipe = if rand_bool(rng, PET_SPIN_CHANCE * focus) {
            if rand_bool(rng, 0.5) { PET_SWIPE } else { -PET_SWIPE }
        } else {
            0.0
        };
    }

    /// How hard the shots coming at the pet are to read: fireballs and more
    /// than one real ball at once.
    fn pet_pressure(&self) -> f32 {
        let mut incoming = 0;
        let mut fire = false;
        for b in &self.balls {
            if !b.decoy && b.held.is_none() && b.vx > 0.0 {
                incoming += 1;
                fire |= b.fire;
            }
        }
        let mut pressure = 0.0;
        if fire {
            pressure += PET_FIRE_PRESSURE;
        }
        if incoming > 1 {
            pressure += PET_MULTI_PRESSURE;
        }
        pressure
    }

    /// A skill from 0 to 1 for a stat from 0 to 1, warming up over the match.
    fn pet_skill(&self, stat: f32) -> f32 {
        let played = (self.scores[0] + self.scores[1]) as f32;
        let warmup = PET_WARMUP_START + (1.0 - PET_WARMUP_START) * (played / PET_WARMUP_POINTS).min(1.0);
        (PET_STAT_FLOOR + (1.0 - PET_STAT_FLOOR) * stat) * warmup
    }

    /// The free ball heading for the pet that will get there first. Decoys
    /// only count when the pet has been fooled.
    fn incoming_ball(&self) -> Option<Ball> {
        let mut best: Option<(Ball, f32)> = None;
        for b in &self.balls {
            if b.held.is_some() || b.vx <= 0.0 || (b.decoy && !self.pet_fooled) {
                continue;
            }
            let eta = (PET_X as f32 - b.x - b.size as f32) / b.vx;
            if eta < 0.0 {
                continue;
            }
            if best.map_or(true, |(_, t)| eta < t) {
                best = Some((*b, eta));
            }
        }
        best.map(|(b, _)| b)
    }

    fn update_pet(&mut self, rng: &mut u32, dt: f32) {
        let incoming = self.incoming_ball();
        // Moving on to another ball (the last was returned or got past)
        // takes a fresh read.
        let eta = incoming.map_or(f32::MAX, |b| (PET_X as f32 - b.x - b.size as f32) / b.vx);
        if eta > self.pet_last_eta + PET_SWITCH_ETA && eta < f32::MAX {
            self.pet_needs_read = true;
        }
        self.pet_last_eta = eta;
        if self.pet_needs_read {
            self.pet_needs_read = false;
            self.new_pet_read(rng);
        }

        let holding = self.balls.iter().any(|b| b.held.is_some() && b.last_hitter == Side::Pet);
        if holding {
            self.pet_target = self.pet_hold_target;
        } else if let Some(b) = incoming {
            if self.pet_react_timer > 0.0 {
                self.pet_react_timer -= dt;
            } else if self.is_hidden(&b) {
                // A ghost ball it can't see: keep heading for the last read.
            } else {
                // Chase the ball while it's far off, then move over to where
                // it will land as it gets closer.
                let commit = (1.0 - eta / self.pet_lookahead).clamp(0.0, 1.0);
                // It only partly allows for spin, so curves fool it until
                // they bend.
                let focus = self.pet_skill(self.pet_focus);
                let spin_read = PET_SPIN_READ_MIN + (PET_SPIN_READ_MAX - PET_SPIN_READ_MIN) * focus;
                let landing = Ball { spin: b.spin * spin_read, ..b }.predict_y(PET_X as f32);
                let mut target = b.center_y() + (landing - b.center_y()) * commit + self.pet_aim;
                if eta < PET_SWIPE_TIME {
                    target += self.pet_swipe;
                }
                self.pet_target = target;
            }
        } else {
            self.pet_target = DISPLAY_H as f32 / 2.0;
        }

        // Hold the button toward the target, letting go early enough to
        // coast to a stop on it.
        let p = &mut self.paddles[Side::Pet.idx()];
        let dist = self.pet_target - p.y;
        let stopping = p.vy * p.vy / (2.0 * PADDLE_FRICTION);
        let coasting_in = p.vy * dist > 0.0 && stopping >= dist.abs();
        let dir = if dist.abs() < PET_DEADBAND || coasting_in {
            0
        } else if dist > 0.0 {
            1
        } else {
            -1
        };
        p.drive(dir, dt);
    }

    /// Keeps magnet balls on their paddles and launches them when it's time:
    /// on A for the player, and when the shot looks good for the pet.
    fn update_held(&mut self, launch_pressed: bool, dt: f32) {
        let player = self.paddles[Side::Player.idx()];
        let mut player_launched = false;
        for i in 0..self.balls.len() {
            let mut b = self.balls[i];
            let Some(mut held) = b.held else {
                continue;
            };
            let side = b.last_hitter;
            let paddle = self.paddles[side.idx()];
            held.timer += dt;
            // The paddle may have shrunk since the catch.
            let half = paddle.height() / 2.0;
            held.offset = held.offset.clamp(-half, half);
            b.y = paddle.y + held.offset - b.size as f32 / 2.0;
            b.x = side.rest_x(b.size);
            b.held = Some(held);

            let launch = held.timer >= MAGNET_MAX_HOLD
                || match side {
                    Side::Player => launch_pressed,
                    Side::Pet => {
                        if held.timer < held.wait {
                            false
                        } else if held.timer >= held.wait + PET_HOLD_PATIENCE {
                            true
                        } else {
                            let (mut shot, mut p) = (b, paddle);
                            shot.hit(Side::Pet, &mut p);
                            let land = shot.predict_y((PLAYER_X + PADDLE_W) as f32);
                            (land - player.y).abs() > player.height() / 2.0 + 4.0
                        }
                    }
                };
            if launch {
                b.hit(side, &mut self.paddles[side.idx()]);
                self.count_hit();
                player_launched |= side == Side::Player;
            }
            self.balls[i] = b;
        }
        if player_launched {
            self.pet_needs_read = true;
        }
    }

    fn touches_paddle(&self, b: &Ball, side: Side) -> bool {
        let px = side.paddle_x() as f32;
        let p = &self.paddles[side.idx()];
        b.x < px + PADDLE_W as f32
            && b.x + b.size as f32 > px
            && b.y + b.size as f32 > p.top()
            && b.y < p.top() + p.height()
    }

    /// The ball reached `side`'s paddle: a magnet catches it, otherwise it
    /// bounces off.
    fn paddle_contact(&mut self, rng: &mut u32, b: &mut Ball, side: Side) {
        let p = &mut self.paddles[side.idx()];
        if p.magnet == 0 {
            b.hit(side, p);
            self.count_hit();
            if side == Side::Player {
                self.pet_needs_read = true;
            }
            return;
        }
        p.magnet -= 1;
        b.last_hitter = side;
        b.spin = 0.0;
        b.fire = false;
        b.x = side.rest_x(b.size);
        b.held = Some(Held {
            offset: b.center_y() - p.y,
            timer: 0.0,
            wait: rand_range_f32(rng, PET_HOLD_MIN, PET_HOLD_MAX),
        });
        if side == Side::Pet {
            self.pet_hold_target = rand_range_f32(rng, 12.0, (DISPLAY_H - 12) as f32);
        }
    }

    /// Moves every free ball, handling paddles, barriers, powerup blocks and
    /// points.
    fn update_balls(&mut self, rng: &mut u32, dt: f32) {
        let dt = if self.big_timer > 0.0 { dt * BIG_SPEED_SCALE } else { dt };
        let mut i = 0;
        'balls: while i < self.balls.len() {
            let mut b = self.balls[i];
            if b.held.is_some() {
                i += 1;
                continue;
            }
            // About one pixel per sub-step so the ball can't skip a paddle.
            let steps = (b.vx.abs().max(b.vy.abs()) * dt) as i32 + 1;
            let sub = dt / steps as f32;
            let mut split = false;
            let mut decoy = false;
            for _ in 0..steps {
                b.integrate(sub);

                if b.decoy {
                    if b.x + (b.size as f32) < 0.0 || b.x > DISPLAY_W as f32 {
                        self.balls.swap_remove(i);
                        continue 'balls;
                    }
                    continue;
                }

                if b.vx < 0.0 && self.touches_paddle(&b, Side::Player) {
                    self.paddle_contact(rng, &mut b, Side::Player);
                } else if b.vx > 0.0 && self.touches_paddle(&b, Side::Pet) {
                    self.paddle_contact(rng, &mut b, Side::Pet);
                }
                if b.held.is_some() {
                    break;
                }

                let player = &mut self.paddles[Side::Player.idx()];
                if b.vx < 0.0 && b.x <= 1.0 && player.barrier {
                    player.barrier = false;
                    b.x = 1.0;
                    b.vx = -b.vx;
                }
                let pet = &mut self.paddles[Side::Pet.idx()];
                if b.vx > 0.0 && b.x + b.size as f32 >= (DISPLAY_W - 1) as f32 && pet.barrier {
                    pet.barrier = false;
                    b.x = (DISPLAY_W - 1 - b.size) as f32;
                    b.vx = -b.vx;
                }

                let scorer = if b.x + (b.size as f32) < 0.0 {
                    Some(Side::Pet)
                } else if b.x > DISPLAY_W as f32 {
                    Some(Side::Player)
                } else {
                    None
                };
                if let Some(scorer) = scorer {
                    self.balls.swap_remove(i);
                    self.score_point(rng, scorer);
                    if self.state != State::Playing {
                        return;
                    }
                    continue 'balls;
                }

                let (bx, by) = (b.x as i32, b.y as i32);
                let mut j = 0;
                while j < self.blocks.len() {
                    let block = self.blocks[j];
                    let (kx, ky) = (block.x, block.y as i32);
                    if bx < kx + BLOCK_SIZE && bx + b.size > kx && by < ky + BLOCK_SIZE && by + b.size > ky {
                        self.blocks.swap_remove(j);
                        let side = b.last_hitter;
                        let p = &mut self.paddles[side.idx()];
                        match block.kind {
                            Powerup::Long => p.long_timer = LONG_TIME,
                            Powerup::Multi => split = true,
                            Powerup::Magnet => p.magnet = MAGNET_CATCHES,
                            Powerup::Barrier => p.barrier = true,
                            Powerup::Big => self.big_timer = BIG_TIME,
                            Powerup::Decoy => decoy = true,
                            Powerup::Freeze => self.paddles[side.other().idx()].frozen_timer = FREEZE_TIME,
                            Powerup::Fireball => p.fireball = true,
                            Powerup::Ghost => p.ghost_timer = GHOST_TIME,
                            Powerup::Rough => p.rough_timer = ROUGH_TIME,
                        }
                    } else {
                        j += 1;
                    }
                }
            }
            self.balls[i] = b;
            if split {
                self.split_ball(b);
                if b.vx > 0.0 {
                    self.pet_needs_read = true;
                }
            }
            if decoy {
                self.spawn_decoy(b);
            }
            i += 1;
        }
    }

    fn real_balls(&self) -> usize {
        self.balls.iter().filter(|b| !b.decoy).count()
    }

    /// Multi-ball: two more balls fan out from this one, kept within the
    /// usual bounce angles so none of them crawls up and down the screen.
    fn split_ball(&mut self, b: Ball) {
        let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
        let dir = if b.vx < 0.0 { -1.0 } else { 1.0 };
        let base = b.vy.atan2(b.vx.abs());
        for spread in [MULTI_SPREAD, -MULTI_SPREAD] {
            if self.real_balls() >= MAX_REAL_BALLS {
                break;
            }
            let angle = (base + spread).clamp(-MAX_ANGLE, MAX_ANGLE);
            let _ = self.balls.push(Ball {
                vx: dir * speed * angle.cos(),
                vy: speed * angle.sin(),
                ..b
            });
        }
    }

    /// A hollow fake that heads at the collector's opponent on the mirror of
    /// the real ball's angle.
    fn spawn_decoy(&mut self, b: Ball) {
        let vx = b.last_hitter.dir() * b.vx.abs();
        let vy = if b.vy.abs() > 8.0 {
            -b.vy
        } else if b.vy < 0.0 {
            vx.abs() * 0.5
        } else {
            -vx.abs() * 0.5
        };
        let _ = self.balls.push(Ball {
            x: b.x - 0.5,
            y: b.y - 0.5,
            vx,
            vy,
            spin: -b.spin,
            size: b.size + 1,
            decoy: true,
            fire: false,
            ..b
        });
    }

    fn count_hit(&mut self) {
        self.rally_hits += 1;
        if self.rally_hits >= GOOD_VOLLEY_HITS && !self.rally_counted {
            self.rally_counted = true;
            self.session_volleys += 1;
        }
    }

    fn end_rally(&mut self) {
        self.rally_hits = 0;
        self.rally_counted = false;
    }

    fn score_point(&mut self, rng: &mut u32, scorer: Side) {
        // A miss ends the rally, even if other balls are still in play.
        self.end_rally();
        self.scores[scorer.idx()] += 1;
        self.session_points += 1;
        if self.scores[scorer.idx()] >= WIN_SCORE {
            self.end_match(scorer);
        } else if self.real_balls() == 0 {
            self.serve_to = scorer.other();
            self.serve(rng);
        }
    }

    fn end_match(&mut self, winner: Side) {
        self.state = State::MatchOver;
        self.balls.clear();
        if winner == Side::Player {
            self.session_wins += 1;
        }
        let mut msg: String<64> = String::new();
        let _ = msg.push_str(match winner {
            Side::Player => t!("You Win!"),
            Side::Pet => t!("You Lose!"),
        });
        let _ = write!(msg, "\n{}-{}\n", self.scores[0], self.scores[1]);
        let _ = msg.push_str(t!("A: New Game"));
        self.result_popup.set_text(msg.as_str(), false, true);
    }

    fn update_blocks(&mut self, rng: &mut u32, dt: f32) {
        let mut i = 0;
        while i < self.blocks.len() {
            self.blocks[i].y += BLOCK_FALL_SPEED * dt;
            if self.blocks[i].y > DISPLAY_H as f32 {
                self.blocks.swap_remove(i);
            } else {
                i += 1;
            }
        }

        self.block_timer -= dt;
        if self.block_timer > 0.0 {
            return;
        }
        self.reset_block_timer(rng);
        if self.blocks.is_full() {
            return;
        }
        let kind = Powerup::ALL[rand_range_u32(rng, 0, Powerup::ALL.len() as u32 - 1) as usize];
        let _ = self.blocks.push(Block {
            x: rand_range_u32(rng, BLOCK_MIN_X as u32, BLOCK_MAX_X as u32) as i32,
            y: -BLOCK_SIZE as f32,
            kind,
        });
    }

    /// Draws a side's held powerups in a row starting at `x`, stepping by
    /// `step` so the player's grow leftward and the pet's rightward.
    fn draw_held_powerups(renderer: &mut Renderer, p: &Paddle, x: i32, step: i32) {
        let mut x = x;
        let held = [
            (p.long_timer > 0.0, Powerup::Long),
            (p.magnet > 0, Powerup::Magnet),
            (p.barrier, Powerup::Barrier),
            (p.fireball, Powerup::Fireball),
            (p.ghost_timer > 0.0, Powerup::Ghost),
            (p.rough_timer > 0.0, Powerup::Rough),
        ];
        for (on, kind) in held {
            if on {
                draw_glyph(renderer, kind.glyph(), x, HUD_Y);
                x += step;
            }
        }
    }

    /// Calls `f` with each line of the powerup info page: its index, the
    /// powerup whose name it is (if it's a name line) and its text. Returns
    /// how many lines there are.
    fn for_each_info_line(mut f: impl FnMut(usize, Option<Powerup>, &str)) -> usize {
        f(0, None, t!("Powerups"));
        let mut n = 1;
        for kind in Powerup::ALL {
            // A blank row before each powerup keeps the entries apart.
            f(n, None, "");
            f(n + 1, Some(kind), kind.name());
            n += 2;
            let mut line: String<{ INFO_CPL * 4 }> = String::new();
            let mut chars = 0;
            for word in kind.description().split(' ') {
                let len = word.chars().count();
                if chars > 0 && chars + 1 + len > INFO_CPL {
                    f(n, None, &line);
                    n += 1;
                    line.clear();
                    chars = 0;
                }
                if chars > 0 {
                    let _ = line.push(' ');
                    chars += 1;
                }
                let _ = line.push_str(word);
                chars += len;
            }
            if chars > 0 {
                f(n, None, &line);
                n += 1;
            }
        }
        n
    }

    fn info_max_scroll() -> usize {
        Self::for_each_info_line(|_, _, _| {}).saturating_sub(INFO_ROWS)
    }

    fn update_info(&mut self, buttons: &mut Buttons) {
        if buttons.was_just_pressed(Button::B) {
            self.info_open = false;
        } else if buttons.was_just_pressed(Button::Up) {
            self.info_scroll = self.info_scroll.saturating_sub(1);
        } else if buttons.was_just_pressed(Button::Down) {
            self.info_scroll = (self.info_scroll + 1).min(Self::info_max_scroll());
        }
    }

    fn draw_info(&self, renderer: &mut Renderer) {
        let scroll = self.info_scroll;
        Self::for_each_info_line(|n, kind, text| {
            if n < scroll || n >= scroll + INFO_ROWS {
                return;
            }
            let y = (n - scroll) as i32 * INFO_ROW_H;
            if let Some(kind) = kind {
                draw_glyph(renderer, kind.glyph(), 2, y + 2);
            }
            // The title sits at the left edge; everything else lines up
            // beside the glyphs.
            let x = if n == 0 { 2 } else { INFO_TEXT_X };
            renderer.draw_text(text, Point::new(x, y));
        });

        let opts = SpriteOpts::default();
        if scroll > 0 {
            renderer.draw_sprite_raw(icons::UP_ARROW, icons::ARROW_W, icons::ARROW_H, Point::new(119, 0), opts);
        }
        if scroll < Self::info_max_scroll() {
            renderer.draw_sprite_raw(icons::DOWN_ARROW, icons::ARROW_W, icons::ARROW_H, Point::new(119, 56), opts);
        }
    }

    fn draw_message(renderer: &mut Renderer, text: &str) {
        let w = text.chars().count() as i32 * CHAR_W;
        let x = (DISPLAY_W - w) / 2;
        renderer.fill_rect_off(Point::new(x - 1, MESSAGE_Y - 1), Size::new((w + 2) as u32, 12));
        renderer.draw_text(text, Point::new(x, MESSAGE_Y));
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        if self.session_points == 0 {
            return;
        }
        let reward = (self.session_points as f32 / WIN_SCORE as f32).sqrt();
        let fulfillment_mult = if self.session_wins > 0 { 4.0 } else { 2.0 };
        let volleys = self.session_volleys.min(MAX_VOLLEY_BONUSES) as f32;
        let stats = reward * (1.0 + VOLLEY_BONUS * volleys);
        ctx.apply_stat_changes(&[
            (StatId::Playfulness, 5.0 * stats),
            (StatId::Fitness,     4.0 * stats),
            (StatId::Focus,       3.0 * stats),
            (StatId::Sociability, 2.0 * stats),
            (StatId::Loyalty,     1.0 * stats),
            (StatId::Fulfillment, fulfillment_mult * stats),
        ]);
        let coins = (3.0 * reward) as i32 + 3 * self.session_wins as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }
}

/// Draws a 5x5 powerup glyph with its top-left at (`x`, `y`).
pub(super) fn draw_glyph(renderer: &mut Renderer, glyph: &[u8; 5], x: i32, y: i32) {
    for (dy, row) in glyph.iter().enumerate() {
        for dx in 0..GLYPH_SIZE {
            if row & (1 << (GLYPH_SIZE - 1 - dx)) != 0 {
                renderer.draw_pixel(Point::new(x + dx, y + dy as i32), true);
            }
        }
    }
}

impl Scene for PawPongScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.pet_fitness = ctx.fitness.clamp(0.0, 100.0) / 100.0;
        self.pet_focus = ctx.focus.clamp(0.0, 100.0) / 100.0;

        let mut rng = ctx.rng;
        self.session_points = 0;
        self.session_wins = 0;
        self.session_volleys = 0;
        self.new_match(&mut rng);
        self.state = State::Ready;
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
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        if buttons.was_just_pressed(Button::Menu2) {
            self.info_open = !self.info_open;
            self.info_scroll = 0;
            return None;
        }
        if self.info_open {
            self.update_info(buttons);
            return None;
        }

        let mut rng = ctx.rng;
        let a = buttons.was_just_pressed(Button::A);
        match self.state {
            State::Ready if a => self.state = State::Serve,
            State::MatchOver if a => self.new_match(&mut rng),
            _ => {}
        }

        if self.state != State::MatchOver {
            self.update_player(buttons, dt);
        }
        if matches!(self.state, State::Serve | State::Playing) {
            self.update_pet(&mut rng, dt);
        }

        if self.state == State::Serve {
            self.timer += dt;
            if self.timer >= SERVE_DELAY {
                self.state = State::Playing;
            }
        } else if self.state == State::Playing {
            for p in &mut self.paddles {
                p.long_timer = (p.long_timer - dt).max(0.0);
                p.frozen_timer = (p.frozen_timer - dt).max(0.0);
                p.ghost_timer = (p.ghost_timer - dt).max(0.0);
                p.rough_timer = (p.rough_timer - dt).max(0.0);
            }
            self.big_timer = (self.big_timer - dt).max(0.0);
            self.resize_balls();
            self.update_held(a, dt);
            self.update_balls(&mut rng, dt);
            if self.state == State::Playing {
                self.update_blocks(&mut rng, dt);
            }
        }
        ctx.rng = rng;

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.info_open {
            self.draw_info(renderer);
            return;
        }
        for side in [Side::Player, Side::Pet] {
            let p = &self.paddles[side.idx()];
            let (x, top) = (side.paddle_x(), p.top() as i32);
            if p.frozen_timer > 0.0 {
                // Frozen paddles are drawn dashed.
                for y in (top..top + p.height() as i32).step_by(2) {
                    renderer.draw_rect(Point::new(x, y), Size::new(PADDLE_W as u32, 1), true);
                }
            } else {
                renderer.draw_rect(Point::new(x, top), Size::new(PADDLE_W as u32, p.height() as u32), true);
            }
            if p.rough_timer > 0.0 {
                // A rough paddle has a row of bumps a pixel in front of it.
                let dots_x = match side {
                    Side::Player => x + PADDLE_W + 1,
                    Side::Pet => x - 2,
                };
                for y in (top..top + p.height() as i32).step_by(2) {
                    renderer.draw_pixel(Point::new(dots_x, y), true);
                }
            }
            if p.barrier {
                let x = match side {
                    Side::Player => 0,
                    Side::Pet => DISPLAY_W - 1,
                };
                for y in (0..DISPLAY_H).step_by(2) {
                    renderer.draw_pixel(Point::new(x, y), true);
                }
            }
        }

        for block in &self.blocks {
            let y = block.y as i32;
            renderer.draw_rect(
                Point::new(block.x, y),
                Size::new(BLOCK_SIZE as u32, BLOCK_SIZE as u32),
                false,
            );
            draw_glyph(renderer, block.kind.glyph(), block.x + 2, y + 2);
        }

        for b in &self.balls {
            let level = self.ghost_level(b);
            if level == 0 {
                continue;
            }
            let (x0, y0) = (b.x as i32, b.y as i32);
            for dy in 0..b.size {
                for dx in 0..b.size {
                    // Decoys are hollow.
                    let edge = dx == 0 || dy == 0 || dx == b.size - 1 || dy == b.size - 1;
                    if (b.decoy && !edge) || GHOST_DITHER[(dy % 2) as usize][(dx % 2) as usize] >= level {
                        continue;
                    }
                    renderer.draw_pixel(Point::new(x0 + dx, y0 + dy), true);
                }
            }
            if b.fire && level == GHOST_LEVELS {
                // A short trail behind a fireball.
                let speed = (b.vx * b.vx + b.vy * b.vy).sqrt().max(1.0);
                for k in [3.0, 5.0, 8.0] {
                    let tx = b.center_x() - b.vx / speed * k;
                    let ty = b.center_y() - b.vy / speed * k;
                    renderer.draw_pixel(Point::new(tx as i32, ty as i32), true);
                }
            }
        }

        let mut s: String<4> = String::new();
        let _ = write!(s, "{}", self.scores[0]);
        let w = s.len() as i32 * SMALL_CHAR_W;
        renderer.draw_text_small(s.as_str(), Point::new(DISPLAY_W / 2 - 6 - w, 0));
        s.clear();
        let _ = write!(s, "{}", self.scores[1]);
        renderer.draw_text_small(s.as_str(), Point::new(DISPLAY_W / 2 + 6, 0));

        Self::draw_held_powerups(renderer, &self.paddles[0], DISPLAY_W / 2 - 6 - GLYPH_SIZE, -HUD_STEP);
        Self::draw_held_powerups(renderer, &self.paddles[1], DISPLAY_W / 2 + 6, HUD_STEP);

        match self.state {
            State::Ready => Self::draw_message(renderer, t!("A: Start")),
            State::MatchOver => self.result_popup.draw(renderer, false),
            State::Serve | State::Playing => {}
        }
    }
}
