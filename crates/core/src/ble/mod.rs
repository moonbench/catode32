//! Bluetooth LE for the Remote tool: the device pairs as a keyboard and
//! remote control (HID over GATT) and sends key presses.
//!
//! On firmware the BLE host (TrouBLE) runs as an embassy task on its own
//! scheduler thread, started at boot (see the firmware `main`). The radio
//! is only powered while a session runs: [`start`] brings the controller
//! up and [`stop`] drops it again. The game loop never blocks on BLE; it
//! talks to the task through the functions here.
//!
//! Pairing keys are handed in at [`start`] and new ones come back through
//! [`take_new_bond`] for the game thread to store (see [`bonds`]); the BLE
//! task never touches flash.
//!
//! Desktop has no radio: [`status`] is always [`Status::Unavailable`].

pub mod bonds;
pub mod keys;

use heapless::{String, Vec};

use crate::pet_seed::{absorb, next_expanded};
pub use bonds::{BondRecord, MAX_BONDS};
pub use keys::{Key, Mode};

/// Longest device name, in bytes: TrouBLE's GAP Device Name limit (the
/// scan response itself would fit 29).
pub const NAME_MAX: usize = 22;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// No Bluetooth on this build (the desktop simulator).
    Unavailable,
    Off,
    /// Bringing the controller up.
    Starting,
    /// Waiting for a phone or computer to connect.
    Advertising,
    /// Connected; `secure` once the link is encrypted (paired), which is
    /// when keys actually reach the other side.
    Connected { secure: bool },
    /// The controller or host failed to start.
    Failed,
}

impl Status {
    /// The radio is in use, from start until fully stopped.
    pub fn is_active(self) -> bool {
        !matches!(self, Status::Off | Status::Unavailable)
    }
}

/// What a session needs to start.
pub struct StartConfig {
    pub name: String<NAME_MAX>,
    /// Our static random address; see [`static_address`].
    pub address: [u8; 6],
    pub bonds: Vec<BondRecord, MAX_BONDS>,
}

/// A static random BLE address derived from the chip, so it is the same
/// on every boot (phones match bonds by address). Bytes are little-endian
/// as on the air; the top two bits of the last byte must be set for a
/// static random address.
pub fn static_address(device_id: u64) -> [u8; 6] {
    let mut s = 0x424C_455F_4144_4452; // "BLE_ADDR"
    absorb(&mut s, device_id);
    let bits = next_expanded(&mut s).to_le_bytes();
    let mut addr = [0u8; 6];
    addr.copy_from_slice(&bits[..6]);
    addr[5] |= 0xC0;
    addr
}

/// The advertised name: the pet's name, or "Catode32" before there is one.
pub fn device_name(pet_name: &str) -> String<NAME_MAX> {
    let mut name = String::new();
    let source = if pet_name.trim().is_empty() { "Catode32" } else { pet_name.trim() };
    for c in source.chars() {
        if name.push(c).is_err() {
            break;
        }
    }
    name
}

#[cfg(not(feature = "desktop"))]
mod gatt;
#[cfg(not(feature = "desktop"))]
mod host;
#[cfg(not(feature = "desktop"))]
pub use host::{send, spawn, start, status, stop, take_new_bond};

#[cfg(feature = "desktop")]
mod desktop {
    use super::{BondRecord, Key, StartConfig, Status};

    pub fn status() -> Status {
        Status::Unavailable
    }
    pub fn start(_config: StartConfig) {}
    pub fn stop() {}
    pub fn send(_key: Key) -> bool {
        false
    }
    pub fn take_new_bond() -> Option<BondRecord> {
        None
    }
}

#[cfg(feature = "desktop")]
pub use desktop::{send, start, status, stop, take_new_bond};

/// Stop the session and wait (up to `timeout_ms`) for the radio to be
/// released, so WiFi can be used straight after. Feeds the watchdog.
pub fn stop_and_wait(timeout_ms: u64) -> bool {
    use crate::platform::{time, watchdog};
    stop();
    let deadline = time::now_ms() + timeout_ms;
    while status().is_active() {
        if time::now_ms() >= deadline {
            return false;
        }
        watchdog::feed();
        time::delay(time::Duration::from_millis(5));
    }
    true
}
