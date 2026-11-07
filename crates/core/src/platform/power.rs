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
#[cfg(not(feature = "desktop"))]
pub fn light_sleep_until(deadline: Instant) {
    use esp_hal::{
        peripherals::LPWR,
        rtc_cntl::sleep::{LowPower, RtcSleepConfig},
    };

    if deadline <= Instant::now() {
        return;
    }
    // Safety: `LowPower` only touches the PMU/LP timer registers, and
    // nothing else in the firmware holds LPWR (deep sleep steals it too).
    let mut lp = LowPower::new(unsafe { LPWR::steal() });
    lp.set_wakeup_deadline(deadline);
    lp.sleep_light(RtcSleepConfig::default());
    lp.clear_wakeup_deadline();
}

#[cfg(feature = "desktop")]
pub fn light_sleep_until(deadline: Instant) {
    crate::platform::time::delay_until(deadline);
}
