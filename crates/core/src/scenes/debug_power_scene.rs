use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
use crate::t;

use crate::{
    context::{GameContext, PowerAction},
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
};

const ROW_HEIGHT: i32 = 16;
const CONTENT_WIDTH: u32 = 128;
/// Crash page: FONT_6X10 rows and columns on the 128x64 screen.
const TEXT_ROW_HEIGHT: i32 = 10;
const TEXT_COLS: usize = 21;
const TEXT_ROWS: usize = 6;

struct Entry {
    label: &'static str,
    action: PowerAction,
}

const ENTRIES: &[Entry] = &[
    Entry { label: t!("Reboot"),      action: PowerAction::Reboot },
    Entry { label: t!("Light Sleep"), action: PowerAction::LightSleep },
    Entry { label: t!("Deep Sleep"),  action: PowerAction::DeepSleep },
];

pub struct DebugPowerScene {
    selected: usize,
    /// Showing the last panic instead of the power menu. Only reachable
    /// when `ctx.last_crash` is set.
    crash_page: bool,
    /// First crash-page line shown, for Up/Down scrolling.
    crash_scroll: usize,
}

impl DebugPowerScene {
    pub fn new() -> Self {
        Self {
            selected: 0,
            crash_page: false,
            crash_scroll: 0,
        }
    }

    fn update_crash_page(&mut self, ctx: &GameContext, buttons: &mut Buttons) {
        if buttons.was_just_pressed(Button::Left) || buttons.was_just_pressed(Button::B) {
            self.crash_page = false;
            return;
        }
        let max_scroll = crash_lines(ctx).len().saturating_sub(TEXT_ROWS);
        if buttons.was_just_pressed(Button::Up) {
            self.crash_scroll = self.crash_scroll.saturating_sub(1);
        }
        if buttons.was_just_pressed(Button::Down) {
            self.crash_scroll = (self.crash_scroll + 1).min(max_scroll);
        }
    }
}

/// The last panic, hard-wrapped to the screen width, followed by one line
/// per recorded return address.
fn crash_lines(ctx: &GameContext) -> heapless::Vec<String<TEXT_COLS>, 16> {
    let mut lines = heapless::Vec::new();
    let Some(crash) = &ctx.last_crash else {
        return lines;
    };
    let mut line: String<TEXT_COLS> = String::new();
    for c in crash.msg.chars() {
        if line.push(c).is_err() {
            let _ = lines.push(core::mem::take(&mut line));
            let _ = line.push(c);
        }
    }
    if !line.is_empty() {
        let _ = lines.push(line);
    }
    for pc in crash.pcs.iter() {
        let mut line = String::new();
        let _ = write!(line, "@ 0x{:08x}", pc);
        let _ = lines.push(line);
    }
    lines
}

impl Scene for DebugPowerScene {
    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        if self.crash_page {
            self.update_crash_page(ctx, buttons);
            return None;
        }
        if buttons.was_just_pressed(Button::Right) && ctx.last_crash.is_some() {
            self.crash_page = true;
            self.crash_scroll = 0;
            return None;
        }
        if buttons.was_just_pressed(Button::B)
            || buttons.was_just_pressed(Button::Menu1)
            || buttons.was_just_pressed(Button::Menu2)
        {
            return Some(ctx.last_main_scene);
        }
        if buttons.was_just_pressed(Button::Up) && self.selected > 0 {
            self.selected -= 1;
        }
        if buttons.was_just_pressed(Button::Down) && self.selected + 1 < ENTRIES.len() {
            self.selected += 1;
        }
        if buttons.was_just_pressed(Button::A) {
            ctx.pending_power = Some(ENTRIES[self.selected].action);
            return Some(ctx.last_main_scene);
        }
        None
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.crash_page {
            let lines = crash_lines(ctx);
            for (row, line) in lines.iter().skip(self.crash_scroll).take(TEXT_ROWS).enumerate() {
                renderer.draw_text(line.as_str(), Point::new(1, row as i32 * TEXT_ROW_HEIGHT));
            }
            return;
        }

        // Below the three 16px rows. Shown on-device because brownouts and
        // crashes mostly happen off USB, where there is no serial log to read.
        let mut line: String<32> = String::new();
        if ctx.last_crash.is_some() {
            let _ = line.push_str("Crashed! > details");
        } else {
            let _ = write!(line, "Reset: {}", ctx.last_reset_reason);
        }
        renderer.draw_text(line.as_str(), Point::new(2, ENTRIES.len() as i32 * ROW_HEIGHT + 4));

        for (i, entry) in ENTRIES.iter().enumerate() {
            let y = i as i32 * ROW_HEIGHT;
            let selected = i == self.selected;
            if selected {
                renderer.draw_rect(
                    Point::new(0, y),
                    Size::new(CONTENT_WIDTH, ROW_HEIGHT as u32),
                    true,
                );
            }
            let text_y = y + (ROW_HEIGHT - 8) / 2;
            if selected {
                renderer.draw_text_inverted(entry.label, Point::new(2, text_y));
            } else {
                renderer.draw_text(entry.label, Point::new(2, text_y));
            }
        }
    }
}
