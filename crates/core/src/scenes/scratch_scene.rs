//! Scratch. Eight-ball pool against the pet on a table the size of the
//! screen. Left/right aims (up/down nudges finely), A starts a power gauge
//! that swings up and down, and A again shoots, the same as Mini Golf. The
//! cue ball's path is projected up to one cushion bounce.
//!
//! The first ball potted gives the shooter that group. Potting one of your
//! own keeps the turn; a foul (scratch, hitting nothing, or hitting the
//! wrong ball first) passes it. The pet plays real shots, aiming through
//! the same UI, and the two take turns breaking.

use core::f32::consts::PI;
use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::minigame_assets::CAT_THIN_DOWN,
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_bool, rand_range_f32, rand_range_u32},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

const SCREEN_W: i32 = 128;
const SCREEN_H: i32 = 64;

// Table, in px with pixel i's centre at i. The cushion lines are drawn at
// x = 1 and 126, y = 1 and 62; a ball's centre stays 3px inside them so its
// 5x5 sprite sits right against the line.
const X_MIN: f32 = 4.0;
const X_MAX: f32 = 123.0;
const Y_MIN: f32 = 4.0;
const Y_MAX: f32 = 59.0;
const MID_X: f32 = (X_MIN + X_MAX) / 2.0;
const MID_Y: f32 = (Y_MIN + Y_MAX) / 2.0;
/// Corner pockets take a ball once its distances to the two cushions add up
/// to no more than this.
const CORNER_MOUTH: f32 = 2.5;
/// Side pockets take a ball against the cushion within this of the middle.
const SIDE_MOUTH: f32 = 3.0;
const SIDE_DEPTH: f32 = 0.5;
/// Where the pet aims an object ball for each pocket: just past the mouth.
const POCKETS: [(f32, f32); 6] = [
    (X_MIN - 1.5, Y_MIN - 1.5),
    (MID_X, Y_MIN - 2.0),
    (X_MAX + 1.5, Y_MIN - 1.5),
    (X_MIN - 1.5, Y_MAX + 1.5),
    (MID_X, Y_MAX + 2.0),
    (X_MAX + 1.5, Y_MAX + 1.5),
];

const HEAD_X: f32 = 32.0;
const FOOT_X: f32 = 92.0;
/// Rows of the rack, a whole ball apart so the sprites line up.
const ROW_DX: f32 = 5.0;

// Balls: 0 is the cue ball, 1..=7 solids, 8 the eight, 9..=15 stripes.
const N: usize = 16;
const CUE: usize = 0;
const EIGHT: usize = 8;

// Physics, in px and seconds.
const BALL_R: f32 = 2.5;
const FRICTION: f32 = 30.0;
const V_MAX: f32 = 240.0;
const V_MIN: f32 = 12.0;
const BALL_RESTITUTION: f32 = 1.0;
const CUSHION_RESTITUTION: f32 = 0.8;
/// The break is hit harder than any other shot, so the rack opens up.
const BREAK_BOOST: f32 = 1.4;
/// Longest substep, kept under the ball radius so balls can't pass through
/// each other.
const MAX_STEP: f32 = 1.5;

const POWER_PERIOD: f32 = 1.4;
const AIM_TAP: f32 = 1.5 * PI / 180.0;
const AIM_FINE: f32 = 0.3 * PI / 180.0;
const AIM_RATE: f32 = 60.0 * PI / 180.0;
const AIM_FINE_RATE: f32 = 4.0 * PI / 180.0;
const AIM_HOLD_DELAY: f32 = 0.25;
/// Radius of the ring kept round the cue ball so it stands out.
const AIM_RING: i32 = 5;
/// Length of the line showing where a struck ball heads.
const OBJECT_LINE: f32 = 10.0;

const PET_THINK: f32 = 0.6;
const PET_AIM_RATE: f32 = 70.0 * PI / 180.0;
const PET_SETTLE: f32 = 0.4;
/// Cut angles past this (cosine of ~75°) are too thin for the pet to try.
const PET_MAX_CUT_COS: f32 = 0.26;

const NOTICE_TIME: f32 = 1.0;
const NOTICE_LINE_TIME: f32 = 1.0;
const GAME_OVER_DELAY: f32 = 0.5;

const PLAYER: u8 = 0;
const PET: u8 = 1;

// 5x5 sprites, bit 4 is the left column. The corners are left alone.
const SOLID: [u8; 5] = [0b01110, 0b11111, 0b11111, 0b11111, 0b01110];
const STRIPE: [u8; 5] = [0b01110, 0b10001, 0b10001, 0b10001, 0b01110];
const CUE_BALL: [u8; 5] = [0b01110, 0b10001, 0b10101, 0b10001, 0b01110];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Cue,
    Solid,
    Stripe,
    Eight,
}

fn kind(i: usize) -> Kind {
    match i {
        CUE => Kind::Cue,
        EIGHT => Kind::Eight,
        1..=7 => Kind::Solid,
        _ => Kind::Stripe,
    }
}

fn other_group(g: Kind) -> Kind {
    if g == Kind::Solid { Kind::Stripe } else { Kind::Solid }
}

fn other(who: u8) -> u8 {
    1 - who
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScratchAction {
    NewGame,
}

const OPTIONS_ITEMS: &[MenuItem<ScratchAction>] = &[
    MenuItem { label: t!("New Game"), icon: None, submenu: None, action: Some(ScratchAction::NewGame), confirm: None, confirm_on_vacation: None },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// A popup about the last shot, then the next turn.
    Notice,
    Aim,
    Power,
    PetThink,
    PetAim,
    PetPower,
    Rolling,
    GameOver,
}

// ---------------------------------------------------------------------------
// Ball physics.

#[derive(Clone, Copy, Debug)]
struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    on: bool,
}

impl Ball {
    const fn off() -> Self {
        Self { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, on: false }
    }

    const fn at(x: f32, y: f32) -> Self {
        Self { x, y, vx: 0.0, vy: 0.0, on: true }
    }

    fn moving(&self) -> bool {
        self.on && (self.vx != 0.0 || self.vy != 0.0)
    }
}

/// What happened during a shot, for the rules.
struct ShotLog {
    first_hit: Option<u8>,
    potted: Vec<u8, N>,
}

impl ShotLog {
    const fn new() -> Self {
        Self { first_hit: None, potted: Vec::new() }
    }
}

fn in_pocket(x: f32, y: f32) -> bool {
    let dx = if x < MID_X { x - X_MIN } else { X_MAX - x };
    let dy = if y < MID_Y { y - Y_MIN } else { Y_MAX - y };
    dx + dy <= CORNER_MOUTH || (dy <= SIDE_DEPTH && (x - MID_X).abs() <= SIDE_MOUTH)
}

/// Advance every ball by `dt`. Returns whether any are still rolling.
fn step(balls: &mut [Ball; N], dt: f32, log: &mut ShotLog) -> bool {
    let fastest = balls
        .iter()
        .filter(|b| b.moving())
        .map(|b| b.vx * b.vx + b.vy * b.vy)
        .fold(0.0, f32::max)
        .sqrt();
    if fastest == 0.0 {
        return false;
    }
    let n = ((fastest * dt / MAX_STEP).ceil() as u32).clamp(1, 64);
    let h = dt / n as f32;
    for _ in 0..n {
        for b in balls.iter_mut().filter(|b| b.moving()) {
            let s = (b.vx * b.vx + b.vy * b.vy).sqrt();
            let k = (s - FRICTION * h).max(0.0) / s;
            b.vx *= k;
            b.vy *= k;
            b.x += b.vx * h;
            b.y += b.vy * h;
        }
        collide_balls(balls, h, log);
        for b in balls.iter_mut().filter(|b| b.on) {
            cushions(b);
        }
        for (i, b) in balls.iter_mut().enumerate() {
            if b.on && in_pocket(b.x, b.y) {
                *b = Ball { on: false, ..Ball::at(b.x, b.y) };
                let _ = log.potted.push(i as u8);
            }
        }
    }
    balls.iter().any(|b| b.moving())
}

/// Balls are found overlapping after a substep, so each pair is wound back
/// to the moment they touched and bounced from there. Bouncing from where
/// they overlap would send them off at the wrong angle, and the projection
/// would be off by several degrees on thin cuts.
fn collide_balls(balls: &mut [Ball; N], h: f32, log: &mut ShotLog) {
    const D: f32 = 2.0 * BALL_R;
    for i in 0..N {
        for j in i + 1..N {
            let (a, b) = (balls[i], balls[j]);
            if !a.on || !b.on {
                continue;
            }
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            if dx.abs() >= D || dy.abs() >= D {
                continue;
            }
            let d2 = dx * dx + dy * dy;
            if d2 >= D * D {
                continue;
            }
            // Time since they touched: |d + w·back| = D, w the closing velocity.
            let (wx, wy) = (a.vx - b.vx, a.vy - b.vy);
            let w2 = wx * wx + wy * wy;
            let dw = dx * wx + dy * wy;
            let back = if w2 > 1e-6 && dw > 0.0 {
                ((-dw + (dw * dw - w2 * (d2 - D * D)).sqrt()) / w2).min(h)
            } else {
                0.0
            };
            let (a, b) = (&mut balls[i..=j]).split_at_mut(j - i);
            let (a, b) = (&mut a[0], &mut b[0]);
            a.x -= a.vx * back;
            a.y -= a.vy * back;
            b.x -= b.vx * back;
            b.y -= b.vy * back;
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let d = (dx * dx + dy * dy).sqrt();
            let (nx, ny) = if d > 1e-4 { (dx / d, dy / d) } else { (1.0, 0.0) };
            // Whatever overlap is left (resting balls nudged together) is
            // pushed apart.
            let push = (D - d).max(0.0) / 2.0;
            let vn = (a.vx - b.vx) * nx + (a.vy - b.vy) * ny;
            let imp = if vn > 0.0 { (1.0 + BALL_RESTITUTION) / 2.0 * vn } else { 0.0 };
            a.vx -= imp * nx;
            a.vy -= imp * ny;
            b.vx += imp * nx;
            b.vy += imp * ny;
            a.x += a.vx * back - nx * push;
            a.y += a.vy * back - ny * push;
            b.x += b.vx * back + nx * push;
            b.y += b.vy * back + ny * push;
            if i == CUE && log.first_hit.is_none() {
                log.first_hit = Some(j as u8);
            }
        }
    }
}

/// Bounce off a cushion. The distance travelled past it comes back out
/// at the bounced velocity, so the path matches the projection.
fn cushions(b: &mut Ball) {
    let e = CUSHION_RESTITUTION;
    if b.x < X_MIN {
        b.x = (X_MIN + (X_MIN - b.x) * e).min(X_MAX);
        b.vx = b.vx.abs() * e;
    } else if b.x > X_MAX {
        b.x = (X_MAX - (b.x - X_MAX) * e).max(X_MIN);
        b.vx = -b.vx.abs() * e;
    }
    if b.y < Y_MIN {
        b.y = (Y_MIN + (Y_MIN - b.y) * e).min(Y_MAX);
        b.vy = b.vy.abs() * e;
    } else if b.y > Y_MAX {
        b.y = (Y_MAX - (b.y - Y_MAX) * e).max(Y_MIN);
        b.vy = -b.vy.abs() * e;
    }
}

/// Put the fifteen object balls in a triangle on the foot spot, the eight in
/// the middle and one of each group in the back corners, and the cue ball
/// on the head spot.
fn rack(balls: &mut [Ball; N], rng: &mut u32) {
    let mut rest: Vec<u8, 12> = (2..=7).chain(10..=15).collect();
    for i in (1..rest.len()).rev() {
        let j = rand_range_u32(rng, 0, i as u32) as usize;
        rest.swap(i, j);
    }
    let (left, right) = if rand_bool(rng, 0.5) { (1, 9) } else { (9, 1) };
    let mut rest = rest.into_iter();
    let mut slot = 0;
    for row in 0..5 {
        for k in 0..=row {
            let id = match slot {
                4 => EIGHT as u8,
                10 => left,
                14 => right,
                _ => rest.next().unwrap(),
            };
            balls[id as usize] = Ball::at(
                FOOT_X + row as f32 * ROW_DX,
                MID_Y + (k as f32 - row as f32 / 2.0) * 2.0 * BALL_R,
            );
            slot += 1;
        }
    }
    balls[CUE] = Ball::at(HEAD_X, MID_Y);
}

fn spot_free(balls: &[Ball; N], x: f32, y: f32) -> bool {
    let d = 2.0 * BALL_R + 0.2;
    balls.iter().all(|b| !b.on || (b.x - x) * (b.x - x) + (b.y - y) * (b.y - y) >= d * d)
}

/// Back on the head spot, or the nearest free place round it.
fn respot_cue(balls: &mut [Ball; N]) {
    for dx in [0.0, -6.0, 6.0, -12.0, 12.0] {
        for dy in [0.0, 6.0, -6.0, 12.0, -12.0, 18.0, -18.0, 24.0, -24.0] {
            let (x, y) = (HEAD_X + dx, MID_Y + dy);
            if spot_free(balls, x, y) {
                balls[CUE] = Ball::at(x, y);
                return;
            }
        }
    }
    balls[CUE] = Ball::at(HEAD_X, MID_Y);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Ball(usize),
    /// A cushion; true for the left or right one.
    Cushion(bool),
}

/// How far a ball at `(x, y)` travels along unit `(c, s)` before it touches
/// another ball or a cushion.
fn cast(balls: &[Ball; N], x: f32, y: f32, c: f32, s: f32, skip: usize) -> (f32, Hit) {
    let tx = if c > 1e-6 { (X_MAX - x) / c } else if c < -1e-6 { (X_MIN - x) / c } else { f32::MAX };
    let ty = if s > 1e-6 { (Y_MAX - y) / s } else if s < -1e-6 { (Y_MIN - y) / s } else { f32::MAX };
    let mut best = if tx < ty { (tx.max(0.0), Hit::Cushion(true)) } else { (ty.max(0.0), Hit::Cushion(false)) };
    const D2: f32 = 4.0 * BALL_R * BALL_R;
    for (j, b) in balls.iter().enumerate() {
        if !b.on || j == skip {
            continue;
        }
        let (px, py) = (x - b.x, y - b.y);
        let half_b = c * px + s * py;
        let cc = px * px + py * py - D2;
        let t = if cc < 0.0 {
            if half_b >= 0.0 {
                continue;
            }
            0.0
        } else {
            let disc = half_b * half_b - cc;
            if disc < 0.0 {
                continue;
            }
            -half_b - disc.sqrt()
        };
        if t >= 0.0 && t < best.0 {
            best = (t, Hit::Ball(j));
        }
    }
    best
}

/// Nothing but `skip` lies in the way of a ball rolling from a to b.
fn path_clear(balls: &[Ball; N], a: (f32, f32), b: (f32, f32), skip: [usize; 2]) -> bool {
    let (ex, ey) = (b.0 - a.0, b.1 - a.1);
    let len2 = ex * ex + ey * ey;
    let d = 2.0 * BALL_R - 0.2;
    balls.iter().enumerate().all(|(j, q)| {
        if !q.on || skip.contains(&j) {
            return true;
        }
        let t = if len2 > 0.0 { (((q.x - a.0) * ex + (q.y - a.1) * ey) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let (cx, cy) = (a.0 + ex * t - q.x, a.1 + ey * t - q.y);
        cx * cx + cy * cy >= d * d
    })
}

/// Gauge reading, 0..1, swinging smoothly up and down.
fn gauge(phase: f32) -> f32 {
    (1.0 - phase.cos()) / 2.0
}

fn speed_for(g: f32) -> f32 {
    V_MIN + g * (V_MAX - V_MIN)
}

/// Shortest turn from angle a to angle b.
fn angle_diff(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(2.0 * PI);
    if d > PI { d - 2.0 * PI } else { d }
}

fn px(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

// ---------------------------------------------------------------------------

pub struct ScratchScene {
    balls: [Ball; N],
    state: State,
    timer: f32,
    shooter: u8,
    /// The player's group, once someone has potted a ball.
    player_group: Option<Kind>,
    is_break: bool,
    /// The shooter had cleared their group when the shot was played.
    on_eight: bool,
    log: ShotLog,

    aim: f32,
    aim_hold: f32,
    phase: f32,

    pet_skill: f32,
    pet_aim: f32,
    pet_gauge: f32,

    popup: Popup,
    /// Popup line the "You: <group>" line is on, to draw ball sprites by.
    group_line: Option<usize>,

    game_no: u32,
    wins: [u32; 2],
    session_games: u32,
    session_wins: u32,

    options_menu: Menu<ScratchAction>,
    menu_active: bool,
}

impl ScratchScene {
    pub fn new() -> Self {
        Self {
            balls: [Ball::off(); N],
            state: State::Notice,
            timer: 0.0,
            shooter: PLAYER,
            player_group: None,
            is_break: true,
            on_eight: false,
            log: ShotLog::new(),

            aim: 0.0,
            aim_hold: 0.0,
            phase: 0.0,

            pet_skill: 0.5,
            pet_aim: 0.0,
            pet_gauge: 0.0,

            popup: Popup::new(2, 22, 124, 20),
            group_line: None,

            game_no: 0,
            wins: [0; 2],
            session_games: 0,
            session_wins: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn new_game(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        rack(&mut self.balls, &mut rng);
        ctx.rng = rng;
        self.shooter = if self.game_no % 2 == 0 { PLAYER } else { PET };
        self.player_group = None;
        self.is_break = true;
        self.aim = 0.0;
        self.show_notice(&[self.turn_text()], None);
    }

    fn group_of(&self, who: u8) -> Option<Kind> {
        self.player_group.map(|g| if who == PLAYER { g } else { other_group(g) })
    }

    fn left_in(&self, g: Kind) -> usize {
        (1..N).filter(|&i| kind(i) == g && self.balls[i].on).count()
    }

    /// Balls the shooter may legally hit first.
    fn targets(&self, who: u8) -> impl Iterator<Item = usize> + '_ {
        let own = self.group_of(who);
        let on_eight = own.map_or(false, |g| self.left_in(g) == 0);
        (1..N).filter(move |&i| {
            self.balls[i].on
                && match own {
                    _ if on_eight => i == EIGHT,
                    None => i != EIGHT,
                    Some(g) => kind(i) == g,
                }
        })
    }

    fn turn_text(&self) -> &'static str {
        if self.shooter == PLAYER { t!("Your shot") } else { t!("Pet's shot") }
    }

    fn group_name(g: Kind) -> &'static str {
        if g == Kind::Solid { t!("Solids") } else { t!("Stripes") }
    }

    /// Popup of a few lines, sized to fit, before the next turn starts.
    fn show_notice(&mut self, lines: &[&str], group_line: Option<usize>) {
        let mut text: String<96> = String::new();
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                let _ = text.push('\n');
            }
            let _ = text.push_str(l);
        }
        self.size_popup(lines.len());
        self.popup.set_text(&text, false, true);
        self.group_line = group_line;
        self.state = State::Notice;
        self.timer = NOTICE_TIME + NOTICE_LINE_TIME * (lines.len() - 1) as f32;
    }

    fn size_popup(&mut self, lines: usize) {
        let h = lines as i32 * 10 + 2 * self.popup.padding;
        self.popup.height = h as u32;
        self.popup.y = (SCREEN_H - h) / 2;
    }

    fn begin_turn(&mut self) {
        self.timer = 0.0;
        self.state = if self.shooter == PLAYER { State::Aim } else { State::PetThink };
    }

    fn shoot(&mut self, g: f32) {
        let v = speed_for(g) * if self.is_break { BREAK_BOOST } else { 1.0 };
        let cue = &mut self.balls[CUE];
        cue.vx = self.aim.cos() * v;
        cue.vy = self.aim.sin() * v;
        self.on_eight = self.group_of(self.shooter).map_or(false, |g| self.left_in(g) == 0);
        self.log = ShotLog::new();
        self.state = State::Rolling;
    }

    /// Apply the rules once everything has stopped.
    fn resolve(&mut self, ctx: &mut GameContext) {
        let who = self.shooter;
        let own = self.group_of(who);
        let potted = |i: usize| self.log.potted.contains(&(i as u8));
        let scratch = potted(CUE);
        let foul = if scratch {
            Some(t!("Cue ball sunk"))
        } else {
            match self.log.first_hit.map(|i| kind(i as usize)) {
                None => Some(t!("No ball hit")),
                Some(Kind::Eight) if !self.on_eight => Some(t!("Hit the 8 first")),
                Some(Kind::Eight) => None,
                Some(k) if self.on_eight || own.map_or(false, |g| k != g) => Some(t!("Wrong ball first")),
                _ => None,
            }
        };

        if potted(EIGHT) {
            if self.is_break {
                let mut rng = ctx.rng;
                rack(&mut self.balls, &mut rng);
                ctx.rng = rng;
                self.aim = 0.0;
                self.show_notice(&[t!("Re-rack"), self.turn_text()], None);
            } else {
                if self.on_eight && foul.is_none() {
                    self.game_over(who, None);
                } else {
                    let why = if !self.on_eight {
                        t!("8 too early")
                    } else if scratch {
                        t!("Scratch on 8")
                    } else {
                        t!("Foul on 8")
                    };
                    self.game_over(other(who), Some((who, why)));
                }
            }
            return;
        }
        if scratch {
            respot_cue(&mut self.balls);
        }

        let mut assigned = false;
        if self.player_group.is_none() && foul.is_none() {
            if let Some(&b) = self.log.potted.iter().find(|&&b| b as usize != CUE) {
                let k = kind(b as usize);
                self.player_group = Some(if who == PLAYER { k } else { other_group(k) });
                assigned = true;
            }
        }
        let own = self.group_of(who);
        let keep = foul.is_none() && own.map_or(false, |g| self.log.potted.iter().any(|&b| kind(b as usize) == g));
        self.is_break = false;
        if !keep {
            self.shooter = other(who);
        }

        let (mut you, mut pet): (String<24>, String<24>) = (String::new(), String::new());
        let mut lines: Vec<&str, 4> = Vec::new();
        let mut group_line = None;
        if let Some(why) = foul {
            let _ = lines.push(if scratch { t!("Scratch!") } else { t!("Foul!") });
            let _ = lines.push(why);
        }
        if assigned {
            let g = self.player_group.unwrap();
            let _ = write!(you, "{}: {}", t!("You"), Self::group_name(g));
            let _ = write!(pet, "{}: {}", t!("Pet"), Self::group_name(other_group(g)));
            group_line = Some(lines.len());
            let _ = lines.push(&you);
            let _ = lines.push(&pet);
        }
        if !lines.is_empty() || !keep {
            let _ = lines.push(self.turn_text());
            self.show_notice(&lines, group_line);
        } else {
            self.begin_turn();
        }
    }

    /// `lost_by` is who gave the game away on the 8 and how, if they did.
    fn game_over(&mut self, winner: u8, lost_by: Option<(u8, &str)>) {
        self.wins[winner as usize] += 1;
        self.session_games += 1;
        if winner == PLAYER {
            self.session_wins += 1;
        }
        let mut text: String<96> = String::new();
        let _ = text.push_str(if winner == PLAYER { t!("You Win!") } else { t!("You Lose!") });
        if let Some((who, why)) = lost_by {
            let _ = write!(text, "\n{}: {}", if who == PLAYER { t!("You") } else { t!("Pet") }, why);
        }
        let _ = write!(text, "\n{}-{}\n", self.wins[0], self.wins[1]);
        let _ = text.push_str(t!("A: New Game"));
        self.size_popup(3 + lost_by.is_some() as usize);
        self.popup.set_text(&text, false, true);
        self.group_line = None;
        self.state = State::GameOver;
        self.timer = 0.0;
    }

    fn handle_menu(&mut self, ctx: &mut GameContext, buttons: &mut Buttons) {
        match self.options_menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => self.menu_active = false,
            MenuResult::Action(action) => {
                self.menu_active = false;
                match action {
                    ScratchAction::NewGame => self.new_game(ctx),
                }
            }
        }
    }

    /// Left/right turn coarsely and up/down finely: a tap steps, a hold
    /// keeps turning after a moment.
    fn update_aim(&mut self, buttons: &mut Buttons, dt: f32) {
        const KEYS: [(Button, f32, f32); 4] = [
            (Button::Left, -AIM_TAP, -AIM_RATE),
            (Button::Right, AIM_TAP, AIM_RATE),
            (Button::Up, -AIM_FINE, -AIM_FINE_RATE),
            (Button::Down, AIM_FINE, AIM_FINE_RATE),
        ];
        if let Some(&(_, tap, _)) = KEYS.iter().find(|k| buttons.was_just_pressed(k.0)) {
            self.aim += tap;
            self.aim_hold = 0.0;
        } else if let Some(&(_, _, rate)) = KEYS.iter().find(|k| buttons.is_pressed(k.0)) {
            self.aim_hold += dt;
            if self.aim_hold > AIM_HOLD_DELAY {
                self.aim += rate * dt;
            }
        }
        self.aim = self.aim.rem_euclid(2.0 * PI);
    }

    // --- the pet's shots ----------------------------------------------------

    /// Pick a shot, then spoil it a little by how sharp the pet is.
    fn plan_pet_shot(&mut self, rng: &mut u32) {
        let (aim, g) = self.best_shot();
        let miss = 1.0 - self.pet_skill;
        let spread = (0.5 + 5.0 * miss) * PI / 180.0;
        self.pet_aim = (aim + rand_range_f32(rng, -spread, spread)).rem_euclid(2.0 * PI);
        let wobble = 0.12 * miss;
        self.pet_gauge = (g + rand_range_f32(rng, -wobble, wobble)).clamp(0.05, 1.0);
    }

    /// Ghost-ball aiming: for each legal ball and pocket, the spot the cue
    /// ball must reach to send the ball at the pocket. The straightest
    /// clear shot wins.
    fn best_shot(&self) -> (f32, f32) {
        let cue = self.balls[CUE];
        let toward = |x: f32, y: f32| (y - cue.y).atan2(x - cue.x);
        if self.is_break {
            // The apex of the rack, full power.
            let apex = (FOOT_X, MID_Y);
            return (toward(apex.0, apex.1), 1.0);
        }
        let mut best: Option<(f32, f32, f32)> = None;
        for i in self.targets(self.shooter) {
            let b = self.balls[i];
            for (p, &(px, py)) in POCKETS.iter().enumerate() {
                let (ex, ey) = (px - b.x, py - b.y);
                let to_pocket = (ex * ex + ey * ey).sqrt();
                let (ux, uy) = (ex / to_pocket, ey / to_pocket);
                // Side pockets only take balls coming in fairly square on.
                if (p == 1 || p == 4) && uy.abs() < 0.6 {
                    continue;
                }
                let gx = b.x - ux * 2.0 * BALL_R;
                let gy = b.y - uy * 2.0 * BALL_R;
                if gx < X_MIN || gx > X_MAX || gy < Y_MIN || gy > Y_MAX {
                    continue;
                }
                let (cx, cy) = (gx - cue.x, gy - cue.y);
                let to_ghost = (cx * cx + cy * cy).sqrt();
                if to_ghost < 0.5 {
                    continue;
                }
                let cut = (cx * ux + cy * uy) / to_ghost;
                if cut < PET_MAX_CUT_COS
                    || !path_clear(&self.balls, (cue.x, cue.y), (gx, gy), [CUE, i])
                    || !path_clear(&self.balls, (b.x, b.y), (px, py), [CUE, i])
                {
                    continue;
                }
                let score = cut * 100.0 - (to_ghost + to_pocket) * 0.3;
                if best.map_or(true, |(s, _, _)| score > s) {
                    // Enough pace for the object ball to reach the pocket
                    // with some to spare, after the cue ball rolls there
                    // and gives up its speed along the line of centres.
                    let v_obj2 = 2.0 * FRICTION * (to_pocket + 25.0);
                    let at_contact = v_obj2.sqrt() / (cut * (1.0 + BALL_RESTITUTION) / 2.0);
                    let v = (at_contact * at_contact + 2.0 * FRICTION * to_ghost).sqrt();
                    let g = ((v - V_MIN) / (V_MAX - V_MIN)).clamp(0.1, 1.0);
                    best = Some((score, toward(gx, gy), g));
                }
            }
        }
        if let Some((_, aim, g)) = best {
            return (aim, g);
        }
        // Nothing on: hit the nearest legal ball and hope.
        let near = self.targets(self.shooter).min_by(|&a, &b| {
            let d = |i: usize| {
                let (dx, dy) = (self.balls[i].x - cue.x, self.balls[i].y - cue.y);
                dx * dx + dy * dy
            };
            d(a).partial_cmp(&d(b)).unwrap_or(core::cmp::Ordering::Equal)
        });
        match near {
            Some(i) => (toward(self.balls[i].x, self.balls[i].y), 0.55),
            None => (self.aim, 0.5),
        }
    }

    // --- drawing -----------------------------------------------------------

    fn draw_table(&self, r: &mut Renderer) {
        let (l, rt, t, b) = (1, SCREEN_W - 2, 1, SCREEN_H - 2);
        let mid = SCREEN_W / 2;
        // Cushions, broken at the pockets.
        for y in [t, b] {
            r.draw_line(Point::new(4, y), Point::new(mid - 4, y));
            r.draw_line(Point::new(mid + 3, y), Point::new(SCREEN_W - 5, y));
        }
        for x in [l, rt] {
            r.draw_line(Point::new(x, 4), Point::new(x, SCREEN_H - 5));
        }
        // Pockets, set out to the screen edge.
        for (y, edge) in [(t, 0), (b, SCREEN_H - 1)] {
            r.draw_line(Point::new(mid - 3, edge), Point::new(mid + 2, edge));
            for (x, side) in [(l, 0), (rt, SCREEN_W - 1)] {
                let dx = if x == l { 1 } else { -1 };
                let dy = if y == t { 1 } else { -1 };
                r.draw_line(Point::new(x, edge), Point::new(x + dx * 2, edge));
                r.draw_line(Point::new(side, y), Point::new(side, y + dy * 2));
            }
        }
    }

    fn draw_ball(r: &mut Renderer, mask: &[u8; 5], cx: i32, cy: i32) {
        for (row, bits) in mask.iter().enumerate() {
            for col in 0..5 {
                if (row == 0 || row == 4) && (col == 0 || col == 4) {
                    continue;
                }
                let on = bits & (0b10000 >> col) != 0;
                r.draw_pixel(Point::new(cx - 2 + col, cy - 2 + row as i32), on);
            }
        }
    }

    fn mask(i: usize) -> &'static [u8; 5] {
        match kind(i) {
            Kind::Cue | Kind::Eight => &CUE_BALL,
            Kind::Solid => &SOLID,
            Kind::Stripe => &STRIPE,
        }
    }

    fn draw_balls(&self, r: &mut Renderer) {
        for (i, b) in self.balls.iter().enumerate() {
            if b.on {
                Self::draw_ball(r, Self::mask(i), px(b.x), px(b.y));
            }
        }
    }

    /// A dotted ring where the cue ball will touch another.
    fn draw_ghost(r: &mut Renderer, x: f32, y: f32) {
        let (cx, cy) = (px(x), px(y));
        for (row, bits) in STRIPE.iter().enumerate() {
            for col in 0..5 {
                if bits & (0b10000 >> col) != 0 && (row as i32 + col) % 2 == 0 {
                    r.draw_pixel(Point::new(cx - 2 + col, cy - 2 + row as i32), true);
                }
            }
        }
    }

    fn dotted(r: &mut Renderer, a: (f32, f32), b: (f32, f32)) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = dx.abs().max(dy.abs()).ceil() as i32;
        for i in (0..=n).step_by(2) {
            let t = if n > 0 { i as f32 / n as f32 } else { 0.0 };
            r.draw_pixel(Point::new(px(a.0 + dx * t), px(a.1 + dy * t)), true);
        }
    }

    /// Show where the cue ball will go: on to the first ball it meets, or
    /// off one cushion to whatever it meets next.
    fn draw_projection(&self, r: &mut Renderer) {
        let cue = self.balls[CUE];
        let (mut c, mut s) = (self.aim.cos(), self.aim.sin());
        let (mut x, mut y) = (cue.x, cue.y);
        // Start the line clear of the aim ring.
        let mut skip_len = (AIM_RING + 2) as f32;
        for bounce in 0..2 {
            let (t, hit) = cast(&self.balls, x, y, c, s, CUE);
            let (hx, hy) = (x + c * t, y + s * t);
            if t > skip_len {
                Self::dotted(r, (x + c * skip_len, y + s * skip_len), (hx, hy));
            }
            skip_len = 0.0;
            match hit {
                Hit::Ball(j) => {
                    Self::draw_ghost(r, hx, hy);
                    let b = self.balls[j];
                    let (nx, ny) = ((b.x - hx) / (2.0 * BALL_R), (b.y - hy) / (2.0 * BALL_R));
                    let from = (b.x + nx * (BALL_R + 2.0), b.y + ny * (BALL_R + 2.0));
                    Self::dotted(r, from, (from.0 + nx * OBJECT_LINE, from.1 + ny * OBJECT_LINE));
                    return;
                }
                Hit::Cushion(side) => {
                    if bounce == 1 || in_pocket(hx, hy) {
                        return;
                    }
                    // The cushion takes some of the speed into it, so the
                    // ball comes off at a steeper angle than a mirror.
                    if side { c = -c * CUSHION_RESTITUTION } else { s = -s * CUSHION_RESTITUTION }
                    let l = (c * c + s * s).sqrt();
                    (c, s) = (c / l, s / l);
                    (x, y) = (hx, hy);
                }
            }
        }
    }

    fn draw_power(&self, r: &mut Renderer, g: f32) {
        const W: i32 = 5;
        const H: i32 = 23;
        let (bx, by) = (px(self.balls[CUE].x), px(self.balls[CUE].y));
        let x = if bx - AIM_RING - 3 - W >= 0 { bx - AIM_RING - 3 - W } else { bx + AIM_RING + 3 };
        let y = (by - H / 2).clamp(0, SCREEN_H - H);
        r.fill_rect_off(Point::new(x - 1, y - 1), Size::new(W as u32 + 2, H as u32 + 2));
        r.draw_rect(Point::new(x, y), Size::new(W as u32, H as u32), false);
        let fill = (g * (H - 2) as f32 + 0.5) as i32;
        if fill > 0 {
            r.draw_rect(Point::new(x + 1, y + H - 1 - fill), Size::new(W as u32 - 2, fill as u32), true);
        }
    }

    /// A cat head above the cue ball (below, near the top cushion) so it's
    /// clear the pet is the one shooting.
    fn draw_pet_mark(&self, r: &mut Renderer) {
        let (bx, by) = (px(self.balls[CUE].x), px(self.balls[CUE].y));
        let above = by - AIM_RING - 10;
        let y = if above >= 0 { above } else { by + AIM_RING + 2 };
        let x = (bx - 3).clamp(0, SCREEN_W - 6);
        r.fill_rect_off(Point::new(x - 1, y - 1), Size::new(8, 10));
        r.draw_sprite_raw(CAT_THIN_DOWN, 6, 8, Point::new(x, y), SpriteOpts::default());
    }

    /// Ball sprites beside the "You: Solids" / "Pet: Stripes" lines.
    fn draw_group_balls(&self, r: &mut Renderer) {
        let (Some(line), Some(g)) = (self.group_line, self.player_group) else { return };
        let p = &self.popup;
        for (k, group) in [g, other_group(g)].into_iter().enumerate() {
            let y = p.y + p.padding + (line + k) as i32 * 10 + 3;
            let x = p.x + p.padding + 2;
            let mask = if group == Kind::Solid { &SOLID } else { &STRIPE };
            Self::draw_ball(r, mask, x, y);
        }
    }
}

impl Scene for ScratchScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let focus = ctx.focus.clamp(0.0, 100.0) / 100.0;
        let intelligence = ctx.intelligence.clamp(0.0, 100.0) / 100.0;
        self.pet_skill = 0.6 * focus + 0.4 * intelligence;
        self.game_no = 0;
        self.wins = [0; 2];
        self.session_games = 0;
        self.session_wins = 0;
        self.menu_active = false;
        self.new_game(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.session_games == 0 {
            return;
        }
        let scale = (self.session_games as f32 / 3.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Focus,        4.0 * scale),
            (StatId::Playfulness,  3.0 * scale),
            (StatId::Sociability,  3.0 * scale),
            (StatId::Intelligence, 2.0 * scale),
        ]);
        ctx.coins += 3 * self.session_games as i32 + 2 * self.session_wins as i32;
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

        if self.menu_active {
            self.handle_menu(ctx, buttons);
            return None;
        }

        if buttons.was_just_pressed(Button::Menu2) {
            self.menu_active = true;
            self.options_menu.reset_to(OPTIONS_ITEMS);
            return None;
        }

        match self.state {
            State::Notice => {
                self.timer -= dt;
                if buttons.was_just_pressed(Button::A) || self.timer <= 0.0 {
                    self.begin_turn();
                }
            }
            State::Aim => {
                self.update_aim(buttons, dt);
                if buttons.was_just_pressed(Button::A) {
                    self.phase = 0.0;
                    self.state = State::Power;
                }
            }
            State::Power => {
                self.phase = (self.phase + dt * 2.0 * PI / POWER_PERIOD) % (2.0 * PI);
                if buttons.was_just_pressed(Button::A) {
                    self.shoot(gauge(self.phase));
                } else if buttons.was_just_pressed(Button::B) {
                    self.state = State::Aim;
                }
            }
            State::PetThink => {
                self.timer += dt;
                if self.timer >= PET_THINK {
                    let mut rng = ctx.rng;
                    self.plan_pet_shot(&mut rng);
                    ctx.rng = rng;
                    self.timer = 0.0;
                    self.state = State::PetAim;
                }
            }
            State::PetAim => {
                let d = angle_diff(self.aim, self.pet_aim);
                let turn = PET_AIM_RATE * dt;
                if d.abs() > turn {
                    self.aim = (self.aim + if d > 0.0 { turn } else { -turn }).rem_euclid(2.0 * PI);
                } else {
                    self.aim = self.pet_aim;
                    self.timer += dt;
                    if self.timer >= PET_SETTLE {
                        self.phase = 0.0;
                        self.state = State::PetPower;
                    }
                }
            }
            State::PetPower => {
                // Let go on the first upswing past the planned reading.
                let release = (1.0 - 2.0 * self.pet_gauge).clamp(-1.0, 1.0).acos();
                self.phase += dt * 2.0 * PI / POWER_PERIOD;
                if self.phase >= release {
                    self.phase = release;
                    self.shoot(self.pet_gauge);
                }
            }
            State::Rolling => {
                if !step(&mut self.balls, dt, &mut self.log) {
                    self.resolve(ctx);
                }
            }
            State::GameOver => {
                self.timer += dt;
                if self.timer >= GAME_OVER_DELAY && buttons.was_just_pressed(Button::A) {
                    self.game_no += 1;
                    self.new_game(ctx);
                }
            }
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }
        self.draw_table(renderer);
        let aiming = matches!(self.state, State::Aim | State::Power | State::PetAim | State::PetPower);
        if aiming {
            self.draw_projection(renderer);
        }
        let cue = self.balls[CUE];
        if cue.on {
            renderer.draw_circle_outline(Point::new(px(cue.x), px(cue.y)), AIM_RING);
        }
        self.draw_balls(renderer);
        if matches!(self.state, State::PetThink | State::PetAim | State::PetPower) {
            self.draw_pet_mark(renderer);
        }
        match self.state {
            State::Power | State::PetPower => self.draw_power(renderer, gauge(self.phase)),
            State::Notice | State::GameOver => {
                self.popup.draw(renderer, false);
                self.draw_group_balls(renderer);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_breaks_keep_balls_on_the_table() {
        for seed in 1..40u32 {
            let mut rng = seed.wrapping_mul(2654435761);
            let mut balls = [Ball::off(); N];
            rack(&mut balls, &mut rng);
            let a = rand_range_f32(&mut rng, -0.3, 0.3);
            balls[CUE].vx = a.cos() * V_MAX * BREAK_BOOST;
            balls[CUE].vy = a.sin() * V_MAX * BREAK_BOOST;
            let mut log = ShotLog::new();
            let mut frames = 0;
            while step(&mut balls, 1.0 / 12.0, &mut log) {
                frames += 1;
                assert!(frames < 2000, "seed {} never stopped", seed);
                for b in balls.iter().filter(|b| b.on) {
                    assert!(b.x.is_finite() && b.y.is_finite(), "seed {} {:?}", seed, b);
                    assert!((X_MIN..=X_MAX).contains(&b.x) && (Y_MIN..=Y_MAX).contains(&b.y), "seed {} {:?}", seed, b);
                }
            }
            for i in 0..N {
                for j in i + 1..N {
                    let (a, b) = (balls[i], balls[j]);
                    if a.on && b.on {
                        let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                        assert!(d > 2.0 * BALL_R - 0.5, "seed {} balls {} {} at {}", seed, i, j, d);
                    }
                }
            }
        }
    }
}
