//! Debug screen for the ESP-NOW transport. Exists to smoke-test the
//! radio at the byte level before the social / playdate code calls into
//! it. Shows the local MAC address, registered-peer count, send and
//! receive counters, and a scrollable log of the most recent packets.
//!
//! Activates the [`crate::peer_manager::PeerManager`] on `enter` and
//! parks it on `exit`, so opening the scene is the only thing on the
//! device that powers up the radio for ESP-NOW traffic until the social
//! scene is implemented.
//!
//! Controls:
//!   * A: broadcast a 4-byte `b"png "` tag followed by a u32 counter.
//!   * Up / Down: scroll the log.
//!   * B: back to the main scene.

use core::fmt::Write as _;
use crate::transport::Transport;

use embedded_graphics::prelude::Point;
use heapless::{String, Vec};

use crate::{
    context::GameContext,
    peer_manager,
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
    location_tracker::format_addr,
};

const LINES_VISIBLE: usize = 8;
const LINE_HEIGHT: i32 = 8;
const LOG_LINE_LEN: usize = 24;
/// Cap on log lines we retain. Older entries fall off the bottom of the
/// `heapless::Deque`-style ring buffer below.
const LOG_CAPACITY: usize = 32;

/// 4-byte tag the debug scene broadcasts on every A press.
const PING_TAG: [u8; 4] = *b"png ";

type Line = String<LOG_LINE_LEN>;

pub struct DebugPeerScene {
    /// Ring buffer of formatted log lines (most recent at the end).
    log: Vec<Line, LOG_CAPACITY>,
    /// 0 = pinned to the bottom (auto-scroll on new packets). Anything
    /// else freezes the view so the player can read older entries.
    scroll_from_bottom: usize,
    send_count: u32,
    recv_count: u32,
}

impl DebugPeerScene {
    pub fn new() -> Self {
        Self {
            log: Vec::new(),
            scroll_from_bottom: 0,
            send_count: 0,
            recv_count: 0,
        }
    }

    fn push_log<F: FnOnce(&mut Line)>(&mut self, f: F) {
        let mut s = Line::new();
        f(&mut s);
        if self.log.is_full() {
            // Drop the oldest entry to make room.
            let _ = self.log.remove(0);
        }
        let _ = self.log.push(s);
    }

    /// Drain the manager's inbox into our log. Run every frame so the
    /// scene reflects packets that arrived between ticks even when no
    /// input was pressed.
    fn ingest_inbox(&mut self, ctx: &mut GameContext) {
        let Some(peers) = ctx.peers.as_mut() else {
            return;
        };
        let drained = peers.drain();
        for item in drained.iter() {
            self.recv_count = self.recv_count.saturating_add(1);
            // Format: `<XX:YY tag N`. Last two MAC bytes (enough to
            // distinguish nearby devices), 4-byte ASCII tag if present,
            // and total payload length.
            let tag_bytes: [u8; 4] = if item.data.len() >= 4 {
                [item.data[0], item.data[1], item.data[2], item.data[3]]
            } else {
                [b'?', b'?', b'?', b'?']
            };
            self.push_log(|s| {
                let _ = write!(
                    s,
                    "<{:02x}:{:02x} {}{}{}{} {}",
                    item.src.as_bytes()[4],
                    item.src.as_bytes()[5],
                    printable(tag_bytes[0]),
                    printable(tag_bytes[1]),
                    printable(tag_bytes[2]),
                    printable(tag_bytes[3]),
                    item.data.len(),
                );
            });
        }
    }

    fn send_ping(&mut self, ctx: &mut GameContext) {
        let Some(peers) = ctx.peers.as_mut() else {
            self.push_log(|s| {
                let _ = s.push_str("(no radio)");
            });
            return;
        };
        self.send_count = self.send_count.saturating_add(1);
        let counter = self.send_count;
        // 4-byte tag + 4-byte big-endian counter. Big-endian is just a
        // convention. The receiver doesn't decode it yet (Phase 2 is
        // pure transport).
        let mut frame: [u8; 8] = [0; 8];
        frame[0..4].copy_from_slice(&PING_TAG);
        frame[4..8].copy_from_slice(&counter.to_be_bytes());
        peers.send_broadcast(&frame);
        self.push_log(|s| {
            let _ = write!(s, ">> png  #{}", counter);
        });
    }
}

/// Render a single byte as a printable ASCII char, falling back to `.`
/// for control/extended bytes so the log column stays a fixed width.
fn printable(b: u8) -> char {
    if (0x20..=0x7e).contains(&b) {
        b as char
    } else {
        '.'
    }
}

impl Scene for DebugPeerScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.log.clear();
        self.scroll_from_bottom = 0;
        self.send_count = 0;
        self.recv_count = 0;
        if peer_manager::start_session(ctx) {
            self.push_log(|s| {
                let _ = s.push_str("radio on");
            });
        } else {
            self.push_log(|s| {
                let _ = s.push_str("(no radio)");
            });
        }
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        peer_manager::stop_session(ctx);
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
            self.send_ping(ctx);
            self.scroll_from_bottom = 0;
        }
        self.ingest_inbox(ctx);

        let total = self.log.len();
        let max_scroll = total.saturating_sub(LINES_VISIBLE);
        if buttons.was_just_pressed(Button::Up) && self.scroll_from_bottom < max_scroll {
            self.scroll_from_bottom += 1;
        }
        if buttons.was_just_pressed(Button::Down) && self.scroll_from_bottom > 0 {
            self.scroll_from_bottom -= 1;
        }
        None
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        // Header rows: own MAC, counters. Each row is 8 px tall and the
        // log occupies the remaining 6 lines (LINES_VISIBLE - 2).
        let own_mac = ctx
            .peers
            .as_ref()
            .and_then(|m| m.own_id())
            .map(|m| format_addr(m.as_bytes()))
            .unwrap_or_else(|| {
                let mut s: String<17> = String::new();
                let _ = s.push_str("--");
                s
            });
        renderer.draw_text(own_mac.as_str(), Point::new(0, 0));

        let mut counters: String<24> = String::new();
        let _ = write!(&mut counters, "TX {} RX {}", self.send_count, self.recv_count);
        renderer.draw_text(counters.as_str(), Point::new(0, LINE_HEIGHT));

        // Log: show the most-recent slice anchored to the bottom, with
        // `scroll_from_bottom` letting the player walk back through the
        // ring buffer.
        let visible = LINES_VISIBLE - 2;
        let total = self.log.len();
        let end = total.saturating_sub(self.scroll_from_bottom);
        let start = end.saturating_sub(visible);
        for (row, line) in self.log[start..end].iter().enumerate() {
            renderer.draw_text(
                line.as_str(),
                Point::new(0, (2 + row as i32) * LINE_HEIGHT),
            );
        }
    }
}
