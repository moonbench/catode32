//! Clock-style formatting for the timer tools.

use core::fmt::Write;
use heapless::String;

/// `M:SS`, or `H:MM:SS` from an hour up. With `tenths`, adds `.d`.
pub fn clock(ms: u64, tenths: bool) -> String<12> {
    let total_s = ms / 1000;
    let (h, m, s) = (total_s / 3600, total_s / 60 % 60, total_s % 60);
    let mut out = String::new();
    if h > 0 {
        let _ = write!(out, "{}:{:02}:{:02}", h, m, s);
    } else {
        let _ = write!(out, "{}:{:02}", m, s);
    }
    if tenths {
        let _ = write!(out, ".{}", ms / 100 % 10);
    }
    out
}

/// Countdown display: rounds up, so "0:01" shows until the very end.
pub fn remaining(ms: u64) -> String<12> {
    clock(ms.div_ceil(1000) * 1000, false)
}
