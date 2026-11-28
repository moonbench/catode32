//! Bluetooth Remote mechanics that don't need a radio: key mapping, HID
//! report bytes, the bond store, and the desktop stub.

#![cfg(feature = "desktop")]

use catode32_core::{
    ble::{
        self,
        bonds::{self, BondRecord, MAX_BONDS},
        keys::{self, usage, Key, Mode},
        Status,
    },
    context::GameContext,
    input::{Button, ButtonPin, Buttons},
    led::Led,
    render::Renderer,
    ring::{Flash, SECTOR_SIZE},
    scene::Scene,
    scenes::remote_scene::RemoteScene,
};
use heapless::Vec;

/// One erasable sector with NOR write semantics (writes can only clear bits).
struct FakeFlash {
    data: [u8; SECTOR_SIZE],
    fail_writes: bool,
}

impl FakeFlash {
    fn new() -> Self {
        Self { data: [0xFF; SECTOR_SIZE], fail_writes: false }
    }
}

impl Flash for FakeFlash {
    fn sectors(&self) -> usize {
        1
    }
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> bool {
        buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
        true
    }
    fn write(&mut self, offset: usize, data: &[u8]) -> bool {
        if self.fail_writes {
            return false;
        }
        for (d, s) in self.data[offset..offset + data.len()].iter_mut().zip(data) {
            *d &= *s;
        }
        true
    }
    fn erase(&mut self, sector: usize) -> bool {
        assert_eq!(sector, 0);
        self.data = [0xFF; SECTOR_SIZE];
        true
    }
}

fn bond(n: u8) -> BondRecord {
    BondRecord {
        addr_kind: n % 2,
        addr: [n, 1, 2, 3, 4, 0xC0 | n],
        irk: if n % 3 == 0 { None } else { Some(0x1234_5678_9ABC_DEF0_0000_0000_0000_0000 | n as u128) },
        ltk: 0xFEED_FACE_0000_0000_0000_0000_0000_0000 | n as u128,
        authenticated: n % 2 == 0,
    }
}

// --- Keys ----------------------------------------------------------------

#[test]
fn camera_shutter_is_volume_up() {
    assert_eq!(keys::key_for(Mode::Camera, Button::A), Some(Key::Consumer(usage::VOLUME_UP)));
    assert_eq!(keys::key_for(Mode::Camera, Button::B), Some(Key::Consumer(usage::VOLUME_DOWN)));
}

#[test]
fn slides_step_both_ways() {
    for b in [Button::A, Button::Right] {
        assert_eq!(keys::key_for(Mode::Slides, b), Some(Key::Keyboard(usage::PAGE_DOWN)));
    }
    for b in [Button::B, Button::Left] {
        assert_eq!(keys::key_for(Mode::Slides, b), Some(Key::Keyboard(usage::PAGE_UP)));
    }
}

#[test]
fn media_covers_every_key_button() {
    for b in keys::KEY_BUTTONS {
        assert!(keys::key_for(Mode::Media, b).is_some(), "{b:?}");
    }
}

#[test]
fn menu_buttons_never_send_keys() {
    for mode in keys::MODES {
        assert_eq!(keys::key_for(mode, Button::Menu1), None);
        assert_eq!(keys::key_for(mode, Button::Menu2), None);
    }
}

#[test]
fn reports_encode_press_and_release() {
    assert_eq!(keys::keyboard_report(usage::PAGE_DOWN), [0, 0, 0x4E, 0, 0, 0, 0, 0]);
    assert_eq!(keys::consumer_report(usage::VOLUME_UP), [0xE9, 0x00]);
    assert_eq!(keys::consumer_report(0x0123), [0x23, 0x01]);
    assert_eq!(keys::KEYBOARD_RELEASE, [0; 8]);
    assert_eq!(keys::CONSUMER_RELEASE, [0; 2]);
}

#[test]
fn report_map_declares_both_reports() {
    let map = &keys::REPORT_MAP;
    // Report ID items (0x85) for each report, and balanced collections.
    let ids: std::vec::Vec<u8> = map.windows(2).filter(|w| w[0] == 0x85).map(|w| w[1]).collect();
    assert_eq!(ids, [keys::KEYBOARD_REPORT_ID, keys::CONSUMER_REPORT_ID]);
    let opens = map.windows(2).filter(|w| w[0] == 0xA1).count();
    let closes = map.iter().filter(|&&b| b == 0xC0).count();
    assert_eq!(opens, 2);
    assert_eq!(closes, 2);
    assert_eq!(*map.last().unwrap(), 0xC0);
}

// --- Bonds ---------------------------------------------------------------

#[test]
fn bonds_round_trip_through_flash() {
    let mut flash = FakeFlash::new();
    assert!(bonds::load(&mut flash).is_empty(), "blank flash has no bonds");
    let list = [bond(1), bond(2), bond(3)];
    assert!(bonds::store(&mut flash, &list));
    assert_eq!(bonds::load(&mut flash).as_slice(), &list);
    // Rewriting replaces rather than appending.
    assert!(bonds::store(&mut flash, &[bond(4)]));
    assert_eq!(bonds::load(&mut flash).as_slice(), &[bond(4)]);
}

#[test]
fn forgetting_leaves_no_bonds() {
    let mut flash = FakeFlash::new();
    bonds::store(&mut flash, &[bond(1), bond(2)]);
    assert!(bonds::store(&mut flash, &[]));
    assert!(bonds::load(&mut flash).is_empty());
}

#[test]
fn damaged_bond_records_read_as_none() {
    let mut flash = FakeFlash::new();
    bonds::store(&mut flash, &[bond(1), bond(2)]);
    // Flip one key byte: the CRC no longer matches.
    flash.data[30] ^= 0x01;
    assert!(bonds::load(&mut flash).is_empty());

    let mut flash = FakeFlash::new();
    bonds::store(&mut flash, &[bond(1)]);
    flash.data[0] = b'X'; // magic
    assert!(bonds::load(&mut flash).is_empty());

    let mut flash = FakeFlash::new();
    bonds::store(&mut flash, &[bond(1)]);
    flash.data[5] = (MAX_BONDS + 1) as u8; // impossible count
    assert!(bonds::load(&mut flash).is_empty());
}

#[test]
fn failed_write_is_reported() {
    let mut flash = FakeFlash::new();
    flash.fail_writes = true;
    assert!(!bonds::store(&mut flash, &[bond(1)]));
}

#[test]
fn newest_bond_first_and_oldest_dropped() {
    let mut list: Vec<BondRecord, MAX_BONDS> = Vec::new();
    for n in 1..=MAX_BONDS as u8 {
        bonds::remember(&mut list, bond(n));
    }
    assert_eq!(list[0], bond(MAX_BONDS as u8));
    // One more pushes out the oldest (bond 1).
    bonds::remember(&mut list, bond(9));
    assert_eq!(list.len(), MAX_BONDS);
    assert_eq!(list[0], bond(9));
    assert!(!list.contains(&bond(1)));
    // Re-pairing a known peer replaces its entry and moves it to the front.
    let mut again = bond(3);
    again.ltk = 42;
    bonds::remember(&mut list, again);
    assert_eq!(list.len(), MAX_BONDS);
    assert_eq!(list[0], again);
    assert_eq!(list.iter().filter(|b| b.addr == again.addr).count(), 1);
}

// --- Address and name ------------------------------------------------------

#[test]
fn static_address_is_stable_and_marked_static() {
    let a = ble::static_address(0x1122_3344_5566);
    assert_eq!(a, ble::static_address(0x1122_3344_5566));
    assert_ne!(a, ble::static_address(0x1122_3344_5567));
    assert_eq!(a[5] & 0xC0, 0xC0, "top two bits set for a static random address");
}

#[test]
fn device_name_falls_back_and_fits() {
    assert_eq!(ble::device_name("Mittens").as_str(), "Mittens");
    assert_eq!(ble::device_name("  ").as_str(), "Catode32");
    let long = "A name far too long to fit in a scan response";
    assert_eq!(ble::device_name(long).len(), ble::NAME_MAX);
}

// --- Desktop -------------------------------------------------------------

#[test]
fn desktop_has_no_bluetooth() {
    assert_eq!(ble::status(), Status::Unavailable);
    assert!(!ble::status().is_active());
    assert!(!ble::send(Key::Consumer(usage::VOLUME_UP)));
    assert!(ble::stop_and_wait(10));
}

#[test]
fn remote_scene_runs_without_holding_the_device_awake() {
    let mut ctx = GameContext::new(Led::new());
    let mut scene = RemoteScene::new();
    let mut b = Buttons::new([0, 1, 2, 3, 4, 5, 6, 7].map(ButtonPin::new));
    let mut r = Renderer::new();
    scene.enter(&mut ctx);
    for _ in 0..10 {
        assert!(scene.update(&mut ctx, &mut b, 1.0 / 12.0).is_none());
        r.clear();
        scene.draw(&ctx, &mut r, 83);
    }
    assert!(!ctx.keep_awake);
    scene.exit(&mut ctx);
    assert!(!ctx.keep_awake);
}

#[test]
fn key_guide_matches_what_each_button_sends() {
    use catode32_core::scenes::remote_scene::key_rows;
    for mode in keys::MODES {
        let shown: std::vec::Vec<Button> =
            key_rows(mode).iter().flatten().filter_map(|c| c.map(|(b, _)| b)).collect();
        for b in keys::KEY_BUTTONS {
            assert_eq!(
                shown.contains(&b),
                keys::key_for(mode, b).is_some(),
                "{mode:?} {b:?}: guide and key map disagree"
            );
        }
        let mut unique = shown.clone();
        unique.sort_by_key(|b| *b as u8);
        unique.dedup();
        assert_eq!(unique.len(), shown.len(), "{mode:?} lists a button twice");
    }
}
