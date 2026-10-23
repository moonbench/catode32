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
// The H2 board relies on weak internal pull-ups, so keep the bus slower.
#[cfg(feature = "h2")]
pub const I2C_FREQ_KHZ: u32 = 100;
#[cfg(not(feature = "h2"))]
pub const I2C_FREQ_KHZ: u32 = 400;

#[cfg(not(feature = "desktop"))]
mod firmware {
    use esp_hal::{
        gpio::{Input, InputConfig, Pull},
        i2c::master::{Config, I2c},
        peripherals::{Peripherals, FLASH, FROM_CPU_INTR0, TIMG0},
        rng::Rng,
        time::Rate,
        Blocking,
    };

    use super::I2C_FREQ_KHZ;
    use crate::platform::radio::WIFI;
    use crate::{input::Buttons, led::Led};

    #[cfg(any(
        all(feature = "c6", feature = "c3"),
        all(feature = "c6", feature = "h2"),
        all(feature = "c3", feature = "h2")
    ))]
    compile_error!("Features `c6`, `c3` and `h2` are mutually exclusive, enable exactly one.");

    #[cfg(not(any(feature = "c6", feature = "c3", feature = "h2")))]
    compile_error!("Enable feature `c6`, `c3` or `h2` to select the target board.");

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
        #[cfg(feature = "h2")]
        let i2c = I2c::new(peripherals.I2C0, i2c_config)
            .unwrap()
            .with_sda(peripherals.GPIO0)
            .with_scl(peripherals.GPIO1);

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
        // H2: UP=13 DOWN=23 LEFT=24 RIGHT=14 A=12 B=11 MENU1=10 MENU2=2
        #[cfg(feature = "h2")]
        let buttons = Buttons::new([
            Input::new(peripherals.GPIO13, input_config),
            Input::new(peripherals.GPIO23, input_config),
            Input::new(peripherals.GPIO24, input_config),
            Input::new(peripherals.GPIO14, input_config),
            Input::new(peripherals.GPIO12, input_config),
            Input::new(peripherals.GPIO11, input_config),
            Input::new(peripherals.GPIO10, input_config),
            Input::new(peripherals.GPIO2, input_config),
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
            #[cfg(not(any(feature = "desktop", feature = "h2")))]
            wifi: peripherals.WIFI,
            #[cfg(any(feature = "desktop", feature = "h2"))]
            wifi: WIFI::new(),
        }
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{init, Board};
