//! Metronome. The LED does the beat: the frame loop only runs at 12 FPS,
//! so beats are queued as timed LED cues (`ctx.schedule_led`) that the
//! main loop fires to the millisecond while it waits between frames. The
//! screen follows along at frame rate.
//!
//! Up/Down sets the tempo (hold to speed up), Left/Right the beats per
//! bar (the first is accented), A starts and stops, B taps a tempo.

use core::fmt::Write;
use embedded_graphics::prelude::Point;
use heapless::{String, Vec};

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    led::scale_rgb,
    platform::time::now_us,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{repeat::HoldRepeat, tiny_font},
};

pub const BPM_MIN: u16 = 30;
pub const BPM_MAX: u16 = 240;
pub const BEATS_MAX: u8 = 8;

const ACCENT: (u8, u8, u8) = (0, 90, 255);
const BEAT: (u8, u8, u8) = (255, 60, 0);
const LED_LEVEL: u8 = 160;
const FLASH_US: u64 = 90_000;
/// How far ahead beats are queued. A bit over two frames.
const LOOKAHEAD_US: u64 = 200_000;
/// Taps further apart than this start a new tap tempo.
const TAP_RESET_US: u64 = 2_000_000;

/// Microseconds between beats.
pub fn beat_period_us(bpm: u16) -> u64 {
    60_000_000 / bpm.clamp(BPM_MIN, BPM_MAX) as u64
}

/// Tempo from tap times (oldest first): the average gap, as BPM.
pub fn tapped_bpm(taps: &[u64]) -> Option<u16> {
    if taps.len() < 2 {
        return None;
    }
    let span = taps[taps.len() - 1] - taps[0];
    let gap = span / (taps.len() as u64 - 1);
    if gap == 0 {
        return None;
    }
    Some((60_000_000 / gap).clamp(BPM_MIN as u64, BPM_MAX as u64) as u16)
}

pub struct MetronomeScene {
    bpm: u16,
    beats: u8,
    running: bool,
    repeat: HoldRepeat,
    /// When the next unqueued beat falls, and its place in the bar.
    next_beat_us: u64,
    next_index: u8,
    /// Beats queued recently, for drawing: (time, place in bar).
    recent: Vec<(u64, u8), 4>,
    taps: Vec<u64, 5>,
    now: u64,
}

impl MetronomeScene {
    pub fn new() -> Self {
        Self {
            bpm: 100,
            beats: 4,
            running: false,
            repeat: HoldRepeat::new(),
            next_beat_us: 0,
            next_index: 0,
            recent: Vec::new(),
            taps: Vec::new(),
            now: 0,
        }
    }

    fn start(&mut self, ctx: &mut GameContext) {
        ctx.led_cues.clear();
        self.recent.clear();
        self.next_beat_us = self.now + 50_000;
        self.next_index = 0;
        self.running = true;
    }

    fn stop(&mut self, ctx: &mut GameContext) {
        self.running = false;
        ctx.led_cues.clear();
        ctx.led.off();
    }

    /// Queue every beat due before the lookahead runs out.
    fn queue_beats(&mut self, ctx: &mut GameContext) {
        while self.next_beat_us <= self.now + LOOKAHEAD_US {
            let at = self.next_beat_us;
            let accent = self.next_index == 0 && self.beats > 1;
            let colour = scale_rgb(if accent { ACCENT } else { BEAT }, LED_LEVEL);
            ctx.schedule_led(at, colour);
            ctx.schedule_led(at + FLASH_US, (0, 0, 0));
            if self.recent.is_full() {
                self.recent.remove(0);
            }
            let _ = self.recent.push((at, self.next_index));
            self.next_beat_us += beat_period_us(self.bpm);
            self.next_index = (self.next_index + 1) % self.beats;
        }
    }

    /// The latest beat that has sounded, and whether it's still flashing.
    fn current_beat(&self) -> Option<(u8, bool)> {
        self.recent
            .iter()
            .rev()
            .find(|&&(at, _)| at <= self.now)
            .map(|&(at, i)| (i, self.now - at < FLASH_US + 40_000))
    }

    fn tap(&mut self) {
        if self.taps.last().is_some_and(|&t| self.now - t > TAP_RESET_US) {
            self.taps.clear();
        }
        if self.taps.is_full() {
            self.taps.remove(0);
        }
        let _ = self.taps.push(self.now);
        if let Some(bpm) = tapped_bpm(&self.taps) {
            self.bpm = bpm;
        }
    }
}

impl Scene for MetronomeScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.bpm = ctx.tool_prefs.metronome_bpm.clamp(BPM_MIN, BPM_MAX);
        self.beats = ctx.tool_prefs.metronome_beats.clamp(1, BEATS_MAX);
        self.running = false;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        self.stop(ctx);
        ctx.keep_awake = false;
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.now = now_us();
        if let Some(step) = self.repeat.poll(buttons, Button::Down, Button::Up, dt) {
            let size = if step.held > 1.0 { 5 } else { 1 };
            self.bpm = (self.bpm as i32 + step.dir * size).clamp(BPM_MIN as i32, BPM_MAX as i32) as u16;
        }
        if buttons.was_just_pressed(Button::Left) && self.beats > 1 {
            self.beats -= 1;
        }
        if buttons.was_just_pressed(Button::Right) && self.beats < BEATS_MAX {
            self.beats += 1;
        }
        if self.next_index >= self.beats {
            self.next_index = 0;
        }
        if buttons.was_just_pressed(Button::A) {
            if self.running {
                self.stop(ctx);
            } else {
                self.start(ctx);
            }
        }
        if buttons.was_just_pressed(Button::B) {
            self.tap();
        }
        if self.running {
            self.queue_beats(ctx);
        }
        ctx.keep_awake = self.running;
        ctx.tool_prefs.metronome_bpm = self.bpm;
        ctx.tool_prefs.metronome_beats = self.beats;
        None
    }

    /// The menu is open: stop, like leaving.
    fn tick_background(&mut self, ctx: &mut GameContext, _dt: f32) {
        if self.running {
            self.stop(ctx);
            ctx.keep_awake = false;
        }
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        // Tempo, large, with "BPM" beside it.
        let mut bpm: String<4> = String::new();
        let _ = write!(bpm, "{}", self.bpm);
        let w = tiny_font::str_width(&bpm, 4);
        let label = "BPM";
        let total = w + 4 + 18;
        let x = 64 - total / 2;
        tiny_font::draw_str_scaled(r, &bpm, x, 4, 4, true);
        r.draw_text(label, Point::new(x + w + 4, 14));

        // One dot per beat in the bar; the sounding one filled.
        let current = if self.running { self.current_beat() } else { None };
        let pitch = 14;
        let x0 = 64 - (self.beats as i32 - 1) * pitch / 2;
        for i in 0..self.beats {
            let c = Point::new(x0 + i as i32 * pitch, 38);
            let radius = if i == 0 && self.beats > 1 { 5 } else { 4 };
            match current {
                Some((beat, flashing)) if beat == i && flashing => r.draw_circle_filled(c, radius),
                Some((beat, _)) if beat == i => {
                    r.draw_circle_outline(c, radius);
                    r.draw_circle_filled(c, 1);
                }
                _ => r.draw_circle_outline(c, radius),
            }
        }

        let hint = if self.running { t!("A: stop  B: tap") } else { t!("A: start  B: tap") };
        let hw = hint.chars().count() as i32 * 6;
        r.draw_text(hint, Point::new(64 - hw / 2, 53));
    }
}
