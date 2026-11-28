//! Paired phones, remembered across reboots so they reconnect without
//! pairing again. Kept in their own flash sector (the unused `phy_init`
//! partition on the device), never in the cat's save.
//!
//! Sector layout: magic `BND1`, version, count, then `count` fixed-size
//! entries and a CRC-16 over everything before it. Anything that doesn't
//! check out reads as "no bonds", which just means pairing again.

use heapless::Vec;

use crate::ring::{crc16, Flash};

pub const MAX_BONDS: usize = 4;

const MAGIC: [u8; 4] = *b"BND1";
const VERSION: u8 = 1;
const HEADER: usize = 6;
const ENTRY: usize = 1 + 6 + 1 + 16 + 16 + 1;
const LEN_MAX: usize = HEADER + MAX_BONDS * ENTRY + 2;

/// One bonded peer, as plain bytes so this module needs no BLE types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BondRecord {
    /// Address kind as the controller reports it (public / random).
    pub addr_kind: u8,
    pub addr: [u8; 6],
    /// The peer's identity resolving key, when it shared one.
    pub irk: Option<u128>,
    pub ltk: u128,
    /// True for a MITM-protected (authenticated) key.
    pub authenticated: bool,
}

impl BondRecord {
    fn encode(&self, out: &mut [u8]) {
        out[0] = self.addr_kind;
        out[1..7].copy_from_slice(&self.addr);
        out[7] = self.irk.is_some() as u8;
        out[8..24].copy_from_slice(&self.irk.unwrap_or(0).to_le_bytes());
        out[24..40].copy_from_slice(&self.ltk.to_le_bytes());
        out[40] = self.authenticated as u8;
    }

    fn decode(b: &[u8]) -> Self {
        let u128_at = |i: usize| {
            let mut x = [0u8; 16];
            x.copy_from_slice(&b[i..i + 16]);
            u128::from_le_bytes(x)
        };
        let mut addr = [0u8; 6];
        addr.copy_from_slice(&b[1..7]);
        Self {
            addr_kind: b[0],
            addr,
            irk: (b[7] != 0).then(|| u128_at(8)),
            ltk: u128_at(24),
            authenticated: b[40] != 0,
        }
    }
}

/// Put `bond` at the front (most recent), replacing any earlier bond with
/// the same peer and dropping the oldest when full.
pub fn remember(list: &mut Vec<BondRecord, MAX_BONDS>, bond: BondRecord) {
    list.retain(|b| !(b.addr == bond.addr && b.addr_kind == bond.addr_kind));
    if list.is_full() {
        list.pop();
    }
    let _ = list.insert(0, bond);
}

pub fn encode(bonds: &[BondRecord], out: &mut [u8; LEN_MAX]) -> usize {
    let n = bonds.len().min(MAX_BONDS);
    out[..4].copy_from_slice(&MAGIC);
    out[4] = VERSION;
    out[5] = n as u8;
    for (i, b) in bonds.iter().take(n).enumerate() {
        b.encode(&mut out[HEADER + i * ENTRY..HEADER + (i + 1) * ENTRY]);
    }
    let body = HEADER + n * ENTRY;
    let crc = crc16(&out[..body]);
    out[body..body + 2].copy_from_slice(&crc.to_le_bytes());
    body + 2
}

pub fn decode(bytes: &[u8]) -> Vec<BondRecord, MAX_BONDS> {
    let mut out = Vec::new();
    if bytes.len() < HEADER + 2 || bytes[..4] != MAGIC || bytes[4] != VERSION {
        return out;
    }
    let n = bytes[5] as usize;
    let body = HEADER + n * ENTRY;
    if n > MAX_BONDS || bytes.len() < body + 2 {
        return out;
    }
    let crc = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
    if crc != crc16(&bytes[..body]) {
        return out;
    }
    for i in 0..n {
        let _ = out.push(BondRecord::decode(&bytes[HEADER + i * ENTRY..]));
    }
    out
}

/// Read the bonds from the start of `flash`. Empty if none or unreadable.
pub fn load<F: Flash>(flash: &mut F) -> Vec<BondRecord, MAX_BONDS> {
    let mut buf = [0u8; LEN_MAX];
    if !flash.read(0, &mut buf) {
        return Vec::new();
    }
    decode(&buf)
}

/// Replace the stored bonds with `bonds` (an empty slice forgets them all).
pub fn store<F: Flash>(flash: &mut F, bonds: &[BondRecord]) -> bool {
    if !flash.erase(0) {
        return false;
    }
    if bonds.is_empty() {
        return true;
    }
    let mut buf = [0xFFu8; LEN_MAX];
    let len = encode(bonds, &mut buf);
    // NOR writes in whole words; pad with erased bytes.
    let padded = len.div_ceil(4) * 4;
    flash.write(0, &buf[..padded])
}
