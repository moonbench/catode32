#![cfg_attr(not(feature = "desktop"), no_std)]

// The firmware's global allocator (esp-alloc) exists for the wifi stack.
// Core only allocates briefly at boot, to migrate a legacy save.
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
pub mod character;
pub mod clock;
pub mod context;
pub mod entities;
pub mod environment;
pub mod espnow_manager;
pub mod espnow_msg;
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
pub mod ring;
pub mod render;
pub mod save;
pub mod save_v1;
pub mod scene;
pub mod scenes;
pub mod sky;
pub mod sleep_manager;
pub mod storage;
pub mod temperature_system;
pub mod time_system;
pub mod transition;
pub mod ui;
pub mod weather_system;
pub mod wifi_tracker;
