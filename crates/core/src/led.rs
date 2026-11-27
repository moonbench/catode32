//! On-board WS2812 RGB LED (GPIO8, single pixel). Driven by the RMT
//! peripheral via `esp-hal-smartled`; writes are blocking and take well
//! under a frame at 12 FPS.
//!
//! Desktop builds get a no-op `Led` so the existing `ctx.led.set(...)` /
//! `ctx.led.off()` call sites compile unchanged.

#[cfg(not(feature = "desktop"))]
mod firmware {
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

#[cfg(not(feature = "desktop"))]
pub use firmware::Led;

#[cfg(feature = "desktop")]
mod desktop {
    pub struct Led;

    impl Led {
        pub fn new() -> Self {
            Self
        }

        pub fn set(&mut self, _r: u8, _g: u8, _b: u8) {}

        pub fn off(&mut self) {}
    }

    impl Default for Led {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "desktop")]
pub use desktop::Led;

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
