//! Stat changes for time spent in deep sleep.
//!
//! Stats have no passive decay, so waking from deep sleep applies one scaled
//! "slept" delta modelled on a full sleeping bonus.

use crate::context::StatId;

/// Fraction of the real time asleep that counts towards the stat change.
pub const SCALE: f32 = 0.25;
/// Upper bound on the effective time, in seconds.
pub const CAP_S: f32 = 2.0 * 3600.0;
/// Effective seconds that earn a complete sleeping bonus.
pub const FULL_SLEEP_S: f32 = 8.0 * 3600.0;

/// Stat changes for a complete sleep.
const FULL_BONUS: [(StatId, f32); 6] = [
    (StatId::Energy, 35.0),
    (StatId::Focus, 9.0),
    (StatId::Comfort, 10.0),
    (StatId::Playfulness, 15.0),
    (StatId::Fullness, -5.0),
    (StatId::Cleanliness, -1.9),
];

/// Effective seconds of sleep for `elapsed_s` real seconds.
pub fn effective_seconds(elapsed_s: f32) -> f32 {
    (elapsed_s.max(0.0) * SCALE).min(CAP_S)
}

/// Stat changes to apply after `elapsed_s` seconds in deep sleep.
pub fn catch_up(elapsed_s: f32) -> [(StatId, f32); 6] {
    let fraction = effective_seconds(elapsed_s) / FULL_SLEEP_S;
    let mut out = FULL_BONUS;
    for (_, delta) in out.iter_mut() {
        *delta *= fraction;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_changes_nothing() {
        assert!(catch_up(0.0).iter().all(|(_, d)| *d == 0.0));
        assert!(catch_up(-5.0).iter().all(|(_, d)| *d == 0.0));
    }

    #[test]
    fn normal_sleep_is_scaled() {
        assert_eq!(effective_seconds(3600.0), 900.0);
        let energy = catch_up(3600.0)[0].1;
        assert!((energy - 35.0 * 900.0 / FULL_SLEEP_S).abs() < 1e-4);
    }

    #[test]
    fn huge_sleep_is_capped() {
        assert_eq!(effective_seconds(1.0e9), CAP_S);
        assert_eq!(catch_up(1.0e9)[0].1, catch_up(1.0e8)[0].1);
        assert!(catch_up(1.0e9)[0].1 < 35.0);
    }
}
