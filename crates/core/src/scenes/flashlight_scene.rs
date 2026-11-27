//! Flashlight tool, driving the RGB LED.
//!
//! Preset modes: Left/Right picks a mode, Up/Down sets brightness.
//! RGB mode: Up/Down moves between the mode name and the R, G and B
//! sliders; Left/Right changes whichever is selected (hold to speed up).
//! A switches the light on and off in every mode.
//!
//! The device won't idle-sleep while the light is on (`ctx.keep_awake`).
//! The LED goes dark as soon as the menu opens and on leaving: the WS2812
//! latches its colour, so it would otherwise stay lit. Mode, brightness
//! and colour are kept in `ctx.flashlight` for next time.

use core::fmt::Write;
use embedded_graphics::prelude::{Point, Size};
use heapless::String;

use crate::t;
use crate::{
    context::{FlashlightPrefs, GameContext},
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::repeat::HoldRepeat,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    White,
    Warm,
    Red,
    Rainbow,
    Siren,
    Sos,
    Rgb,
}

const MODES: [Mode; 7] = [Mode::White, Mode::Warm, Mode::Red, Mode::Rainbow, Mode::Siren, Mode::Sos, Mode::Rgb];

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::White => t!("White"),
            Mode::Warm => t!("Calm"),
            Mode::Red => t!("Night red"),
            Mode::Rainbow => t!("Rainbow"),
            Mode::Siren => t!("Siren"),
            Mode::Sos => t!("SOS"),
            Mode::Rgb => "RGB",
        }
    }
}

/// Brightness steps on a rough perceptual curve; the LED has no hardware
/// dimming so colours are scaled by these.
const LEVELS: [u8; 8] = [3, 8, 18, 34, 60, 100, 160, 255];

const WHITE: (u8, u8, u8) = (255, 255, 255);
const WARM: (u8, u8, u8) = (255, 120, 20);
const RED: (u8, u8, u8) = (255, 0, 0);
const BLUE: (u8, u8, u8) = (0, 0, 255);

/// Siren as (colour, units): two red flashes then two blue, one unit
/// being one frame at 12 FPS. One cycle is a second.
const SIREN_UNIT: f32 = 1.0 / 12.0;
const SIREN: [(Option<(u8, u8, u8)>, u8); 8] = [
    (Some(RED), 2), (None, 1), (Some(RED), 2), (None, 1),
    (Some(BLUE), 2), (None, 1), (Some(BLUE), 2), (None, 1),
];
/// Seconds for one trip round the colour wheel.
const RAINBOW_PERIOD: f32 = 12.0;
/// Morse time unit. Three frames at 12 FPS.
const SOS_UNIT: f32 = 0.25;
/// `... --- ...` then a word gap, as (lit, units) pairs.
const SOS: [(bool, u8); 18] = [
    (true, 1), (false, 1), (true, 1), (false, 1), (true, 1), (false, 3),
    (true, 3), (false, 1), (true, 3), (false, 1), (true, 3), (false, 3),
    (true, 1), (false, 1), (true, 1), (false, 1), (true, 1), (false, 7),
];

/// RGB slider steps: a tap, and each frame while held.
const STEP_TAP: i32 = 5;
const STEP_HELD: i32 = 15;

// RGB layout.
const SLIDER_Y: [i32; 3] = [15, 28, 41];
const TRACK_X: i32 = 13;
const TRACK_W: i32 = 88;
const HEX_Y: i32 = 54;

fn siren_colour(t: f32) -> Option<(u8, u8, u8)> {
    let total: u32 = SIREN.iter().map(|&(_, u)| u as u32).sum();
    let mut unit = (t / SIREN_UNIT) as u32 % total;
    for &(colour, len) in SIREN.iter() {
        if unit < len as u32 {
            return colour;
        }
        unit -= len as u32;
    }
    None
}

fn sos_lit(t: f32) -> bool {
    let total: u32 = SOS.iter().map(|&(_, u)| u as u32).sum();
    let mut unit = (t / SOS_UNIT) as u32 % total;
    for &(lit, len) in SOS.iter() {
        if unit < len as u32 {
            return lit;
        }
        unit -= len as u32;
    }
    false
}

fn scale((r, g, b): (u8, u8, u8), level: u8) -> (u8, u8, u8) {
    let s = |c: u8| ((c as u16 * level as u16 + 127) / 255) as u8;
    (s(r), s(g), s(b))
}

/// Fully saturated colour at `hue` in 0..1.
fn wheel(hue: f32) -> (u8, u8, u8) {
    let h = (hue - (hue as i32) as f32) * 6.0;
    let sector = h as u8;
    let f = h - sector as f32;
    let up = (f * 255.0) as u8;
    let down = 255 - up;
    match sector {
        0 => (255, up, 0),
        1 => (down, 255, 0),
        2 => (0, 255, up),
        3 => (0, down, 255),
        4 => (up, 0, 255),
        _ => (255, 0, down),
    }
}

pub struct FlashlightScene {
    mode: usize,
    level: usize,
    rgb: [u8; 3],
    /// RGB mode only: 0 is the mode name, 1-3 the R, G and B sliders.
    focus: usize,
    repeat: HoldRepeat,
    on: bool,
    /// Drives the animated modes.
    t: f32,
    /// Colour last sent to the LED, so it's only written on change.
    sent: Option<(u8, u8, u8)>,
}

impl FlashlightScene {
    pub fn new() -> Self {
        Self {
            mode: 0,
            level: FlashlightPrefs::DEFAULT.level as usize,
            rgb: FlashlightPrefs::DEFAULT.rgb,
            focus: 0,
            repeat: HoldRepeat::new(),
            on: true,
            t: 0.0,
            sent: None,
        }
    }

    fn mode(&self) -> Mode {
        MODES[self.mode]
    }

    /// Colour the LED should show right now, brightness applied.
    fn colour(&self) -> Option<(u8, u8, u8)> {
        if !self.on {
            return None;
        }
        let level = LEVELS[self.level];
        let preset = match self.mode() {
            Mode::White => Some(WHITE),
            Mode::Warm => Some(WARM),
            Mode::Red => Some(RED),
            Mode::Rainbow => Some(wheel(self.t / RAINBOW_PERIOD)),
            Mode::Siren => siren_colour(self.t),
            Mode::Sos => sos_lit(self.t).then_some(WHITE),
            // Exactly what the sliders say.
            Mode::Rgb => return Some((self.rgb[0], self.rgb[1], self.rgb[2])),
        };
        preset.map(|c| scale(c, level))
    }

    fn apply(&mut self, ctx: &mut GameContext) {
        ctx.keep_awake = self.on;
        let rgb = self.colour().unwrap_or((0, 0, 0));
        if self.sent != Some(rgb) {
            ctx.led.set(rgb.0, rgb.1, rgb.2);
            self.sent = Some(rgb);
        }
    }

    /// LED off and sleep allowed, without touching the on/off setting.
    fn go_dark(&mut self, ctx: &mut GameContext) {
        ctx.keep_awake = false;
        if self.sent != Some((0, 0, 0)) {
            ctx.led.off();
            self.sent = Some((0, 0, 0));
        }
    }

    fn advance(&mut self, ctx: &mut GameContext, dt: f32) {
        self.t += dt;
        // Keep the float small. 102 s is a whole number of siren (1 s),
        // rainbow (12 s) and SOS (8.5 s) cycles, so nothing jumps.
        if self.t > 102.0 {
            self.t -= 102.0;
        }
        self.apply(ctx);
    }

    fn step_mode(&mut self, delta: i32) {
        let n = MODES.len() as i32;
        self.mode = (self.mode as i32 + delta).rem_euclid(n) as usize;
        self.focus = 0;
        self.t = 0.0;
    }

    /// Left/Right on an RGB slider: one step per tap, then fast repeats
    /// while held.
    fn slide(&mut self, buttons: &mut Buttons, dt: f32) {
        if let Some(step) = self.repeat.poll(buttons, Button::Left, Button::Right, dt) {
            let size = if step.held > 0.0 { STEP_HELD } else { STEP_TAP };
            let c = &mut self.rgb[self.focus - 1];
            *c = (*c as i32 + step.dir * size).clamp(0, 255) as u8;
        }
    }

    fn save_prefs(&self, ctx: &mut GameContext) {
        ctx.flashlight = FlashlightPrefs {
            mode: self.mode as u8,
            level: self.level as u8,
            rgb: self.rgb,
        };
    }

    fn draw_title(&self, r: &mut Renderer) {
        let label = self.mode().label();
        let w = label.chars().count() as i32 * 6;
        let x = 64 - w / 2;
        let selected = self.mode() == Mode::Rgb && self.focus == 0;
        if selected {
            r.draw_rect(Point::new(x - 2, 0), Size::new(w as u32 + 3, 11), true);
            r.draw_text_inverted(label, Point::new(x, 1));
        } else {
            r.draw_text(label, Point::new(x, 1));
        }
        draw_arrow(r, 4, 5, -1);
        draw_arrow(r, 123, 5, 1);
    }

    fn draw_sliders(&self, r: &mut Renderer) {
        for (i, (&y, name)) in SLIDER_Y.iter().zip(["R", "G", "B"]).enumerate() {
            let value = self.rgb[i];
            if self.focus == i + 1 {
                r.draw_rect(Point::new(0, y - 1), Size::new(9, 11), true);
                r.draw_text_inverted(name, Point::new(2, y));
            } else {
                r.draw_text(name, Point::new(2, y));
            }
            r.draw_rect(Point::new(TRACK_X, y + 1), Size::new(TRACK_W as u32, 7), false);
            let fill = value as i32 * (TRACK_W - 4) / 255;
            if fill > 0 {
                r.draw_rect(Point::new(TRACK_X + 2, y + 3), Size::new(fill as u32, 3), true);
            }
            let mut s: String<3> = String::new();
            let _ = write!(s, "{}", value);
            r.draw_text(&s, Point::new(128 - s.len() as i32 * 6, y));
        }
        let mut hex: String<8> = String::new();
        let _ = write!(hex, "#{:02X}{:02X}{:02X}", self.rgb[0], self.rgb[1], self.rgb[2]);
        r.draw_text(&hex, Point::new(64 - 21, HEX_Y));
        if !self.on {
            r.draw_line(Point::new(64 - 23, HEX_Y + 4), Point::new(64 + 21, HEX_Y + 4));
        }
    }
}

impl Scene for FlashlightScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let prefs = ctx.flashlight;
        self.mode = (prefs.mode as usize).min(MODES.len() - 1);
        self.level = (prefs.level as usize).min(LEVELS.len() - 1);
        self.rgb = prefs.rgb;
        self.focus = 0;
        self.on = true;
        self.t = 0.0;
        self.sent = None;
        self.apply(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        self.save_prefs(ctx);
        self.go_dark(ctx);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        let rgb_mode = self.mode() == Mode::Rgb;
        if rgb_mode {
            if buttons.was_just_pressed(Button::Up) && self.focus > 0 {
                self.focus -= 1;
            }
            if buttons.was_just_pressed(Button::Down) && self.focus < 3 {
                self.focus += 1;
            }
        } else {
            if buttons.was_just_pressed(Button::Up) && self.level + 1 < LEVELS.len() {
                self.level += 1;
            }
            if buttons.was_just_pressed(Button::Down) && self.level > 0 {
                self.level -= 1;
            }
        }
        if rgb_mode && self.focus > 0 {
            self.slide(buttons, dt);
        } else {
            if buttons.was_just_pressed(Button::Left) {
                self.step_mode(-1);
            }
            if buttons.was_just_pressed(Button::Right) {
                self.step_mode(1);
            }
        }
        if buttons.was_just_pressed(Button::A) {
            self.on = !self.on;
            self.t = 0.0;
        }
        self.advance(ctx, dt);
        self.save_prefs(ctx);
        None
    }

    /// The menu is open over the flashlight, which is how the player
    /// leaves, so the light goes out. Closing the menu relights it.
    fn tick_background(&mut self, ctx: &mut GameContext, _dt: f32) {
        self.go_dark(ctx);
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        self.draw_title(r);

        if self.mode() == Mode::Rgb {
            self.draw_sliders(r);
            return;
        }

        // A lens that mirrors the LED, so the state shows on desktop too.
        let centre = Point::new(64, 30);
        r.draw_circle_outline(centre, 13);
        if self.colour().is_some() {
            r.draw_circle_filled(centre, 10);
        }

        // Brightness bar.
        let seg_w = 12;
        let x0 = 64 - (LEVELS.len() as i32 * seg_w) / 2;
        for i in 0..LEVELS.len() as i32 {
            let x = x0 + i * seg_w + 1;
            let h = 2 + i;
            let y = 62 - h;
            r.draw_rect(Point::new(x, y), Size::new(seg_w as u32 - 2, h as u32), i <= self.level as i32);
        }
    }
}

/// 4px-tall triangle pointing left (`dir` = -1) or right (`dir` = 1), tip at x.
fn draw_arrow(r: &mut Renderer, tip_x: i32, cy: i32, dir: i32) {
    for k in 0..4 {
        let x = tip_x - dir * k;
        r.draw_line(Point::new(x, cy - k), Point::new(x, cy + k));
    }
}
