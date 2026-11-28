//! WiFi Scanner tool. Lists nearby networks by signal strength. Up/Down
//! scroll, B rescans. A shows a network's details, kept live: its channel
//! is re-scanned every couple of seconds and the signal graphed, which
//! also makes it handy for finding where an access point is.
//!
//! Scans with a [`ChannelSweep`], one channel per frame, drawing a bar per
//! channel as it goes. The sweep leaves the familiar-network tracking
//! alone, so scanning here can't change where the cat thinks it lives.

use core::fmt::Write;
use embedded_graphics::prelude::{Point, Size};
use heapless::{Deque, String, Vec};

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    platform::time::now_ms,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{list_nav::ListNav, scrollbar::Scrollbar, wrap},
    wifi_tracker::{format_bssid, ChannelSweep, ChannelWatch, ScanAp, ScanError, SWEEP_CHANNELS},
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

// Detail view.
/// Time between scans of the selected network's channel.
const TRACK_INTERVAL_MS: u64 = 2000;
/// Readings kept for the graph: 2 px each across the screen, so about two
/// minutes at one per scan.
pub const TRACK_HISTORY: usize = 64;
const GRAPH_TOP: i32 = 41;
const GRAPH_BASE: i32 = 61;
/// dBm at the bottom and top of the graph.
const GRAPH_MIN_DBM: i32 = -90;
const GRAPH_MAX_DBM: i32 = -40;

/// One network's recent readings, `None` where a scan missed it.
pub struct SignalTrack {
    history: Deque<Option<i8>, TRACK_HISTORY>,
}

impl Default for SignalTrack {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalTrack {
    pub const fn new() -> Self {
        Self { history: Deque::new() }
    }

    pub fn reset(&mut self) {
        self.history.clear();
    }

    /// Add a scan's reading, `None` if the network wasn't heard.
    pub fn push(&mut self, reading: Option<i8>) {
        if self.history.is_full() {
            self.history.pop_front();
        }
        let _ = self.history.push_back(reading);
    }

    pub fn latest(&self) -> Option<i8> {
        self.history.back().copied().flatten()
    }

    /// Oldest first.
    pub fn history(&self) -> impl Iterator<Item = Option<i8>> + '_ {
        self.history.iter().copied()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Scanning,
    List,
    /// The selected network's details, re-scanned live.
    Detail,
    Error(ScanError),
}

pub struct WifiScanScene {
    state: State,
    aps: Vec<ScanAp, MAX_APS>,
    nav: ListNav,
    sweep: ChannelSweep,
    watch: ChannelWatch,
    track: SignalTrack,
    next_scan_ms: u64,
    now: u64,
}

impl WifiScanScene {
    pub fn new() -> Self {
        Self {
            state: State::Scanning,
            aps: Vec::new(),
            nav: ListNav::new(),
            sweep: ChannelSweep::new(),
            watch: ChannelWatch::new(),
            track: SignalTrack::new(),
            next_scan_ms: 0,
            now: 0,
        }
    }

    /// Start tracking the selected network: hold the radio on its channel.
    fn start_tracking(&mut self, ctx: &mut GameContext) {
        let Some(ap) = self.aps.get(self.nav.selected) else { return };
        let (channel, rssi) = (ap.channel, ap.rssi);
        match self.watch.begin(ctx, channel) {
            Ok(()) => {
                self.track.reset();
                // The list's reading is the first point.
                self.track.push(Some(rssi));
                self.next_scan_ms = now_ms() + TRACK_INTERVAL_MS;
                ctx.keep_awake = true;
                self.state = State::Detail;
            }
            Err(e) => self.state = State::Error(e),
        }
    }

    fn stop_tracking(&mut self, ctx: &mut GameContext) {
        self.watch.end(ctx);
        ctx.keep_awake = false;
    }

    /// Re-scan the tracked network's channel when it's due.
    fn track_step(&mut self, ctx: &mut GameContext) {
        if self.now < self.next_scan_ms {
            return;
        }
        let Some(ap) = self.aps.get_mut(self.nav.selected) else { return };
        // A failed scan counts as not hearing it; the next one may work.
        let reading = self
            .watch
            .scan(ctx)
            .ok()
            .and_then(|found| found.iter().find(|f| f.bssid == ap.bssid).map(|f| f.rssi));
        if let Some(rssi) = reading {
            ap.rssi = rssi;
        }
        self.track.push(reading);
        // Timed from the end of the scan, which blocks for its dwell.
        self.now = now_ms();
        self.next_scan_ms = self.now + TRACK_INTERVAL_MS;
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
        let name = ssid_or_hidden(ap);
        let end = name.char_indices().nth(21).map_or(name.len(), |(i, _)| i);
        r.draw_text(&name[..end], Point::new(0, 0));

        // BSSID (several access points can share a name) and channel.
        r.draw_text_small(&format_bssid(&ap.bssid), Point::new(0, 11));
        let mut ch: String<24> = String::new();
        let _ = write!(ch, "{} {}", t!("Channel"), ap.channel);
        r.draw_text_small(&ch, Point::new(128 - ch.chars().count() as i32 * wrap::SMALL_CHAR_W, 11));

        // The latest reading, big, or dashes if the last scan missed it.
        let mut big: String<8> = String::new();
        match self.track.latest() {
            Some(rssi) => {
                let _ = write!(big, "{}", rssi);
            }
            None => {
                let _ = big.push_str("--");
            }
        }
        r.draw_text_scaled(&big, Point::new(0, 19), 2, true);
        let big_w = big.len() as i32 * 12;
        r.draw_text_small("dBm", Point::new(big_w + 2, 32));

        let sec = if ap.secured { t!("Secured") } else { t!("Open") };
        r.draw_text(sec, Point::new(128 - sec.chars().count() as i32 * 6, 24));

        self.draw_graph(r);
    }

    /// Readings as bars, newest at the right; a miss leaves a gap.
    fn draw_graph(&self, r: &mut Renderer) {
        let n = self.track.history().count() as i32;
        let x0 = 128 - n * 2;
        let span = GRAPH_BASE - GRAPH_TOP;
        for (i, reading) in self.track.history().enumerate() {
            let Some(rssi) = reading else { continue };
            let level = (rssi as i32).clamp(GRAPH_MIN_DBM, GRAPH_MAX_DBM) - GRAPH_MIN_DBM;
            let h = (level * span / (GRAPH_MAX_DBM - GRAPH_MIN_DBM)).max(1);
            let x = x0 + i as i32 * 2;
            r.draw_line(Point::new(x, GRAPH_BASE - h + 1), Point::new(x, GRAPH_BASE));
        }
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
        // Don't leave the radio held if we're leaving mid-sweep or
        // mid-track.
        self.sweep.abort(ctx);
        self.stop_tracking(ctx);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, _dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.now = now_ms();
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
                    self.start_tracking(ctx);
                }
                if buttons.was_just_pressed(Button::B) {
                    self.state = State::Scanning;
                }
            }
            State::Detail => {
                if buttons.was_just_pressed(Button::B) || buttons.was_just_pressed(Button::A) {
                    self.stop_tracking(ctx);
                    self.state = State::List;
                } else {
                    self.track_step(ctx);
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
