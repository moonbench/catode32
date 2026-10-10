//! ESP32 board init. Picks up GPIO/I2C/RMT/software-interrupt resources from
//! the runtime peripherals and hands them to the rest of the firmware via
//! the `Board` struct. The whole module is firmware-only; desktop builds
//! construct equivalents (a SimulatorDisplay, a keyboard-driven `Buttons`,
//! no-op LED, etc.) directly in the `desktop` crate.
//!
//! `DISPLAY_WIDTH` / `DISPLAY_HEIGHT` are visible on both platforms because
//! game logic (transitions, zoomies) references them.

pub const DISPLAY_WIDTH: u16 = 128;
pub const DISPLAY_HEIGHT: u16 = 64;
pub const I2C_FREQ_KHZ: u32 = 400;

#[cfg(not(feature = "desktop"))]
mod firmware {
    use esp_hal::{
        gpio::{Input, InputConfig, Pull},
        i2c::master::{Config, I2c},
        peripherals::{Peripherals, FLASH, FROM_CPU_INTR0, TIMG0, WIFI},
        rng::Rng,
        time::Rate,
        Blocking,
    };

    use super::I2C_FREQ_KHZ;
    use crate::{input::Buttons, led::Led};

    #[cfg(all(feature = "c6", feature = "c3"))]
    compile_error!("Features `c6` and `c3` are mutually exclusive, enable exactly one.");

    #[cfg(not(any(feature = "c6", feature = "c3")))]
    compile_error!("Enable either feature `c6` or `c3` to select the target board.");

    pub struct Board {
        pub i2c: I2c<'static, Blocking>,
        pub buttons: Buttons,
        pub rng: Rng,
        pub led: Led,
        pub flash: FLASH<'static>,
        pub timg0: TIMG0<'static>,
        pub sw_int0: FROM_CPU_INTR0<'static>,
        pub wifi: WIFI<'static>,
    }

    pub fn init(peripherals: Peripherals) -> Board {
        let i2c_config = Config::default().with_frequency(Rate::from_khz(I2C_FREQ_KHZ));

        #[cfg(feature = "c6")]
        let i2c = I2c::new(peripherals.I2C0, i2c_config)
            .unwrap()
            .with_sda(peripherals.GPIO4)
            .with_scl(peripherals.GPIO7);
        #[cfg(feature = "c3")]
        let i2c = I2c::new(peripherals.I2C0, i2c_config)
            .unwrap()
            .with_sda(peripherals.GPIO6)
            .with_scl(peripherals.GPIO7);

        let input_config = InputConfig::default().with_pull(Pull::Up);
        // Button order: UP, DOWN, LEFT, RIGHT, A, B, MENU1, MENU2
        #[cfg(feature = "c6")]
        let buttons = Buttons::new([
            Input::new(peripherals.GPIO14, input_config),
            Input::new(peripherals.GPIO18, input_config),
            Input::new(peripherals.GPIO20, input_config),
            Input::new(peripherals.GPIO19, input_config),
            Input::new(peripherals.GPIO1, input_config),
            Input::new(peripherals.GPIO0, input_config),
            Input::new(peripherals.GPIO3, input_config),
            Input::new(peripherals.GPIO2, input_config),
        ]);
        #[cfg(feature = "c3")]
        let buttons = Buttons::new([
            Input::new(peripherals.GPIO0, input_config),
            Input::new(peripherals.GPIO1, input_config),
            Input::new(peripherals.GPIO2, input_config),
            Input::new(peripherals.GPIO3, input_config),
            Input::new(peripherals.GPIO4, input_config),
            Input::new(peripherals.GPIO5, input_config),
            Input::new(peripherals.GPIO10, input_config),
            Input::new(peripherals.GPIO11, input_config),
        ]);

        let led = Led::new(peripherals.RMT, peripherals.GPIO8);

        Board {
            i2c,
            buttons,
            rng: Rng::new(),
            led,
            flash: peripherals.FLASH,
            timg0: peripherals.TIMG0,
            sw_int0: peripherals.FROM_CPU_INTR0,
            wifi: peripherals.WIFI,
        }
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{init, Board};
