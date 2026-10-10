//! Hardware watchdog. A hang anywhere (driver spin, infinite loop) resets the
//! chip instead of freezing the last frame on screen forever.
//!
//! Uses the RTC watchdog (RWDT). Its stage-0 action is a core reset: the main
//! system, CPU and radio are reset but the RTC domain is not, so the records
//! in [`super::persist`] survive. The reset reason reads back as a watchdog
//! reset, which shows on the Power debug screen.
//!
//! `feed` is three register writes (unlock, feed, relock), so calling it every
//! frame costs nothing measurable. Every loop that can legitimately run longer
//! than [`TIMEOUT`] without returning to the game loop must feed it too.
//!
//! The RWDT pauses during hardware sleep. Desktop builds have no watchdog and
//! every function is a no-op.

/// Longest the game loop may go without feeding before the chip resets. Must
/// clear the slowest legitimate blocking call (a wifi scan or radio init is
/// ~1-3 s, a save is a few flash erases) with a wide margin.
pub const TIMEOUT_SECS: u64 = 10;

#[cfg(not(feature = "desktop"))]
mod firmware {
    use esp_hal::{
        peripherals::RTC_TIMER,
        rtc_cntl::{Rtc, RwdtStage, RwdtStageAction},
        time::Duration,
    };

    use super::TIMEOUT_SECS;

    /// The RWDT registers are global and `Rtc` is a zero-sized wrapper, so
    /// stealing the peripheral per call costs nothing and spares threading a
    /// handle through every loop that has to feed.
    fn rtc() -> Rtc<'static> {
        // SAFETY: nothing else in the firmware holds RTC_TIMER, and esp-hal's
        // own sleep code steals it the same way.
        Rtc::new(unsafe { RTC_TIMER::steal() })
    }

    /// Arm the watchdog. Call once at boot, after `esp_hal::init` (which
    /// disables it).
    pub fn start() {
        let mut rtc = rtc();
        rtc.rwdt.enable();
        rtc.rwdt
            .set_stage_action(RwdtStage::Stage0, RwdtStageAction::ResetCore);
        rtc.rwdt
            .set_timeout(RwdtStage::Stage0, Duration::from_secs(TIMEOUT_SECS));
        rtc.rwdt.feed();
    }

    pub fn feed() {
        rtc().rwdt.feed();
    }

    /// Disarm before deep sleep. The RWDT pauses in sleep anyway; this just
    /// keeps a slow pad-hold or wake-arm sequence from tripping it.
    pub fn stop() {
        rtc().rwdt.disable();
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{feed, start, stop};

#[cfg(feature = "desktop")]
pub fn start() {}

#[cfg(feature = "desktop")]
pub fn feed() {}

#[cfg(feature = "desktop")]
pub fn stop() {}
