//! BLE location-scan filter: stable addresses, RSSI floor, dedup and cap.

use catode32_core::ble_filter::{filter_reports, ADDR_PUBLIC, ADDR_RANDOM, BLE_MAX_REPORTS};

const NONE: &[u8] = &[];

fn addr(last: u8, top: u8) -> [u8; 6] {
    [last, 0, 0, 0, 0, top]
}

#[test]
fn keeps_public_and_static_random_only() {
    let r = [
        (ADDR_PUBLIC, addr(1, 0x10), -50, NONE),
        (ADDR_RANDOM, addr(2, 0xC1), -50, NONE),
        (ADDR_RANDOM, addr(3, 0x41), -50, NONE),
        (ADDR_RANDOM, addr(4, 0x01), -50, NONE),
    ];
    let out = filter_reports(r);
    assert_eq!(out.len(), 2);
}

#[test]
fn drops_weak_and_catode_adverts() {
    let ours: &[u8] = &[4, 0xFF, 0xC3, 0x32, 0x01];
    let r = [
        (ADDR_PUBLIC, addr(1, 0x10), -90, NONE),
        (ADDR_PUBLIC, addr(2, 0x10), -40, ours),
        (ADDR_PUBLIC, addr(3, 0x10), -40, NONE),
    ];
    let out = filter_reports(r);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].addr[0], 3);
}

#[test]
fn dedups_keeping_strongest_and_reads_name() {
    let named: &[u8] = &[4, 0x09, b'T', b'V', b'1'];
    let r = [
        (ADDR_PUBLIC, addr(1, 0x10), -70, named),
        (ADDR_PUBLIC, addr(1, 0x10), -55, NONE),
    ];
    let out = filter_reports(r);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].rssi, -55);
    assert_eq!(out[0].name.as_str(), "TV1");
}

#[test]
fn caps_to_strongest_when_crowded() {
    let mut v: Vec<(u8, [u8; 6], i8, &[u8])> = Vec::new();
    for i in 0..40u8 {
        v.push((ADDR_PUBLIC, addr(i, 0x10), -84 + (i as i8 % 30), NONE));
    }
    let out = filter_reports(v);
    assert!(out.len() <= BLE_MAX_REPORTS);
    assert!(out.windows(2).all(|w| w[0].rssi >= w[1].rssi));
}
