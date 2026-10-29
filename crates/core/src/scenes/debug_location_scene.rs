//! Debug screen for the wifi tracker. Shows the familiar/recent AP lists,
//! the last live-scan results, and the `in_familiar_location` flag. A press
//! of A triggers a fresh scan (handled by `Game::update` via
//! `ctx.location_scan_requested`).

use core::fmt::Write as _;

use embedded_graphics::prelude::Point;
use heapless::{String, Vec};

use crate::{
    context::{GameContext, LocationEntry, LOCATION_FAMILIAR_MAX, LOCATION_RECENT_MAX},
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
};

const LINES_VISIBLE: usize = 8;
const LINE_HEIGHT: i32 = 8;
/// Cap on the total lines we render. Familiar (16) + recent (8) + headers
/// + spacers comfortably fit; pad for safety.
const MAX_LINES: usize = 64;
type Line = String<24>;

pub struct DebugLocationScene {
    lines: Vec<Line, MAX_LINES>,
    scroll: usize,
}

impl DebugLocationScene {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            scroll: 0,
        }
    }

    fn rebuild(&mut self, ctx: &GameContext) {
        self.lines.clear();
        push_line(&mut self.lines, |s| {
            let flag = if ctx.in_familiar_location { 'Y' } else { 'N' };
            let _ = write!(s, "Home? {}", flag);
        });
        push_line(&mut self.lines, |s| {
            let _ = write!(s, "Reset: {}", ctx.last_reset_reason);
        });
        push_blank(&mut self.lines);

        push_line(&mut self.lines, |s| {
            let _ = write!(s, "Familiar: {}/{}", ctx.loc_familiar.len(), LOCATION_FAMILIAR_MAX);
        });
        if ctx.loc_familiar.is_empty() {
            push_static(&mut self.lines, "(none yet)");
        } else {
            for e in ctx.loc_familiar.iter() {
                push_entry(&mut self.lines, e);
            }
        }
        push_blank(&mut self.lines);

        push_line(&mut self.lines, |s| {
            let _ = write!(s, "Recent: {}/{}", ctx.loc_recent.len(), LOCATION_RECENT_MAX);
        });
        if ctx.loc_recent.is_empty() {
            push_static(&mut self.lines, "(none yet)");
        } else {
            for e in ctx.loc_recent.iter() {
                push_entry(&mut self.lines, e);
            }
        }
        push_blank(&mut self.lines);
        push_static(&mut self.lines, "A: rescan");
        push_static(&mut self.lines, "B: back");
    }
}

fn push_blank(lines: &mut Vec<Line, MAX_LINES>) {
    let _ = lines.push(Line::new());
}

fn push_static(lines: &mut Vec<Line, MAX_LINES>, text: &str) {
    let mut s = Line::new();
    let _ = s.push_str(text);
    let _ = lines.push(s);
}

fn push_line<F: FnOnce(&mut Line)>(lines: &mut Vec<Line, MAX_LINES>, f: F) {
    let mut s = Line::new();
    f(&mut s);
    let _ = lines.push(s);
}

fn push_entry(lines: &mut Vec<Line, MAX_LINES>, e: &LocationEntry) {
    let mut s = Line::new();
    let label: &str = if e.name.is_empty() {
        // No name, so fall back to the first 10 chars of the address for some
        // identifying info.
        let _ = write!(
            s,
            "{:02x}:{:02x}:{:02x} {:.1}",
            e.addr[3], e.addr[4], e.addr[5], e.count
        );
        let _ = lines.push(s);
        return;
    } else {
        e.name.as_str()
    };
    // Truncate name to 10 chars for layout.
    let mut shown: String<10> = String::new();
    for c in label.chars().take(10) {
        let _ = shown.push(c);
    }
    let _ = write!(s, "{:10} {:.1}", shown.as_str(), e.count);
    let _ = lines.push(s);
}

impl Scene for DebugLocationScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.scroll = 0;
        self.rebuild(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::B) {
            return Some(ctx.last_main_scene);
        }
        if buttons.was_just_pressed(Button::A) {
            ctx.location_scan_requested = true;
            // Refresh the displayed lines next frame so the new scan
            // result lands as soon as the game loop has run it.
            self.scroll = 0;
            self.rebuild(ctx);
            return None;
        }
        let max_scroll = self.lines.len().saturating_sub(LINES_VISIBLE);
        if buttons.was_just_pressed(Button::Up) && self.scroll > 0 {
            self.scroll -= 1;
        }
        if buttons.was_just_pressed(Button::Down) && self.scroll < max_scroll {
            self.scroll += 1;
        }
        // Live-rebuild so updates from a just-finished scan show up.
        self.rebuild(ctx);
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        let end = (self.scroll + LINES_VISIBLE).min(self.lines.len());
        for (i, line) in self.lines[self.scroll..end].iter().enumerate() {
            renderer.draw_text(line.as_str(), Point::new(0, i as i32 * LINE_HEIGHT));
        }
    }
}
