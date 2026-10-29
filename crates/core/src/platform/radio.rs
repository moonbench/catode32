//! WiFi/radio handles. Firmware re-exports the real `esp_radio::wifi` types
//! and the `esp_hal::peripherals::WIFI` marker; desktop swaps in
//! never-constructed stubs so `GameContext` keeps the same shape. The
//! `crate::radio` and `crate::location_tracker` modules drop to no-op
//! implementations on desktop, so the stubs here are only there to satisfy
//! the type system; they're never touched at runtime.

#[cfg(not(any(feature = "desktop", feature = "h2")))]
pub use esp_hal::peripherals::WIFI;
/// On the H2 the "radio peripheral" slot holds the Bluetooth peripheral.
#[cfg(feature = "h2")]
pub use esp_hal::peripherals::BT as WIFI;
#[cfg(not(any(feature = "desktop", feature = "h2")))]
pub use esp_radio::wifi::WifiController;

#[cfg(feature = "desktop")]
mod desktop {
    use core::marker::PhantomData;

    /// Stub for `esp_hal::peripherals::WIFI<'a>`. Desktop builds need a
    /// value to feed `Game::new`'s `radio_peripheral` parameter, but the
    /// stub is never read. `radio::acquire` is a no-op on desktop.
    pub struct WIFI<'a>(PhantomData<&'a ()>);

    impl<'a> WIFI<'a> {
        pub fn new() -> Self {
            Self(PhantomData)
        }
    }

    impl<'a> Default for WIFI<'a> {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Stub for `esp_radio::wifi::WifiController<'a>`. Same story:
    /// `GameContext::wifi` is always `None` on desktop.
    pub struct WifiController<'a>(PhantomData<&'a ()>);
}

#[cfg(feature = "desktop")]
pub use desktop::{WifiController, WIFI};

#[cfg(feature = "h2")]
pub struct WifiController<'a>(core::marker::PhantomData<&'a ()>);
