//! WiFi/radio handles. Firmware re-exports the real `esp_radio::wifi` types
//! and the `esp_hal::peripherals::WIFI` marker; desktop swaps in
//! never-constructed stubs so `GameContext` keeps the same shape. The
//! `crate::radio` and `crate::wifi_tracker` modules drop to no-op
//! implementations on desktop, so the stubs here are only there to satisfy
//! the type system; they're never touched at runtime.

#[cfg(not(any(feature = "desktop", feature = "h2")))]
pub use esp_hal::peripherals::WIFI;
#[cfg(not(any(feature = "desktop", feature = "h2")))]
pub use esp_radio::wifi::WifiController;

#[cfg(any(feature = "desktop", feature = "h2"))]
mod desktop {
    use core::marker::PhantomData;

    /// Stub for `esp_hal::peripherals::WIFI<'a>`. Desktop builds need a
    /// value to feed `Game::new`'s `wifi_peripheral` parameter, but the
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

#[cfg(any(feature = "desktop", feature = "h2"))]
pub use desktop::{WifiController, WIFI};
