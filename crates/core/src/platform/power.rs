//! Software reset / process exit and light sleep. Firmware drops into the
//! chip's reset path; desktop exits the process.

use crate::platform::time::Instant;

#[cfg(not(feature = "desktop"))]
pub fn software_reset() -> ! {
    esp_hal::system::software_reset()
}

#[cfg(feature = "desktop")]
pub fn software_reset() -> ! {
    std::process::exit(0)
}

/// Put the chip into light sleep until `deadline` or until an armed GPIO
/// (see `Buttons::arm_wakeup`) fires. RAM and peripheral state are kept and
/// esp-hal advances the system timer by the time slept, so `Instant` stays
/// correct. May return early if a wake source is already asserted.
///
/// The radio must be down (`ctx.radio_users == 0`): light sleep stalls it.
#[cfg(not(feature = "desktop"))]
pub fn light_sleep_until(deadline: Instant) {
    use esp_hal::{peripherals::LPWR, rtc_cntl::sleep::LowPower};

    if deadline <= Instant::now() {
        return;
    }
    // Safety: `LowPower` only touches the PMU/LP timer registers, and
    // nothing else in the firmware holds LPWR (deep sleep steals it too).
    let mut lp = LowPower::new(unsafe { LPWR::steal() });
    lp.set_wakeup_deadline(deadline);
    lp.sleep_light(sleep_config());
    lp.clear_wakeup_deadline();
}

/// Light-sleep power domains. esp-hal's default already stops the XTAL and
/// fast RC and drops the core to 0.6 V. On the C6 we also power off the
/// 32 kHz RC, which is not the slow clock (RC_SLOW is).
///
/// Don't power off the modem domain (`set_pd_modem(true)`): tried, and wifi
/// stopped working after the first light sleep, even though the radio was
/// down for the sleep and brought up again afterwards.
///
/// TODO(c3): the C3 keeps esp-hal's default. Its equivalent would be
/// `set_wifi_pd_en(true)` + `set_bt_pd_en(true)`, untested on hardware.
#[cfg(all(not(feature = "desktop"), feature = "c6"))]
fn sleep_config() -> esp_hal::rtc_cntl::sleep::RtcSleepConfig {
    let mut cfg = esp_hal::rtc_cntl::sleep::RtcSleepConfig::default();
    cfg.pd_flags.set_pd_rc32k(true);
    cfg
}

#[cfg(all(not(feature = "desktop"), not(feature = "c6")))]
fn sleep_config() -> esp_hal::rtc_cntl::sleep::RtcSleepConfig {
    esp_hal::rtc_cntl::sleep::RtcSleepConfig::default()
}

#[cfg(feature = "desktop")]
pub fn light_sleep_until(deadline: Instant) {
    crate::platform::time::delay_until(deadline);
}
