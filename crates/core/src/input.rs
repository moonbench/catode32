//! Eight on-board buttons with edge detection and debounce.
//!
//! On firmware the pins are `esp_hal::gpio::Input<'static>`. On desktop the
//! pin type is a thin wrapper around an atomic `bool` per button index. The
//! desktop binary writes those atomics from SDL keyboard events, and the
//! same `Buttons::is_pressed` / `was_just_pressed` API works unchanged.

use crate::platform::time::{Duration, Instant};
use crate::t;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Button {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    A = 4,
    B = 5,
    Menu1 = 6,
    Menu2 = 7,
}

impl Button {
    pub const ALL: [Button; 8] = [
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::A,
        Button::B,
        Button::Menu1,
        Button::Menu2,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Button::Up => "U",
            Button::Down => "D",
            Button::Left => "L",
            Button::Right => t!("R"),
            Button::A => "A",
            Button::B => t!("B"),
            Button::Menu1 => "M1",
            Button::Menu2 => "M2",
        }
    }
}

const DEBOUNCE: Duration = Duration::from_millis(50);

#[cfg(not(feature = "desktop"))]
pub type ButtonPin = esp_hal::gpio::Input<'static>;

#[cfg(feature = "desktop")]
mod desktop_pin {
    use core::sync::atomic::{AtomicBool, Ordering};

    /// Backing state for the 8 desktop "pins". The desktop binary writes
    /// these from keyboard events; `ButtonPin::is_low()` reads them. Active
    /// low matches the firmware convention so the rest of `input.rs` is
    /// identical.
    static STATE: [AtomicBool; 8] = [
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
    ];

    pub struct ButtonPin {
        idx: usize,
    }

    impl ButtonPin {
        pub fn new(idx: usize) -> Self {
            Self { idx }
        }

        pub fn is_low(&self) -> bool {
            STATE[self.idx].load(Ordering::Relaxed)
        }
    }

    /// Called by the desktop binary on each SDL event tick to push fresh
    /// keyboard state into the shared pin store.
    pub fn set_pressed(idx: usize, pressed: bool) {
        STATE[idx].store(pressed, Ordering::Relaxed);
    }
}

#[cfg(feature = "desktop")]
pub use desktop_pin::{set_pressed as set_desktop_button, ButtonPin};

pub struct Buttons {
    pins: [ButtonPin; 8],
    edge_state: [bool; 8],
    last_press: [Option<Instant>; 8],
}

impl Buttons {
    pub fn new(pins: [ButtonPin; 8]) -> Self {
        Self {
            pins,
            edge_state: [false; 8],
            last_press: [None; 8],
        }
    }

    pub fn is_pressed(&self, button: Button) -> bool {
        self.pins[button as usize].is_low()
    }

    pub fn was_just_pressed(&mut self, button: Button) -> bool {
        let idx = button as usize;
        let pressed = self.pins[idx].is_low();
        let was = self.edge_state[idx];
        let debounced = self.last_press[idx].map_or(true, |t| t.elapsed() > DEBOUNCE);

        if pressed && !was && debounced {
            self.edge_state[idx] = true;
            self.last_press[idx] = Some(Instant::now());
            return true;
        }
        if !pressed && was {
            self.edge_state[idx] = false;
        }
        false
    }

    pub fn pressed_mask(&self) -> u8 {
        let mut mask = 0u8;
        for i in 0..8 {
            if self.pins[i].is_low() {
                mask |= 1 << i;
            }
        }
        mask
    }

    pub fn any_pressed(&self) -> bool {
        self.pressed_mask() != 0
    }

    /// Mark every currently-held button as already seen, so the next
    /// `was_just_pressed()` call will not report it as a fresh press.
    /// Called by the sleep manager on wake so the button that triggered
    /// the wake is not also passed through as a game action.
    pub fn consume_all(&mut self) {
        let now = Instant::now();
        for i in 0..8 {
            self.edge_state[i] = self.pins[i].is_low();
            self.last_press[i] = Some(now);
        }
    }

    /// Make every button a light-sleep wake source (falling to low level).
    /// Pair with [`Self::disarm_wakeup`] once the sleep returns.
    ///
    /// TODO(c3): untested on the C3. Light sleep should accept any GPIO
    /// there too, but verify on hardware.
    #[cfg(not(feature = "desktop"))]
    pub fn arm_wakeup(&mut self) {
        for pin in &mut self.pins {
            pin.listen(esp_hal::gpio::Event::LowLevel);
        }
    }

    #[cfg(feature = "desktop")]
    pub fn arm_wakeup(&mut self) {}

    /// Stop the buttons acting as wake sources, back to plain polled pins.
    #[cfg(not(feature = "desktop"))]
    pub fn disarm_wakeup(&mut self) {
        for pin in &mut self.pins {
            pin.unlisten();
        }
    }

    #[cfg(feature = "desktop")]
    pub fn disarm_wakeup(&mut self) {}

    /// True if a button ended the last light sleep. Catches a tap that was
    /// already released by the time the CPU got to read the pins.
    #[cfg(not(feature = "desktop"))]
    pub fn caused_wakeup(&self) -> bool {
        self.pins.iter().any(|pin| pin.caused_wakeup())
    }

    #[cfg(feature = "desktop")]
    pub fn caused_wakeup(&self) -> bool {
        false
    }

    pub fn direction(&self) -> (i8, i8) {
        let mut dx = 0;
        let mut dy = 0;
        if self.is_pressed(Button::Up) {
            dy -= 1;
        }
        if self.is_pressed(Button::Down) {
            dy += 1;
        }
        if self.is_pressed(Button::Left) {
            dx -= 1;
        }
        if self.is_pressed(Button::Right) {
            dx += 1;
        }
        (dx, dy)
    }
}
