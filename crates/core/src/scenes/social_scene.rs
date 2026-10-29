//! Social menu: peer discovery and playdate handshake.
//!
//! State machine:
//!
//! ```text
//!                          [B]
//!  +-------------------+ <-----+
//!  |     Browsing      |       |
//!  +---------+---------+       |
//!            | A on nearby     | vno / timeout
//!            v                 |
//!  +-------------------+-------+
//!  |     Inviting      |---vok-+---> visit (Some) + change_scene(Inside)
//!  +-------------------+
//!
//!  +-------------------+
//!  |  Browsing on vreq |---> Invited (popup)
//!  +-------------------+
//!            | A on prompt  -> send vok, visit (Some), change_scene(Inside)
//!            | B on prompt  -> send vno -> back to Browsing
//! ```
//!
//! Wire format: clean-break binary tags defined in [`crate::peer_msg`]
//! (`hi  `, `vreq`, `vok `, `vno `). Discovery broadcasts a hello every
//! 2 s; nearby peers age out after 6 s without a fresh hello; an
//! outgoing invite times out after 10 s.
//!
//! Radio refcount handoff: on a successful handshake the visit owns the
//! refcount, so the scene's `exit` does *not* release if `ctx.visit`
//! got set during the scene. The Phase-5 visit manager owns the final
//! release.

use core::fmt::Write as _;

use embedded_graphics::prelude::Point;
use heapless::{String, Vec};
use crate::t;
use crate::transport::{PeerId, Transport};

use crate::{
    context::{GameContext, VisitRole, VisitState, PET_NAME_MAX},
    peer_manager,
    peer_msg::{
        self, PetName, NAMED_FRAME_MAX, TAG_HELLO, TAG_VNO, TAG_VOK, TAG_VREQ,
    },
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::confirm::{Confirm, ConfirmResult},
};

const HELLO_INTERVAL: f32 = 2.0;
const NEARBY_TIMEOUT: f32 = 6.0;
const INVITE_TIMEOUT: f32 = 10.0;
/// How long the inviter waits for a reply. BLE links must pair first, which
/// can take a while, so the inviter waits longer than the invitee's prompt.
#[cfg(feature = "h2")]
const INVITER_WAIT: f32 = 35.0;
#[cfg(not(feature = "h2"))]
const INVITER_WAIT: f32 = INVITE_TIMEOUT;

const MAX_NEARBY: usize = 5;
const LINE_HEIGHT: i32 = 8;
const LINES_VISIBLE: usize = 5;

type MacAddr = PeerId;

#[derive(Clone)]
struct NearbyEntry {
    mac: MacAddr,
    name: PetName,
    /// Seconds since the last `hi  ` from this peer; entries with
    /// `since_hello > NEARBY_TIMEOUT` are pruned each frame.
    since_hello: f32,
}

enum State {
    Browsing,
    Inviting {
        peer_id: MacAddr,
        peer_name: PetName,
        elapsed: f32,
    },
    Invited {
        peer_id: MacAddr,
        peer_name: PetName,
        /// Seconds the confirm has been on screen. Mirrors the
        /// inviter's `INVITE_TIMEOUT` so the prompt vanishes on its
        /// own (and sends a `vno`) if the user does not answer in
        /// time.
        elapsed: f32,
    },
    /// Active playdate. The user navigated back into the Social menu
    /// to leave. Shows an "End visit?" confirm; A ends, B returns to
    /// the previous scene without disturbing the visit.
    Visiting,
}

pub struct SocialScene {
    state: State,
    nearby: Vec<NearbyEntry, MAX_NEARBY>,
    selected: usize,
    hello_timer: f32,
    /// Seconds left in which a recent pairing prompt suggests a peer is
    /// connecting, so the empty list reads "Waiting..." not "No cats nearby".
    connecting: f32,
    /// Seconds since the pairing prompt closed while still inviting.
    pair_idle: Option<f32>,
    /// Shared yes/no dialog used both for incoming `vreq` invites
    /// (Invited state) and the leave-visit prompt (Visiting state).
    confirm: Confirm,
}

impl SocialScene {
    pub fn new() -> Self {
        Self {
            state: State::Browsing,
            nearby: Vec::new(),
            selected: 0,
            hello_timer: 0.0,
            connecting: 0.0,
            pair_idle: None,
            confirm: Confirm::new(),
        }
    }

    fn broadcast_hello(&mut self, ctx: &mut GameContext) {
        let Some(peers) = ctx.peers.as_mut() else {
            return;
        };
        let mut buf = [0u8; NAMED_FRAME_MAX];
        let n = peer_msg::encode_named(TAG_HELLO, ctx.pet_name.as_str(), &mut buf);
        peers.send_broadcast(&buf[..n]);
    }

    /// Add or refresh a peer in the nearby list. Drops on overflow
    /// rather than evicting; a missing peer just won't appear in the
    /// list until an older entry ages out.
    fn note_hello(&mut self, mac: MacAddr, name: PetName) {
        if let Some(entry) = self.nearby.iter_mut().find(|e| e.mac == mac) {
            entry.name = name;
            entry.since_hello = 0.0;
            return;
        }
        let entry = NearbyEntry {
            mac,
            name,
            since_hello: 0.0,
        };
        let _ = self.nearby.push(entry);
    }

    /// Tick `since_hello` on every entry, drop the stale ones, and clamp
    /// `selected` so the cursor never points past the end.
    fn prune_nearby(&mut self, dt: f32) {
        for e in self.nearby.iter_mut() {
            e.since_hello += dt;
        }
        self.nearby.retain(|e| e.since_hello <= NEARBY_TIMEOUT);
        if !self.nearby.is_empty() && self.selected >= self.nearby.len() {
            self.selected = self.nearby.len() - 1;
        }
    }

    fn drain_and_dispatch(&mut self, ctx: &mut GameContext) {
        let inbox = match ctx.peers.as_mut() {
            Some(m) => m.drain(),
            None => Default::default(),
        };
        for item in inbox.iter() {
            let src = item.src;
            let data = &item.data[..];
            if let Some(name) = peer_msg::decode_named(TAG_HELLO, data) {
                self.note_hello(src, name);
                continue;
            }
            if let Some(name) = peer_msg::decode_named(TAG_VREQ, data) {
                self.handle_vreq(ctx, src, name);
                continue;
            }
            if let Some(name) = peer_msg::decode_named(TAG_VOK, data) {
                self.handle_vok(ctx, src, name);
                continue;
            }
            if peer_msg::matches_tag(TAG_VNO, data) {
                self.handle_vno(src);
                continue;
            }
            // Other tags belong to the visit runtime (Phase 5).
        }
    }

    fn handle_vreq(&mut self, ctx: &mut GameContext, src: MacAddr, name: PetName) {
        // Only honor invites while Browsing. If we're already
        // inviting someone or showing a prompt, the new request is
        // dropped. The other side will see an invite timeout.
        if !matches!(self.state, State::Browsing) {
            return;
        }
        if let Some(peers) = ctx.peers.as_mut() {
            peers.add_peer(src);
        }
        let mut prompt: String<32> = String::new();
        let _ = prompt.push_str(name.as_str());
        let _ = prompt.push_str(t!(" wants to play!"));
        self.confirm.open(prompt.as_str());
        self.state = State::Invited {
            peer_id: src,
            peer_name: name,
            elapsed: 0.0,
        };
    }

    fn handle_vok(&mut self, ctx: &mut GameContext, src: MacAddr, name: PetName) {
        let State::Inviting { peer_id, .. } = &self.state else {
            return;
        };
        if *peer_id != src {
            return;
        }
        self.start_visit(ctx, src, name, VisitRole::Inviter);
    }

    fn handle_vno(&mut self, src: MacAddr) {
        let State::Inviting { peer_id, .. } = &self.state else {
            return;
        };
        if *peer_id != src {
            return;
        }
        self.state = State::Browsing;
    }

    fn send_invite(&mut self, ctx: &mut GameContext, mac: MacAddr) {
        let Some(peers) = ctx.peers.as_mut() else {
            return;
        };
        peers.add_peer(mac);
        let mut buf = [0u8; NAMED_FRAME_MAX];
        let n = peer_msg::encode_named(TAG_VREQ, ctx.pet_name.as_str(), &mut buf);
        peers.send_to(mac, &buf[..n]);
    }

    fn send_vok(&mut self, ctx: &mut GameContext, mac: MacAddr) {
        let Some(peers) = ctx.peers.as_mut() else {
            return;
        };
        let mut buf = [0u8; NAMED_FRAME_MAX];
        let n = peer_msg::encode_named(TAG_VOK, ctx.pet_name.as_str(), &mut buf);
        peers.send_to(mac, &buf[..n]);
    }

    fn send_vno(&mut self, ctx: &mut GameContext, mac: MacAddr) {
        let Some(peers) = ctx.peers.as_mut() else {
            return;
        };
        let frame = peer_msg::encode_empty(TAG_VNO);
        peers.send_to(mac, &frame);
    }

    fn start_visit(
        &mut self,
        ctx: &mut GameContext,
        peer_id: MacAddr,
        peer_name: PetName,
        role: VisitRole,
    ) {
        ctx.visit = Some(VisitState {
            peer_id,
            peer_name,
            role,
            play_time: 0.0,
            greeted: false,
        });
        // Take a dedicated radio refcount on behalf of the visit so
        // it survives this scene's `exit` (which always releases the
        // refcount it took in `enter`). Phase 5b's `end_visit` drops
        // this one.
        peer_manager::start_session(ctx);
    }

    fn draw_browsing(&self, renderer: &mut Renderer) {
        renderer.draw_text(t!("Social"), Point::new(0, 0));

        if self.nearby.is_empty() {
            let msg = if self.connecting > 0.0 {
                t!("Waiting...")
            } else {
                t!("No cats nearby...")
            };
            renderer.draw_text(msg, Point::new(0, 18));
        } else {
            for (i, entry) in self.nearby.iter().take(LINES_VISIBLE).enumerate() {
                let mut row: String<24> = String::new();
                let arrow = if i == self.selected { '>' } else { ' ' };
                let _ = write!(&mut row, "{}{}", arrow, entry.name.as_str());
                renderer.draw_text(row.as_str(), Point::new(0, 14 + i as i32 * LINE_HEIGHT));
            }
        }

        renderer.draw_text(t!("A=invite  B=back"), Point::new(0, 56));
    }

    fn draw_inviting(&self, renderer: &mut Renderer, peer_name: &str, _elapsed: f32) {
        renderer.draw_text(t!("Inviting..."), Point::new(0, 0));
        renderer.draw_text(peer_name, Point::new(0, 16));
        renderer.draw_text(t!("Waiting..."), Point::new(0, 32));
        renderer.draw_text(t!("B=cancel"), Point::new(0, 56));
    }
}

impl Scene for SocialScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.nearby.clear();
        self.selected = 0;
        self.hello_timer = 0.0;
        self.connecting = 0.0;
        peer_manager::start_session(ctx);
        // If we arrived while a visit is already in progress (the
        // user re-opened the menu mid-playdate), jump straight to
        // Visiting and offer to leave.
        if ctx.visit.is_some() {
            self.state = State::Visiting;
            self.confirm.open("End visit?");
        } else {
            self.state = State::Browsing;
        }
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        // Always release the refcount this scene took in `enter`.
        // A handshake-success (or pre-existing visit kept open) just
        // means the *visit* still holds its own separate refcount, so
        // the radio stays up regardless.
        peer_manager::stop_session(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        // Periodic hello broadcast while browsing or inviting (so
        // others can still see us). Suppressed during an Invited
        // prompt since responding to that takes priority.
        if !matches!(self.state, State::Invited { .. }) {
            self.hello_timer += dt;
            if self.hello_timer >= HELLO_INTERVAL {
                self.hello_timer -= HELLO_INTERVAL;
                self.broadcast_hello(ctx);
            }
        }

        self.drain_and_dispatch(ctx);
        self.prune_nearby(dt);
        if ctx.peers.as_ref().is_some_and(|p| p.pairing_code().is_some()) {
            self.connecting = 4.0;
            self.pair_idle = Some(0.0);
        } else {
            if self.connecting > 0.0 {
                self.connecting -= dt;
            }
            if let Some(t) = &mut self.pair_idle {
                *t += dt;
            }
        }
        // Pairing ended without a visit starting (declined or failed): the
        // BLE link sends no "no" reply, so stop waiting on the invite.
        if matches!(self.state, State::Inviting { .. }) && self.pair_idle.is_some_and(|t| t >= 6.0) {
            self.pair_idle = None;
            self.state = State::Browsing;
        }
        if !matches!(self.state, State::Inviting { .. }) {
            self.pair_idle = None;
        }

        match &mut self.state {
            State::Browsing => {
                if buttons.was_just_pressed(Button::B) {
                    return Some(ctx.last_main_scene);
                }
                if !self.nearby.is_empty() {
                    if buttons.was_just_pressed(Button::Up) && self.selected > 0 {
                        self.selected -= 1;
                    }
                    if buttons.was_just_pressed(Button::Down)
                        && self.selected + 1 < self.nearby.len()
                    {
                        self.selected += 1;
                    }
                    if buttons.was_just_pressed(Button::A) {
                        let entry = self.nearby[self.selected].clone();
                        self.send_invite(ctx, entry.mac);
                        self.state = State::Inviting {
                            peer_id: entry.mac,
                            peer_name: entry.name,
                            elapsed: 0.0,
                        };
                    }
                }
            }
            State::Inviting { elapsed, .. } => {
                *elapsed += dt;
                let timed_out = *elapsed >= INVITER_WAIT;
                if timed_out || buttons.was_just_pressed(Button::B) {
                    self.state = State::Browsing;
                }
            }
            State::Visiting => {
                match self.confirm.handle_input(buttons) {
                    ConfirmResult::Confirmed => {
                        peer_manager::end_visit(ctx, true);
                        return Some(ctx.last_main_scene);
                    }
                    ConfirmResult::Cancelled => {
                        return Some(ctx.last_main_scene);
                    }
                    ConfirmResult::Pending => {}
                }
            }
            State::Invited {
                peer_id, elapsed, ..
            } => {
                *elapsed += dt;
                let mac = *peer_id;
                let timed_out = *elapsed >= INVITE_TIMEOUT;
                let input = self.confirm.handle_input(buttons);
                match (input, timed_out) {
                    (ConfirmResult::Confirmed, _) => {
                        self.send_vok(ctx, mac);
                        let name = if let State::Invited { peer_name, .. } = &self.state {
                            peer_name.clone()
                        } else {
                            PetName::new()
                        };
                        self.start_visit(ctx, mac, name, VisitRole::Invitee);
                    }
                    (ConfirmResult::Cancelled, _) | (ConfirmResult::Pending, true) => {
                        // Either the player declined or the prompt
                        // aged out alongside the inviter's own 10 s
                        // timer. Either way, drop back to Browsing
                        // and tell the inviter so they aren't left
                        // staring at "Inviting...".
                        self.send_vno(ctx, mac);
                        self.confirm.close();
                        self.state = State::Browsing;
                    }
                    (ConfirmResult::Pending, false) => {}
                }
            }
        }

        // A successful handshake (either side) sets `ctx.visit`
        // during dispatch or input handling above. Return the scene
        // swap here so it actually fires.
        if ctx.visit.is_some() && !matches!(self.state, State::Visiting) {
            return Some(SceneId::Inside);
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        match &self.state {
            State::Browsing => self.draw_browsing(renderer),
            State::Inviting {
                peer_name, elapsed, ..
            } => self.draw_inviting(renderer, peer_name.as_str(), *elapsed),
            State::Invited { .. } | State::Visiting => {
                self.confirm.draw(renderer);
            }
        }
    }
}

// Silence unused-import warnings until phase 5 reaches in here.
#[allow(dead_code)]
const _: usize = PET_NAME_MAX;
