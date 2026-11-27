//! Morse code tool, in three modes picked on the top row:
//!
//!   Learn  browse letters and digits (Left/Right); A plays one on the LED
//!   Quiz   a code is shown and played; pick its letter from four (B replays)
//!   Key    tap A for dots and hold for dashes; letters are decoded as you
//!          go. Left/Right sets the speed, B deletes.
//!
//! Up from a mode returns to the top row, where Left/Right changes mode
//! and Down goes back in. Playback uses timed LED cues so the code keeps
//! good rhythm despite the 12 FPS frame loop.

use core::fmt::Write;
use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    led::scale_rgb,
    platform::time::{now_ms, now_us},
    rand::rand_range_u32,
    render::Renderer,
    scene::{Scene, SceneId},
};

/// International Morse for A-Z and 0-9.
pub const CODES: [(char, &str); 36] = [
    ('A', ".-"), ('B', "-..."), ('C', "-.-."), ('D', "-.."), ('E', "."), ('F', "..-."),
    ('G', "--."), ('H', "...."), ('I', ".."), ('J', ".---"), ('K', "-.-"), ('L', ".-.."),
    ('M', "--"), ('N', "-."), ('O', "---"), ('P', ".--."), ('Q', "--.-"), ('R', ".-."),
    ('S', "..."), ('T', "-"), ('U', "..-"), ('V', "...-"), ('W', ".--"), ('X', "-..-"),
    ('Y', "-.--"), ('Z', "--.."), ('0', "-----"), ('1', ".----"), ('2', "..---"),
    ('3', "...--"), ('4', "....-"), ('5', "....."), ('6', "-...."), ('7', "--..."),
    ('8', "---.."), ('9', "----."),
];
const LETTERS: usize = 26;

pub fn encode(c: char) -> Option<&'static str> {
    let c = c.to_ascii_uppercase();
    CODES.iter().find(|&&(k, _)| k == c).map(|&(_, code)| code)
}

pub fn decode(code: &str) -> Option<char> {
    CODES.iter().find(|&&(_, v)| v == code).map(|&(k, _)| k)
}

/// Dot length in ms at `wpm` words per minute (the PARIS standard).
pub fn unit_ms(wpm: u8) -> u64 {
    1200 / wpm.max(1) as u64
}

/// A key press of `held_ms` is a dash from two dot lengths up.
pub fn classify(held_ms: u64, unit_ms: u64) -> char {
    if held_ms < 2 * unit_ms { '.' } else { '-' }
}

pub const WPM_MIN: u8 = 5;
pub const WPM_MAX: u8 = 20;
const TEXT_MAX: usize = 63;
const SYMBOLS_MAX: usize = 6;
const WARM: (u8, u8, u8) = (255, 190, 110);
const LED_LEVEL: u8 = 130;

// Symbol drawing.
const DOT_R: i32 = 3;
const DASH_W: i32 = 13;
const SYM_H: i32 = 7;
const SYM_GAP: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Learn,
    Quiz,
    Key,
}

const MODES: [Mode; 3] = [Mode::Learn, Mode::Quiz, Mode::Key];

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Learn => t!("Learn"),
            Mode::Quiz => t!("Quiz"),
            Mode::Key => t!("Key"),
        }
    }
}

struct Quiz {
    answer: usize,
    choices: [usize; 4],
    cursor: usize,
    picked: Option<usize>,
    right: u16,
    asked: u16,
}

pub struct MorseScene {
    mode: usize,
    on_title: bool,
    wpm: u8,
    /// Learn: the character shown.
    learn: usize,
    quiz: Quiz,
    /// Playback in progress: what, and when it started (us).
    playing: Option<(&'static str, u64)>,
    // Key mode.
    key_down_at: Option<u64>,
    key_up_at: u64,
    symbols: String<SYMBOLS_MAX>,
    text: String<TEXT_MAX>,
    word_gap_pending: bool,
    now_ms: u64,
}

impl MorseScene {
    pub fn new() -> Self {
        Self {
            mode: 0,
            on_title: true,
            wpm: 8,
            learn: 0,
            quiz: Quiz { answer: 0, choices: [0; 4], cursor: 0, picked: None, right: 0, asked: 0 },
            playing: None,
            key_down_at: None,
            key_up_at: 0,
            symbols: String::new(),
            text: String::new(),
            word_gap_pending: false,
            now_ms: 0,
        }
    }

    fn mode(&self) -> Mode {
        MODES[self.mode]
    }

    fn unit(&self) -> u64 {
        unit_ms(self.wpm)
    }

    /// Queue `code` on the LED, starting just after now.
    fn play(&mut self, ctx: &mut GameContext, code: &'static str) {
        ctx.led_cues.clear();
        ctx.led.off();
        let unit_us = self.unit() * 1000;
        let start = now_us() + 100_000;
        let mut t = start;
        let colour = scale_rgb(WARM, LED_LEVEL);
        for s in code.chars() {
            let len = if s == '-' { 3 } else { 1 } * unit_us;
            ctx.schedule_led(t, colour);
            ctx.schedule_led(t + len, (0, 0, 0));
            t += len + unit_us;
        }
        self.playing = Some((code, start));
    }

    fn stop_playing(&mut self, ctx: &mut GameContext) {
        self.playing = None;
        ctx.led_cues.clear();
        ctx.led.off();
    }

    /// Which symbol of the playing code is lit now, if any.
    fn lit_symbol(&self) -> Option<usize> {
        let (code, start) = self.playing?;
        let unit = self.unit() * 1000;
        let now = now_us();
        let mut t = start;
        for (i, s) in code.chars().enumerate() {
            let len = if s == '-' { 3 } else { 1 } * unit;
            if now >= t && now < t + len {
                return Some(i);
            }
            t += len + unit;
        }
        None
    }

    fn new_question(&mut self, ctx: &mut GameContext) {
        let q = &mut self.quiz;
        q.answer = rand_range_u32(&mut ctx.rng, 0, LETTERS as u32 - 1) as usize;
        let mut picks: Vec<usize, 4> = Vec::new();
        let _ = picks.push(q.answer);
        while !picks.is_full() {
            let c = rand_range_u32(&mut ctx.rng, 0, LETTERS as u32 - 1) as usize;
            if !picks.contains(&c) {
                let _ = picks.push(c);
            }
        }
        // Shuffle so the answer isn't always first.
        for i in (1..4).rev() {
            let j = rand_range_u32(&mut ctx.rng, 0, i as u32) as usize;
            picks.swap(i, j);
        }
        q.choices.copy_from_slice(&picks);
        q.cursor = 0;
        q.picked = None;
        let code = CODES[self.quiz.answer].1;
        self.play(ctx, code);
    }

    fn enter_mode(&mut self, ctx: &mut GameContext) {
        self.stop_playing(ctx);
        if self.mode() == Mode::Quiz {
            self.new_question(ctx);
        }
    }

    // --- Key mode -------------------------------------------------------

    fn key_update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons) {
        let down = buttons.is_pressed(Button::A);
        let now = self.now_ms;
        let unit = self.unit();
        match (self.key_down_at, down) {
            (None, true) => {
                self.key_down_at = Some(now);
                ctx.led.set_rgb(scale_rgb(WARM, LED_LEVEL));
            }
            (Some(at), false) => {
                self.key_down_at = None;
                self.key_up_at = now;
                ctx.led.off();
                if self.symbols.push(classify(now - at, unit)).is_err() {
                    // Longer than any code: start the letter again.
                    self.symbols.clear();
                }
            }
            _ => {}
        }
        if self.key_down_at.is_none() {
            let quiet = now.saturating_sub(self.key_up_at);
            if !self.symbols.is_empty() && quiet >= 3 * unit {
                let c = decode(&self.symbols).unwrap_or('?');
                self.push_text(c);
                self.symbols.clear();
                self.word_gap_pending = true;
            } else if self.symbols.is_empty() && self.word_gap_pending && quiet >= 7 * unit {
                self.push_text(' ');
                self.word_gap_pending = false;
            }
        }
        if buttons.was_just_pressed(Button::B) {
            if !self.symbols.is_empty() {
                self.symbols.clear();
            } else {
                self.text.pop();
            }
            self.word_gap_pending = false;
        }
        if buttons.was_just_pressed(Button::Left) && self.wpm > WPM_MIN {
            self.wpm -= 1;
        }
        if buttons.was_just_pressed(Button::Right) && self.wpm < WPM_MAX {
            self.wpm += 1;
        }
    }

    fn push_text(&mut self, c: char) {
        if self.text.push(c).is_err() {
            // Full: drop the oldest character.
            let rest: String<TEXT_MAX> = self.text.chars().skip(1).collect();
            self.text = rest;
            let _ = self.text.push(c);
        }
    }

    // --- drawing --------------------------------------------------------

    fn draw_title(&self, r: &mut Renderer) {
        let label = self.mode().label();
        let w = label.chars().count() as i32 * 6;
        let x = 64 - w / 2;
        if self.on_title {
            r.draw_rect(Point::new(x - 2, 0), Size::new(w as u32 + 3, 11), true);
            r.draw_text_inverted(label, Point::new(x, 1));
        } else {
            r.draw_text(label, Point::new(x, 1));
        }
        draw_arrow(r, 4, 5, -1);
        draw_arrow(r, 123, 5, 1);
    }

    fn draw_hint(&self, r: &mut Renderer, text: &str) {
        let w = text.chars().count() as i32 * 6;
        r.draw_text(text, Point::new(64 - w / 2, 53));
    }

    fn draw_learn(&self, r: &mut Renderer) {
        let (c, code) = CODES[self.learn];
        let mut s: String<4> = String::new();
        let _ = s.push(c);
        r.draw_text_scaled(&s, Point::new(8, 13), 3, true);
        if !self.on_title {
            draw_arrow(r, 3, 27, -1);
            draw_arrow(r, 30, 27, 1);
        }
        draw_code(r, code, 40, 27, self.lit_symbol());
        self.draw_hint(r, t!("A: play"));
    }

    fn draw_quiz(&self, r: &mut Renderer) {
        let q = &self.quiz;
        let code = CODES[q.answer].1;
        let w = code_width(code);
        draw_code(r, code, 64 - w / 2, 22, self.lit_symbol());

        for (i, &choice) in q.choices.iter().enumerate() {
            let x = 12 + i as i32 * 28;
            let y = 34;
            let mut s: String<4> = String::new();
            let _ = s.push(CODES[choice].0);
            let cursor = !self.on_title && i == q.cursor && q.picked.is_none();
            let highlight = cursor || (q.picked.is_some() && choice == q.answer);
            if highlight {
                r.draw_rect(Point::new(x, y), Size::new(20, 14), true);
                r.draw_text_inverted(&s, Point::new(x + 7, y + 2));
            } else {
                r.draw_rect(Point::new(x, y), Size::new(20, 14), false);
                r.draw_text(&s, Point::new(x + 7, y + 2));
            }
            // A wrong pick gets crossed out.
            if q.picked == Some(i) && choice != q.answer {
                r.draw_line(Point::new(x, y), Point::new(x + 19, y + 13));
                r.draw_line(Point::new(x + 19, y), Point::new(x, y + 13));
            }
        }

        let mut line: String<32> = String::new();
        match q.picked {
            Some(i) if q.choices[i] == q.answer => {
                let _ = write!(line, "{} {}/{}", t!("Yes!"), q.right, q.asked);
            }
            Some(_) => {
                let _ = write!(line, "{} {}/{}", t!("Not quite"), q.right, q.asked);
            }
            None => {
                let _ = write!(line, "{} {}/{}", t!("B: replay"), q.right, q.asked);
            }
        }
        self.draw_hint(r, &line);
    }

    fn draw_key(&self, r: &mut Renderer) {
        // The last two lines of decoded text.
        let chars: Vec<char, TEXT_MAX> = self.text.chars().collect();
        let per_line = 21;
        let shown = chars.len().min(per_line * 2);
        let tail = &chars[chars.len() - shown..];
        for (row, chunk) in tail.chunks(per_line).enumerate() {
            let line: String<24> = chunk.iter().collect();
            r.draw_text(&line, Point::new(1, 13 + row as i32 * 10));
        }
        // A cursor after the text.
        let col = (tail.len() % per_line) as i32;
        let row = if tail.is_empty() { 0 } else { ((tail.len() - 1) / per_line) as i32 + (col == 0) as i32 };
        if self.now_ms / 400 % 2 == 0 && row < 2 {
            r.draw_line(Point::new(1 + col * 6, 21 + row * 10), Point::new(5 + col * 6, 21 + row * 10));
        }

        // The letter being keyed, and what it decodes to so far.
        draw_code(r, &self.symbols, 4, 41, None);
        if let Some(c) = decode(&self.symbols) {
            let mut s: String<4> = String::new();
            let _ = s.push(c);
            r.draw_text_scaled(&s, Point::new(100, 32), 2, true);
        }
        if self.key_down_at.is_some() {
            r.draw_rect(Point::new(84, 37), Size::new(8, 8), true);
        }

        let mut hint: String<40> = String::new();
        let _ = write!(hint, "{} wpm  {}", self.wpm, t!("B: del"));
        self.draw_hint(r, &hint);
    }
}

impl Scene for MorseScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.wpm = ctx.tool_prefs.morse_wpm.clamp(WPM_MIN, WPM_MAX);
        self.on_title = true;
        self.enter_mode(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        ctx.tool_prefs.morse_wpm = self.wpm;
        self.stop_playing(ctx);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, _dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.now_ms = now_ms();
        if self.playing.is_some() && ctx.led_cues.is_empty() {
            self.playing = None;
        }

        if self.on_title {
            let n = MODES.len();
            if buttons.was_just_pressed(Button::Left) {
                self.mode = (self.mode + n - 1) % n;
                self.enter_mode(ctx);
            }
            if buttons.was_just_pressed(Button::Right) {
                self.mode = (self.mode + 1) % n;
                self.enter_mode(ctx);
            }
            if buttons.was_just_pressed(Button::Down) || buttons.was_just_pressed(Button::A) {
                self.on_title = false;
                // Don't let this A press also act inside the mode.
                return None;
            }
            return None;
        }
        if buttons.was_just_pressed(Button::Up) {
            self.on_title = true;
            ctx.led.off();
            self.key_down_at = None;
            return None;
        }

        match self.mode() {
            Mode::Learn => {
                let n = CODES.len();
                if buttons.was_just_pressed(Button::Left) {
                    self.learn = (self.learn + n - 1) % n;
                    self.stop_playing(ctx);
                }
                if buttons.was_just_pressed(Button::Right) {
                    self.learn = (self.learn + 1) % n;
                    self.stop_playing(ctx);
                }
                if buttons.was_just_pressed(Button::A) {
                    self.play(ctx, CODES[self.learn].1);
                }
            }
            Mode::Quiz => {
                let a = buttons.was_just_pressed(Button::A);
                if self.quiz.picked.is_some() {
                    if a {
                        self.new_question(ctx);
                    }
                } else {
                    if buttons.was_just_pressed(Button::Left) {
                        self.quiz.cursor = (self.quiz.cursor + 3) % 4;
                    }
                    if buttons.was_just_pressed(Button::Right) {
                        self.quiz.cursor = (self.quiz.cursor + 1) % 4;
                    }
                    if a {
                        let q = &mut self.quiz;
                        q.picked = Some(q.cursor);
                        q.asked += 1;
                        if q.choices[q.cursor] == q.answer {
                            q.right += 1;
                        }
                    }
                }
                if buttons.was_just_pressed(Button::B) {
                    self.play(ctx, CODES[self.quiz.answer].1);
                }
            }
            Mode::Key => self.key_update(ctx, buttons),
        }
        ctx.tool_prefs.morse_wpm = self.wpm;
        None
    }

    fn tick_background(&mut self, ctx: &mut GameContext, _dt: f32) {
        if self.playing.is_some() || self.key_down_at.is_some() {
            self.stop_playing(ctx);
            self.key_down_at = None;
        }
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        self.draw_title(r);
        match self.mode() {
            Mode::Learn => self.draw_learn(r),
            Mode::Quiz => self.draw_quiz(r),
            Mode::Key => self.draw_key(r),
        }
    }
}

fn symbol_width(s: char) -> i32 {
    if s == '-' { DASH_W } else { DOT_R * 2 + 1 }
}

fn code_width(code: &str) -> i32 {
    let n = code.chars().count() as i32;
    code.chars().map(symbol_width).sum::<i32>() + SYM_GAP * (n - 1).max(0)
}

/// Draw a code as dots and dashes from `x`, centred on `cy`. The `lit`
/// symbol gets a bar underneath while it plays.
fn draw_code(r: &mut Renderer, code: &str, x: i32, cy: i32, lit: Option<usize>) {
    let mut x = x;
    for (i, s) in code.chars().enumerate() {
        let w = symbol_width(s);
        if s == '-' {
            r.draw_rect(Point::new(x, cy - SYM_H / 2), Size::new(w as u32, SYM_H as u32), true);
        } else {
            r.draw_circle_filled(Point::new(x + DOT_R, cy), DOT_R);
        }
        if lit == Some(i) {
            r.draw_rect(Point::new(x, cy + SYM_H / 2 + 3), Size::new(w as u32, 2), true);
        }
        x += w + SYM_GAP;
    }
}

/// 4px-tall triangle pointing left (`dir` = -1) or right (`dir` = 1), tip at x.
fn draw_arrow(r: &mut Renderer, tip_x: i32, cy: i32, dir: i32) {
    for k in 0..4 {
        let x = tip_x - dir * k;
        r.draw_line(Point::new(x, cy - k), Point::new(x, cy + k));
    }
}
