//! Platform shims. On firmware (`c6`/`c3` features), these re-export the
//! corresponding esp-hal / esp-println / esp-storage / esp-radio surface. On
//! the `desktop` feature, they swap in std-backed implementations so the same
//! game code can run in a desktop simulator and in host-side unit tests.
//!
//! Code inside this crate must never `use esp_hal::...` or `use
//! esp_println::...` directly. Go through these submodules so the desktop
//! build doesn't break.

pub mod persist;
pub mod power;
pub mod radio;
pub mod rng;
pub mod time;
pub mod watchdog;
