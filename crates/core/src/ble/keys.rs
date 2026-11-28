//! What the Remote sends: which key each button presses in each mode, and
//! the HID report bytes for it. Pure, so it's tested on the host.

use crate::input::Button;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// A takes a photo (volume up, which phone camera apps treat as the
    /// shutter).
    Camera,
    /// Next and previous slide, blank screen, start the show.
    Slides,
    /// Play/pause, tracks and volume.
    Media,
}

pub const MODES: [Mode; 3] = [Mode::Camera, Mode::Slides, Mode::Media];

/// One key press on one of the two input reports.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// A keyboard usage (HID usage page 0x07).
    Keyboard(u8),
    /// A consumer-control usage (HID usage page 0x0C).
    Consumer(u16),
}

pub mod usage {
    // Keyboard page.
    pub const B: u8 = 0x05;
    pub const F5: u8 = 0x3E;
    pub const PAGE_UP: u8 = 0x4B;
    pub const PAGE_DOWN: u8 = 0x4E;
    // Consumer page.
    pub const NEXT_TRACK: u16 = 0xB5;
    pub const PREV_TRACK: u16 = 0xB6;
    pub const PLAY_PAUSE: u16 = 0xCD;
    pub const MUTE: u16 = 0xE2;
    pub const VOLUME_UP: u16 = 0xE9;
    pub const VOLUME_DOWN: u16 = 0xEA;
}

/// The key a button sends in a mode, if any.
pub fn key_for(mode: Mode, button: Button) -> Option<Key> {
    use usage::*;
    use Key::{Consumer, Keyboard};
    match (mode, button) {
        (Mode::Camera, Button::A) => Some(Consumer(VOLUME_UP)),
        (Mode::Camera, Button::B) => Some(Consumer(VOLUME_DOWN)),
        (Mode::Slides, Button::A | Button::Right) => Some(Keyboard(PAGE_DOWN)),
        (Mode::Slides, Button::B | Button::Left) => Some(Keyboard(PAGE_UP)),
        (Mode::Slides, Button::Down) => Some(Keyboard(B)),
        (Mode::Slides, Button::Up) => Some(Keyboard(F5)),
        (Mode::Media, Button::A) => Some(Consumer(PLAY_PAUSE)),
        (Mode::Media, Button::B) => Some(Consumer(MUTE)),
        (Mode::Media, Button::Left) => Some(Consumer(PREV_TRACK)),
        (Mode::Media, Button::Right) => Some(Consumer(NEXT_TRACK)),
        (Mode::Media, Button::Up) => Some(Consumer(VOLUME_UP)),
        (Mode::Media, Button::Down) => Some(Consumer(VOLUME_DOWN)),
        _ => None,
    }
}

/// Buttons the Remote reads for keys. Menu1/Menu2 stay with the game.
pub const KEY_BUTTONS: [Button; 6] =
    [Button::A, Button::B, Button::Up, Button::Down, Button::Left, Button::Right];

pub const KEYBOARD_REPORT_ID: u8 = 1;
pub const CONSUMER_REPORT_ID: u8 = 2;

/// Keyboard input report: modifiers, reserved, six key slots.
pub type KeyboardReport = [u8; 8];
/// Consumer input report: one 16-bit usage, little-endian.
pub type ConsumerReport = [u8; 2];

pub const KEYBOARD_RELEASE: KeyboardReport = [0; 8];
pub const CONSUMER_RELEASE: ConsumerReport = [0; 2];

pub fn keyboard_report(usage: u8) -> KeyboardReport {
    [0, 0, usage, 0, 0, 0, 0, 0]
}

pub fn consumer_report(usage: u16) -> ConsumerReport {
    usage.to_le_bytes()
}

pub const REPORT_MAP_LEN: usize = 72;

/// HID report descriptor: a boot-style keyboard (report 1) and a consumer
/// control (report 2). No LED output report; nothing here has caps lock.
pub const REPORT_MAP: [u8; REPORT_MAP_LEN] = [
    // Keyboard.
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x06, // Usage (Keyboard)
    0xA1, 0x01, // Collection (Application)
    0x85, KEYBOARD_REPORT_ID, // Report ID
    0x05, 0x07, //   Usage Page (Keyboard)
    0x19, 0xE0, 0x29, 0xE7, // Usage Min/Max (modifiers)
    0x15, 0x00, 0x25, 0x01, // Logical 0..1
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, // 8 x 1 bit, Input (Data, Var, Abs)
    0x95, 0x01, 0x75, 0x08, 0x81, 0x01, // 1 x 8 bit, Input (Const): reserved
    0x95, 0x06, 0x75, 0x08, // 6 x 8 bit
    0x15, 0x00, 0x25, 0x65, // Logical 0..101
    0x05, 0x07, 0x19, 0x00, 0x29, 0x65, // Usage Min/Max (keys)
    0x81, 0x00, // Input (Data, Array)
    0xC0, // End Collection
    // Consumer control.
    0x05, 0x0C, // Usage Page (Consumer)
    0x09, 0x01, // Usage (Consumer Control)
    0xA1, 0x01, // Collection (Application)
    0x85, CONSUMER_REPORT_ID, // Report ID
    0x15, 0x00, 0x26, 0xFF, 0x03, // Logical 0..1023
    0x19, 0x00, 0x2A, 0xFF, 0x03, // Usage Min/Max
    0x75, 0x10, 0x95, 0x01, // 1 x 16 bit
    0x81, 0x00, // Input (Data, Array)
    0xC0, // End Collection
];
