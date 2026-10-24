//! Radio-agnostic peer identity and messaging interface.
//!
//! C3/C6 implement [`Transport`] over ESP-NOW (peers are WiFi MACs). The H2
//! will implement it over BLE (peers are short ids). Scenes only see
//! [`PeerId`] and the trait methods, never a MAC.

use heapless::Vec;

/// Max bytes buffered for a single inbound or outbound message.
pub const MAX_PAYLOAD: usize = 250;

/// Inbound messages held between polls before the oldest is dropped.
pub const INBOX_CAPACITY: usize = 8;

/// Identity of a remote device. ESP-NOW: the 6-byte MAC. BLE: a short id
/// stored in the first two bytes (rest zero) so one type serves both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PeerId([u8; 6]);

impl PeerId {
    /// Destination meaning "everyone in range".
    pub const BROADCAST: PeerId = PeerId([0xFF; 6]);

    pub const fn from_mac(mac: [u8; 6]) -> Self {
        Self(mac)
    }

    pub const fn from_short(id: u16) -> Self {
        let b = id.to_be_bytes();
        Self([b[0], b[1], 0, 0, 0, 0])
    }

    pub const fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

/// One received frame. The payload's first 4 bytes are the message tag
/// (e.g. `b"vst "`); the rest is the message-specific binary body.
pub struct InboxItem {
    pub src: PeerId,
    pub data: Vec<u8, MAX_PAYLOAD>,
}

/// Messaging backend. `add_peer` is only meaningful on ESP-NOW; other
/// transports may treat it as a no-op.
pub trait Transport {
    fn own_id(&self) -> Option<PeerId>;
    fn is_active(&self) -> bool;
    fn activate(&mut self);
    fn deactivate(&mut self);
    fn add_peer(&mut self, peer: PeerId);
    fn send_broadcast(&mut self, data: &[u8]);
    fn send_to(&mut self, peer: PeerId, data: &[u8]);
    fn poll(&mut self);
    fn drain(&mut self) -> Vec<InboxItem, INBOX_CAPACITY>;

    /// Numeric-comparison code awaiting the player's yes/no, if any (BLE only).
    fn pairing_code(&self) -> Option<u32> {
        None
    }
    /// Answer the pending [`Transport::pairing_code`] prompt.
    fn answer_pairing(&mut self, _accept: bool) {}
}
