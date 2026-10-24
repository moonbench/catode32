//! BLE advert encoding, address derivation and interaction with the location filter.

use catode32_core::ble_filter::filter_reports;
use catode32_core::ble_proto::{
    address_for, encode_advert, id_from_address, parse_advert, short_id, ADVERT_MAX,
    ADV_PAYLOAD_MAX,
};
use catode32_core::transport::PeerId;

#[test]
fn advert_round_trips() {
    let id = PeerId::from_short(0xABCD);
    let mut buf = [0u8; ADVERT_MAX];
    let n = encode_advert(id, 7, b"hi  \x03Bob", &mut buf).unwrap();
    let (pid, seq, payload) = parse_advert(&buf[..n]).unwrap();
    assert_eq!(pid, id);
    assert_eq!(seq, 7);
    assert_eq!(payload, b"hi  \x03Bob");
}

#[test]
fn advert_fits_legacy_pdu() {
    let mut buf = [0u8; ADVERT_MAX];
    let n = encode_advert(PeerId::from_short(1), 0, &[0u8; ADV_PAYLOAD_MAX], &mut buf).unwrap();
    assert_eq!(n, ADVERT_MAX);
    assert!(encode_advert(PeerId::from_short(1), 0, &[0u8; ADV_PAYLOAD_MAX + 1], &mut buf).is_none());
}

#[test]
fn foreign_adverts_are_not_ours() {
    assert!(parse_advert(&[2, 0x01, 0x06]).is_none());
    assert!(parse_advert(&[4, 0xFF, 0x4C, 0x00, 0x01]).is_none());
    assert!(parse_advert(&[]).is_none());
}

#[test]
fn location_filter_ignores_our_adverts() {
    let mut buf = [0u8; ADVERT_MAX];
    let n = encode_advert(PeerId::from_short(0x0102), 0, &[], &mut buf).unwrap();
    let r = [(0u8, [1, 2, 3, 4, 5, 0x10], -40i8, &buf[..n])];
    assert!(filter_reports(r).is_empty());
}

#[test]
fn address_round_trips_and_is_static_random() {
    let a = address_for(0xBEEF);
    assert_eq!(a[5] & 0xC0, 0xC0);
    assert_eq!(id_from_address(&a), Some(0xBEEF));
    assert_eq!(id_from_address(&[1, 2, 3, 4, 5, 0xC0]), None);
    assert_eq!(id_from_address(&[1, 2, 3]), None);
}

#[test]
fn short_id_matches_peer_id() {
    assert_eq!(short_id(PeerId::from_short(0x1234)), 0x1234);
}
