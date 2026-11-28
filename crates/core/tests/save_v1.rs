//! v1 save format: exact round trips, size budget, forward/backward
//! compatibility rules, migration from v0 saves, and section fallback.

#![cfg(feature = "desktop")]

mod common;

use std::fs;

use catode32_core::{
    context::{GameContext, ToyVariant},
    led::Led,
    ring::{self, crc16_update, MAX_PAYLOAD},
    save::{self, LoadOutcome},
    save_v1::{self, SECTION_PET, SECTION_WORLD},
    storage,
};
use common::{assert_same_cat, fixture, maxed::maxed_context, CwdGuard, MemFlash};

/// Largest rounding error on plant water (0.25 h) or fertilizer (0.5).
const PLANT_TOL: f32 = 0.5;

fn fresh() -> GameContext {
    GameContext::new(Led::new())
}

fn encode(ctx: &GameContext) -> Vec<u8> {
    let mut buf = vec![0u8; MAX_PAYLOAD];
    let n = save_v1::encode(ctx, &mut buf).expect("encode");
    buf.truncate(n);
    buf
}

/// Decode a whole payload the way `save::load` does for one snapshot.
fn decode(payload: &[u8], ctx: &mut GameContext) {
    save_v1::decode_pet(save_v1::section(payload, SECTION_PET).expect("pet"), ctx);
    save_v1::decode_world(save_v1::section(payload, SECTION_WORLD).expect("world"), ctx);
}

/// Build `[id][len][crc][body]`.
fn section(id: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![id];
    out.extend_from_slice(&(body.len() as u16).to_le_bytes());
    let crc = crc16_update(crc16_update(0xFFFF, &out), body);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn field(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    out.extend_from_slice(value);
    out
}

#[test]
fn maxed_context_round_trips() {
    // Exact apart from plant water and fertilizer rounding.
    let ctx = maxed_context();
    let mut back = fresh();
    decode(&encode(&ctx), &mut back);
    assert_same_cat(&ctx, &back, PLANT_TOL);
}

#[test]
fn plant_water_and_fertilizer_round_to_nearest() {
    let mut ctx = fresh();
    ctx.plants.truncate(1);
    for (water, fert, want_water, want_fert) in [
        (41.59998, 99.775, 41.5, 100.0),
        (41.76, 0.49, 42.0, 0.0),
        (0.0, 200.0, 0.0, 200.0),
        (384.9, 0.5, 385.0, 1.0),
    ] {
        ctx.plants[0].water_debt = water;
        ctx.plants[0].fertilizer = fert;
        let mut back = fresh();
        decode(&encode(&ctx), &mut back);
        assert_eq!(back.plants[0].water_debt, want_water, "water {water}");
        assert_eq!(back.plants[0].fertilizer, want_fert, "fert {fert}");
    }
}

#[test]
fn aged_flag_survives_a_save() {
    // v0 never stored `aged`; v1 does.
    let mut ctx = maxed_context();
    ctx.plants[3].aged = true;
    let mut back = fresh();
    decode(&encode(&ctx), &mut back);
    assert!(back.plants[3].aged);
    assert!(!back.plants[4].aged);
}

#[test]
fn maxed_snapshot_fits_in_one_sector() {
    let maxed = encode(&maxed_context());
    let typical = encode(&fresh());
    println!(
        "v1 snapshot: maxed {} B ({} slots), fresh pet {} B ({} slots)",
        maxed.len(),
        ring::slots_for(maxed.len()),
        typical.len(),
        ring::slots_for(typical.len())
    );
    assert!(maxed.len() <= MAX_PAYLOAD);
    // Budget check: well under the v0 worst case (~20 KB).
    assert!(maxed.len() < 3_200, "maxed snapshot grew to {} bytes", maxed.len());
}

#[test]
fn unknown_sections_and_tags_are_skipped() {
    let ctx = maxed_context();
    let payload = encode(&ctx);
    let pet = save_v1::section(&payload, SECTION_PET).unwrap();
    let world = save_v1::section(&payload, SECTION_WORLD).unwrap();

    // A newer firmware might add a section and new tags. Put the unknowns
    // first so the reader has to step over them.
    let mut pet2 = field(200, b"future pet field");
    pet2.extend_from_slice(pet);
    let mut world2 = field(201, &[1, 2, 3]);
    world2.extend_from_slice(world);
    world2.extend(field(202, &[]));
    let mut newer = section(9, b"a whole future section");
    newer.extend(section(SECTION_WORLD, &world2));
    newer.extend(section(SECTION_PET, &pet2));

    let mut back = fresh();
    decode(&newer, &mut back);
    assert_same_cat(&ctx, &back, PLANT_TOL);
}

#[test]
fn trailing_bytes_in_fixed_values_are_ignored() {
    // Newer firmware may append stats or clock fields. Older readers must
    // take what they know and ignore the rest.
    let mut stats = Vec::new();
    for i in 0..20 {
        stats.extend_from_slice(&(10.0f32 + i as f32).to_le_bytes());
    }
    let body = field(3, &stats);
    let mut ctx = fresh();
    save_v1::decode_pet(&body, &mut ctx);
    assert_eq!(ctx.fullness, 10.0);
    assert_eq!(ctx.mischievousness, 26.0);
}

#[test]
fn missing_fields_keep_safe_defaults() {
    let defaults = fresh();
    // A PET section holding only the seed (as an older or partial writer
    // might produce).
    let body = field(1, &0x55u64.to_le_bytes());
    let mut ctx = fresh();
    save_v1::decode_pet(&body, &mut ctx);
    assert_eq!(ctx.pet_seed, 0x55);
    assert_eq!(ctx.fullness, defaults.fullness);
    assert_eq!(ctx.energy, defaults.energy);
    assert!(ctx.fullness > 0.0, "a missing stat must never mean a starving cat");

    // Short stats value: the stats it has are applied, the rest keep defaults.
    let body = field(3, &[0u8; 8]);
    let mut ctx = fresh();
    save_v1::decode_pet(&body, &mut ctx);
    assert_eq!((ctx.fullness, ctx.energy), (0.0, 0.0));
    assert_eq!(ctx.comfort, defaults.comfort);
}

#[test]
fn plant_records_tolerate_extra_and_missing_trailing_fields() {
    let ctx = maxed_context();
    let mut one = fresh();
    one.plants.clear();
    let _ = one.plants.push(ctx.plants[7]);
    let payload = encode(&one);
    let world = save_v1::section(&payload, SECTION_WORLD).unwrap();

    // Find the PLANTS field (tag 4) and its single record.
    let mut i = 0;
    let (start, len) = loop {
        let tag = world[i];
        let len = u16::from_le_bytes([world[i + 1], world[i + 2]]) as usize;
        if tag == 4 {
            break (i + 3, len);
        }
        i += 3 + len;
    };
    let value = &world[start..start + len];
    assert_eq!(value[0], 1, "one record");
    let rec = &value[2..2 + value[1] as usize];

    // A newer record with two extra trailing bytes.
    let mut longer = rec.to_vec();
    longer.extend_from_slice(&[0xAB, 0xCD]);
    let mut list = vec![1u8, longer.len() as u8];
    list.extend_from_slice(&longer);
    let mut back = fresh();
    save_v1::decode_world(&field(4, &list), &mut back);
    assert_eq!(back.plants.len(), 1);
    assert_eq!(back.plants[0].id, ctx.plants[7].id);
    assert_eq!(back.plants[0].age_hours, ctx.plants[7].age_hours);
    assert_eq!(back.plants[0].planted_day, ctx.plants[7].planted_day);

    // An older record with only the fixed bytes: the rest get defaults.
    let fixed = 2 + 4; // 2-byte varint id + scene, stage, seed/layer, pot/flags
    let mut list = vec![1u8, fixed as u8];
    list.extend_from_slice(&rec[..fixed]);
    let mut back = fresh();
    save_v1::decode_world(&field(4, &list), &mut back);
    assert_eq!(back.plants.len(), 1);
    let p = &back.plants[0];
    assert_eq!((p.id, p.scene, p.stage, p.pot), (ctx.plants[7].id, ctx.plants[7].scene, ctx.plants[7].stage, ctx.plants[7].pot));
    assert_eq!((p.age_hours, p.water_debt, p.planted_day), (0, 0.0, None));
}

#[test]
fn damaged_section_is_rejected_by_crc() {
    let mut payload = encode(&maxed_context());
    let pet_len = u16::from_le_bytes([payload[1], payload[2]]) as usize;
    payload[5 + pet_len + 5 + 10] ^= 0x01; // a byte inside WORLD's body
    assert!(save_v1::section(&payload, SECTION_PET).is_some());
    assert!(save_v1::section(&payload, SECTION_WORLD).is_none());
}

#[test]
fn save_and_load_through_the_ring() {
    let _g = CwdGuard::enter();
    let mut ctx = maxed_context();
    assert!(save::save(&mut ctx));
    assert_eq!(fs::metadata(storage::IMAGE_PATH).unwrap().len(), 6 * 4096);
    let mut back = fresh();
    assert_eq!(save::load(&mut back), LoadOutcome::Loaded);
    assert_same_cat(&ctx, &back, PLANT_TOL);
    assert!(!back.first_impressions);
}

#[test]
fn fresh_device_has_no_save() {
    let _g = CwdGuard::enter();
    let mut ctx = fresh();
    assert_eq!(save::load(&mut ctx), LoadOutcome::NoSave);
    assert!(!save::has_save());
}

/// Load a v0 fixture, check it migrated to v1 with the JSON kept, then load
/// the v1 snapshot and check it's the same cat.
fn migrate_fixture(name: &str) -> GameContext {
    let _g = CwdGuard::enter();
    fs::copy(fixture(name), storage::LEGACY_PATH).expect("copy fixture");

    let mut from_v0 = fresh();
    assert_eq!(save::load(&mut from_v0), LoadOutcome::Loaded);
    assert!(fs::metadata(storage::IMAGE_PATH).is_ok(), "migration wrote a v1 snapshot");
    assert!(fs::metadata(storage::LEGACY_PATH).is_ok(), "v0 save kept as a backup");

    let mut from_v1 = fresh();
    assert_eq!(save::load(&mut from_v1), LoadOutcome::Loaded);
    assert_same_cat(&from_v0, &from_v1, PLANT_TOL);
    from_v1
}

#[test]
fn migrates_typical_v0_save() {
    let cat = migrate_fixture("v0_typical.json");
    assert_eq!(cat.pet_name.as_str(), "Speedy");
    assert_eq!(cat.plants.len(), 8);
    assert_eq!(cat.coins, 228);
}

#[test]
fn migrates_largest_possible_v0_save() {
    // Written by the v0 encoder itself: the biggest save it could produce
    // (74 plants with every other list full, 20,374 bytes, 5 sectors).
    let cat = migrate_fixture("v0_maxed.json");
    let mut want = maxed_context();
    want.plants.truncate(74);
    // v0 predates the feather ball.
    want.toys.retain(|t| t.variant != ToyVariant::FeatherBall);
    assert_same_cat(&want, &cat, PLANT_TOL);
}

#[test]
fn feather_ball_round_trips_as_its_own_toy() {
    let mut ctx = fresh();
    ctx.toys.clear();
    assert!(ctx.add_toy(ToyVariant::Ball));
    assert!(ctx.add_toy(ToyVariant::FeatherBall));
    ctx.fav_toy = Some(ToyVariant::FeatherBall);
    ctx.least_fav_toy = Some(ToyVariant::Ball);
    let mut back = fresh();
    decode(&encode(&ctx), &mut back);
    let toys: Vec<_> = back.toys.iter().map(|t| t.variant).collect();
    assert_eq!(toys, [ToyVariant::Ball, ToyVariant::FeatherBall]);
    assert_eq!(back.fav_toy, Some(ToyVariant::FeatherBall));
    assert_eq!(back.least_fav_toy, Some(ToyVariant::Ball));
}

#[test]
fn newer_v1_wins_over_kept_v0_backup() {
    let _g = CwdGuard::enter();
    fs::copy(fixture("v0_typical.json"), storage::LEGACY_PATH).unwrap();
    let mut ctx = fresh();
    assert_eq!(save::load(&mut ctx), LoadOutcome::Loaded);
    ctx.coins = 4242;
    assert!(save::save(&mut ctx));

    let mut back = fresh();
    assert_eq!(save::load(&mut back), LoadOutcome::Loaded);
    assert_eq!(back.coins, 4242);
}

/// Offset in the image of the newest snapshot's payload.
fn newest_payload_offset() -> (usize, ring::Snap) {
    let mut flash = MemFlash::from_bytes(fs::read(storage::IMAGE_PATH).unwrap());
    let scan = ring::scan(&mut flash);
    let s = scan.snaps[0];
    (s.slot * ring::SLOT_SIZE + ring::HEADER_LEN, s)
}

#[test]
fn damaged_world_falls_back_to_older_snapshot_but_keeps_newest_pet() {
    let _g = CwdGuard::enter();
    let mut ctx = maxed_context();
    assert!(save::save(&mut ctx));
    let older_plants = ctx.plants.len();

    ctx.coins = 1;
    ctx.plants.truncate(3);
    assert!(save::save(&mut ctx));

    // Flip a byte inside the newest snapshot's WORLD body.
    let (off, _) = newest_payload_offset();
    let mut image = fs::read(storage::IMAGE_PATH).unwrap();
    let pet_len = u16::from_le_bytes([image[off + 1], image[off + 2]]) as usize;
    image[off + 5 + pet_len + 5 + 20] ^= 0x40;
    fs::write(storage::IMAGE_PATH, &image).unwrap();

    let mut back = fresh();
    assert_eq!(save::load(&mut back), LoadOutcome::Loaded);
    assert_eq!(back.coins, 1, "pet from the newest snapshot");
    assert_eq!(back.plants.len(), older_plants, "world from the older snapshot");
}

#[test]
fn damaged_pet_falls_back_to_older_snapshot() {
    let _g = CwdGuard::enter();
    let mut ctx = maxed_context();
    ctx.coins = 10;
    assert!(save::save(&mut ctx));
    ctx.coins = 20;
    assert!(save::save(&mut ctx));

    let (off, _) = newest_payload_offset();
    let mut image = fs::read(storage::IMAGE_PATH).unwrap();
    image[off + 5 + 3] ^= 0x01;
    fs::write(storage::IMAGE_PATH, &image).unwrap();

    let mut back = fresh();
    assert_eq!(save::load(&mut back), LoadOutcome::Loaded);
    assert_eq!(back.coins, 10, "pet restored from the last good snapshot");
}
