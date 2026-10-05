//! Panic handler: log the panic, record it to RTC RAM for the next boot, and
//! reset. The default esp-backtrace handler halts forever, which on a device
//! off USB looks like a frozen screen with no clue as to why.
//!
//! The record (message plus a few return addresses) is shown on the Power
//! debug screen after the reset and printed at boot. Decode addresses with
//! `addr2line -e target/riscv32imac-unknown-none-elf/release/catode32-firmware 0x...`.

use core::{
    fmt::{self, Write},
    panic::PanicInfo,
    sync::atomic::{AtomicBool, Ordering},
};

use catode32_core::platform::{
    persist::{self, CRASH_MSG_MAX, CRASH_PCS_MAX},
    power::software_reset,
};
use esp_backtrace::Backtrace;
use esp_println::println;

/// Set on entry so a panic inside the handler goes straight to reset instead
/// of recursing. Load + store rather than swap: the C3 has no atomic RMW.
static PANICKING: AtomicBool = AtomicBool::new(false);

/// `fmt::Write` into a fixed buffer, silently truncating. Never fails, so
/// formatting can't panic again.
struct Truncating {
    buf: [u8; CRASH_MSG_MAX],
    len: usize,
}

impl Write for Truncating {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if self.len == self.buf.len() {
                break;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
        Ok(())
    }
}

impl Truncating {
    fn as_str(&self) -> &str {
        // Truncation may split a UTF-8 sequence; keep the valid prefix.
        match core::str::from_utf8(&self.buf[..self.len]) {
            Ok(s) => s,
            Err(e) => core::str::from_utf8(&self.buf[..e.valid_up_to()]).unwrap_or(""),
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if PANICKING.load(Ordering::Relaxed) {
        software_reset();
    }
    PANICKING.store(true, Ordering::Relaxed);

    println!("");
    println!("====================== PANIC ======================");
    println!("{}", info);

    // Compact form for the 21-column debug screen: file name (no
    // directories), line, then the message.
    let mut text = Truncating { buf: [0; CRASH_MSG_MAX], len: 0 };
    if let Some(loc) = info.location() {
        let file = loc.file().rsplit('/').next().unwrap_or(loc.file());
        let _ = write!(text, "{}:{} ", file, loc.line());
    }
    let _ = write!(text, "{}", info.message());

    let backtrace = Backtrace::capture();
    let mut pcs = [0u32; CRASH_PCS_MAX];
    let mut npcs = 0;
    println!("Backtrace:");
    for frame in backtrace.frames() {
        let pc = frame.program_counter() as u32;
        println!("0x{:08x}", pc);
        if npcs < pcs.len() {
            pcs[npcs] = pc;
            npcs += 1;
        }
    }

    persist::crash_write(text.as_str(), &pcs[..npcs]);
    println!("Recorded to RTC RAM, resetting");
    software_reset()
}
