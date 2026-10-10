//! Flash-backed save storage on the ESP-IDF `nvs` partition.
//!
//! We don't use ESP-IDF's NVS *format* (this is a bare-metal Rust firmware);
//! the `nvs` partition is just a convenient 24 KB region reserved by the
//! bootloader. We use it as a ring of N x 4 KB sectors and pack records into
//! it as variable-length spans.
//!
//! On-disk layout per record:
//!   - first sector starts with a 16-byte header:
//!       4 B magic     ("SAVE")
//!       4 B sequence  (u32 LE, monotonically increasing per save)
//!       4 B length    (u32 LE, payload bytes)
//!       4 B reserved  (zero, room for a future CRC)
//!   - payload bytes follow, spilling into successive sectors with
//!     wrap-around as needed.
//!
//! Write order matters for crash safety: payload first, header (with magic)
//! last. A mid-write power loss leaves no magic at the new record's start,
//! so the previously-committed record remains the latest valid entry.
//!
//! Wear leveling: each save starts on the sector after the prior record's
//! last-used sector, wrapping around. With a 24 KB partition and a typical
//! 1-sector save that's ~6x rotation. Chunky saves (more plants, more
//! sectors) get proportionally less rotation. Once a save grows to the
//! whole-partition-minus-one-sector ceiling, writes always reuse a sector
//! the prior record occupied.

#[cfg(not(feature = "desktop"))]
const MAGIC: [u8; 4] = *b"SAVE";
const HEADER_LEN: usize = 16;
const SECTOR_SIZE: usize = 4096;
#[cfg(not(feature = "desktop"))]
const WORD_SIZE: usize = 4;

/// Cap on payload bytes per save. Sized so a save never consumes the entire
/// partition: there's always at least one sector left between consecutive
/// records, which gives us baseline wear leveling.
pub const MAX_PAYLOAD: usize = SECTOR_SIZE * 5 - HEADER_LEN;

#[cfg(not(feature = "desktop"))]
mod firmware {
use esp_bootloader_esp_idf::partitions::{
    read_partition_table, DataPartitionSubType, PartitionType, PARTITION_TABLE_MAX_LEN,
};
use esp_hal::peripherals::FLASH;
use esp_storage::FlashStorage;

use super::{HEADER_LEN, MAGIC, MAX_PAYLOAD, SECTOR_SIZE, WORD_SIZE};


static mut FLASH_STORAGE: Option<FlashStorage<'static>> = None;

/// Take the FLASH peripheral and build the storage singleton. Must be called
/// once during boot before any save/load API.
pub fn init(flash: FLASH<'static>) {
    // SAFETY: single-threaded init path, called once during boot.
    unsafe {
        let slot = &mut *core::ptr::addr_of_mut!(FLASH_STORAGE);
        if slot.is_none() {
            *slot = Some(FlashStorage::new(flash));
        }
    }
}

fn flash() -> Option<&'static mut FlashStorage<'static>> {
    // SAFETY: single-threaded; the only writer is `init`, called once.
    unsafe { (*core::ptr::addr_of_mut!(FLASH_STORAGE)).as_mut() }
}

#[derive(Clone, Copy)]
struct PartitionInfo {
    offset: u32,
    sectors: usize,
}

fn find_nvs_partition(flash: &mut FlashStorage) -> Option<PartitionInfo> {
    let mut table_buf = [0u8; PARTITION_TABLE_MAX_LEN];
    let pt = read_partition_table(flash, &mut table_buf).ok()?;
    let entry = pt
        .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
        .ok()??;
    Some(PartitionInfo {
        offset: entry.offset(),
        sectors: (entry.len() as usize) / SECTOR_SIZE,
    })
}

#[derive(Clone, Copy)]
struct Record {
    start_sector: usize,
    seq: u32,
    len: u32,
}

impl Record {
    /// Number of sectors this record's header+payload occupies.
    fn sectors(self) -> usize {
        sectors_for(self.len as usize)
    }
}

fn sectors_for(payload_len: usize) -> usize {
    let total = HEADER_LEN + payload_len;
    total.div_ceil(SECTOR_SIZE).max(1)
}

fn read_header(flash: &mut FlashStorage, part: PartitionInfo, sector: usize) -> Option<Record> {
    let mut buf = [0u8; HEADER_LEN];
    let addr = part.offset + (sector * SECTOR_SIZE) as u32;
    if let Err(e) = flash.read_nor(addr, &mut buf) {
        println!("[Storage] Header read failed at sector {}: {:?}", sector, e);
        return None;
    }
    if buf[..4] != MAGIC {
        return None;
    }
    Some(Record {
        start_sector: sector,
        seq: u32::from_le_bytes(buf[4..8].try_into().ok()?),
        len: u32::from_le_bytes(buf[8..12].try_into().ok()?),
    })
}

fn find_latest(flash: &mut FlashStorage, part: PartitionInfo) -> Option<Record> {
    let mut best: Option<Record> = None;
    for i in 0..part.sectors {
        if let Some(r) = read_header(flash, part, i) {
            match best {
                Some(b) if b.seq >= r.seq => {}
                _ => best = Some(r),
            }
        }
    }
    best
}

/// True when at least one sector holds a syntactically valid save record.
pub fn has_save() -> bool {
    let Some(flash) = flash() else {
        return false;
    };
    let Some(part) = find_nvs_partition(flash) else {
        return false;
    };
    find_latest(flash, part).is_some()
}

/// Read the latest save payload into `buf`, walking across sector boundaries.
///
/// `esp-storage`'s `FlashStorage::read_nor` requires the read length to be a
/// multiple of WORD_SIZE (4 bytes). The on-disk payload is padded with 0xFF
/// to that alignment by `write_chunk`, so we read into an aligned scratch
/// region and copy the meaningful bytes out.
pub fn read_latest(buf: &mut [u8]) -> Option<usize> {
    let flash = flash()?;
    let part = find_nvs_partition(flash)?;
    let r = find_latest(flash, part)?;
    let len = r.len as usize;
    if len > buf.len() || len > MAX_PAYLOAD {
        println!("[Storage] Save payload too large: {}", len);
        return None;
    }

    let mut scratch = [0u8; SECTOR_SIZE];
    let mut read = 0;
    let mut sector_idx = r.start_sector;
    let mut sector_offset = HEADER_LEN;
    while read < len {
        let space = SECTOR_SIZE - sector_offset;
        let chunk = space.min(len - read);
        // Round up to WORD_SIZE. esp-storage rejects unaligned reads. The
        // tail bytes on flash were written as 0xFF padding, so we just
        // discard them after the read.
        let aligned_chunk = (chunk + WORD_SIZE - 1) & !(WORD_SIZE - 1);
        let aligned_chunk = aligned_chunk.min(space);
        let addr = part.offset + (sector_idx * SECTOR_SIZE + sector_offset) as u32;
        if let Err(e) = flash.read_nor(addr, &mut scratch[..aligned_chunk]) {
            println!(
                "[Storage] Payload read failed at sector {} offset {}: {:?}",
                sector_idx, sector_offset, e
            );
            return None;
        }
        buf[read..read + chunk].copy_from_slice(&scratch[..chunk]);
        read += chunk;
        sector_offset += chunk;
        if sector_offset >= SECTOR_SIZE {
            sector_idx = (sector_idx + 1) % part.sectors;
            sector_offset = 0;
        }
    }
    Some(len)
}

/// Pad `chunk` up to the next 4-byte multiple with 0xFF (erased flash) and
/// write it to flash. `write_nor` requires word-aligned lengths. (Plain
/// `FlashStorage::write` is a read-modify-write that erases every sector it
/// touches, which would break the header-last commit below.)
fn write_chunk(
    flash: &mut FlashStorage,
    addr: u32,
    chunk: &[u8],
    scratch: &mut [u8; SECTOR_SIZE],
) -> bool {
    let padded = chunk.len().div_ceil(WORD_SIZE) * WORD_SIZE;
    if padded == chunk.len() {
        flash.write_nor(addr, chunk).is_ok()
    } else {
        scratch[..chunk.len()].copy_from_slice(chunk);
        for b in &mut scratch[chunk.len()..padded] {
            *b = 0xFF;
        }
        flash.write_nor(addr, &scratch[..padded]).is_ok()
    }
}

/// Wipe every sector of the nvs partition so `has_save` returns false on
/// next boot. Used for the debug factory-reset flow.
pub fn erase_all() -> bool {
    let Some(flash) = flash() else {
        return false;
    };
    let Some(part) = find_nvs_partition(flash) else {
        return false;
    };
    for i in 0..part.sectors {
        let addr = part.offset + (i * SECTOR_SIZE) as u32;
        if flash.erase(addr, addr + SECTOR_SIZE as u32).is_err() {
            println!("[Storage] Factory erase failed at sector {}", i);
            return false;
        }
    }
    println!("[Storage] Factory reset, wiped {} sectors", part.sectors);
    true
}

/// Persist `payload` as the next record. Returns true on success.
pub fn write_next(payload: &[u8]) -> bool {
    if payload.len() > MAX_PAYLOAD {
        println!(
            "[Storage] Save payload {} > MAX_PAYLOAD {}",
            payload.len(),
            MAX_PAYLOAD
        );
        return false;
    }
    let Some(flash) = flash() else {
        println!("[Storage] FLASH not initialised");
        return false;
    };
    let Some(part) = find_nvs_partition(flash) else {
        println!("[Storage] No nvs partition found");
        return false;
    };
    let sectors_total = part.sectors;
    if sectors_total == 0 {
        return false;
    }
    let needed = sectors_for(payload.len());
    if needed > sectors_total {
        println!(
            "[Storage] Save needs {} sectors but partition has {}",
            needed, sectors_total
        );
        return false;
    }

    let latest = find_latest(flash, part);
    let (start, next_seq) = match latest {
        Some(r) => (
            (r.start_sector + r.sectors()) % sectors_total,
            r.seq.wrapping_add(1),
        ),
        None => (0, 1),
    };

    // Erase the sectors we're about to write.
    for i in 0..needed {
        let sector = (start + i) % sectors_total;
        let addr = part.offset + (sector * SECTOR_SIZE) as u32;
        if flash.erase(addr, addr + SECTOR_SIZE as u32).is_err() {
            println!("[Storage] Sector {} erase failed", sector);
            return false;
        }
    }

    // Stream the payload across sectors (skipping the 16-byte header at the
    // start of the first sector; it's filled in last so a mid-write power
    // loss leaves no MAGIC and the previous record remains canonical).
    let mut scratch = [0u8; SECTOR_SIZE];
    let mut written = 0;
    let mut sector_idx = start;
    let mut sector_offset = HEADER_LEN;
    while written < payload.len() {
        let space = SECTOR_SIZE - sector_offset;
        let chunk_len = space.min(payload.len() - written);
        let addr = part.offset + (sector_idx * SECTOR_SIZE + sector_offset) as u32;
        let chunk = &payload[written..written + chunk_len];
        if !write_chunk(flash, addr, chunk, &mut scratch) {
            println!("[Storage] Payload write failed at sector {}", sector_idx);
            return false;
        }
        written += chunk_len;
        sector_offset += chunk_len;
        if sector_offset >= SECTOR_SIZE {
            sector_idx = (sector_idx + 1) % sectors_total;
            sector_offset = 0;
        }
    }

    // Commit by writing the header last.
    let mut header = [0u8; HEADER_LEN];
    header[..4].copy_from_slice(&MAGIC);
    header[4..8].copy_from_slice(&next_seq.to_le_bytes());
    header[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    let start_addr = part.offset + (start * SECTOR_SIZE) as u32;
    if let Err(e) = flash.write_nor(start_addr, &header) {
        println!("[Storage] Header write failed: {:?}", e);
        return false;
    }

    // Verify-after-write: read the header back and confirm it persisted as
    // we expect before declaring success. Catches silent flash failures
    // (write-cache anomalies, partial erases, etc) so save errors surface
    // immediately rather than at next boot.
    let mut readback = [0u8; HEADER_LEN];
    if let Err(e) = flash.read_nor(start_addr, &mut readback) {
        println!("[Storage] Header verify-read failed: {:?}", e);
        return false;
    }
    if readback != header {
        println!(
            "[Storage] Header verify mismatch, wrote {:?}, read {:?}",
            &header[..12],
            &readback[..12]
        );
        return false;
    }

    println!(
        "[Storage] Saved {} bytes spanning {} sector(s) starting at {} (seq {})",
        payload.len(),
        needed,
        start,
        next_seq
    );
    true
}

} // mod firmware

#[cfg(not(feature = "desktop"))]
pub use firmware::{erase_all, has_save, init, read_latest, write_next};

#[cfg(feature = "desktop")]
mod desktop {
    //! Desktop save backend: a single JSON file in the current working
    //! directory. No header / no wear leveling. `payload` is the raw
    //! bytes written by `save.rs`, stored verbatim. `init()` is a no-op
    //! (kept so call sites match the firmware signature shape).

    use std::fs;
    use std::io::Read;
    use std::path::Path;


    const SAVE_PATH: &str = "./catode32-save.json";

    pub fn init() {}

    pub fn has_save() -> bool {
        Path::new(SAVE_PATH).is_file()
    }

    pub fn read_latest(buf: &mut [u8]) -> Option<usize> {
        let mut f = fs::File::open(SAVE_PATH).ok()?;
        let mut n = 0;
        loop {
            match f.read(&mut buf[n..]) {
                Ok(0) => break,
                Ok(k) => n += k,
                Err(_) => return None,
            }
            if n == buf.len() {
                break;
            }
        }
        Some(n)
    }

    pub fn write_next(payload: &[u8]) -> bool {
        match fs::write(SAVE_PATH, payload) {
            Ok(()) => {
                println!("[Storage] Saved {} bytes to {}", payload.len(), SAVE_PATH);
                true
            }
            Err(e) => {
                println!("[Storage] Save failed: {:?}", e);
                false
            }
        }
    }

    pub fn erase_all() -> bool {
        if !has_save() {
            return true;
        }
        match fs::remove_file(SAVE_PATH) {
            Ok(()) => {
                println!("[Storage] Erased {}", SAVE_PATH);
                true
            }
            Err(e) => {
                println!("[Storage] Erase failed: {:?}", e);
                false
            }
        }
    }
}

#[cfg(feature = "desktop")]
pub use desktop::{erase_all, has_save, init, read_latest, write_next};
