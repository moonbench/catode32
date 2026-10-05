//! Refcounted lifecycle for the shared WiFi/ESP-NOW radio.
//!
//! esp-radio 0.18 does not expose a `WifiController::stop`. The only way
//! to truly power the WiFi MAC down is to drop the controller. This
//! module owns that drop/recreate dance so the rest of the codebase can
//! request the radio without thinking about it.
//!
//! At rest, the WiFi peripheral handle sits in `ctx.wifi_peripheral` and
//! the radio is fully off. The first call to [`acquire`] consumes the
//! peripheral via `esp_radio::wifi::new`, parks the new controller into
//! `ctx.wifi`, and binds the freshly-minted ESP-NOW handle onto
//! `ctx.espnow`. Each subsequent caller just bumps the refcount.
//! [`release`] decrements; on the last release the controller and
//! ESP-NOW handle are dropped (which de-inits the WiFi MAC) and the
//! peripheral is recovered via `unsafe { WIFI::steal() }` for the next
//! cycle.
//!
//! Desktop builds have no radio. `acquire` always returns `false` and
//! `release` is a no-op. Code that handles the no-radio case (the
//! "you've gone offline" branch) keeps working unchanged.

#[cfg(feature = "desktop")]
use crate::context::GameContext;

#[cfg(not(feature = "desktop"))]
mod firmware {
    use esp_hal::peripherals::WIFI;

    use crate::context::GameContext;

    /// Bring the radio up (or just bump the refcount). Returns `true` if
    /// the radio is usable on exit, `false` if init failed.
    pub fn acquire(ctx: &mut GameContext) -> bool {
        if ctx.radio_users == 0 {
            if !init_radio(ctx) {
                return false;
            }
            if let Some(espnow) = ctx.espnow.as_mut() {
                espnow.activate();
            }
        }
        ctx.radio_users = ctx.radio_users.saturating_add(1);
        true
    }

    /// Decrement the refcount and tear the radio down on the last release.
    pub fn release(ctx: &mut GameContext) {
        if ctx.radio_users == 0 {
            println!("[radio] unbalanced release");
            return;
        }
        ctx.radio_users -= 1;
        if ctx.radio_users == 0 {
            if let Some(espnow) = ctx.espnow.as_mut() {
                espnow.deactivate();
            }
            teardown_radio(ctx);
        }
    }

    fn init_radio(ctx: &mut GameContext) -> bool {
        let Some(peripheral) = ctx.wifi_peripheral.take() else {
            println!("[radio] no wifi peripheral available");
            return false;
        };
        match esp_radio::wifi::new(peripheral, Default::default()) {
            Ok((controller, interfaces)) => {
                let mac = interfaces.station.mac_address();
                ctx.wifi = Some(controller);
                if let Some(espnow) = ctx.espnow.as_mut() {
                    espnow.attach(interfaces.esp_now, mac);
                }
                true
            }
            Err(e) => {
                println!("[radio] wifi::new failed: {:?}", e);
                ctx.wifi_peripheral = Some(unsafe { WIFI::steal() });
                false
            }
        }
    }

    fn teardown_radio(ctx: &mut GameContext) {
        if let Some(espnow) = ctx.espnow.as_mut() {
            drop(espnow.detach());
        }
        drop(ctx.wifi.take());
        ctx.wifi_peripheral = Some(unsafe { WIFI::steal() });
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{acquire, release};

#[cfg(feature = "desktop")]
pub fn acquire(_ctx: &mut GameContext) -> bool {
    false
}

#[cfg(feature = "desktop")]
pub fn release(_ctx: &mut GameContext) {}
