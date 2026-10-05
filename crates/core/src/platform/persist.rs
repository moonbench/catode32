//! State that survives a reset without touching flash, plus the hardware
//! reset reason.
//!
//! Two records live in RTC fast RAM: the scene-change intent and the last
//! panic (see [`crash_write`]). esp-hal zeroes that
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

const CRASH_MAGIC: u32 = 0x4352_5348; // "CRSH"

/// Bytes of panic text kept. Six 21-column lines on the debug screen.
pub const CRASH_MSG_MAX: usize = 124;
/// Return addresses kept from the panic backtrace, innermost first.
pub const CRASH_PCS_MAX: usize = 4;

const CRASH_HEAD_WORDS: usize = 3 + CRASH_PCS_MAX;

/// Header is `[magic, msg_len, pc_count, pcs.., checksum]`. The message is a
/// separate byte array so it can be filled without packing.
#[cfg(not(feature = "desktop"))]
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut CRASH_HEAD: [u32; CRASH_HEAD_WORDS + 1] = [0; CRASH_HEAD_WORDS + 1];
#[cfg(not(feature = "desktop"))]
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut CRASH_MSG: [u8; CRASH_MSG_MAX] = [0; CRASH_MSG_MAX];

#[cfg(feature = "desktop")]
static mut CRASH_HEAD: [u32; CRASH_HEAD_WORDS + 1] = [0; CRASH_HEAD_WORDS + 1];
#[cfg(feature = "desktop")]
static mut CRASH_MSG: [u8; CRASH_MSG_MAX] = [0; CRASH_MSG_MAX];

/// The panic that caused the previous reset.
pub struct CrashRecord {
    pub msg: heapless::String<CRASH_MSG_MAX>,
    pub pcs: heapless::Vec<u32, CRASH_PCS_MAX>,
}

fn crash_checksum(head: &[u32], msg: &[u8]) -> u32 {
    let mut sum = CRASH_MAGIC;
    for w in head.iter().copied().chain(msg.iter().map(|&b| b as u32)) {
        sum = (sum ^ w).wrapping_mul(0x9E37_79B1).rotate_left(5);
    }
    sum
}

/// Record a panic. Called from the firmware panic handler right before it
/// resets, so it must not allocate or panic. `msg` is truncated to
/// [`CRASH_MSG_MAX`] bytes on a char boundary; extra `pcs` are dropped.
pub fn crash_write(msg: &str, pcs: &[u32]) {
    let mut len = msg.len().min(CRASH_MSG_MAX);
    while !msg.is_char_boundary(len) {
        len -= 1;
    }
    let npcs = pcs.len().min(CRASH_PCS_MAX);

    let mut head = [0u32; CRASH_HEAD_WORDS + 1];
    head[0] = CRASH_MAGIC;
    head[1] = len as u32;
    head[2] = npcs as u32;
    head[3..3 + npcs].copy_from_slice(&pcs[..npcs]);
    let mut body = [0u8; CRASH_MSG_MAX];
    body[..len].copy_from_slice(&msg.as_bytes()[..len]);
    head[CRASH_HEAD_WORDS] = crash_checksum(&head[..CRASH_HEAD_WORDS], &body);

    // SAFETY: the panic handler is the only writer and the boot path the only
    // reader; they never overlap.
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(CRASH_MSG), body);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(CRASH_HEAD), head);
    }
}

/// Return and clear the recorded panic, if the previous reset left a valid
/// one. Call once at boot.
pub fn crash_take() -> Option<CrashRecord> {
    // SAFETY: see `crash_write`.
    let (head, body) = unsafe {
        let head = core::ptr::read_volatile(core::ptr::addr_of!(CRASH_HEAD));
        let body = core::ptr::read_volatile(core::ptr::addr_of!(CRASH_MSG));
        core::ptr::write_volatile(core::ptr::addr_of_mut!(CRASH_HEAD), [0; CRASH_HEAD_WORDS + 1]);
        (head, body)
    };
    let (len, npcs) = (head[1] as usize, head[2] as usize);
    if head[0] != CRASH_MAGIC
        || len > CRASH_MSG_MAX
        || npcs > CRASH_PCS_MAX
        || head[CRASH_HEAD_WORDS] != crash_checksum(&head[..CRASH_HEAD_WORDS], &body)
    {
        return None;
    }
    let text = core::str::from_utf8(&body[..len]).ok()?;
    let mut msg = heapless::String::new();
    let _ = msg.push_str(text);
    let pcs = heapless::Vec::from_slice(&head[3..3 + npcs]).ok()?;
    Some(CrashRecord { msg, pcs })
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
