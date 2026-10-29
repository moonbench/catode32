//! Wire-level constants and pure helpers for H2 BLE messaging.
//!
//! Design: every H2 advertises a small connectable catode32 advert while
//! idle. The advert carries the device's short id, a sequence number and
//! the latest broadcast frame (hello, vocalize), so broadcasts need no
//! connection. Directed messages (visit request/reply, state) go over a
//! two-characteristic GATT link: the inviter is the central, the invitee
//! the peripheral. Payloads are the same bytes ESP-NOW carries, so the
//! game protocol is unchanged.
//!
//! Each device uses a static-random address derived from its short id, so
//! a connected peer's id can be read straight from its address.

use crate::ble_filter::CATODE_MAGIC;
use crate::transport::PeerId;

/// Advert/protocol version.
pub const PROTO_VER: u8 = 2;

/// Largest advert (legacy advertising PDU).
pub const ADVERT_MAX: usize = 31;

/// Largest broadcast frame that fits in an advert next to the header.
pub const ADV_PAYLOAD_MAX: usize = 20;

/// Largest frame carried in one GATT write/notify. Every game message
/// fits (the biggest is 17 bytes), and the default ATT MTU allows 20.
pub const GATT_PAYLOAD_MAX: usize = 20;

/// 128-bit service UUID, little-endian as sent on air.
pub const SERVICE_UUID: [u8; 16] = [
    0x32, 0xC3, 0x5E, 0x1A, 0x7D, 0x4B, 0x4C, 0x91, 0xA2, 0x0F, 0x6B, 0x83, 0x00, 0x1C, 0x32, 0xC3,
];
/// Central -> peripheral: write-without-response.
pub const RX_CHAR_UUID: [u8; 16] = {
    let mut u = SERVICE_UUID;
    u[12] = 0x01;
    u
};
/// Peripheral -> central: notify.
pub const TX_CHAR_UUID: [u8; 16] = {
    let mut u = SERVICE_UUID;
    u[12] = 0x02;
    u
};

/// Flags (LE general discoverable, no BR/EDR) + manufacturer data
/// `[C3 32][ver][id 2B][seq][payload]`. Returns the byte count written, or
/// `None` if the payload is too long.
pub fn encode_advert(id: PeerId, seq: u8, payload: &[u8], out: &mut [u8; ADVERT_MAX]) -> Option<usize> {
    if payload.len() > ADV_PAYLOAD_MAX {
        return None;
    }
    let b = id.as_bytes();
    out[..3].copy_from_slice(&[2, 0x01, 0x06]);
    out[3] = (7 + payload.len()) as u8;
    out[4] = 0xFF;
    out[5] = CATODE_MAGIC[0];
    out[6] = CATODE_MAGIC[1];
    out[7] = PROTO_VER;
    out[8] = b[0];
    out[9] = b[1];
    out[10] = seq;
    out[11..11 + payload.len()].copy_from_slice(payload);
    Some(11 + payload.len())
}

/// Extract `(sender id, sequence, payload)` from a catode32 advert.
pub fn parse_advert(data: &[u8]) -> Option<(PeerId, u8, &[u8])> {
    let mut i = 0;
    while i + 1 < data.len() {
        let len = data[i] as usize;
        if len == 0 || i + 1 + len > data.len() {
            return None;
        }
        let body = &data[i + 1..i + 1 + len];
        if body.len() >= 7 && body[0] == 0xFF && body[1..3] == CATODE_MAGIC && body[3] == PROTO_VER {
            let id = PeerId::from_short(u16::from_be_bytes([body[4], body[5]]));
            return Some((id, body[6], &body[7..]));
        }
        i += 1 + len;
    }
    None
}

/// Short id of a peer, as the first two bytes of its [`PeerId`].
pub fn short_id(peer: PeerId) -> u16 {
    let b = peer.as_bytes();
    u16::from_be_bytes([b[0], b[1]])
}

/// Static-random address (little-endian, as the controller takes it) for a
/// short id. The top byte's two high bits are set as the spec requires.
pub fn address_for(id: u16) -> [u8; 6] {
    let b = id.to_be_bytes();
    [b[1], b[0], CATODE_MAGIC[1], CATODE_MAGIC[0], 0x5E, 0xC0]
}

/// Inverse of [`address_for`]; `None` if the address is not a catode32 one.
pub fn id_from_address(addr: &[u8]) -> Option<u16> {
    if addr.len() != 6 || addr[2..] != address_for(0)[2..] {
        return None;
    }
    Some(u16::from_be_bytes([addr[1], addr[0]]))
}

/// Fold a 6-byte hardware MAC into a short id.
pub fn id_from_mac(mac: [u8; 6]) -> u16 {
    u16::from_be_bytes([mac[0], mac[1]]) ^ u16::from_be_bytes([mac[2], mac[3]]) ^ u16::from_be_bytes([mac[4], mac[5]])
}
