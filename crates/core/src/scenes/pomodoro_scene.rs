//! Pomodoro tool: focus for a while, take a short break, and after every
//! fourth focus session take a long one. The cat naps while you work and
//! stretches when it's break time.
//!
//! Layout (128x64):
//!   x=0..91    settings, or the phase, time left and the session dots
//!   x=92..127  the cat
//!
//! Before starting: Up/Down picks a setting, Left/Right changes it, A
//! starts. Running: A pauses; paused, B resets. When a phase ends the LED
//! glows and the timer waits for A to start the next one.
//!
//! Times come from the real clock, so they ignore the debug time speed and
//! keep counting under the menu. The timer stops when the scene is left.

use embedded_graphics::prelude::{Point, Size};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::t;
use crate::{
    assets::character::PoseId,
    context::GameContext,
    entities::character::Character,
    input::{Button, Buttons},
    led::{scale_rgb, LedLatch},
    platform::time::now_ms,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{repeat::HoldRepeat, timefmt, tiny_font, wrap},
};

/// Focus sessions per set; the break after the last one is long.
pub const SESSIONS_PER_SET: u32 = 4;

/// Minimum, maximum and step for each setting, in minutes:
/// focus, break, long break.
const LIMITS: [(u8, u8, u8); 3] = [(5, 90, 5), (1, 30, 1), (5, 60, 5)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Focus,
    Break,
    LongBreak,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Phase::Focus => t!("Focus"),
            Phase::Break => t!("Break"),
            Phase::LongBreak => t!("Long break"),
        }
    }
}

/// The phase after `phase`, given how many focus sessions are done
/// (including one that just ended).
pub fn next_phase(phase: Phase, focus_done: u32) -> Phase {
    match phase {
        Phase::Focus if focus_done % SESSIONS_PER_SET == 0 => Phase::LongBreak,
        Phase::Focus => Phase::Break,
        Phase::Break | Phase::LongBreak => Phase::Focus,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Setup,
    /// Counting down `phase`; `running` false when paused.
    Timing { phase: Phase, running: bool },
    /// `ended` just finished; A starts the next phase.
    Ended { ended: Phase, since: u64 },
}

const PANEL_CX: i32 = 108;
const AREA_CX: i32 = 45;
const GLOW_MS: u64 = 60_000;
const STRETCH_MS: u64 = 4_000;
const WARM: (u8, u8, u8) = (255, 140, 40);

pub struct PomodoroScene {
    state: State,
    minutes: [u8; 3],
    focus: usize,
    repeat: HoldRepeat,
    started_ms: u64,
    banked_ms: u64,
    /// Focus sessions finished this visit.
    focus_done: u32,
    led: LedLatch,
    now: u64,
    character: Character,
}

impl PomodoroScene {
    pub fn new() -> Self {
        Self {
            state: State::Setup,
            minutes: [25, 5, 15],
            focus: 0,
            repeat: HoldRepeat::new(),
            started_ms: 0,
            banked_ms: 0,
            focus_done: 0,
            led: LedLatch::new(),
            now: 0,
            character: Character::new(Point::new(PANEL_CX, 63)),
        }
    }

    fn phase_ms(&self, phase: Phase) -> u64 {
        let m = match phase {
            Phase::Focus => self.minutes[0],
            Phase::Break => self.minutes[1],
            Phase::LongBreak => self.minutes[2],
        };
        m as u64 * 60_000
    }

    fn elapsed(&self) -> u64 {
        let live = match self.state {
            State::Timing { running: true, .. } => self.now.saturating_sub(self.started_ms),
            _ => 0,
        };
        self.banked_ms + live
    }

    fn start_phase(&mut self, phase: Phase) {
        self.banked_ms = 0;
        self.started_ms = self.now;
        self.state = State::Timing { phase, running: true };
    }

    fn pose(&self) -> PoseId {
        match self.state {
            State::Setup => PoseId::SittingSideNeutral,
            State::Timing { phase: Phase::Focus, running: true } => PoseId::SleepingSideSploot,
            State::Timing { running: false, .. } => PoseId::SittingSideLookingDown,
            State::Timing { .. } if self.elapsed() < STRETCH_MS => PoseId::LeaningForwardSideStretch,
            State::Timing { .. } => PoseId::SittingSideHappy,
            // Waking up for a break; settled and ready after one.
            State::Ended { ended: Phase::Focus, since } if self.now - since < STRETCH_MS => {
                PoseId::LeaningForwardSideStretch
            }
            State::Ended { .. } => PoseId::SittingSideHappy,
        }
    }

    fn advance(&mut self, ctx: &mut GameContext) {
        self.now = now_ms();
        if let State::Timing { phase, running: true } = self.state {
            if self.elapsed() >= self.phase_ms(phase) {
                if phase == Phase::Focus {
                    self.focus_done += 1;
                }
                self.state = State::Ended { ended: phase, since: self.now };
            }
        }
        let glow = match self.state {
            State::Ended { since, .. } if self.now - since < GLOW_MS => {
                // A slow, soft pulse.
                let t = (self.now - since) as f32 / 1000.0;
                let level = 20.0 + 70.0 * (0.5 - 0.5 * (t * core::f32::consts::PI).cos());
                Some(scale_rgb(WARM, level as u8))
            }
            _ => None,
        };
        self.led.show(&mut ctx.led, glow.unwrap_or((0, 0, 0)));
        ctx.keep_awake = matches!(self.state, State::Timing { running: true, .. }) || glow.is_some();
    }

    fn save_prefs(&self, ctx: &mut GameContext) {
        ctx.tool_prefs.pomodoro = self.minutes;
    }

    fn draw_setting(&self, r: &mut Renderer, y: i32, label: &str, minutes: u8, selected: bool) {
        use core::fmt::Write;
        let mut value: heapless::String<8> = heapless::String::new();
        let _ = write!(value, "{}m", minutes);
        let vx = 91 - value.len() as i32 * 6;
        if selected {
            r.draw_rect(Point::new(0, y - 1), Size::new(92, 11), true);
            r.draw_text_inverted(label, Point::new(1, y));
            r.draw_text_inverted(&value, Point::new(vx, y));
        } else {
            r.draw_text(label, Point::new(1, y));
            r.draw_text(&value, Point::new(vx, y));
        }
    }

    fn draw_centered(&self, r: &mut Renderer, text: &str, y: i32) {
        let lines: heapless::Vec<&str, 3> = wrap::wrap(text, 15);
        wrap::draw_centered(r, &lines, AREA_CX, y, 10);
    }

    /// Dots for the focus sessions in this set, filled when done.
    fn draw_dots(&self, r: &mut Renderer) {
        let in_set = match self.state {
            // Straight after the fourth, show the full set.
            State::Ended { ended: Phase::Focus, .. } | State::Timing { phase: Phase::LongBreak, .. }
                if self.focus_done > 0 && self.focus_done % SESSIONS_PER_SET == 0 =>
            {
                SESSIONS_PER_SET
            }
            _ => self.focus_done % SESSIONS_PER_SET,
        };
        for i in 0..SESSIONS_PER_SET {
            let c = Point::new(AREA_CX - 24 + i as i32 * 16, 42);
            if i < in_set {
                r.draw_circle_filled(c, 3);
            } else {
                r.draw_circle_outline(c, 3);
            }
        }
    }
}

impl Scene for PomodoroScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.minutes = ctx.tool_prefs.pomodoro;
        for (m, &(lo, hi, _)) in self.minutes.iter_mut().zip(LIMITS.iter()) {
            *m = (*m).clamp(lo, hi);
        }
        self.state = State::Setup;
        self.focus_done = 0;
        self.led.forget();
        self.advance(ctx);
        self.character.set_pose(self.pose());
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        // No reward yet. `focus_done` counts this visit's finished focus
        // sessions, ready for one.
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
        match self.state {
            State::Setup => {
                if buttons.was_just_pressed(Button::Up) && self.focus > 0 {
                    self.focus -= 1;
                    self.repeat.reset();
                }
                if buttons.was_just_pressed(Button::Down) && self.focus < 2 {
                    self.focus += 1;
                    self.repeat.reset();
                }
                if let Some(step) = self.repeat.poll(buttons, Button::Left, Button::Right, dt) {
                    let (lo, hi, by) = LIMITS[self.focus];
                    let m = &mut self.minutes[self.focus];
                    *m = (*m as i32 + step.dir * by as i32).clamp(lo as i32, hi as i32) as u8;
                    self.save_prefs(ctx);
                }
                if a {
                    self.start_phase(Phase::Focus);
                }
            }
            State::Timing { phase, running } => {
                if a {
                    if running {
                        self.banked_ms = self.elapsed();
                    } else {
                        self.started_ms = self.now;
                    }
                    self.state = State::Timing { phase, running: !running };
                } else if b && !running {
                    self.state = State::Setup;
                    self.focus_done = 0;
                }
            }
            State::Ended { ended, .. } => {
                if a {
                    self.start_phase(next_phase(ended, self.focus_done));
                } else if b {
                    self.state = State::Setup;
                    self.focus_done = 0;
                }
            }
        }
        self.advance(ctx);
        self.character.set_pose(self.pose());
        self.character.animate(dt);
        None
    }

    fn tick_background(&mut self, ctx: &mut GameContext, _dt: f32) {
        self.advance(ctx);
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        match self.state {
            State::Setup => {
                self.draw_centered(r, t!("Pomodoro"), 1);
                self.draw_setting(r, 14, t!("Focus"), self.minutes[0], self.focus == 0);
                self.draw_setting(r, 26, t!("Break"), self.minutes[1], self.focus == 1);
                self.draw_setting(r, 38, t!("Long break"), self.minutes[2], self.focus == 2);
                self.draw_centered(r, t!("A: start"), 53);
            }
            State::Timing { phase, running } => {
                self.draw_centered(r, phase.label(), 1);
                let left = self.phase_ms(phase).saturating_sub(self.elapsed());
                let text = timefmt::remaining(left);
                let w = tiny_font::str_width(&text, 3);
                tiny_font::draw_str_scaled(r, &text, AREA_CX - w / 2, 16, 3, true);
                self.draw_dots(r);
                let hint = if running { t!("A: pause") } else { t!("A: go  B: reset") };
                self.draw_centered(r, hint, 53);
            }
            State::Ended { ended, .. } => {
                let message = match next_phase(ended, self.focus_done) {
                    Phase::Focus => t!("Back to it!"),
                    Phase::Break => t!("Break time!"),
                    Phase::LongBreak => t!("Long break time!"),
                };
                self.draw_centered(r, message, 8);
                self.draw_dots(r);
                self.draw_centered(r, t!("A: start"), 53);
            }
        }
        self.character.draw(r, 0);
    }
}
