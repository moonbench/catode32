//! ESP32 board init. Picks up GPIO/I2C/RMT/software-interrupt resources from
//! the runtime peripherals and hands them to the rest of the firmware via
//! the `Board` struct. The whole module is firmware-only; desktop builds
//! construct equivalents (a SimulatorDisplay, a keyboard-driven `Buttons`,
//! no-op LED, etc.) directly in the `desktop` crate.
//!
//! C3 Supermini pinout: SDA=GPIO20, SCL=GPIO21 (the UART0 RX/TX pins,
//! free because the board talks over native USB); buttons UP=0, DOWN=1,
//! LEFT=3, RIGHT=4, A=5, B=6, MENU1=7, MENU2=10; the blue LED on GPIO8.
//! The ROM boot log still goes out on GPIO21, but with SDA idle-high on
//! GPIO20 it can never form an I2C start condition.
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
        peripherals::{Peripherals, BT, FLASH, FROM_CPU_INTR0, TIMG0, WIFI},
        rng::Rng,
        time::Rate,
        Blocking,
    };

    use super::I2C_FREQ_KHZ;
    use crate::{input::Buttons, led::Led, platform::rng::EntropySource};

    #[cfg(all(feature = "c6", feature = "c3"))]
    compile_error!("Features `c6` and `c3` are mutually exclusive, enable exactly one.");

    #[cfg(not(any(feature = "c6", feature = "c3")))]
    compile_error!("Enable either feature `c6` or `c3` to select the target board.");

    pub struct Board {
        pub i2c: I2c<'static, Blocking>,
        pub buttons: Buttons,
        pub rng: Rng,
        pub entropy: EntropySource,
        pub led: Led,
        pub flash: FLASH<'static>,
        pub timg0: TIMG0<'static>,
        pub sw_int0: FROM_CPU_INTR0<'static>,
        pub wifi: WIFI<'static>,
        pub bt: BT<'static>,
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
            .with_sda(peripherals.GPIO20)
            .with_scl(peripherals.GPIO21);

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
            Input::new(peripherals.GPIO3, input_config),
            Input::new(peripherals.GPIO4, input_config),
            Input::new(peripherals.GPIO5, input_config),
            Input::new(peripherals.GPIO6, input_config),
            Input::new(peripherals.GPIO7, input_config),
            Input::new(peripherals.GPIO10, input_config),
        ]);

        // Both boards have their LED on GPIO8: a WS2812 on the C6, a
        // plain blue LED on the C3 Supermini.
        #[cfg(feature = "c6")]
        let led = Led::new(peripherals.RMT, peripherals.GPIO8);
        #[cfg(feature = "c3")]
        let led = Led::new(peripherals.LEDC, peripherals.GPIO8);

        Board {
            i2c,
            buttons,
            rng: Rng::new(),
            entropy: EntropySource::new(peripherals.RNG, peripherals.ADC1),
            led,
            flash: peripherals.FLASH,
            timg0: peripherals.TIMG0,
            sw_int0: peripherals.FROM_CPU_INTR0,
            wifi: peripherals.WIFI,
            bt: peripherals.BT,
        }
    }
}

#[cfg(not(feature = "desktop"))]
pub use firmware::{init, Board};
