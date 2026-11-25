//! Snapshot ring on a simulated NOR flash: wear levelling and lifespan,
//! power loss at every byte, wrap-around, and v0 records it must not erase.

#![cfg(feature = "desktop")]

mod common;

use catode32_core::{
    context::GameContext,
    led::Led,
    ring::{self, Flash, MAX_PAYLOAD, SECTOR_SIZE, SLOTS_PER_SECTOR},
    save_v1,
};
use common::{maxed::maxed_context, MemFlash};

const SECTORS: usize = 6;
const ERASE_CYCLES: f64 = 100_000.0;
const SAVES_PER_DAY: f64 = 26.0;

fn encode(ctx: &GameContext) -> Vec<u8> {
    let mut buf = vec![0u8; MAX_PAYLOAD];
    let n = save_v1::encode(ctx, &mut buf).expect("encode");
    buf.truncate(n);
    buf
}

/// Stamp a counter into a payload so every save differs.
fn payload_n(base: &[u8], n: u32) -> Vec<u8> {
    let mut p = base.to_vec();
    let len = p.len();
    p[len - 4..].copy_from_slice(&n.to_le_bytes());
    p
}

fn newest_payload(flash: &mut MemFlash) -> Option<(u32, Vec<u8>)> {
    let scan = ring::scan(flash);
    let s = *scan.snaps.first()?;
    let mut buf = vec![0u8; MAX_PAYLOAD];
    let p = ring::read_snapshot(flash, &scan, &s, &mut buf)?.to_vec();
    Some((s.seq, p))
}

/// Run `saves` writes of `payload` and report (years of life, per-sector
/// erases, whether any snapshot wrapped past the partition end).
fn simulate(payload: &[u8], saves: u32) -> (f64, Vec<u32>, bool) {
    let mut flash = MemFlash::new(SECTORS);
    let mut wrapped = false;
    for n in 0..saves {
        let p = payload_n(payload, n);
        let scan = ring::scan(&mut flash);
        let s = ring::write(&mut flash, &scan, &p, 0).expect("write");
        wrapped |= s.slot + ring::slots_for(s.len) > SECTORS * SLOTS_PER_SECTOR;
        // Every save reads back as the newest.
        let (seq, back) = newest_payload(&mut flash).unwrap();
        assert_eq!(seq, s.seq);
        assert_eq!(back, p, "save {n} read back");
    }
    let total: u32 = flash.erases.iter().sum();
    let per_save = total as f64 / saves as f64;
    let years = (ERASE_CYCLES * SECTORS as f64) / per_save / SAVES_PER_DAY / 365.25;
    (years, flash.erases, wrapped)
}

#[test]
fn worst_case_save_lifespan_and_even_wear() {
    let payload = encode(&maxed_context());
    let (years, erases, wrapped) = simulate(&payload, 10_000);
    println!(
        "maxed {} B: {:.1} years at {} saves/day, erases per sector {:?}",
        payload.len(),
        years,
        SAVES_PER_DAY,
        erases
    );
    let (max, min) = (*erases.iter().max().unwrap(), *erases.iter().min().unwrap());
    assert!(max - min <= 1, "uneven wear: {erases:?}");
    assert!(wrapped, "snapshots should wrap past the partition end");
    // Today's maxed v0 save lasts ~11 years. Regressions below this floor
    // mean the save grew or the ring stopped packing snapshots.
    assert!(years >= 80.0, "worst-case lifespan fell to {years:.1} years");
}

#[test]
fn typical_save_lifespan() {
    let payload = encode(&GameContext::new(Led::new()));
    let (years, erases, _) = simulate(&payload, 10_000);
    println!("fresh pet {} B: {:.0} years, erases per sector {:?}", payload.len(), years, erases);
    assert!(years >= 300.0);
}

#[test]
fn power_loss_at_every_byte_never_loses_the_last_save() {
    let base = encode(&maxed_context());
    let mut flash = MemFlash::new(SECTORS);
    // Fill part of the ring so the interrupted write lands mid-sector and
    // spills into the next sector.
    for n in 0..3 {
        let scan = ring::scan(&mut flash);
        ring::write(&mut flash, &scan, &payload_n(&base, n), 0).unwrap();
    }
    let (good_seq, good) = newest_payload(&mut flash).unwrap();
    let next = payload_n(&base, 99);
    let full_cost = ring::slots_for(next.len()) * ring::SLOT_SIZE + SECTOR_SIZE;

    for budget in 0..full_cost {
        let mut f = flash.clone();
        f.budget = Some(budget);
        let scan = ring::scan(&mut f);
        let ok = ring::write(&mut f, &scan, &next, 0).is_ok();
        f.budget = None; // power back on

        let (seq, back) = newest_payload(&mut f).expect("a save must survive");
        if ok {
            assert_eq!((seq, &back), (good_seq + 1, &next), "budget {budget}");
        } else {
            assert_eq!((seq, &back), (good_seq, &good), "budget {budget}: previous save lost");
        }

        // The next save after the outage works and becomes newest, even if
        // the interrupted one left stray bytes behind.
        let after = payload_n(&base, 1234);
        let scan = ring::scan(&mut f);
        ring::write(&mut f, &scan, &after, 0).unwrap_or_else(|e| panic!("budget {budget}: {e:?}"));
        assert_eq!(newest_payload(&mut f).unwrap().1, after, "budget {budget}");
    }
}

/// Write a v0 record (header at a sector start, payload spanning sectors).
fn put_v0(flash: &mut MemFlash, sector: usize, seq: u32, len: usize) {
    let total = flash.data.len();
    let start = sector * SECTOR_SIZE;
    let mut rec = Vec::new();
    rec.extend_from_slice(b"SAVE");
    rec.extend_from_slice(&seq.to_le_bytes());
    rec.extend_from_slice(&(len as u32).to_le_bytes());
    rec.extend_from_slice(&[0; 4]);
    rec.extend((0..len).map(|i| b"{\"json\":true}"[i % 13]));
    for (i, b) in rec.iter().enumerate() {
        flash.data[(start + i) % total] = *b;
    }
}

#[test]
fn migration_never_erases_the_v0_save() {
    // Largest v0 record (5 sectors), wrapping: sectors 3, 4, 5, 0, 1.
    let mut flash = MemFlash::new(SECTORS);
    let v0_len = SECTOR_SIZE * 5 - 16;
    put_v0(&mut flash, 3, 41, v0_len);
    let v0_image = flash.data.clone();

    let scan = ring::scan(&mut flash);
    let legacy = scan.legacy.expect("v0 record found");
    assert_eq!((legacy.sector, legacy.seq, legacy.len), (3, 41, v0_len));

    // Interrupted migration at every byte: the v0 record is untouched and
    // still the newest record.
    let payload = encode(&maxed_context());
    for budget in (0..payload.len() + 64).step_by(7) {
        let mut f = flash.clone();
        f.budget = Some(budget);
        let scan = ring::scan(&mut f);
        let _ = ring::write(&mut f, &scan, &payload, 0);
        f.budget = None;
        let scan = ring::scan(&mut f);
        let v1_newer = scan.snaps.first().map_or(false, |s| s.seq > 41);
        if !v1_newer {
            assert_eq!(scan.legacy.map(|l| l.seq), Some(41), "budget {budget}");
        }
        let mask = ring::legacy_mask(&legacy, SECTORS);
        for s in 0..SECTORS {
            if mask & (1 << s) != 0 {
                assert_eq!(f.erases[s], 0, "budget {budget}: erased v0 sector {s}");
                assert_eq!(
                    f.data[s * SECTOR_SIZE..(s + 1) * SECTOR_SIZE],
                    v0_image[s * SECTOR_SIZE..(s + 1) * SECTOR_SIZE],
                    "budget {budget}: v0 sector {s} changed"
                );
            }
        }
    }

    // Completed migration: v1 goes in the one free sector, seq follows v0.
    let s = ring::write(&mut flash, &scan, &payload, 0).unwrap();
    assert_eq!(s.seq, 42);
    assert_eq!(s.slot / SLOTS_PER_SECTOR, 2);
    assert_eq!(flash.erases, vec![0, 0, 1, 0, 0, 0]);
    let scan = ring::scan(&mut flash);
    assert_eq!(scan.snaps[0].seq, 42);
    assert_eq!(scan.legacy.map(|l| l.seq), Some(41));
}

#[test]
fn protected_sectors_are_never_erased() {
    let payload = encode(&GameContext::new(Led::new()));
    let mut flash = MemFlash::new(SECTORS);
    let protect = 0b001100; // sectors 2 and 3
    for n in 0..500 {
        let scan = ring::scan(&mut flash);
        ring::write(&mut flash, &scan, &payload_n(&payload, n), protect).unwrap();
    }
    assert_eq!((flash.erases[2], flash.erases[3]), (0, 0));
}

#[test]
fn scan_keeps_newest_snapshots_in_order() {
    let payload = encode(&GameContext::new(Led::new()));
    let mut flash = MemFlash::new(SECTORS);
    for n in 0..40 {
        let scan = ring::scan(&mut flash);
        ring::write(&mut flash, &scan, &payload_n(&payload, n), 0).unwrap();
    }
    let scan = ring::scan(&mut flash);
    let seqs: Vec<u32> = scan.snaps.iter().map(|s| s.seq).collect();
    assert_eq!(seqs, (33..=40).rev().collect::<Vec<_>>());
    assert_eq!(flash.sectors(), SECTORS);
}
