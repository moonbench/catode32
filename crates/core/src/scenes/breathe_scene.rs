//! Breathing guide. A circle grows as you breathe in and shrinks as you
//! breathe out, and the LED swells and fades with it, so the screen can
//! be ignored (or the lights turned off).
//!
//! Left/Right picks a pattern, Up/Down sets the LED (off to bright), A
//! starts and pauses, B resets.

use core::fmt::Write;
use embedded_graphics::prelude::{Point, Size};
use heapless::String;

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    led::{scale_rgb, LedLatch},
    platform::time::now_ms,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{timefmt, tiny_font, wrap},
};

/// A breathing pattern: seconds of in, hold, out, hold.
pub struct Pattern {
    pub name: &'static str,
    pub secs: [f32; 4],
}

pub const PATTERNS: [Pattern; 4] = [
    Pattern { name: t!("Box"), secs: [4.0, 4.0, 4.0, 4.0] },
    Pattern { name: t!("Relax"), secs: [4.0, 7.0, 8.0, 0.0] },
    Pattern { name: t!("Even"), secs: [5.0, 0.0, 5.0, 0.0] },
    Pattern { name: t!("Unwind"), secs: [4.0, 0.0, 6.0, 0.0] },
];

/// Where a pattern is `t` seconds in: the step (0 in, 1 hold, 2 out,
/// 3 hold), progress through it (0..1), and seconds left in it.
pub fn step_at(p: &Pattern, t: f32) -> (usize, f32, f32) {
    let cycle: f32 = p.secs.iter().sum();
    let mut t = t % cycle;
    for (i, &len) in p.secs.iter().enumerate() {
        if len > 0.0 && t < len {
            return (i, t / len, len - t);
        }
        t -= len;
    }
    (0, 0.0, p.secs[0])
}

/// How full the lungs are, 0..1, at that point.
pub fn fullness(step: usize, progress: f32) -> f32 {
    let ease = progress * progress * (3.0 - 2.0 * progress);
    match step {
        0 => ease,
        1 => 1.0,
        2 => 1.0 - ease,
        _ => 0.0,
    }
}

const LED_LEVELS: [u8; 5] = [0, 30, 80, 150, 255];
const TEAL: (u8, u8, u8) = (0, 150, 170);
const CIRCLE: Point = Point::new(34, 38);
const R_MIN: i32 = 5;
const R_MAX: i32 = 24;
const RIGHT_CX: i32 = 97;
/// How long the LED level shows after changing it.
const LEVEL_SHOW_MS: u64 = 1500;

pub struct BreatheScene {
    pattern: usize,
    level: usize,
    running: bool,
    started_ms: u64,
    banked_ms: u64,
    level_shown_at: Option<u64>,
    led: LedLatch,
    now: u64,
}

impl BreatheScene {
    pub fn new() -> Self {
        Self {
            pattern: 0,
            level: 2,
            running: false,
            started_ms: 0,
            banked_ms: 0,
            level_shown_at: None,
            led: LedLatch::new(),
            now: 0,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.banked_ms + if self.running { self.now.saturating_sub(self.started_ms) } else { 0 }
    }

    fn pattern(&self) -> &'static Pattern {
        &PATTERNS[self.pattern]
    }

    fn reset(&mut self) {
        self.running = false;
        self.banked_ms = 0;
    }

    fn current(&self) -> (usize, f32, f32) {
        step_at(self.pattern(), self.elapsed_ms() as f32 / 1000.0)
    }

    fn advance(&mut self, ctx: &mut GameContext) {
        self.now = now_ms();
        let rgb = if self.running {
            let (step, p, _) = self.current();
            let level = LED_LEVELS[self.level] as f32 * fullness(step, p);
            scale_rgb(TEAL, level as u8)
        } else {
            (0, 0, 0)
        };
        self.led.show(&mut ctx.led, rgb);
        ctx.keep_awake = self.running;
    }

    fn title(&self) -> String<40> {
        let p = self.pattern();
        let mut s = String::new();
        let _ = write!(s, "{} ", p.name);
        let mut first = true;
        for &len in p.secs.iter().filter(|&&l| l > 0.0) {
            let _ = write!(s, "{}{}", if first { "" } else { "-" }, len as u32);
            first = false;
        }
        s
    }
}

impl Scene for BreatheScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.pattern = (ctx.tool_prefs.breathe_pattern as usize).min(PATTERNS.len() - 1);
        self.level = (ctx.tool_prefs.breathe_level as usize).min(LED_LEVELS.len() - 1);
        self.reset();
        self.led.forget();
        self.advance(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        ctx.tool_prefs.breathe_pattern = self.pattern as u8;
        ctx.tool_prefs.breathe_level = self.level as u8;
        ctx.keep_awake = false;
        self.led.forget();
        self.led.off(&mut ctx.led);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, _dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.now = now_ms();
        let n = PATTERNS.len();
        if buttons.was_just_pressed(Button::Left) {
            self.pattern = (self.pattern + n - 1) % n;
            self.reset();
        }
        if buttons.was_just_pressed(Button::Right) {
            self.pattern = (self.pattern + 1) % n;
            self.reset();
        }
        if buttons.was_just_pressed(Button::Up) && self.level + 1 < LED_LEVELS.len() {
            self.level += 1;
            self.level_shown_at = Some(self.now);
        }
        if buttons.was_just_pressed(Button::Down) && self.level > 0 {
            self.level -= 1;
            self.level_shown_at = Some(self.now);
        }
        if buttons.was_just_pressed(Button::A) {
            if self.running {
                self.banked_ms = self.elapsed_ms();
            } else {
                self.started_ms = self.now;
            }
            self.running = !self.running;
        }
        if buttons.was_just_pressed(Button::B) {
            self.reset();
        }
        ctx.tool_prefs.breathe_pattern = self.pattern as u8;
        ctx.tool_prefs.breathe_level = self.level as u8;
        self.advance(ctx);
        None
    }

    /// Under the menu the guide pauses and the light goes out.
    fn tick_background(&mut self, ctx: &mut GameContext, _dt: f32) {
        self.now = now_ms();
        if self.running {
            self.banked_ms = self.elapsed_ms();
            self.running = false;
        }
        self.advance(ctx);
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        let title = self.title();
        let w = title.chars().count() as i32 * 6;
        r.draw_text(&title, Point::new(64 - w / 2, 1));
        draw_arrow(r, 4, 5, -1);
        draw_arrow(r, 123, 5, 1);

        let started = self.elapsed_ms() > 0;
        let (step, p, left) = self.current();
        let full = if started { fullness(step, p) } else { 0.0 };

        // The breath, with the full size dotted in as a guide.
        let radius = R_MIN + ((R_MAX - R_MIN) as f32 * full) as i32;
        for k in 0..32 {
            let a = k as f32 * core::f32::consts::TAU / 32.0;
            let (s, c) = sin_cos(a);
            let p = Point::new(CIRCLE.x + (c * R_MAX as f32) as i32, CIRCLE.y + (s * R_MAX as f32) as i32);
            r.draw_pixel(p, true);
        }
        r.draw_circle_outline(CIRCLE, radius);
        r.draw_circle_outline(CIRCLE, (radius - 1).max(1));
        r.draw_circle_filled(CIRCLE, 2);

        if !started {
            let lines: heapless::Vec<&str, 3> = wrap::wrap(t!("A: start"), 10);
            wrap::draw_centered(r, &lines, RIGHT_CX, 26, 10);
        } else {
            let word = match step {
                0 => t!("In"),
                2 => t!("Out"),
                _ => t!("Hold"),
            };
            let ww = word.chars().count() as i32 * 6;
            if ww * 2 <= 60 {
                r.draw_text_scaled(word, Point::new(RIGHT_CX - ww, 14), 2, true);
            } else {
                r.draw_text(word, Point::new(RIGHT_CX - ww / 2, 18));
            }
            let mut secs: String<4> = String::new();
            let _ = write!(secs, "{}", left.ceil_u32());
            let sw = tiny_font::str_width(&secs, 3);
            tiny_font::draw_str_scaled(r, &secs, RIGHT_CX - sw / 2, 36, 3, true);
            if !self.running {
                let pw = t!("Paused").chars().count() as i32 * 6;
                r.draw_text(t!("Paused"), Point::new(RIGHT_CX - pw / 2, 54));
            }
        }

        // Session length, or the LED level just after changing it.
        let showing_level = self.level_shown_at.is_some_and(|t| self.now - t < LEVEL_SHOW_MS);
        if showing_level {
            for i in 0..(LED_LEVELS.len() - 1) as i32 {
                let h = 2 + i * 2;
                r.draw_rect(Point::new(2 + i * 5, 62 - h), Size::new(3, h as u32), i < self.level as i32);
            }
        } else if started && self.running {
            let clock = timefmt::clock(self.elapsed_ms(), false);
            r.draw_text(&clock, Point::new(RIGHT_CX - clock.len() as i32 * 3, 54));
        }
    }
}

trait CeilU32 {
    fn ceil_u32(self) -> u32;
}

impl CeilU32 for f32 {
    fn ceil_u32(self) -> u32 {
        let t = self as u32;
        if (t as f32) < self { t + 1 } else { t }
    }
}

fn sin_cos(a: f32) -> (f32, f32) {
    #[cfg(not(feature = "desktop"))]
    use micromath::F32Ext;
    (a.sin(), a.cos())
}

/// 4px-tall triangle pointing left (`dir` = -1) or right (`dir` = 1), tip at x.
fn draw_arrow(r: &mut Renderer, tip_x: i32, cy: i32, dir: i32) {
    for k in 0..4 {
        let x = tip_x - dir * k;
        r.draw_line(Point::new(x, cy - k), Point::new(x, cy + k));
    }
}
