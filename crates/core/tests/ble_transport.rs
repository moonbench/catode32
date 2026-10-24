//! Radio-independent BLE transport state: advert dedupe, outbox and link requests.

use catode32_core::ble_proto;
use catode32_core::ble_transport::Shared;
use catode32_core::transport::{PeerId, INBOX_CAPACITY};

fn advert_from(id: u16, seq: u8, payload: &[u8]) -> ([u8; 31], usize) {
    let mut b = [0u8; 31];
    let n = ble_proto::encode_advert(PeerId::from_short(id), seq, payload, &mut b).unwrap();
    (b, n)
}

#[test]
fn broadcast_adverts_are_delivered_once() {
    let mut me = Shared::new(1);
    let (b, n) = advert_from(2, 5, b"hi  \x03Bob");
    me.ingest_advert(&b[..n]);
    me.ingest_advert(&b[..n]);
    let got = me.drain();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].src, PeerId::from_short(2));
    assert_eq!(&got[0].data[..], b"hi  \x03Bob");

    let (b, n) = advert_from(2, 6, b"hi  \x03Bob");
    me.ingest_advert(&b[..n]);
    assert_eq!(me.drain().len(), 1);
}

#[test]
fn own_and_empty_adverts_are_ignored() {
    let mut me = Shared::new(1);
    let (b, n) = advert_from(1, 1, b"hi  ");
    me.ingest_advert(&b[..n]);
    let (b, n) = advert_from(2, 1, b"");
    me.ingest_advert(&b[..n]);
    assert!(me.drain().is_empty());
}

#[test]
fn set_broadcast_bumps_sequence() {
    let mut me = Shared::new(7);
    assert!(me.set_broadcast(b"voc "));
    let mut a = [0u8; 31];
    let n = me.advert(&mut a);
    let mut peer = Shared::new(8);
    peer.ingest_advert(&a[..n]);
    assert_eq!(peer.drain().len(), 1);
    assert!(me.set_broadcast(b"voc "));
    let n = me.advert(&mut a);
    peer.ingest_advert(&a[..n]);
    assert_eq!(peer.drain().len(), 1);
    assert!(!me.set_broadcast(&[0u8; 21]));
}

#[test]
fn send_to_requests_a_link_once() {
    let mut me = Shared::new(1);
    assert!(me.queue(9, b"vreq"));
    assert_eq!(me.want, Some(9));
    assert!(me.queue(9, b"vst "));
    assert_eq!(me.take_outbox(9).unwrap().as_slice(), b"vreq");
    assert_eq!(me.take_outbox(9).unwrap().as_slice(), b"vst ");
    assert!(me.take_outbox(9).is_none());
}

#[test]
fn send_over_existing_link_does_not_want() {
    let mut me = Shared::new(1);
    me.link = Some(9);
    me.queue(9, b"vok ");
    assert_eq!(me.want, None);
    assert!(me.has_outbox(9));
    me.drop_outbox(9);
    assert!(!me.has_outbox(9));
}

#[test]
fn oversize_frames_are_rejected() {
    let mut me = Shared::new(1);
    assert!(!me.queue(2, &[0u8; 21]));
    assert_eq!(me.want, None);
}

#[test]
fn inbox_drops_oldest_when_full() {
    let mut me = Shared::new(1);
    for i in 0..(INBOX_CAPACITY as u8 + 2) {
        me.push_inbox(2, &[i]);
    }
    let got = me.drain();
    assert_eq!(got.len(), INBOX_CAPACITY);
    assert_eq!(got[0].data[0], 2);
}
