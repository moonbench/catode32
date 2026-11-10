//! Hardware RNG handle. Firmware re-exports `esp_hal::rng::Rng` (the
//! peripheral); desktop provides a `getrandom`-free xorshift64 backed by a
//! one-time OS-keyed seed, which is plenty for picking pet seeds and shuffling
//! menus on a simulator.
//!
//! `Rng` alone is only pseudo-random on the chip unless RF or an ADC noise
//! source is running. [`EntropySource`] is for the few reads that need real
//! noise (adoption seeds).

#[cfg(not(feature = "desktop"))]
pub use esp_hal::rng::Rng;

#[cfg(not(feature = "desktop"))]
mod firmware {
    use esp_hal::{
        delay::Delay,
        peripherals::{ADC1, RNG},
        rng::{Rng, TrngSource},
    };

    /// Gap before each sample, so new ADC noise reaches the RNG between reads.
    const SAMPLE_GAP_US: u32 = 100;

    /// Owns the peripherals `TrngSource` needs while no one is sampling.
    pub struct EntropySource {
        rng: RNG<'static>,
        adc: ADC1<'static>,
    }

    impl EntropySource {
        pub fn new(rng: RNG<'static>, adc: ADC1<'static>) -> Self {
            Self { rng, adc }
        }

        /// Fill `out` with RNG words drawn while a physical noise source runs.
        ///
        /// With the radio up, RF already feeds the RNG, and tearing down
        /// `TrngSource` would rewrite analog registers under the live PHY, so
        /// the ADC source is only used while the radio is off.
        pub fn collect(&mut self, rng: &Rng, radio_on: bool, out: &mut [u32]) {
            let _source = if radio_on {
                None
            } else {
                Some(TrngSource::new(self.rng.reborrow(), self.adc.reborrow()))
            };
            let delay = Delay::new();
            for slot in out.iter_mut() {
                delay.delay_micros(SAMPLE_GAP_US);
                *slot = rng.random();
            }
        }
    }

    /// Per-device value from the factory MAC, folded into a `u64`.
    pub fn device_id() -> u64 {
        esp_hal::efuse::base_mac_address()
            .as_bytes()
            .iter()
            .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{device_id, EntropySource};

#[cfg(feature = "desktop")]
mod desktop {
    use std::cell::Cell;

    /// `RandomState` carries OS-provided keys, so this differs per process
    /// without a `getrandom` dependency. The clock is mixed in as well.
    fn seed_from_os() -> u64 {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};
        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);
        (RandomState::new().build_hasher().finish() ^ clock) | 1
    }

    /// xorshift64*, adequate for game decisions on the desktop simulator.
    pub struct Rng(Cell<u64>);

    /// Same surface as the firmware type; the desktop `Rng` is already
    /// OS-seeded, so there is no peripheral to switch on.
    pub struct EntropySource;

    impl EntropySource {
        pub fn new() -> Self {
            Self
        }

        pub fn collect(&mut self, rng: &Rng, _radio_on: bool, out: &mut [u32]) {
            for slot in out.iter_mut() {
                *slot = rng.random();
            }
        }
    }

    impl Default for EntropySource {
        fn default() -> Self {
            Self::new()
        }
    }

    pub fn device_id() -> u64 {
        u64::from(std::process::id())
    }

    impl Rng {
        pub fn new() -> Self {
            Self(Cell::new(seed_from_os()))
        }

        pub fn random(&self) -> u32 {
            self.next_u64() as u32
        }

        pub fn read(&self, buf: &mut [u8]) {
            for chunk in buf.chunks_mut(8) {
                let v = self.next_u64();
                let bytes = v.to_le_bytes();
                let n = chunk.len();
                chunk.copy_from_slice(&bytes[..n]);
            }
        }

        fn next_u64(&self) -> u64 {
            let mut x = self.0.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0.set(x);
            x.wrapping_mul(0x2545F4914F6CDD1D)
        }
    }

    impl Default for Rng {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "desktop")]
pub use desktop::{device_id, EntropySource, Rng};
