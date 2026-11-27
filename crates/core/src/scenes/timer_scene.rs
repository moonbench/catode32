//! Timer tool: stopwatch, countdown and interval timer.
//!
//! The mode name is the top row. When the timer is idle, Up/Down moves
//! between it and the mode's settings, and Left/Right changes whichever is
//! selected. A starts and pauses. B: lap (stopwatch, running) or reset
//! (paused). The LED blips through the last three seconds of a countdown
//! or interval phase, flashes green/blue as work/rest starts, and flashes
//! with the screen when time is up.
//!
//! Times come from the real clock (`now_ms`), not the frame `dt`, so they
//! ignore the debug time speed and keep counting under the menu. The timer
//! stops when the scene is left.

use embedded_graphics::prelude::{Point, Size};
use heapless::Vec;

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    led::{scale_rgb, LedLatch},
    platform::time::now_ms,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{repeat::HoldRepeat, timefmt, tiny_font, wrap},
};

/// Countdown lengths, seconds.
pub const COUNTDOWN_SECS: [u32; 26] = [
    10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 300, 360, 420, 480, 540, 600, 720, 900, 1200, 1500,
    1800, 2400, 2700, 3600, 5400, 7200,
];
/// Interval work and rest lengths, seconds. Rest may be zero.
pub const INTERVAL_SECS: [u32; 23] = [
    0, 5, 10, 15, 20, 25, 30, 40, 45, 50, 60, 75, 90, 120, 150, 180, 240, 300, 360, 420, 480, 540,
    600,
];
pub const MAX_ROUNDS: u8 = 99;

/// Where an interval session is at `elapsed_ms`: the round (1-based),
/// whether it's work, and the ms left in that phase. `None` once every
/// round is done. There's no rest after the last round.
pub fn interval_at(work_s: u32, rest_s: u32, rounds: u8, elapsed_ms: u64) -> Option<(u8, bool, u64)> {
    let work = work_s.max(1) as u64 * 1000;
    let rest = rest_s as u64 * 1000;
    let mut t = elapsed_ms;
    for round in 1..=rounds {
        if t < work {
            return Some((round, true, work - t));
        }
        t -= work;
        if round == rounds {
            break;
        }
        if t < rest {
            return Some((round, false, rest - t));
        }
        t -= rest;
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Stopwatch,
    Countdown,
    Interval,
}

const MODES: [Mode; 3] = [Mode::Stopwatch, Mode::Countdown, Mode::Interval];

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Stopwatch => t!("Stopwatch"),
            Mode::Countdown => t!("Countdown"),
            Mode::Interval => t!("Interval"),
        }
    }

    fn fields(self) -> usize {
        match self {
            Mode::Stopwatch => 0,
            Mode::Countdown => 1,
            Mode::Interval => 3,
        }
    }
}

const LAPS_SHOWN: usize = 3;
/// How long the time's-up alarm flashes before giving up.
const ALARM_MS: u64 = 60_000;
const FLASH_MS: u64 = 300;
const LED_LEVEL: u8 = 160;
const GREEN: (u8, u8, u8) = (0, 255, 40);
const BLUE: (u8, u8, u8) = (0, 80, 255);
const WHITE: (u8, u8, u8) = (255, 255, 255);

pub struct TimerScene {
    mode: usize,
    focus: usize,
    repeat: HoldRepeat,
    countdown: usize,
    work: usize,
    rest: usize,
    rounds: u8,

    running: bool,
    /// When the current run started, and time banked from earlier runs.
    started_ms: u64,
    banked_ms: u64,
    /// Stopwatch laps: count, elapsed at the last lap, newest lap times.
    lap_count: u16,
    last_lap_ms: u64,
    laps: Vec<u64, LAPS_SHOWN>,
    /// When the countdown or interval session ran out.
    alarm_since: Option<u64>,
    /// Interval phase last seen, to flash on a change.
    phase: Option<(u8, bool)>,
    flash: Option<(u64, (u8, u8, u8))>,
    led: LedLatch,
    now: u64,
}

impl TimerScene {
    pub fn new() -> Self {
        Self {
            mode: 0,
            focus: 0,
            repeat: HoldRepeat::new(),
            countdown: 10,
            work: 6,
            rest: 2,
            rounds: 8,
            running: false,
            started_ms: 0,
            banked_ms: 0,
            lap_count: 0,
            last_lap_ms: 0,
            laps: Vec::new(),
            alarm_since: None,
            phase: None,
            flash: None,
            led: LedLatch::new(),
            now: 0,
        }
    }

    fn mode(&self) -> Mode {
        MODES[self.mode]
    }

    fn elapsed(&self) -> u64 {
        self.banked_ms + if self.running { self.now.saturating_sub(self.started_ms) } else { 0 }
    }

    fn idle(&self) -> bool {
        !self.running && self.elapsed() == 0 && self.alarm_since.is_none()
    }

    fn countdown_ms(&self) -> u64 {
        COUNTDOWN_SECS[self.countdown] as u64 * 1000
    }

    fn interval_total_ms(&self) -> u64 {
        let (w, r) = (INTERVAL_SECS[self.work].max(1) as u64, INTERVAL_SECS[self.rest] as u64);
        (w * self.rounds as u64 + r * (self.rounds as u64 - 1)) * 1000
    }

    fn interval_now(&self) -> Option<(u8, bool, u64)> {
        interval_at(INTERVAL_SECS[self.work], INTERVAL_SECS[self.rest], self.rounds, self.elapsed())
    }

    fn reset(&mut self) {
        self.running = false;
        self.banked_ms = 0;
        self.lap_count = 0;
        self.last_lap_ms = 0;
        self.laps.clear();
        self.alarm_since = None;
        self.phase = None;
        self.flash = None;
    }

    fn toggle_run(&mut self) {
        if self.running {
            self.banked_ms = self.elapsed();
            self.running = false;
        } else {
            self.started_ms = self.now;
            self.running = true;
        }
    }

    fn lap(&mut self) {
        let e = self.elapsed();
        if self.laps.is_full() {
            self.laps.pop();
        }
        let _ = self.laps.insert(0, e - self.last_lap_ms);
        self.last_lap_ms = e;
        self.lap_count = self.lap_count.saturating_add(1);
    }

    fn adjust(&mut self, dir: i32) {
        let step = |v: usize, len: usize| (v as i32 + dir).clamp(0, len as i32 - 1) as usize;
        match (self.mode(), self.focus) {
            (_, 0) => {
                let n = MODES.len() as i32;
                self.mode = (self.mode as i32 + dir).rem_euclid(n) as usize;
            }
            (Mode::Countdown, _) => self.countdown = step(self.countdown, COUNTDOWN_SECS.len()),
            (Mode::Interval, 1) => self.work = step(self.work, INTERVAL_SECS.len()).max(1),
            (Mode::Interval, 2) => self.rest = step(self.rest, INTERVAL_SECS.len()),
            (Mode::Interval, _) => {
                self.rounds = (self.rounds as i32 + dir).clamp(1, MAX_ROUNDS as i32) as u8;
            }
            _ => {}
        }
    }

    fn save_prefs(&self, ctx: &mut GameContext) {
        let p = &mut ctx.tool_prefs;
        p.countdown = self.countdown as u8;
        p.interval_work = self.work as u8;
        p.interval_rest = self.rest as u8;
        p.interval_rounds = self.rounds;
    }

    /// Clock-driven bookkeeping: run-outs, phase changes and the LED.
    /// Runs every frame, menu open or not.
    fn advance(&mut self, ctx: &mut GameContext) {
        self.now = now_ms();
        let mut phase_left = None;
        match self.mode() {
            Mode::Stopwatch => {}
            Mode::Countdown => {
                if self.running {
                    let left = self.countdown_ms().saturating_sub(self.elapsed());
                    if left == 0 {
                        self.running = false;
                        self.banked_ms = self.countdown_ms();
                        self.alarm_since = Some(self.now);
                    } else {
                        phase_left = Some(left);
                    }
                }
            }
            Mode::Interval => {
                if self.running {
                    match self.interval_now() {
                        Some((round, work, left)) => {
                            if self.phase != Some((round, work)) {
                                self.phase = Some((round, work));
                                let colour = if work { GREEN } else { BLUE };
                                self.flash = Some((self.now + FLASH_MS, colour));
                            }
                            phase_left = Some(left);
                        }
                        None => {
                            self.running = false;
                            self.banked_ms = self.interval_total_ms();
                            self.alarm_since = Some(self.now);
                        }
                    }
                }
            }
        }

        let alarming = self.alarm_since.is_some_and(|t| self.now - t < ALARM_MS);
        ctx.keep_awake = self.running || alarming;

        let rgb = if alarming {
            if self.now / 250 % 2 == 0 { WHITE } else { (0, 0, 0) }
        } else if let Some((_, colour)) = self.flash.filter(|&(until, _)| self.now < until) {
            colour
        } else if phase_left.is_some_and(|l| l <= 3000 && (l - 1) % 1000 >= 850) {
            // A blip at the start of each of the last three seconds.
            WHITE
        } else {
            (0, 0, 0)
        };
        self.led.show(&mut ctx.led, scale_rgb(rgb, LED_LEVEL));
    }

    fn alarming(&self) -> bool {
        self.alarm_since.is_some()
    }

    // --- drawing --------------------------------------------------------

    fn draw_title(&self, r: &mut Renderer) {
        let label = if self.mode() == Mode::Interval && !self.idle() && !self.alarming() {
            None
        } else if self.alarming() {
            Some(t!("Time's up!"))
        } else {
            Some(self.mode().label())
        };
        let mut phase: heapless::String<24> = heapless::String::new();
        let text = match label {
            Some(l) => l,
            None => {
                use core::fmt::Write;
                if let Some((round, work, _)) = self.interval_now() {
                    let name = if work { t!("Work") } else { t!("Rest") };
                    let _ = write!(phase, "{} {}/{}", name, round, self.rounds);
                }
                phase.as_str()
            }
        };
        let w = text.chars().count() as i32 * 6;
        let x = 64 - w / 2;
        if self.idle() && self.focus == 0 {
            r.draw_rect(Point::new(x - 2, 0), Size::new(w as u32 + 3, 11), true);
            r.draw_text_inverted(text, Point::new(x, 1));
        } else {
            r.draw_text(text, Point::new(x, 1));
        }
        if self.idle() {
            draw_arrow(r, 4, 5, -1);
            draw_arrow(r, 123, 5, 1);
        }
    }

    fn draw_big(&self, r: &mut Renderer, text: &str, selected: bool) {
        let w = tiny_font::str_width(text, 3);
        let x = 64 - w / 2;
        tiny_font::draw_str_scaled(r, text, x, 15, 3, true);
        if selected {
            draw_arrow(r, x - 8, 22, -1);
            draw_arrow(r, x + w + 7, 22, 1);
        }
    }

    fn draw_bar(&self, r: &mut Renderer, left: u64, total: u64) {
        r.draw_rect(Point::new(14, 37), Size::new(100, 5), false);
        let fill = if total == 0 { 0 } else { (left * 98 / total) as u32 };
        if fill > 0 {
            r.draw_rect(Point::new(15, 38), Size::new(fill, 3), true);
        }
    }

    fn draw_hint(&self, r: &mut Renderer) {
        let hint = if self.alarming() {
            t!("A: stop")
        } else if self.running {
            match self.mode() {
                Mode::Stopwatch => t!("B: lap"),
                _ => t!("A: pause"),
            }
        } else if self.idle() {
            t!("A: start")
        } else {
            t!("A: go  B: reset")
        };
        let w = hint.chars().count() as i32 * 6;
        r.draw_text(hint, Point::new(64 - w / 2, 53));
    }

    fn draw_setting(&self, r: &mut Renderer, y: i32, label: &str, value: &str, selected: bool) {
        if selected {
            r.draw_rect(Point::new(0, y - 1), Size::new(128, 11), true);
            r.draw_text_inverted(label, Point::new(4, y));
            r.draw_text_inverted(value, Point::new(124 - value.chars().count() as i32 * 6, y));
        } else {
            r.draw_text(label, Point::new(4, y));
            r.draw_text(value, Point::new(124 - value.chars().count() as i32 * 6, y));
        }
    }
}

impl Scene for TimerScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let p = ctx.tool_prefs;
        self.countdown = (p.countdown as usize).min(COUNTDOWN_SECS.len() - 1);
        self.work = (p.interval_work as usize).clamp(1, INTERVAL_SECS.len() - 1);
        self.rest = (p.interval_rest as usize).min(INTERVAL_SECS.len() - 1);
        self.rounds = p.interval_rounds.clamp(1, MAX_ROUNDS);
        self.reset();
        self.led.forget();
        self.advance(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        self.save_prefs(ctx);
        ctx.keep_awake = false;
        self.led.forget();
        self.led.off(&mut ctx.led);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.now = now_ms();
        let a = buttons.was_just_pressed(Button::A);
        let b = buttons.was_just_pressed(Button::B);

        if self.alarming() {
            if a || b {
                self.reset();
            }
        } else if self.idle() {
            let fields = self.mode().fields();
            if buttons.was_just_pressed(Button::Up) && self.focus > 0 {
                self.focus -= 1;
                self.repeat.reset();
            }
            if buttons.was_just_pressed(Button::Down) && self.focus < fields {
                self.focus += 1;
                self.repeat.reset();
            }
            if let Some(step) = self.repeat.poll(buttons, Button::Left, Button::Right, dt) {
                // The mode only changes on a tap.
                if self.focus > 0 || step.held == 0.0 {
                    self.adjust(step.dir);
                    self.focus = self.focus.min(self.mode().fields());
                    self.save_prefs(ctx);
                }
            }
            if a {
                self.toggle_run();
            }
        } else if self.running {
            if a {
                self.toggle_run();
            } else if b && self.mode() == Mode::Stopwatch {
                self.lap();
            }
        } else {
            // Paused with time on the clock.
            if a {
                self.toggle_run();
            } else if b {
                self.reset();
            }
        }
        self.advance(ctx);
        None
    }

    fn tick_background(&mut self, ctx: &mut GameContext, _dt: f32) {
        self.advance(ctx);
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        // The whole screen flashes with the LED when time is up.
        if self.alarming() && self.now / 250 % 2 == 0 {
            r.set_invert(true);
        }
        self.draw_title(r);
        let editing = self.idle();

        match self.mode() {
            Mode::Stopwatch => {
                self.draw_big(r, &timefmt::clock(self.elapsed(), true), false);
                if self.laps.is_empty() {
                    self.draw_hint(r);
                } else {
                    use core::fmt::Write;
                    for (i, lap) in self.laps.iter().enumerate() {
                        let mut line: heapless::String<24> = heapless::String::new();
                        let n = self.lap_count as usize - i;
                        let _ = write!(line, "{:>2}  {}", n, timefmt::clock(*lap, true));
                        let w = line.chars().count() as i32 * 6;
                        r.draw_text(&line, Point::new(64 - w / 2, 33 + i as i32 * 10));
                    }
                }
            }
            Mode::Countdown => {
                let total = self.countdown_ms();
                let left = total.saturating_sub(self.elapsed());
                self.draw_big(r, &timefmt::remaining(left), editing && self.focus == 1);
                if !editing {
                    self.draw_bar(r, left, total);
                }
                self.draw_hint(r);
            }
            Mode::Interval => {
                if editing {
                    use core::fmt::Write;
                    let work = timefmt::clock(INTERVAL_SECS[self.work] as u64 * 1000, false);
                    let rest = timefmt::clock(INTERVAL_SECS[self.rest] as u64 * 1000, false);
                    let mut rounds: heapless::String<4> = heapless::String::new();
                    let _ = write!(rounds, "{}", self.rounds);
                    self.draw_setting(r, 14, t!("Work"), &work, self.focus == 1);
                    self.draw_setting(r, 26, t!("Rest"), &rest, self.focus == 2);
                    self.draw_setting(r, 38, t!("Rounds"), &rounds, self.focus == 3);
                    self.draw_hint(r);
                } else {
                    let (left, phase_total) = match self.interval_now() {
                        Some((_, work, left)) => {
                            let secs = if work { INTERVAL_SECS[self.work].max(1) } else { INTERVAL_SECS[self.rest] };
                            (left, secs as u64 * 1000)
                        }
                        None => (0, 1),
                    };
                    self.draw_big(r, &timefmt::remaining(left), false);
                    self.draw_bar(r, left, phase_total);
                    if self.running {
                        let total_left = self.interval_total_ms().saturating_sub(self.elapsed());
                        let mut line: heapless::String<24> = heapless::String::new();
                        {
                            use core::fmt::Write;
                            let _ = write!(line, "{} {}", t!("Total"), timefmt::remaining(total_left));
                        }
                        wrap::draw_centered(r, &[line.as_str()], 64, 50, 10);
                    } else {
                        self.draw_hint(r);
                    }
                }
            }
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
