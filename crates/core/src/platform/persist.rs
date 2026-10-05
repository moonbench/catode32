//! State that survives a reset without touching flash, plus the hardware
//! reset reason.
//!
//! The scene-change intent lives in RTC fast RAM. esp-hal zeroes that
//! section only on a chip power-on, so the record survives panics,
//! watchdog resets, software resets and (depending on how the chip reports
//! it) brownouts. Writes are plain SRAM stores: no flash wear, so it is safe
//! to set on every scene change. A magic word and checksum reject garbage
//! left behind by a voltage dip.
//!
//! Desktop keeps the record in an ordinary static, which is lost on process
//! exit, so desktop never resumes an intent.

const MAGIC: u32 = 0x494E_5431; // "INT1"

fn checksum(scene: u32, attempts: u32) -> u32 {
    (MAGIC ^ scene.rotate_left(8) ^ attempts.rotate_left(16)).wrapping_mul(0x9E37_79B1)
}

#[cfg(not(feature = "desktop"))]
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut BOOT_INTENT: [u32; 4] = [0; 4];

#[cfg(feature = "desktop")]
static mut BOOT_INTENT: [u32; 4] = [0; 4];

fn write(record: [u32; 4]) {
    // SAFETY: single-threaded game loop is the only accessor.
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(BOOT_INTENT), record) }
}

fn read() -> [u32; 4] {
    // SAFETY: see `write`.
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(BOOT_INTENT)) }
}

/// Record a scene change that is about to happen. Always resets the attempt
/// count to zero; only `intent_rearm` (the boot path) increments it.
pub fn intent_set(scene: u8) {
    intent_write(scene, 0);
}

/// Re-arm a resumed intent with an incremented attempt count.
pub fn intent_rearm(scene: u8, attempts: u8) {
    intent_write(scene, attempts);
}

fn intent_write(scene: u8, attempts: u8) {
    let (s, a) = (scene as u32, attempts as u32);
    write([MAGIC, s, a, checksum(s, a)]);
}

pub fn intent_clear() {
    write([0; 4]);
}

/// Return `(scene, attempts)` if a valid intent is recorded. Does not clear
/// it; the caller decides whether to re-arm or clear.
pub fn intent_peek() -> Option<(u8, u8)> {
    let [magic, scene, attempts, sum] = read();
    if magic != MAGIC
        || sum != checksum(scene, attempts)
        || scene > u8::MAX as u32
        || attempts > u8::MAX as u32
    {
        return None;
    }
    Some((scene as u8, attempts as u8))
}

/// Why the chip last reset, for logging and the debug screen, as
/// `(label, is_brownout)`. Read from a chip register; nothing is stored.
#[cfg(not(feature = "desktop"))]
pub fn reset_reason() -> (heapless::String<24>, bool) {
    use core::fmt::Write as _;
    use esp_hal::rtc_cntl::SocResetReason;

    let reason = esp_hal::system::reset_reason();
    let mut label = heapless::String::new();
    match reason {
        Some(r) => {
            let _ = write!(label, "{:?}", r);
        }
        None => {
            let _ = label.push_str("Unknown");
        }
    }
    (label, reason == Some(SocResetReason::SysBrownOut))
}

#[cfg(feature = "desktop")]
pub fn reset_reason() -> (heapless::String<24>, bool) {
    let mut label = heapless::String::new();
    let _ = label.push_str("desktop");
    (label, false)
}
