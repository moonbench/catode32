//! Per-chip deep-sleep wake buttons.
//!
//! Deep sleep can only be ended by a low-power (LP) pad, and which pads those
//! are depends on the chip (esp-hal 1.2.2 metadata): C3 GPIO0-5, C6 GPIO0-7,
//! H2 GPIO7-14. The pin numbers here must match `board.rs`.
//!
//! | Chip | Wake buttons                      | Cannot wake            |
//! |------|-----------------------------------|------------------------|
//! | C6   | A, B, Menu1, Menu2                | D-pad (GPIO14/18/19/20)|
//! | C3   | Up, Down, Left, Right, A, B       | Menu1, Menu2 (GPIO10/11)|
//! | H2   | A, B, Menu1, Up, Right            | Down, Left, Menu2      |

use esp_hal::gpio::{Event, Input, InputConfig, Pull, WakeupConfig};
use esp_hal::peripherals::{self};

use crate::input::Button;

#[cfg(feature = "c6")]
pub const DEEP_WAKE_BUTTONS: &[Button] = &[Button::A, Button::B, Button::Menu1, Button::Menu2];
#[cfg(feature = "c3")]
pub const DEEP_WAKE_BUTTONS: &[Button] = &[
    Button::Up,
    Button::Down,
    Button::Left,
    Button::Right,
    Button::A,
    Button::B,
];
#[cfg(feature = "h2")]
pub const DEEP_WAKE_BUTTONS: &[Button] =
    &[Button::A, Button::B, Button::Menu1, Button::Up, Button::Right];

pub const MAX_WAKE_PINS: usize = 8;

/// Re-creates the wake pins as pulled-up inputs that listen for a low level
/// and may wake the chip through the low-power path. The returned pins must
/// stay alive until the sleep call.
///
/// # Safety
/// The caller must not use the `Buttons` inputs afterwards. Deep sleep never
/// returns, so the old drivers are dead.
pub unsafe fn arm_deep_wake() -> heapless::Vec<Input<'static>, MAX_WAKE_PINS> {
    let cfg = InputConfig::default().with_pull(Pull::Up);
    let mut pins: heapless::Vec<Input<'static>, MAX_WAKE_PINS> = heapless::Vec::new();

    macro_rules! wake_pins {
        ($($gpio:ident),+) => {
            $( let _ = pins.push(Input::new(unsafe { peripherals::$gpio::steal() }, cfg)); )+
        };
    }

    #[cfg(feature = "c6")]
    wake_pins!(GPIO1, GPIO0, GPIO3, GPIO2);
    #[cfg(feature = "c3")]
    wake_pins!(GPIO0, GPIO1, GPIO2, GPIO3, GPIO4, GPIO5);
    #[cfg(feature = "h2")]
    wake_pins!(GPIO12, GPIO11, GPIO10, GPIO13, GPIO14);

    // The pull-ups are copied onto the LP-IO pads at sleep entry, so the
    // level trigger does not see a floating pin.
    let wake_config = WakeupConfig::default().with_low_power_path(true);
    for pin in pins.iter_mut() {
        pin.listen(Event::LowLevel);
        pin.apply_wakeup_config(&wake_config).unwrap();
    }
    pins
}
