//! Flash-backed save storage on the ESP-IDF `nvs` partition.
//!
//! We don't use ESP-IDF's NVS *format* (this is a bare-metal Rust firmware);
//! the `nvs` partition is just a convenient 24 KB region reserved by the
//! bootloader. The snapshot ring that lives in it is `crate::ring`; this
//! module only supplies the platform [`Flash`] implementation, access to the
//! legacy (v0 JSON) save, and the save lock.
//!
//! Desktop stands in for the partition with a 24 KB image file
//! (`catode32-save.bin`), so the ring logic runs identically on both. The
//! legacy save on desktop is the old `catode32-save.json` file.

use crate::ring::{self, Scan};

/// Sectors in the desktop image, matching the default 24 KB `nvs` partition.
#[cfg(feature = "desktop")]
const DESKTOP_SECTORS: usize = 6;

/// Set when a save exists but couldn't be read. While locked, nothing may
/// write to the partition, so the unreadable save survives until a firmware
/// that can read it is installed, or the player explicitly erases it.
static mut LOCKED: bool = false;

pub fn lock() {
    // SAFETY: single-threaded game loop is the only accessor.
    unsafe { *core::ptr::addr_of_mut!(LOCKED) = true }
}

pub fn is_locked() -> bool {
    // SAFETY: see `lock`.
    unsafe { *core::ptr::addr_of!(LOCKED) }
}

/// A v0 save available for migration.
#[derive(Clone, Copy, Debug)]
pub struct LegacyRef {
    pub seq: u32,
    pub len: usize,
}

#[cfg(not(feature = "desktop"))]
mod firmware {
    use esp_bootloader_esp_idf::partitions::{
        read_partition_table, DataPartitionSubType, PartitionType, PARTITION_TABLE_MAX_LEN,
    };
    use esp_hal::peripherals::FLASH;
    use esp_storage::FlashStorage;

    use crate::ring::{self, Flash, Scan, SECTOR_SIZE};

    static mut FLASH_STORAGE: Option<FlashStorage<'static>> = None;

    /// Take the FLASH peripheral and build the storage singleton. Must be
    /// called once during boot before any save/load API.
    pub fn init(flash: FLASH<'static>) {
        // SAFETY: single-threaded init path, called once during boot.
        unsafe {
            let slot = &mut *core::ptr::addr_of_mut!(FLASH_STORAGE);
            if slot.is_none() {
                *slot = Some(FlashStorage::new(flash));
            }
        }
    }

    fn storage() -> Option<&'static mut FlashStorage<'static>> {
        // SAFETY: single-threaded; the only writer is `init`, called once.
        unsafe { (*core::ptr::addr_of_mut!(FLASH_STORAGE)).as_mut() }
    }

    /// The `nvs` partition as a [`Flash`].
    pub struct Partition {
        storage: &'static mut FlashStorage<'static>,
        offset: u32,
        sectors: usize,
    }

    impl Flash for Partition {
        fn sectors(&self) -> usize {
            self.sectors
        }

        fn read(&mut self, offset: usize, buf: &mut [u8]) -> bool {
            match self.storage.read_nor(self.offset + offset as u32, buf) {
                Ok(()) => true,
                Err(e) => {
                    println!("[Storage] Read failed at {}: {:?}", offset, e);
                    false
                }
            }
        }

        fn write(&mut self, offset: usize, data: &[u8]) -> bool {
            // `write_nor`, not `write`: plain `FlashStorage::write` is a
            // read-modify-write that erases every sector it touches, which
            // would break the header-last commit.
            match self.storage.write_nor(self.offset + offset as u32, data) {
                Ok(()) => true,
                Err(e) => {
                    println!("[Storage] Write failed at {}: {:?}", offset, e);
                    false
                }
            }
        }

        fn erase(&mut self, sector: usize) -> bool {
            let addr = self.offset + (sector * SECTOR_SIZE) as u32;
            match self.storage.erase(addr, addr + SECTOR_SIZE as u32) {
                Ok(()) => true,
                Err(e) => {
                    println!("[Storage] Erase failed at sector {}: {:?}", sector, e);
                    false
                }
            }
        }
    }

    pub fn open() -> Option<Partition> {
        let storage = storage()?;
        let mut table_buf = [0u8; PARTITION_TABLE_MAX_LEN];
        let pt = read_partition_table(storage, &mut table_buf).ok()?;
        let entry = pt
            .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
            .ok()??;
        let (offset, sectors) = (entry.offset(), (entry.len() as usize) / SECTOR_SIZE);
        Some(Partition {
            storage,
            offset,
            sectors,
        })
    }

    /// On firmware the v0 record lives in the same ring.
    pub fn legacy(scan: &Scan) -> Option<super::LegacyRef> {
        scan.legacy.map(|l| super::LegacyRef { seq: l.seq, len: l.len })
    }

    pub fn read_legacy(flash: &mut Partition, scan: &Scan, buf: &mut [u8]) -> bool {
        match scan.legacy {
            Some(l) => ring::read_legacy(flash, scan, &l, buf),
            None => false,
        }
    }

    pub fn erase_all() -> bool {
        let Some(mut p) = open() else {
            return false;
        };
        let ok = ring::erase_all(&mut p);
        if ok {
            println!("[Storage] Factory reset, wiped {} sectors", p.sectors);
        }
        ok
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{init, Partition};
#[cfg(not(feature = "desktop"))]
use firmware::{erase_all as platform_erase_all, legacy, open, read_legacy as platform_read_legacy};

#[cfg(feature = "desktop")]
mod desktop {
    //! The partition is a fixed-size image file in the working directory,
    //! rewritten after every operation. Writes AND into the existing bytes
    //! like NOR flash, so interrupted-write behaviour matches the hardware.

    use std::fs;
    use std::path::Path;

    use crate::ring::{Flash, Scan, SECTOR_SIZE};

    pub const IMAGE_PATH: &str = "./catode32-save.bin";
    /// The v0 desktop save. Kept after migration as a backup.
    pub const LEGACY_PATH: &str = "./catode32-save.json";

    pub fn init() {}

    pub struct Partition {
        data: Vec<u8>,
    }

    impl Partition {
        fn persist(&self) -> bool {
            match fs::write(IMAGE_PATH, &self.data) {
                Ok(()) => true,
                Err(e) => {
                    println!("[Storage] Image write failed: {:?}", e);
                    false
                }
            }
        }
    }

    impl Flash for Partition {
        fn sectors(&self) -> usize {
            self.data.len() / SECTOR_SIZE
        }

        fn read(&mut self, offset: usize, buf: &mut [u8]) -> bool {
            match self.data.get(offset..offset + buf.len()) {
                Some(src) => {
                    buf.copy_from_slice(src);
                    true
                }
                None => false,
            }
        }

        fn write(&mut self, offset: usize, data: &[u8]) -> bool {
            let Some(dst) = self.data.get_mut(offset..offset + data.len()) else {
                return false;
            };
            for (d, s) in dst.iter_mut().zip(data) {
                *d &= *s;
            }
            self.persist()
        }

        fn erase(&mut self, sector: usize) -> bool {
            let start = sector * SECTOR_SIZE;
            let Some(dst) = self.data.get_mut(start..start + SECTOR_SIZE) else {
                return false;
            };
            dst.fill(0xFF);
            self.persist()
        }
    }

    pub fn open() -> Option<Partition> {
        let size = super::DESKTOP_SECTORS * SECTOR_SIZE;
        let mut data = fs::read(IMAGE_PATH).unwrap_or_default();
        data.resize(size, 0xFF);
        Some(Partition { data })
    }

    pub fn legacy(_scan: &Scan) -> Option<super::LegacyRef> {
        let len = fs::metadata(LEGACY_PATH).ok()?.len() as usize;
        // Sequence 0: any v1 snapshot (sequence >= 1) is newer.
        Some(super::LegacyRef { seq: 0, len })
    }

    pub fn read_legacy(_flash: &mut Partition, _scan: &Scan, buf: &mut [u8]) -> bool {
        match fs::read(LEGACY_PATH) {
            Ok(bytes) if bytes.len() <= buf.len() => {
                buf[..bytes.len()].copy_from_slice(&bytes);
                true
            }
            _ => false,
        }
    }

    pub fn erase_all() -> bool {
        for path in [IMAGE_PATH, LEGACY_PATH] {
            if Path::new(path).exists() {
                if let Err(e) = fs::remove_file(path) {
                    println!("[Storage] Erase failed: {:?}", e);
                    return false;
                }
            }
        }
        println!("[Storage] Erased save files");
        true
    }
}

#[cfg(feature = "desktop")]
pub use desktop::{init, Partition};
#[cfg(feature = "desktop")]
use desktop::{erase_all as platform_erase_all, legacy, open, read_legacy as platform_read_legacy};

/// Open the save partition, or `None` if there isn't one.
pub fn open_partition() -> Option<Partition> {
    open()
}

/// Scan the partition for snapshots and a legacy record.
pub fn scan(flash: &mut Partition) -> Scan {
    ring::scan(flash)
}

/// The legacy (v0 JSON) save, if one exists.
pub fn legacy_save(scan: &Scan) -> Option<LegacyRef> {
    legacy(scan)
}

/// Read the legacy save's JSON into `buf` (at least `LegacyRef::len` long).
pub fn read_legacy(flash: &mut Partition, scan: &Scan, buf: &mut [u8]) -> bool {
    platform_read_legacy(flash, scan, buf)
}

/// True when any save, readable or not, exists.
pub fn has_save() -> bool {
    let Some(mut p) = open() else {
        return false;
    };
    let scan = ring::scan(&mut p);
    !scan.snaps.is_empty() || legacy(&scan).is_some()
}

/// Wipe every save. Used by the explicit factory-reset flows only, which is
/// also the one way out of the save lock.
pub fn erase_all() -> bool {
    let ok = platform_erase_all();
    if ok {
        // SAFETY: see `lock`.
        unsafe { *core::ptr::addr_of_mut!(LOCKED) = false }
    }
    ok
}

#[cfg(feature = "desktop")]
pub use desktop::{IMAGE_PATH, LEGACY_PATH};

