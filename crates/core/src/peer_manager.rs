//! Peer manager: the game-facing transport handle.
//!
//! `PeerManager` is ESP-NOW on C3/C6 and the BLE manager on H2. Desktop
//! builds get a no-op stub.
//!
//! Wraps [`esp_radio::esp_now::EspNow`] with a small in-process inbox
//! and a fixed-size set of registered unicast peers. The manager lives
//! on [`GameContext`] for the program's lifetime, but its inner
//! `EspNow` handle is only bound while the radio is acquired by
//! [`crate::radio`]. Operations are silent no-ops otherwise.
//!
//! Desktop builds keep the same `PeerManager` shape but every method
//! is a no-op. There are no peers and the inbox is always empty, so
//! scenes that use the radio degrade gracefully into "offline" mode.

#![allow(dead_code)]

#[cfg(not(feature = "h2"))]
use heapless::Vec;

use crate::{
    context::GameContext,
    peer_msg::{encode_empty, TAG_VBYE},
    radio,
};

pub use crate::transport::{InboxItem, PeerId, Transport, INBOX_CAPACITY, MAX_PAYLOAD};

/// ESP-NOW addresses peers by raw MAC.
pub type MacAddr = [u8; 6];

/// How many distinct unicast peers we will register at once. The
/// broadcast peer is always registered separately and does not count
/// against this.
pub const MAX_UNICAST_PEERS: usize = 4;

#[cfg(not(any(feature = "desktop", feature = "h2")))]
mod firmware {
    use super::*;
    use esp_radio::esp_now::{
        EspNow, EspNowError, EspNowWifiInterface, PeerInfo, ReceivedData, BROADCAST_ADDRESS,
    };

    /// Queue a frame without waiting for the driver's send-done callback.
    ///
    /// `EspNow::send` returns a `SendWaiter` whose `Drop` busy-waits, with no
    /// timeout, for that callback. If the callback is ever lost the game loop
    /// spins forever. We never read the send status, so forget the waiter.
    /// The driver queues frames itself; the next send just re-arms the flag.
    fn send_detached(inner: &mut EspNow, dst: &MacAddr, data: &[u8]) {
        if let Ok(waiter) = inner.send(dst, data) {
            core::mem::forget(waiter);
        }
    }

    pub struct PeerManager {
        inner: Option<EspNow>,
        own_mac: Option<MacAddr>,
        inbox: Vec<InboxItem, INBOX_CAPACITY>,
        peers: Vec<MacAddr, MAX_UNICAST_PEERS>,
        active: bool,
    }

    impl PeerManager {
        pub fn new() -> Self {
            Self {
                inner: None,
                own_mac: None,
                inbox: Vec::new(),
                peers: Vec::new(),
                active: false,
            }
        }

        pub fn attach(&mut self, inner: EspNow, own_mac: MacAddr) {
            self.inner = Some(inner);
            if self.own_mac.is_none() {
                self.own_mac = Some(own_mac);
            }
        }

        pub fn detach(&mut self) -> Option<EspNow> {
            self.active = false;
            self.inbox.clear();
            self.peers.clear();
            self.inner.take()
        }

    }

    impl Transport for PeerManager {
        fn own_id(&self) -> Option<PeerId> {
            self.own_mac.map(PeerId::from_mac)
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn activate(&mut self) {
            if self.active {
                return;
            }
            let Some(inner) = self.inner.as_mut() else {
                println!("[EspNow] activate without radio");
                return;
            };
            let peer = PeerInfo {
                interface: EspNowWifiInterface::Station,
                peer_address: BROADCAST_ADDRESS,
                lmk: None,
                channel: None,
                encrypt: false,
            };
            if let Err(e) = inner.add_peer(peer) {
                if !matches!(e, EspNowError::Error(_)) {
                    println!("[EspNow] add broadcast peer failed: {:?}", e);
                }
            }
            self.active = true;
        }

        fn deactivate(&mut self) {
            if !self.active {
                return;
            }
            if let Some(inner) = self.inner.as_mut() {
                for mac in self.peers.iter() {
                    let _ = inner.remove_peer(mac);
                }
                let _ = inner.remove_peer(&BROADCAST_ADDRESS);
            }
            self.peers.clear();
            self.inbox.clear();
            self.active = false;
        }

        fn add_peer(&mut self, peer: PeerId) {
            let mac = *peer.as_bytes();
            if !self.active {
                return;
            }
            if self.peers.iter().any(|m| *m == mac) {
                return;
            }
            if self.peers.is_full() {
                return;
            }
            let Some(inner) = self.inner.as_mut() else {
                return;
            };
            let info = PeerInfo {
                interface: EspNowWifiInterface::Station,
                peer_address: mac,
                lmk: None,
                channel: None,
                encrypt: false,
            };
            match inner.add_peer(info) {
                Ok(_) => {
                    let _ = self.peers.push(mac);
                }
                Err(e) => println!("[EspNow] add_peer({:?}) failed: {:?}", mac, e),
            }
        }

        fn send_broadcast(&mut self, data: &[u8]) {
            if !self.active || data.len() > MAX_PAYLOAD {
                return;
            }
            let Some(inner) = self.inner.as_mut() else {
                return;
            };
            send_detached(inner, &BROADCAST_ADDRESS, data);
        }

        fn send_to(&mut self, peer: PeerId, data: &[u8]) {
            let mac = *peer.as_bytes();
            if !self.active || data.len() > MAX_PAYLOAD {
                return;
            }
            if !self.peers.iter().any(|m| *m == mac) {
                return;
            }
            let Some(inner) = self.inner.as_mut() else {
                return;
            };
            send_detached(inner, &mac, data);
        }

        fn poll(&mut self) {
            if !self.active {
                return;
            }
            let own = self.own_mac;
            let Some(inner) = self.inner.as_mut() else {
                return;
            };
            loop {
                let recv: Option<ReceivedData> = inner.receive();
                let Some(frame) = recv else { break };
                let src = frame.info.src_address;
                if Some(src) == own {
                    continue;
                }
                let mut buf: Vec<u8, MAX_PAYLOAD> = Vec::new();
                for b in frame.data() {
                    if buf.push(*b).is_err() {
                        break;
                    }
                }
                if self.inbox.is_full() {
                    let _ = self.inbox.remove(0);
                }
                let _ = self.inbox.push(InboxItem {
                    src: PeerId::from_mac(src),
                    data: buf,
                });
            }
        }

        fn drain(&mut self) -> Vec<InboxItem, INBOX_CAPACITY> {
            core::mem::take(&mut self.inbox)
        }
    }
}

#[cfg(not(any(feature = "desktop", feature = "h2")))]
pub use firmware::PeerManager;

#[cfg(all(feature = "desktop", not(feature = "h2")))]
mod desktop {
    use super::*;

    /// No-op manager. There is no radio on desktop, so every call
    /// silently does nothing and `drain` always returns an empty Vec.
    pub struct PeerManager;

    impl PeerManager {
        pub fn new() -> Self {
            Self
        }
    }

    impl Transport for PeerManager {
        fn own_id(&self) -> Option<PeerId> {
            None
        }
        fn is_active(&self) -> bool {
            false
        }
        fn activate(&mut self) {}
        fn deactivate(&mut self) {}
        fn add_peer(&mut self, _peer: PeerId) {}
        fn send_broadcast(&mut self, _data: &[u8]) {}
        fn send_to(&mut self, _peer: PeerId, _data: &[u8]) {}
        fn poll(&mut self) {}
        fn drain(&mut self) -> Vec<InboxItem, INBOX_CAPACITY> {
            Vec::new()
        }
    }
}

#[cfg(all(feature = "desktop", not(feature = "h2")))]
pub use desktop::PeerManager;

/// The H2 has no WiFi; its "ESP-NOW" transport is BLE.
#[cfg(feature = "h2")]
pub use crate::ble_transport::BleManager as PeerManager;

/// Acquire the radio for ESP-NOW traffic. Scenes call this on `enter`.
/// On desktop this always returns false (no radio).
pub fn start_session(ctx: &mut GameContext) -> bool {
    radio::acquire(ctx)
}

pub fn stop_session(ctx: &mut GameContext) {
    radio::release(ctx);
}

pub fn end_visit(ctx: &mut GameContext, notify_peer: bool) {
    let Some(visit) = ctx.visit.take() else {
        return;
    };
    if notify_peer {
        if let Some(peers) = ctx.peers.as_mut() {
            let frame = encode_empty(TAG_VBYE);
            peers.send_to(visit.peer_id, &frame);
        }
    }
    radio::release(ctx);
}

