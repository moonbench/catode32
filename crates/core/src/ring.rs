//! Snapshot ring on a small flash partition.
//!
//! Every save is one complete snapshot, written into the next free 64-byte
//! slot after the newest snapshot and wrapping around the partition. A
//! sector is erased only when the ring reaches its first slot, so several
//! small saves share one erase. That is what gives the flash its lifespan.
//!
//! Snapshot layout, starting on a slot boundary:
//!   16-byte header:
//!       4 B magic       "SAV1"
//!       4 B sequence    (u32 LE, one higher than the newest record)
//!       2 B length      (u16 LE, payload bytes)
//!       1 B schema
//!       1 B flags       (reserved, 0)
//!       2 B reserved    (0xFFFF)
//!       2 B header CRC  (CRC-16 over the previous 14 bytes)
//!   payload, padded with 0xFF to a 4-byte multiple.
//!
//! Crash safety is the same as the v0 format: the payload is written first
//! and the header last, so a power loss mid-write leaves no header and the
//! previous snapshot stays newest. Slots already holding stray bytes from an
//! interrupted write are never written over; the writer skips ahead to the
//! next sector start instead.
//!
//! The ring also recognises v0 records (16-byte `"SAVE"` header at a sector
//! start, JSON payload spanning whole sectors) so they can be migrated. A v0
//! record is never erased while it is the newest record.
//!
//! Everything here is generic over [`Flash`] so tests can run it against a
//! RAM-backed fake with erase counters and injected power loss.

use heapless::Vec;

pub const SECTOR_SIZE: usize = 4096;
pub const SLOT_SIZE: usize = 64;
pub const SLOTS_PER_SECTOR: usize = SECTOR_SIZE / SLOT_SIZE;
pub const HEADER_LEN: usize = 16;
pub const WORD: usize = 4;

/// Largest snapshot payload. Keeps every snapshot inside one sector's worth
/// of slots, which guarantees the first v1 snapshot always fits in the one
/// sector a maximal (5-sector) v0 record leaves free.
pub const MAX_PAYLOAD: usize = SECTOR_SIZE - HEADER_LEN;

/// Largest v0 record payload (the old `storage::MAX_PAYLOAD`).
pub const V0_MAX_PAYLOAD: usize = SECTOR_SIZE * 5 - HEADER_LEN;

/// Sectors beyond this are ignored, so occupancy fits in a `u32` mask.
pub const MAX_SECTORS: usize = 32;

/// Newest snapshots kept from a scan, for section-level fallback.
pub const KEEP: usize = 8;

const MAGIC_V1: [u8; 4] = *b"SAV1";
const MAGIC_V0: [u8; 4] = *b"SAVE";
pub const SCHEMA: u8 = 1;

/// A flash partition, addressed from 0. Offsets and lengths passed to
/// `read` and `write` are always multiples of [`WORD`]. `write` may only
/// clear bits (NOR semantics); `erase` sets a whole sector to 0xFF.
pub trait Flash {
    fn sectors(&self) -> usize;
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> bool;
    fn write(&mut self, offset: usize, data: &[u8]) -> bool;
    fn erase(&mut self, sector: usize) -> bool;
}

/// A committed v1 snapshot found by [`scan`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snap {
    pub slot: usize,
    pub seq: u32,
    pub len: usize,
}

/// A v0 record found by [`scan`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Legacy {
    pub sector: usize,
    pub seq: u32,
    pub len: usize,
}

#[derive(Default)]
pub struct Scan {
    /// Valid v1 headers, newest first.
    pub snaps: Vec<Snap, KEEP>,
    /// The newest v0 record, if any.
    pub legacy: Option<Legacy>,
    /// Usable sectors in the partition.
    pub sectors: usize,
}

impl Scan {
    pub fn total_slots(&self) -> usize {
        self.sectors * SLOTS_PER_SECTOR
    }

    /// The newest record of either format, as `(seq, occupied-sector mask,
    /// slot just past its end)`.
    fn newest(&self) -> Option<(u32, u32, usize)> {
        let v1 = self.snaps.first().map(|s| {
            (s.seq, snap_mask(s, self.sectors), (s.slot + slots_for(s.len)) % self.total_slots())
        });
        let v0 = self.legacy.map(|l| {
            let n = v0_sectors(l.len);
            (l.seq, legacy_mask(&l, self.sectors), ((l.sector + n) % self.sectors) * SLOTS_PER_SECTOR)
        });
        match (v1, v0) {
            (Some(a), Some(b)) => Some(if b.0 > a.0 { b } else { a }),
            (a, b) => a.or(b),
        }
    }

    /// Sectors holding the newest record. The writer never erases these.
    pub fn newest_mask(&self) -> u32 {
        self.newest().map(|n| n.1).unwrap_or(0)
    }
}

/// Slots a snapshot with `len` payload bytes occupies, header included.
pub fn slots_for(len: usize) -> usize {
    (HEADER_LEN + len).div_ceil(SLOT_SIZE).max(1)
}

fn v0_sectors(len: usize) -> usize {
    (HEADER_LEN + len).div_ceil(SECTOR_SIZE).max(1)
}

fn snap_mask(s: &Snap, sectors: usize) -> u32 {
    let total = sectors * SLOTS_PER_SECTOR;
    let mut mask = 0u32;
    for i in 0..slots_for(s.len) {
        mask |= 1 << (((s.slot + i) % total) / SLOTS_PER_SECTOR);
    }
    mask
}

/// Sectors occupied by a v0 record (it may wrap past the partition end).
pub fn legacy_mask(l: &Legacy, sectors: usize) -> u32 {
    let mut mask = 0u32;
    for i in 0..v0_sectors(l.len).min(sectors) {
        mask |= 1 << ((l.sector + i) % sectors);
    }
    mask
}

/// Sectors occupied by a v1 snapshot.
pub fn snapshot_mask(s: &Snap, sectors: usize) -> u32 {
    snap_mask(s, sectors)
}

fn padded(len: usize) -> usize {
    len.div_ceil(WORD) * WORD
}

/// CRC-16/CCITT-FALSE. Small and table-free; saves are a few KB at most.
pub fn crc16(data: &[u8]) -> u16 {
    crc16_update(0xFFFF, data)
}

pub fn crc16_update(mut crc: u16, data: &[u8]) -> u16 {
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

fn encode_header(seq: u32, len: usize) -> [u8; HEADER_LEN] {
    let mut h = [0xFFu8; HEADER_LEN];
    h[..4].copy_from_slice(&MAGIC_V1);
    h[4..8].copy_from_slice(&seq.to_le_bytes());
    h[8..10].copy_from_slice(&(len as u16).to_le_bytes());
    h[10] = SCHEMA;
    h[11] = 0;
    let crc = crc16(&h[..14]);
    h[14..16].copy_from_slice(&crc.to_le_bytes());
    h
}

fn parse_v1_header(h: &[u8; HEADER_LEN], slot: usize) -> Option<Snap> {
    if h[..4] != MAGIC_V1 {
        return None;
    }
    if u16::from_le_bytes([h[14], h[15]]) != crc16(&h[..14]) {
        return None;
    }
    let len = u16::from_le_bytes([h[8], h[9]]) as usize;
    if len > MAX_PAYLOAD {
        return None;
    }
    Some(Snap {
        slot,
        seq: u32::from_le_bytes([h[4], h[5], h[6], h[7]]),
        len,
    })
}

fn usable_sectors<F: Flash + ?Sized>(flash: &F) -> usize {
    flash.sectors().min(MAX_SECTORS)
}

/// Read every slot header and collect the newest snapshots and v0 record.
pub fn scan<F: Flash + ?Sized>(flash: &mut F) -> Scan {
    let sectors = usable_sectors(flash);
    let mut out = Scan {
        sectors,
        ..Scan::default()
    };
    let mut h = [0u8; HEADER_LEN];
    for slot in 0..sectors * SLOTS_PER_SECTOR {
        if !flash.read(slot * SLOT_SIZE, &mut h) {
            continue;
        }
        if let Some(s) = parse_v1_header(&h, slot) {
            insert_newest(&mut out.snaps, s);
        } else if slot % SLOTS_PER_SECTOR == 0 && h[..4] == MAGIC_V0 {
            let len = u32::from_le_bytes([h[8], h[9], h[10], h[11]]) as usize;
            if len == 0 || len > V0_MAX_PAYLOAD || v0_sectors(len) > sectors {
                continue;
            }
            let l = Legacy {
                sector: slot / SLOTS_PER_SECTOR,
                seq: u32::from_le_bytes([h[4], h[5], h[6], h[7]]),
                len,
            };
            if out.legacy.map_or(true, |b| l.seq > b.seq) {
                out.legacy = Some(l);
            }
        }
    }
    out
}

fn insert_newest(list: &mut Vec<Snap, KEEP>, s: Snap) {
    let pos = list.iter().position(|x| s.seq > x.seq).unwrap_or(list.len());
    if pos >= KEEP {
        return;
    }
    if list.is_full() {
        list.pop();
    }
    let _ = list.insert(pos, s);
}

/// Copy bytes `[offset, offset + out.len())` of the ring's byte space into
/// `out`, wrapping past the partition end. Handles unaligned starts and
/// lengths with a small scratch buffer.
fn read_ring<F: Flash + ?Sized>(flash: &mut F, sectors: usize, offset: usize, out: &mut [u8]) -> bool {
    let total = sectors * SECTOR_SIZE;
    let mut scratch = [0u8; 256];
    let mut done = 0;
    while done < out.len() {
        let pos = (offset + done) % total;
        let aligned = pos - pos % WORD;
        let skip = pos - aligned;
        // Stay inside one sector so a chunk never crosses the wrap point.
        let sector_left = SECTOR_SIZE - aligned % SECTOR_SIZE;
        let want = (out.len() - done + skip).min(scratch.len()).min(sector_left);
        let chunk = padded(want).min(sector_left);
        if !flash.read(aligned, &mut scratch[..chunk]) {
            return false;
        }
        let n = (chunk - skip).min(out.len() - done);
        out[done..done + n].copy_from_slice(&scratch[skip..skip + n]);
        done += n;
    }
    true
}

/// Write `data` at a word-aligned ring offset, wrapping past the partition
/// end. The tail is padded with 0xFF so it stays erased-looking.
fn write_ring<F: Flash + ?Sized>(flash: &mut F, sectors: usize, offset: usize, data: &[u8]) -> bool {
    let total = sectors * SECTOR_SIZE;
    let mut scratch = [0xFFu8; 256];
    let mut done = 0;
    while done < data.len() {
        let pos = (offset + done) % total;
        let sector_left = SECTOR_SIZE - pos % SECTOR_SIZE;
        let n = (data.len() - done).min(scratch.len()).min(sector_left);
        scratch[..n].copy_from_slice(&data[done..done + n]);
        let p = padded(n);
        scratch[n..p].fill(0xFF);
        if !flash.write(pos, &scratch[..p]) {
            return false;
        }
        done += n;
    }
    true
}

/// True when ring bytes `[offset, offset + len)` are all 0xFF.
fn is_blank<F: Flash + ?Sized>(flash: &mut F, sectors: usize, offset: usize, len: usize) -> bool {
    let mut buf = [0u8; 256];
    let mut done = 0;
    while done < len {
        let n = (len - done).min(buf.len());
        if !read_ring(flash, sectors, offset + done, &mut buf[..n]) {
            return false;
        }
        if buf[..n].iter().any(|&b| b != 0xFF) {
            return false;
        }
        done += n;
    }
    true
}

/// Read a snapshot's payload into `buf`. Returns the payload slice.
pub fn read_snapshot<'b, F: Flash + ?Sized>(
    flash: &mut F,
    scan: &Scan,
    snap: &Snap,
    buf: &'b mut [u8],
) -> Option<&'b [u8]> {
    if snap.len > buf.len() {
        return None;
    }
    let start = snap.slot * SLOT_SIZE + HEADER_LEN;
    if !read_ring(flash, scan.sectors, start, &mut buf[..snap.len]) {
        return None;
    }
    Some(&buf[..snap.len])
}

/// Read a v0 record's JSON payload into `buf`.
pub fn read_legacy<F: Flash + ?Sized>(flash: &mut F, scan: &Scan, l: &Legacy, buf: &mut [u8]) -> bool {
    if l.len > buf.len() {
        return false;
    }
    let start = l.sector * SECTOR_SIZE + HEADER_LEN;
    read_ring(flash, scan.sectors, start, &mut buf[..l.len])
}

/// Why a write didn't happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    TooLarge,
    /// No sector could be erased without touching a protected record.
    NoRoom,
    Flash,
    /// The read-back didn't match what was written.
    Verify,
}

/// Append `payload` as the next snapshot. Sectors in `protect`, and the
/// sectors holding the newest record, are never erased. On success returns
/// the committed snapshot; the caller should treat it as newest.
pub fn write<F: Flash + ?Sized>(
    flash: &mut F,
    scan: &Scan,
    payload: &[u8],
    protect: u32,
) -> Result<Snap, WriteError> {
    let sectors = scan.sectors;
    if payload.len() > MAX_PAYLOAD {
        return Err(WriteError::TooLarge);
    }
    if sectors < 2 {
        return Err(WriteError::NoRoom);
    }
    let total = scan.total_slots();
    let need = slots_for(payload.len());
    let protect = protect | scan.newest_mask();
    let (seq, mut base) = match scan.newest() {
        Some((seq, _, end)) => (seq.wrapping_add(1), end),
        None => (1, 0),
    };

    // Find a start slot. Each pass either accepts `base` or moves it to a
    // later sector start, so `sectors + 1` passes always settle.
    let mut placed = false;
    for _ in 0..=sectors {
        let sector = base / SLOTS_PER_SECTOR;
        let mid_sector = base % SLOTS_PER_SECTOR != 0;
        // A mid-sector start writes into space the ring erased earlier. It
        // must still be blank: an interrupted write could have left bytes.
        if mid_sector {
            let room = SLOTS_PER_SECTOR - base % SLOTS_PER_SECTOR;
            let check = need.min(room) * SLOT_SIZE;
            if !is_blank(flash, sectors, base * SLOT_SIZE, check) {
                base = ((sector + 1) % sectors) * SLOTS_PER_SECTOR;
                continue;
            }
        }
        if erase_mask(base, need, total) & protect != 0 {
            base = ((sector + 1) % sectors) * SLOTS_PER_SECTOR;
            continue;
        }
        placed = true;
        break;
    }
    if !placed {
        return Err(WriteError::NoRoom);
    }

    for s in 0..sectors {
        if erase_mask(base, need, total) & (1 << s) != 0 && !flash.erase(s) {
            return Err(WriteError::Flash);
        }
    }

    // Payload first, header last.
    let start = base * SLOT_SIZE;
    if !write_ring(flash, sectors, start + HEADER_LEN, payload) {
        return Err(WriteError::Flash);
    }
    let header = encode_header(seq, payload.len());
    if !write_ring(flash, sectors, start, &header) {
        return Err(WriteError::Flash);
    }

    // Verify: header, then payload in chunks.
    let mut back = [0u8; HEADER_LEN];
    if !read_ring(flash, sectors, start, &mut back) || back != header {
        return Err(WriteError::Verify);
    }
    let mut chunk = [0u8; 256];
    let mut done = 0;
    while done < payload.len() {
        let n = (payload.len() - done).min(chunk.len());
        if !read_ring(flash, sectors, start + HEADER_LEN + done, &mut chunk[..n])
            || chunk[..n] != payload[done..done + n]
        {
            return Err(WriteError::Verify);
        }
        done += n;
    }
    Ok(Snap {
        slot: base,
        seq,
        len: payload.len(),
    })
}

/// Sectors a snapshot of `need` slots starting at `base` would erase: every
/// sector whose first slot falls inside the range.
fn erase_mask(base: usize, need: usize, total: usize) -> u32 {
    let mut mask = 0u32;
    for i in 0..need {
        let slot = (base + i) % total;
        if slot % SLOTS_PER_SECTOR == 0 {
            mask |= 1 << (slot / SLOTS_PER_SECTOR);
        }
    }
    mask
}

/// Erase every sector. Used by the explicit factory reset only.
pub fn erase_all<F: Flash + ?Sized>(flash: &mut F) -> bool {
    (0..usable_sectors(flash)).all(|s| flash.erase(s))
}
