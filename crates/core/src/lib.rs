#![cfg_attr(not(feature = "desktop"), no_std)]

extern crate alloc;

// Cfg-aware `println!`. Firmware routes to `esp_println` (USB serial-jtag /
// UART per the `auto` backend); desktop uses the std macro. Call sites do
// `use crate::println;` and then `println!(...)` as normal.
#[cfg(not(feature = "desktop"))]
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => { ::esp_println::println!($($arg)*) };
}

#[cfg(feature = "desktop")]
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => { ::std::println!($($arg)*) };
}

pub mod platform;

mod macros;

/// Compile-time translation lookup; see `catode32_i18n_macros::t`.
pub use catode32_i18n_macros::t;

pub mod assets;
pub mod i18n;
pub mod behavior;
pub mod behaviors;
pub mod board;
pub mod catch_up;
pub mod character;
pub mod clock;
pub mod context;
pub mod entities;
pub mod environment;
pub mod peer_manager;
pub mod peer_msg;
pub mod game;
pub mod gardening_ui;
pub mod input;
pub mod led;
pub mod location_scene;
pub mod pet_names;
pub mod pet_seed;
pub mod plant_renderer;
pub mod plant_system;
pub mod radio;
pub mod rand;
pub mod render;
pub mod save;
pub mod scene;
pub mod scenes;
pub mod sky;
pub mod sleep_manager;
#[cfg(not(feature = "desktop"))]
pub mod sleep_pins;
pub mod storage;
pub mod temperature_system;
pub mod time_system;
pub mod transition;
pub mod ble_filter;
pub mod ble_proto;
pub mod ble_transport;
pub mod transport;
pub mod ui;
pub mod weather_system;
pub mod location_tracker;
