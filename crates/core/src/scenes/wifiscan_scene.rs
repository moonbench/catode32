//! WiFi Scanner tool. Lists nearby networks by signal strength. Up/Down
//! scroll, A shows a network's details, B rescans.
//!
//! Scans with a [`ChannelSweep`], one channel per frame, drawing a bar per
//! channel as it goes. The sweep leaves the familiar-network tracking
//! alone, so scanning here can't change where the cat thinks it lives.

use core::fmt::Write;
use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{list_nav::ListNav, scrollbar::Scrollbar, wrap},
    wifi_tracker::{ChannelSweep, ScanAp, ScanError, SWEEP_CHANNELS},
};

const MAX_APS: usize = 32;
const ROW_H: i32 = 9;
const LIST_Y: i32 = 9;
const VISIBLE: usize = 6;

// Sweep view: one column per channel.
const COL_W: i32 = 9;
const COL_X0: i32 = (128 - SWEEP_CHANNELS as i32 * COL_W) / 2;
const COL_BASE: i32 = 52;
const BAR_STEP: i32 = 6;
const BAR_MAX: i32 = 36;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Scanning,
    List,
    Detail,
    Error(ScanError),
}

pub struct WifiScanScene {
    state: State,
    aps: Vec<ScanAp, MAX_APS>,
    nav: ListNav,
    sweep: ChannelSweep,
}

impl WifiScanScene {
    pub fn new() -> Self {
        Self {
            state: State::Scanning,
            aps: Vec::new(),
            nav: ListNav::new(),
            sweep: ChannelSweep::new(),
        }
    }

    /// One step of the sweep: start it, or scan the next channel.
    fn scan_step(&mut self, ctx: &mut GameContext) {
        let result = if self.sweep.is_running() {
            self.sweep.step(ctx)
        } else {
            self.sweep.begin(ctx).map(|_| false)
        };
        match result {
            Ok(true) => {
                self.aps.clear();
                for ap in self.sweep.results().iter().take(MAX_APS) {
                    let _ = self.aps.push(ap.clone());
                }
                self.aps.sort_unstable_by(|a, b| b.rssi.cmp(&a.rssi));
                self.nav.reset();
                self.state = State::List;
            }
            Ok(false) => {}
            Err(e) => self.state = State::Error(e),
        }
    }

    fn draw_sweep(&self, r: &mut Renderer) {
        r.draw_text(t!("Scanning..."), Point::new(0, 0));
        let mut found: String<24> = String::new();
        let _ = write!(found, "{}", self.sweep.results().len());
        r.draw_text(&found, Point::new(128 - found.len() as i32 * wrap::CHAR_W, 0));

        let done = self.sweep.channels_done() as usize;
        for (i, &count) in self.sweep.per_channel().iter().enumerate() {
            let x = COL_X0 + i as i32 * COL_W;
            if i < done {
                let h = (count as i32 * BAR_STEP).min(BAR_MAX).max(1);
                r.draw_rect(Point::new(x + 1, COL_BASE - h), Size::new(COL_W as u32 - 2, h as u32), true);
            } else if i == done {
                // The channel being listened to now.
                r.draw_rect(Point::new(x + 1, COL_BASE - BAR_MAX), Size::new(COL_W as u32 - 2, BAR_MAX as u32), false);
            }
            let mut label: String<2> = String::new();
            let _ = write!(label, "{}", i + 1);
            let lw = label.len() as i32 * wrap::SMALL_CHAR_W;
            r.draw_text_small(&label, Point::new(x + (COL_W - lw) / 2 + 1, COL_BASE + 3));
        }
        r.draw_line(Point::new(COL_X0, COL_BASE + 1), Point::new(COL_X0 + SWEEP_CHANNELS as i32 * COL_W - 1, COL_BASE + 1));
    }

    fn draw_list(&self, r: &mut Renderer) {
        let mut header: String<32> = String::new();
        let _ = write!(header, "{} {}", self.aps.len(), t!("networks"));
        r.draw_text_small(&header, Point::new(1, 1));
        let hint = t!("B: rescan");
        let w = hint.chars().count() as i32 * wrap::SMALL_CHAR_W;
        r.draw_text_small(hint, Point::new(127 - w, 1));
        r.draw_line(Point::new(0, LIST_Y - 2), Point::new(127, LIST_Y - 2));

        if self.aps.is_empty() {
            let lines: Vec<&str, 3> = wrap::wrap(t!("No networks found"), 21);
            wrap::draw_centered(r, &lines, 64, 26, 10);
            return;
        }

        for (row, i) in self.nav.visible_range(self.aps.len(), VISIBLE).enumerate() {
            let ap = &self.aps[i];
            let y = LIST_Y + row as i32 * ROW_H;
            let selected = i == self.nav.selected;
            if selected {
                r.draw_rect(Point::new(0, y - 1), Size::new(124, ROW_H as u32), true);
            }
            draw_bars(r, 1, y + 6, bars_for(ap.rssi), !selected);

            let mut rssi: String<6> = String::new();
            let _ = write!(rssi, "{}", ap.rssi);
            let rw = rssi.len() as i32 * wrap::SMALL_CHAR_W;
            // SSID truncated to leave room for the dBm figure.
            let max_chars = ((122 - rw - 12) / wrap::SMALL_CHAR_W) as usize;
            let name = ssid_or_hidden(ap);
            let end = name.char_indices().nth(max_chars).map_or(name.len(), |(i, _)| i);
            if selected {
                r.draw_text_small_inverted(&name[..end], Point::new(12, y));
                r.draw_text_small_inverted(&rssi, Point::new(122 - rw, y));
            } else {
                r.draw_text_small(&name[..end], Point::new(12, y));
                r.draw_text_small(&rssi, Point::new(122 - rw, y));
            }
        }
        Scrollbar::right_edge(LIST_Y - 1, (VISIBLE as i32 * ROW_H) as u32).draw(
            r,
            self.aps.len(),
            VISIBLE,
            self.nav.scroll,
        );
    }

    fn draw_detail(&self, r: &mut Renderer) {
        let Some(ap) = self.aps.get(self.nav.selected) else { return };
        let lines: Vec<&str, 2> = wrap::wrap(ssid_or_hidden(ap), 21);
        for (i, line) in lines.iter().enumerate() {
            r.draw_text(line, Point::new(0, i as i32 * 10));
        }
        let y0 = 22;

        let mut mac: String<20> = String::new();
        for (i, b) in ap.bssid.iter().enumerate() {
            let _ = write!(mac, "{}{:02X}", if i == 0 { "" } else { ":" }, b);
        }
        r.draw_text_small(&mac, Point::new(0, y0));

        let mut sig: String<32> = String::new();
        let _ = write!(sig, "{} {} dBm", t!("Signal"), ap.rssi);
        r.draw_text_small(&sig, Point::new(0, y0 + 8));
        draw_bars(r, 100, y0 + 14, bars_for(ap.rssi), true);

        let mut ch: String<32> = String::new();
        let _ = write!(ch, "{} {}", t!("Channel"), ap.channel);
        r.draw_text_small(&ch, Point::new(0, y0 + 16));

        let sec = if ap.secured { t!("Secured") } else { t!("Open") };
        r.draw_text_small(sec, Point::new(0, y0 + 24));
    }
}

fn ssid_or_hidden(ap: &ScanAp) -> &str {
    if ap.ssid.is_empty() {
        t!("(hidden)")
    } else {
        ap.ssid.as_str()
    }
}

fn bars_for(rssi: i8) -> u8 {
    match rssi {
        -55..=0 => 4,
        -67..=-56 => 3,
        -78..=-68 => 2,
        _ => 1,
    }
}

/// Four rising bars, bottom edge at `base_y`. The first `n` are full
/// height; the rest are 1px stubs so the scale still reads.
fn draw_bars(r: &mut Renderer, x: i32, base_y: i32, n: u8, on: bool) {
    for i in 0..4 {
        let h = if (i as u8) < n { 2 + i * 2 } else { 1 };
        let bx = x + i * 2;
        r.draw_line_color(Point::new(bx, base_y - h + 1), Point::new(bx, base_y), on);
    }
}

impl Scene for WifiScanScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.state = State::Scanning;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        // Don't leave the radio held if we're leaving mid-sweep.
        self.sweep.abort(ctx);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, _dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        match self.state {
            State::Scanning => self.scan_step(ctx),
            State::List => {
                if buttons.was_just_pressed(Button::Up) {
                    self.nav.up(VISIBLE);
                }
                if buttons.was_just_pressed(Button::Down) {
                    self.nav.down(self.aps.len(), VISIBLE);
                }
                if buttons.was_just_pressed(Button::A) && !self.aps.is_empty() {
                    self.state = State::Detail;
                }
                if buttons.was_just_pressed(Button::B) {
                    self.state = State::Scanning;
                }
            }
            State::Detail => {
                if buttons.was_just_pressed(Button::B) || buttons.was_just_pressed(Button::A) {
                    self.state = State::List;
                }
            }
            State::Error(_) => {
                if buttons.was_just_pressed(Button::A) || buttons.was_just_pressed(Button::B) {
                    self.state = State::Scanning;
                }
            }
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        match self.state {
            State::Scanning => self.draw_sweep(r),
            State::List => self.draw_list(r),
            State::Detail => self.draw_detail(r),
            State::Error(e) => {
                let text = match e {
                    ScanError::NoRadio => t!("WiFi radio unavailable. A to retry."),
                    ScanError::Failed => t!("WiFi scan failed. A to retry."),
                };
                let lines: Vec<&str, 4> = wrap::wrap(text, 21);
                let top = 32 - lines.len() as i32 * 5;
                wrap::draw_centered(r, &lines, 64, top, 10);
            }
        }
    }
}
