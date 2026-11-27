//! The board's status LED. Each board gets a real driver for what it has:
//!
//! - C6: WS2812 RGB LED on GPIO8, driven by RMT via `esp-hal-smartled`.
//!   Writes block, but take well under a frame at 12 FPS.
//! - C3 Supermini: the single-colour (blue) LED on GPIO8, active-low,
//!   dimmed with LEDC PWM. A colour shows as its brightest channel.
//! - Desktop: an RGB LED the simulator draws beside the screen.
//!
//! All three take colours, so scenes that use the LED as an accent (timer,
//! Pomodoro, breathing, metronome, Morse) run everywhere. Tools that only
//! make sense in colour are gated on the `rgb-led` feature instead.

#[cfg(all(not(feature = "desktop"), feature = "c6"))]
mod ws2812 {
    use esp_hal::{
        peripherals::{GPIO8, RMT},
        rmt::Rmt,
        time::Rate,
        Blocking,
    };
    use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812_TIMING};
    use smart_leds::{SmartLedsWrite, RGB8};

    const LED_BUFFER: usize = buffer_size::<RGB8>(1);

    type Strip = RmtSmartLeds<'static, LED_BUFFER, Blocking, RGB8, color_order::Grb>;

    pub struct Led {
        strip: Strip,
    }

    impl Led {
        pub fn new(rmt: RMT<'static>, pin: GPIO8<'static>) -> Self {
            let freq = Rate::from_mhz(80);
            let rmt = Rmt::new(rmt, freq).unwrap();
            let strip = Strip::new(WS2812_TIMING, rmt.channel0, pin, freq).unwrap();
            Self { strip }
        }

        pub fn set(&mut self, r: u8, g: u8, b: u8) {
            let _ = self.strip.write(core::iter::once(RGB8 { r, g, b }));
        }

        pub fn off(&mut self) {
            self.set(0, 0, 0);
        }
    }
}

#[cfg(all(not(feature = "desktop"), feature = "c6"))]
pub use ws2812::Led;

#[cfg(all(not(feature = "desktop"), feature = "c3"))]
mod pwm {
    use alloc::boxed::Box;
    use esp_hal::{
        gpio::DriveMode,
        ledc::{
            channel::{self, ChannelHW, ChannelIFace},
            timer::{self, TimerIFace},
            LSGlobalClkSource, Ledc, LowSpeed,
        },
        peripherals::{GPIO8, LEDC},
        time::Rate,
    };

    /// 8-bit duty: the hardware counts 0..=256, where 256 holds the pin
    /// high for the whole period.
    const DUTY_FULL: u32 = 1 << 8;

    pub struct Led {
        channel: channel::Channel<'static, LowSpeed>,
    }

    impl Led {
        pub fn new(ledc: LEDC<'static>, pin: GPIO8<'static>) -> Self {
            let mut ledc = Ledc::new(ledc);
            ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
            let mut timer = ledc.timer::<LowSpeed>(timer::Number::Timer0);
            timer
                .configure(timer::config::Config {
                    duty: timer::config::Duty::Duty8Bit,
                    clock_source: timer::LSClockSource::APBClk,
                    frequency: Rate::from_khz(24),
                })
                .unwrap();
            // The channel keeps a `'static` reference to its timer. Built
            // once at boot, so the leak is a one-off few bytes.
            let timer: &'static timer::Timer<'static, LowSpeed> = Box::leak(Box::new(timer));
            let mut channel = ledc.channel(channel::Number::Channel0, pin);
            channel
                .configure(channel::config::Config {
                    timer,
                    // The LED is active-low: 100% high is off.
                    duty_pct: 100,
                    drive_mode: DriveMode::PushPull,
                })
                .unwrap();
            Self { channel }
        }

        pub fn set(&mut self, r: u8, g: u8, b: u8) {
            let level = r.max(g).max(b) as u32;
            let on = (level * DUTY_FULL + 127) / 255;
            // Active-low: the LED is lit while the pin is low.
            self.channel.set_duty_hw(DUTY_FULL - on);
        }

        pub fn off(&mut self) {
            self.set(0, 0, 0);
        }
    }
}

#[cfg(all(not(feature = "desktop"), feature = "c3"))]
pub use pwm::Led;

#[cfg(feature = "desktop")]
mod simulated {
    /// Holds the colour for the simulator window to draw.
    pub struct Led {
        rgb: (u8, u8, u8),
    }

    impl Led {
        pub fn new() -> Self {
            Self { rgb: (0, 0, 0) }
        }

        pub fn set(&mut self, r: u8, g: u8, b: u8) {
            self.rgb = (r, g, b);
        }

        pub fn off(&mut self) {
            self.set(0, 0, 0);
        }

        /// The colour last written; black when off.
        pub fn rgb(&self) -> (u8, u8, u8) {
            self.rgb
        }
    }

    impl Default for Led {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "desktop")]
pub use simulated::Led;

/// Remembers the colour last written, so a scene that decides the LED
/// colour every frame only talks to the hardware when it changes (a
/// WS2812 write blocks for a moment).
#[derive(Clone, Copy, Default)]
pub struct LedLatch {
    last: Option<(u8, u8, u8)>,
}

impl LedLatch {
    pub const fn new() -> Self {
        Self { last: None }
    }

    pub fn show(&mut self, led: &mut Led, rgb: (u8, u8, u8)) {
        if self.last != Some(rgb) {
            led.set(rgb.0, rgb.1, rgb.2);
            self.last = Some(rgb);
        }
    }

    pub fn off(&mut self, led: &mut Led) {
        self.show(led, (0, 0, 0));
    }

    /// Forget the last colour, so the next `show` always writes. For when
    /// something else may have changed the LED meanwhile.
    pub fn forget(&mut self) {
        self.last = None;
    }
}

impl Led {
    /// `set` taking a tuple.
    pub fn set_rgb(&mut self, (r, g, b): (u8, u8, u8)) {
        self.set(r, g, b);
    }
}

/// Scale a colour by `level` out of 255.
pub fn scale_rgb((r, g, b): (u8, u8, u8), level: u8) -> (u8, u8, u8) {
    let s = |c: u8| ((c as u16 * level as u16 + 127) / 255) as u8;
    (s(r), s(g), s(b))
}
