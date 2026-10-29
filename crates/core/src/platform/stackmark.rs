//! Optional stack high-water-mark diagnostics, enabled by the `stackmark` feature.
//! Paints the unused stack at boot, then reports how much was ever touched.
//! Without the feature every entry point is a no-op.

#[cfg(all(feature = "stackmark", not(feature = "desktop")))]
mod firmware {
    use core::sync::atomic::{AtomicU32, Ordering};

    const PAINT: u32 = 0xDEAD_BEEF;

    extern "C" {
        static _stack_end: u32;
        static _stack_start: u32;
    }

    static MIN_FREE: AtomicU32 = AtomicU32::new(u32::MAX);
    static FEEDS: AtomicU32 = AtomicU32::new(0);

    fn sp() -> u32 {
        let sp: u32;
        // SAFETY: reads the stack pointer register only.
        unsafe { core::arch::asm!("mv {}, sp", out(reg) sp) };
        sp
    }

    /// Paint everything below the current frame. Call first thing in `main`.
    pub fn paint() {
        let end = core::ptr::addr_of!(_stack_end) as u32;
        let top = (sp() - 128) & !3;
        let mut p = end;
        while p < top {
            // SAFETY: the region between _stack_end and the live frame is unused stack.
            unsafe { (p as *mut u32).write_volatile(PAINT) };
            p += 4;
        }
        println!(
            "[Stack] painted 0x{:08x}..0x{:08x}, total {} bytes",
            end,
            top,
            core::ptr::addr_of!(_stack_start) as u32 - end
        );
    }

    /// Bytes of stack never touched so far; also tracks the minimum seen.
    pub fn free_bytes() -> u32 {
        let end = core::ptr::addr_of!(_stack_end) as u32;
        let top = core::ptr::addr_of!(_stack_start) as u32;
        let mut p = end;
        // SAFETY: reads within the stack region.
        while p < top && unsafe { (p as *const u32).read_volatile() } == PAINT {
            p += 4;
        }
        let free = p - end;
        MIN_FREE.fetch_min(free, Ordering::Relaxed);
        free
    }

    pub fn report(tag: &str) {
        let free = free_bytes();
        println!(
            "[Stack] {}: sp=0x{:08x} never-touched={} bytes, min={}",
            tag,
            sp(),
            free,
            MIN_FREE.load(Ordering::Relaxed)
        );
    }

    /// Called from the watchdog feed; samples every 60th call and prints on a new low.
    pub fn tick() {
        if FEEDS.fetch_add(1, Ordering::Relaxed) % 60 != 0 {
            return;
        }
        let before = MIN_FREE.load(Ordering::Relaxed);
        let free = free_bytes();
        if free < before {
            report("new low");
        }
    }
}

#[cfg(all(feature = "stackmark", not(feature = "desktop")))]
pub use firmware::{free_bytes, paint, report, tick};

#[cfg(not(all(feature = "stackmark", not(feature = "desktop"))))]
pub fn paint() {}
#[cfg(not(all(feature = "stackmark", not(feature = "desktop"))))]
pub fn report(_tag: &str) {}
#[cfg(not(all(feature = "stackmark", not(feature = "desktop"))))]
pub fn tick() {}
