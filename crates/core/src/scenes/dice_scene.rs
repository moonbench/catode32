//! Dice tool: dice, coins, or a number from a range. The cat throws, the
//! result lands on the table, and the total shows on the right.
//!
//! Layout (128x64):
//!   x=0..91    the table
//!   x=92..127  result (top) and the cat (bottom)
//!
//! Setup: Left/Right moves between the fields (kind, then the kind's own
//! settings), Up/Down changes the selected one (hold to speed up), A
//! throws. After a throw: A throws again, B goes back to setup.

use core::f32::consts::PI;
use core::fmt::Write;
use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::t;
use crate::{
    assets::character::PoseId,
    context::GameContext,
    entities::character::Character,
    input::{Button, Buttons},
    rand::{rand_range_f32, rand_range_u32},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::{repeat::HoldRepeat, tiny_font},
};

pub const MAX_DICE: usize = 6;
pub const SIDES: [u8; 7] = [4, 6, 8, 10, 12, 20, 100];
pub const MAX_COINS: usize = 6;
pub const RANGE_MAX: u16 = 9999;

/// Roll `count` dice with `sides` faces each.
pub fn roll(count: usize, sides: u8, rng: &mut u32) -> Vec<u8, MAX_DICE> {
    let mut out = Vec::new();
    for _ in 0..count.min(MAX_DICE) {
        let _ = out.push(rand_range_u32(rng, 1, sides as u32) as u8);
    }
    out
}

pub fn total(values: &[u8]) -> u32 {
    values.iter().map(|&v| v as u32).sum()
}

/// Flip `count` coins; `true` is heads.
pub fn flip(count: usize, rng: &mut u32) -> Vec<bool, MAX_COINS> {
    let mut out = Vec::new();
    for _ in 0..count.min(MAX_COINS) {
        let _ = out.push(rand_range_u32(rng, 0, 1) == 1);
    }
    out
}

/// A whole number from `lo` to `hi`, both included, in either order.
pub fn pick_in_range(lo: u16, hi: u16, rng: &mut u32) -> u16 {
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    rand_range_u32(rng, lo as u32, hi as u32) as u16
}

/// Step a range value by `dir` units of `size`, landing on a multiple of
/// `size` (so holding through 7, 8, 9 then jumping by ten goes to 10, 20).
pub fn step_range_value(value: u16, dir: i32, size: u16) -> u16 {
    let v = value as i32;
    let s = size as i32;
    let next = if dir > 0 { (v / s + 1) * s } else { (v + s - 1) / s * s - s };
    next.clamp(0, RANGE_MAX as i32) as u16
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Dice,
    Coin,
    Range,
}

const KINDS: [Kind; 3] = [Kind::Dice, Kind::Coin, Kind::Range];

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Dice => t!("Dice"),
            Kind::Coin => t!("Coin"),
            Kind::Range => t!("Range"),
        }
    }

    /// Fields after the kind itself.
    fn fields(self) -> usize {
        match self {
            Kind::Dice | Kind::Range => 2,
            Kind::Coin => 1,
        }
    }
}

// Table and panel.
const TABLE_W: f32 = 92.0;
const TABLE_H: f32 = 64.0;
const TABLE_CX: i32 = 46;
const PANEL_CX: i32 = 108;

const D6_SIZE: i32 = 14;
const NUM_W: i32 = 18;
const NUM_H: i32 = 14;

/// Velocity lost per second, as a fraction.
const FRICTION: f32 = 1.6;
const WALL_BOUNCE: f32 = 0.7;
/// Below this speed (px/s) a die stops.
const SETTLE_SPEED: f32 = 6.0;
/// Seconds between face changes while tumbling.
const FLICKER: f32 = 0.07;
/// Hard stop so a roll never drags on.
const MAX_ROLL_SECS: f32 = 3.0;

const COIN_R: i32 = 11;
/// How high a coin is tossed, in px.
const COIN_TOSS: f32 = 9.0;
const RANGE_SPIN_SECS: f32 = 1.4;
const HISTORY: usize = 4;

/// 11x11 coin faces, two bytes per row.
const COIN_FACE: u16 = 11;
const COIN_HEADS: [u8; 22] = [0x40, 0x40, 0x60, 0xC0, 0x5F, 0x40, 0x40, 0x40, 0x91, 0x20, 0x80, 0x20, 0x84, 0x20, 0xAE, 0xA0, 0x4A, 0x40, 0x20, 0x80, 0x1F, 0x00];
const COIN_TAILS: [u8; 22] = [0x31, 0x80, 0x31, 0x80, 0xC0, 0x60, 0xC0, 0x60, 0x00, 0x00, 0x0E, 0x00, 0x1F, 0x00, 0x3F, 0x80, 0x3F, 0x80, 0x1B, 0x00, 0x00, 0x00];

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Setup,
    Rolling,
    Done,
}

#[derive(Clone, Copy)]
struct Die {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    shown: u8,
    value: u8,
    flicker: f32,
    settled: bool,
}

#[derive(Clone, Copy)]
struct Coin {
    cx: i32,
    cy: i32,
    heads: bool,
    /// Half turns over the whole flip; even lands heads up.
    half_turns: u32,
    secs: f32,
}

impl Coin {
    fn progress(&self, t: f32) -> f32 {
        (t / self.secs).min(1.0)
    }

    fn angle(&self, t: f32) -> f32 {
        let p = self.progress(t);
        let eased = 1.0 - (1.0 - p) * (1.0 - p);
        PI * self.half_turns as f32 * eased
    }
}

pub struct DiceScene {
    state: State,
    kind: usize,
    /// 0 is the kind, then the kind's own fields.
    focus: usize,
    dice_count: usize,
    sides_idx: usize,
    coin_count: usize,
    range: [u16; 2],
    repeat: HoldRepeat,
    dice: Vec<Die, MAX_DICE>,
    coins: Vec<Coin, MAX_COINS>,
    /// Range: the number shown (spinning, then the result) and earlier
    /// results, newest first.
    range_shown: u16,
    range_result: Option<u16>,
    history: Vec<u16, HISTORY>,
    flicker: f32,
    roll_time: f32,
    character: Character,
}

impl DiceScene {
    pub fn new() -> Self {
        Self {
            state: State::Setup,
            kind: 0,
            focus: 1,
            dice_count: 2,
            sides_idx: 1,
            coin_count: 1,
            range: [1, 100],
            repeat: HoldRepeat::new(),
            dice: Vec::new(),
            coins: Vec::new(),
            range_shown: 0,
            range_result: None,
            history: Vec::new(),
            flicker: 0.0,
            roll_time: 0.0,
            character: Character::new(Point::new(PANEL_CX, 63)),
        }
    }

    fn kind(&self) -> Kind {
        KINDS[self.kind]
    }

    fn sides(&self) -> u8 {
        SIDES[self.sides_idx]
    }

    fn die_size(&self) -> (i32, i32) {
        if self.sides() == 6 { (D6_SIZE, D6_SIZE) } else { (NUM_W, NUM_H) }
    }

    // --- setup ----------------------------------------------------------

    fn adjust(&mut self, dir: i32, held: f32) {
        match (self.kind(), self.focus) {
            (_, 0) => {
                let n = KINDS.len() as i32;
                self.kind = (self.kind as i32 + dir).rem_euclid(n) as usize;
            }
            (Kind::Dice, 1) => {
                self.dice_count = (self.dice_count as i32 + dir).clamp(1, MAX_DICE as i32) as usize;
            }
            (Kind::Dice, _) => {
                let n = SIDES.len() as i32;
                self.sides_idx = (self.sides_idx as i32 + dir).rem_euclid(n) as usize;
            }
            (Kind::Coin, _) => {
                self.coin_count = (self.coin_count as i32 + dir).clamp(1, MAX_COINS as i32) as usize;
            }
            (Kind::Range, f) => {
                // Faster the longer it's held: ones, then tens, then hundreds.
                let size = if held < 1.5 { 1 } else if held < 3.0 { 10 } else { 100 };
                let i = f - 1;
                self.range[i] = step_range_value(self.range[i], dir, size);
                // Keep min <= max by pushing the other end along.
                if self.range[0] > self.range[1] {
                    let other = 1 - i;
                    self.range[other] = self.range[i];
                }
            }
        }
    }

    fn update_setup(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) {
        let fields = self.kind().fields() + 1;
        if buttons.was_just_pressed(Button::Left) {
            self.focus = (self.focus + fields - 1) % fields;
            self.repeat.reset();
        }
        if buttons.was_just_pressed(Button::Right) {
            self.focus = (self.focus + 1) % fields;
            self.repeat.reset();
        }
        if let Some(step) = self.repeat.poll(buttons, Button::Down, Button::Up, dt) {
            self.adjust(step.dir, step.held);
        }
        if buttons.was_just_pressed(Button::A) {
            self.start(ctx);
        }
    }

    // --- throwing -------------------------------------------------------

    fn start(&mut self, ctx: &mut GameContext) {
        self.roll_time = 0.0;
        self.flicker = 0.0;
        match self.kind() {
            Kind::Dice => self.start_dice(ctx),
            Kind::Coin => self.start_coins(ctx),
            Kind::Range => {
                if let Some(last) = self.range_result {
                    if self.history.is_full() {
                        self.history.pop();
                    }
                    let _ = self.history.insert(0, last);
                }
                let result = pick_in_range(self.range[0], self.range[1], &mut ctx.rng);
                self.range_result = Some(result);
                self.range_shown = result;
            }
        }
        self.state = State::Rolling;
        self.character.set_pose(PoseId::SittingSideHappy);
    }

    fn start_dice(&mut self, ctx: &mut GameContext) {
        let sides = self.sides();
        let values = roll(self.dice_count, sides, &mut ctx.rng);
        let (w, h) = self.die_size();
        self.dice.clear();
        for (i, &value) in values.iter().enumerate() {
            // Thrown from the cat's side, fanned out vertically.
            let lane = (TABLE_H - h as f32) * (i as f32 + 0.5) / values.len() as f32;
            let _ = self.dice.push(Die {
                x: TABLE_W - w as f32 - rand_range_f32(&mut ctx.rng, 0.0, 6.0),
                y: lane,
                vx: -rand_range_f32(&mut ctx.rng, 110.0, 190.0),
                vy: rand_range_f32(&mut ctx.rng, -70.0, 70.0),
                shown: rand_range_u32(&mut ctx.rng, 1, sides as u32) as u8,
                value,
                flicker: 0.0,
                settled: false,
            });
        }
    }

    fn start_coins(&mut self, ctx: &mut GameContext) {
        let results = flip(self.coin_count, &mut ctx.rng);
        let n = results.len();
        let per_row = if n <= 3 { n } else { 3 };
        let rows = n.div_ceil(per_row);
        self.coins.clear();
        for (i, &heads) in results.iter().enumerate() {
            let (row, col) = (i / per_row, i % per_row);
            let in_row = if row + 1 == rows { n - row * per_row } else { per_row };
            let pitch = 28;
            let cx = TABLE_CX - (in_row as i32 - 1) * pitch / 2 + col as i32 * pitch;
            let cy = if rows == 1 { 30 } else { 18 + row as i32 * 27 };
            let mut half_turns = rand_range_u32(&mut ctx.rng, 6, 10);
            if (half_turns % 2 == 0) != heads {
                half_turns += 1;
            }
            let _ = self.coins.push(Coin {
                cx,
                cy,
                heads,
                half_turns,
                secs: rand_range_f32(&mut ctx.rng, 1.0, 1.5),
            });
        }
    }

    fn step_dice(&mut self, ctx: &mut GameContext, dt: f32) {
        let sides = self.sides() as u32;
        let (w, h) = self.die_size();
        let (max_x, max_y) = (TABLE_W - w as f32, TABLE_H - h as f32);
        let force_stop = self.roll_time >= MAX_ROLL_SECS;

        for d in self.dice.iter_mut() {
            if d.settled {
                continue;
            }
            d.x += d.vx * dt;
            d.y += d.vy * dt;
            if d.x < 0.0 {
                d.x = 0.0;
                d.vx = -d.vx * WALL_BOUNCE;
            } else if d.x > max_x {
                d.x = max_x;
                d.vx = -d.vx * WALL_BOUNCE;
            }
            if d.y < 0.0 {
                d.y = 0.0;
                d.vy = -d.vy * WALL_BOUNCE;
            } else if d.y > max_y {
                d.y = max_y;
                d.vy = -d.vy * WALL_BOUNCE;
            }
            let keep = (1.0 - FRICTION * dt).max(0.0);
            d.vx *= keep;
            d.vy *= keep;

            d.flicker += dt;
            if d.flicker >= FLICKER {
                d.flicker = 0.0;
                d.shown = rand_range_u32(&mut ctx.rng, 1, sides) as u8;
            }
            let speed = (d.vx * d.vx + d.vy * d.vy).sqrt();
            if speed < SETTLE_SPEED || force_stop {
                d.settled = true;
                d.shown = d.value;
                d.vx = 0.0;
                d.vy = 0.0;
            }
        }

        // Knock overlapping dice apart along the shallower axis and swap
        // their velocity on that axis, like a crude elastic bump.
        let n = self.dice.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.dice[i], self.dice[j]);
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let ox = w as f32 - dx.abs();
                let oy = h as f32 - dy.abs();
                if ox <= 0.0 || oy <= 0.0 || (a.settled && b.settled) {
                    continue;
                }
                let (mut a, mut b) = (a, b);
                if ox < oy {
                    let push = ox / 2.0 * if dx >= 0.0 { 1.0 } else { -1.0 };
                    a.x -= push;
                    b.x += push;
                    core::mem::swap(&mut a.vx, &mut b.vx);
                } else {
                    let push = oy / 2.0 * if dy >= 0.0 { 1.0 } else { -1.0 };
                    a.y -= push;
                    b.y += push;
                    core::mem::swap(&mut a.vy, &mut b.vy);
                }
                a.x = a.x.clamp(0.0, max_x);
                a.y = a.y.clamp(0.0, max_y);
                b.x = b.x.clamp(0.0, max_x);
                b.y = b.y.clamp(0.0, max_y);
                self.dice[i] = a;
                self.dice[j] = b;
            }
        }

        if self.dice.iter().all(|d| d.settled) {
            self.state = State::Done;
        }
    }

    fn step(&mut self, ctx: &mut GameContext, dt: f32) {
        self.roll_time += dt;
        match self.kind() {
            Kind::Dice => self.step_dice(ctx, dt),
            Kind::Coin => {
                if self.coins.iter().all(|c| self.roll_time >= c.secs) {
                    self.state = State::Done;
                }
            }
            Kind::Range => {
                if self.roll_time >= RANGE_SPIN_SECS {
                    self.range_shown = self.range_result.unwrap_or(self.range_shown);
                    self.state = State::Done;
                    return;
                }
                // Spin through numbers, slowing down towards the end.
                let p = self.roll_time / RANGE_SPIN_SECS;
                self.flicker += dt;
                if self.flicker >= 0.04 + 0.25 * p * p {
                    self.flicker = 0.0;
                    self.range_shown = pick_in_range(self.range[0], self.range[1], &mut ctx.rng);
                }
            }
        }
    }

    /// Bring every coin and die to rest now.
    fn hurry(&mut self) {
        self.roll_time = self.roll_time.max(MAX_ROLL_SECS);
    }

    // --- drawing --------------------------------------------------------

    fn draw_die(&self, r: &mut Renderer, d: &Die) {
        let (w, h) = self.die_size();
        let (x, y) = (d.x as i32, d.y as i32);
        // Rounded box: clear it, outline it, knock the corners off.
        r.fill_rect_off(Point::new(x, y), Size::new(w as u32, h as u32));
        r.draw_rect(Point::new(x, y), Size::new(w as u32, h as u32), false);
        for (cx, cy) in [(x, y), (x + w - 1, y), (x, y + h - 1), (x + w - 1, y + h - 1)] {
            r.draw_pixel(Point::new(cx, cy), false);
        }
        if self.sides() == 6 {
            draw_pips(r, x, y, d.shown);
        } else {
            let mut s: String<4> = String::new();
            let _ = write!(s, "{}", d.shown);
            let tw = tiny_font::str_width(&s, 1);
            tiny_font::draw_str(r, &s, Point::new(x + (w - tw) / 2, y + (h - tiny_font::H as i32) / 2));
        }
    }

    fn draw_coin(&self, r: &mut Renderer, c: &Coin, t: f32) {
        let angle = c.angle(t);
        let p = c.progress(t);
        let squash = angle.cos();
        let half_w = ((COIN_R as f32) * squash.abs()).max(1.0) as i32;
        let cy = c.cy - ((PI * p).sin() * COIN_TOSS) as i32;
        fill_ellipse(r, c.cx, cy, half_w, COIN_R, true);
        fill_ellipse(r, c.cx, cy, half_w - 2, COIN_R - 2, false);
        let face = if squash >= 0.0 { &COIN_HEADS } else { &COIN_TAILS };
        let w = ((COIN_FACE as f32) * squash.abs()) as i32;
        draw_squeezed(r, face, c.cx, cy, w);
    }

    fn draw_setup(&self, r: &mut Renderer) {
        // Kind name, boxed when selected.
        let label = self.kind().label();
        let lw = label.chars().count() as i32 * 6;
        let lx = TABLE_CX - lw / 2;
        if self.focus == 0 {
            r.draw_rect(Point::new(lx - 2, 0), Size::new(lw as u32 + 3, 11), true);
            r.draw_text_inverted(label, Point::new(lx, 1));
            draw_arrow(r, lx - 7, 2, -1);
            draw_arrow(r, lx - 7, 8, 1);
        } else {
            r.draw_text(label, Point::new(lx, 1));
        }

        // The kind's fields in large digits; the selected one gets arrows.
        let mut parts: Vec<String<6>, 3> = Vec::new();
        let mut field_of: Vec<Option<usize>, 3> = Vec::new();
        let mut push = |s: String<6>, field: Option<usize>| {
            let _ = parts.push(s);
            let _ = field_of.push(field);
        };
        let num = |v: u32| {
            let mut s: String<6> = String::new();
            let _ = write!(s, "{}", v);
            s
        };
        let scale: u32 = match self.kind() {
            Kind::Dice => {
                push(num(self.dice_count as u32), Some(1));
                let mut d: String<6> = String::new();
                let _ = write!(d, "d{}", self.sides());
                push(d, Some(2));
                3
            }
            Kind::Coin => {
                push(num(self.coin_count as u32), Some(1));
                3
            }
            Kind::Range => {
                push(num(self.range[0] as u32), Some(1));
                let mut dash: String<6> = String::new();
                let _ = dash.push('-');
                push(dash, None);
                push(num(self.range[1] as u32), Some(2));
                2
            }
        };
        let gap = 4;
        let widths: Vec<i32, 3> = parts.iter().map(|p| tiny_font::str_width(p, scale)).collect();
        let mut total_w: i32 = widths.iter().sum::<i32>() + gap * (parts.len() as i32 - 1);
        if self.kind() == Kind::Coin {
            total_w += gap + COIN_R * 2;
        }
        let glyph_h = 5 * scale as i32;
        let y = 32 - glyph_h / 2;
        let mut x = TABLE_CX - total_w / 2;
        for ((part, &w), field) in parts.iter().zip(widths.iter()).zip(field_of.iter()) {
            tiny_font::draw_str_scaled(r, part, x, y, scale, true);
            if *field == Some(self.focus) {
                let cx = x + w / 2;
                draw_arrow(r, cx, y - 6, -1);
                draw_arrow(r, cx, y + glyph_h + 5, 1);
            }
            x += w + gap;
        }
        if self.kind() == Kind::Coin {
            let still = Coin { cx: x + COIN_R, cy: 32, heads: true, half_turns: 0, secs: 1.0 };
            self.draw_coin(r, &still, 0.0);
        }

        let hint = t!("A: roll");
        let hw = hint.chars().count() as i32 * 6;
        r.draw_text(hint, Point::new(TABLE_CX - hw / 2, 53));
    }

    fn draw_result_panel(&self, r: &mut Renderer) {
        match self.kind() {
            Kind::Dice => {
                let values: Vec<u8, MAX_DICE> = self.dice.iter().map(|d| d.value).collect();
                let mut s: String<6> = String::new();
                let _ = write!(s, "{}", total(&values));
                let w = tiny_font::str_width(&s, 2);
                tiny_font::draw_str_scaled(r, &s, (PANEL_CX - w / 2).min(126 - w), 4, 2, true);
            }
            Kind::Coin => {
                let heads = self.coins.iter().filter(|c| c.heads).count();
                let tails = self.coins.len() - heads;
                for (row, (face, n)) in [(&COIN_HEADS, heads), (&COIN_TAILS, tails)].into_iter().enumerate() {
                    let y = 1 + row as i32 * 12;
                    r.draw_sprite_raw(face, COIN_FACE, COIN_FACE, Point::new(97, y), SpriteOpts::default());
                    let mut s: String<2> = String::new();
                    let _ = write!(s, "{}", n);
                    r.draw_text(&s, Point::new(113, y));
                }
            }
            Kind::Range => {}
        }
    }
}

impl Scene for DiceScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.state = State::Setup;
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.character.animate(dt);
        match self.state {
            State::Setup => self.update_setup(ctx, buttons, dt),
            State::Rolling => {
                self.step(ctx, dt);
                if buttons.was_just_pressed(Button::A) {
                    self.hurry();
                }
            }
            State::Done => {
                if buttons.was_just_pressed(Button::A) {
                    self.start(ctx);
                } else if buttons.was_just_pressed(Button::B) {
                    self.state = State::Setup;
                    self.character.set_pose(PoseId::SittingSideNeutral);
                }
            }
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        if self.state == State::Setup {
            self.draw_setup(r);
        } else {
            match self.kind() {
                Kind::Dice => {
                    for d in self.dice.iter() {
                        self.draw_die(r, d);
                    }
                }
                Kind::Coin => {
                    for c in self.coins.iter() {
                        self.draw_coin(r, c, self.roll_time);
                    }
                    if self.coins.len() == 1 && self.state == State::Done {
                        let word = if self.coins[0].heads { t!("Heads") } else { t!("Tails") };
                        let w = word.chars().count() as i32 * 6;
                        r.draw_text(word, Point::new(TABLE_CX - w / 2, 50));
                    }
                }
                Kind::Range => {
                    let mut s: String<6> = String::new();
                    let _ = write!(s, "{}", self.range_shown);
                    let w = tiny_font::str_width(&s, 4);
                    tiny_font::draw_str_scaled(r, &s, TABLE_CX - w / 2, 14, 4, true);
                    // Earlier results along the bottom, newest first.
                    let mut line: String<32> = String::new();
                    for v in self.history.iter() {
                        let _ = write!(line, "{} ", v);
                    }
                    let line = line.trim_end();
                    let w = line.chars().count() as i32 * 6;
                    r.draw_text(line, Point::new(TABLE_CX - w / 2, 50));
                }
            }
            if self.state == State::Done {
                self.draw_result_panel(r);
            }
        }

        self.character.draw(r, 0);
    }
}

/// Pips for a 14x14 d6 face at (x, y).
fn draw_pips(r: &mut Renderer, x: i32, y: i32, value: u8) {
    const L: i32 = 3;
    const M: i32 = 6;
    const R: i32 = 9;
    let spots: &[(i32, i32)] = match value {
        1 => &[(M, M)],
        2 => &[(L, L), (R, R)],
        3 => &[(L, L), (M, M), (R, R)],
        4 => &[(L, L), (R, L), (L, R), (R, R)],
        5 => &[(L, L), (R, L), (M, M), (L, R), (R, R)],
        _ => &[(L, L), (R, L), (L, M), (R, M), (L, R), (R, R)],
    };
    for &(px, py) in spots {
        r.draw_rect(Point::new(x + px, y + py), Size::new(2, 2), true);
    }
}

/// A small 5px-wide triangle pointing up (`dir` = -1) or down (`dir` = 1),
/// with its tip at (`cx`, `tip_y`).
fn draw_arrow(r: &mut Renderer, cx: i32, tip_y: i32, dir: i32) {
    for k in 0..3 {
        let y = tip_y - dir * k;
        r.draw_line(Point::new(cx - k, y), Point::new(cx + k, y));
    }
}

fn fill_ellipse(r: &mut Renderer, cx: i32, cy: i32, a: i32, b: i32, on: bool) {
    if a < 0 || b <= 0 {
        return;
    }
    let (a2, b2) = ((a * a) as i64, (b * b) as i64);
    for dy in -b..=b {
        for dx in -a..=a {
            if (dx * dx) as i64 * b2 + (dy * dy) as i64 * a2 <= a2 * b2 {
                r.draw_pixel(Point::new(cx + dx, cy + dy), on);
            }
        }
    }
}

/// Draw an 11x11 face squeezed to `w` columns, centred on (cx, cy), as
/// the coin turns edge-on.
fn draw_squeezed(r: &mut Renderer, face: &[u8; 22], cx: i32, cy: i32, w: i32) {
    if w <= 0 {
        return;
    }
    let size = COIN_FACE as i32;
    let x0 = cx - w / 2;
    let y0 = cy - size / 2;
    for dx in 0..w {
        let sx = (dx * size / w) as usize;
        for sy in 0..size as usize {
            let byte = face[sy * 2 + sx / 8];
            if byte & (0x80 >> (sx % 8)) != 0 {
                r.draw_pixel(Point::new(x0 + dx, y0 + sy as i32), true);
            }
        }
    }
}
