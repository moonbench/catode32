//! Monotonic time. On firmware, re-exports `esp_hal::time::{Instant, Duration}`.
//! On desktop, a tiny shim around `std::time::Instant` with the same surface.

#[cfg(not(feature = "desktop"))]
pub use esp_hal::time::{Duration, Instant};

#[cfg(feature = "desktop")]
mod desktop {
    use core::ops::{Add, Sub};
    use std::time::Instant as StdInstant;
    use std::sync::OnceLock;

    fn epoch() -> StdInstant {
        static EPOCH: OnceLock<StdInstant> = OnceLock::new();
        *EPOCH.get_or_init(StdInstant::now)
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Instant(StdInstant);

    impl Instant {
        pub fn now() -> Self {
            // Force the epoch to anchor on the first `now()` call so
            // `duration_since_epoch()` starts at zero rather than going
            // negative if something queries it during static init.
            let _ = epoch();
            Self(StdInstant::now())
        }

        pub fn elapsed(&self) -> Duration {
            Duration::from(self.0.elapsed())
        }

        pub fn duration_since_epoch(&self) -> Duration {
            Duration::from(self.0.duration_since(epoch()))
        }
    }

    impl Add<Duration> for Instant {
        type Output = Instant;
        fn add(self, rhs: Duration) -> Instant {
            Instant(self.0 + core::time::Duration::from(rhs))
        }
    }

    impl Sub<Duration> for Instant {
        type Output = Instant;
        fn sub(self, rhs: Duration) -> Instant {
            Instant(self.0 - core::time::Duration::from(rhs))
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Duration(core::time::Duration);

    impl Duration {
        pub const fn from_millis(ms: u64) -> Self {
            Self(core::time::Duration::from_millis(ms))
        }
        pub const fn from_secs(s: u64) -> Self {
            Self(core::time::Duration::from_secs(s))
        }
        pub const fn from_micros(us: u64) -> Self {
            Self(core::time::Duration::from_micros(us))
        }
        pub fn as_millis(&self) -> u64 {
            self.0.as_millis() as u64
        }
        pub fn as_micros(&self) -> u64 {
            self.0.as_micros() as u64
        }
        pub fn as_secs(&self) -> u64 {
            self.0.as_secs()
        }
    }

    impl From<core::time::Duration> for Duration {
        fn from(d: core::time::Duration) -> Self {
            Self(d)
        }
    }

    impl From<Duration> for core::time::Duration {
        fn from(d: Duration) -> Self {
            d.0
        }
    }

    impl Add<Duration> for Duration {
        type Output = Duration;
        fn add(self, rhs: Duration) -> Duration {
            Duration(self.0 + rhs.0)
        }
    }

    impl Sub<Duration> for Duration {
        type Output = Duration;
        fn sub(self, rhs: Duration) -> Duration {
            Duration(self.0 - rhs.0)
        }
    }
}

#[cfg(feature = "desktop")]
pub use desktop::{Duration, Instant};

/// Block the calling task until `deadline`. On firmware this parks the task
/// on the esp-rtos scheduler, whose idle task halts the CPU (`wfi`) until the
/// next interrupt, instead of spinning at full clock.
#[cfg(not(feature = "desktop"))]
pub fn delay_until(deadline: Instant) {
    if deadline > Instant::now() {
        esp_rtos::CurrentThreadHandle::get().delay_until(deadline);
    }
}

#[cfg(feature = "desktop")]
pub fn delay_until(deadline: Instant) {
    let now = Instant::now();
    if deadline > now {
        std::thread::sleep(
            (deadline.duration_since_epoch() - now.duration_since_epoch()).into(),
        );
    }
}

/// Block the calling task for `duration`. See [`delay_until`].
pub fn delay(duration: Duration) {
    delay_until(Instant::now() + duration);
}
