//! Desktop simulator for catode32.
//!
//! Drives `catode32-core` (built with the `desktop` feature) inside an
//! SDL2 window via `embedded-graphics-simulator`. Each game frame:
//!
//! 1. Pump SDL events; map arrow / A / S / Q / W keys onto the
//!    `Button` indices used by `catode32_core::input`.
//! 2. Call `Game::tick()` to run one frame of game logic + drawing.
//! 3. Blit the in-memory 128x64 framebuffer to the `SimulatorDisplay`.
//! 4. `Window::update()` and sleep enough to land near 12 FPS.
//!
//! ESC or closing the window saves the game, then quits.
//!
//! The lit-pixel and background colors can be overridden at launch with
//! `--fg RRGGBB` / `--bg RRGGBB` (see `USAGE`).

use std::process;
use std::thread;
use std::time::{Duration, Instant};

use catode32_core::{
    espnow_manager::EspNowManager,
    game::Game,
    input::{set_desktop_button, ButtonPin, Buttons},
    led::Led,
    platform::{radio::WIFI, rng::Rng},
    render::Renderer,
    storage,
};

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Size},
    Pixel,
};
use embedded_graphics_simulator::{
    sdl2::Keycode, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 64;
const SCALE: u32 = 8;
const FPS: u64 = 12;
const FRAME_MS: u64 = 1000 / FPS;

/// Default "lit pixel" tint.
const DEFAULT_FG: Rgb888 = Rgb888::new(35, 165, 204);
/// Default background (a near-black charcoal, not pure 0,0,0 so the off
/// pixels read as "screen, not void").
const DEFAULT_BG: Rgb888 = Rgb888::new(10, 10, 10);

const USAGE: &str = "\
Usage: catode32-desktop [--fg RRGGBB] [--bg RRGGBB]

Options:
  --fg RRGGBB   Lit pixel color (default 23A5CC)
  --bg RRGGBB   Background color (default 0A0A0A)
  -h, --help    Print this help

Colors are 6-digit hex, with or without a leading '#'.";

/// Colors used to blit the 1-bit framebuffer to the window.
struct Palette {
    fg: Rgb888,
    bg: Rgb888,
}

/// Parse `RRGGBB` or `#RRGGBB` (any case) into a color.
fn parse_hex(s: &str) -> Option<Rgb888> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Rgb888::new(channel(0)?, channel(2)?, channel(4)?))
}

/// Print an error plus usage to stderr and exit with status 2.
fn usage_error(msg: &str) -> ! {
    eprintln!("error: {msg}\n\n{USAGE}");
    process::exit(2);
}

/// Read `--fg` / `--bg` overrides from the command line.
fn parse_args() -> Palette {
    let mut palette = Palette {
        fg: DEFAULT_FG,
        bg: DEFAULT_BG,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            println!("{USAGE}");
            process::exit(0);
        }
        let (flag, inline_value) = match arg.split_once('=') {
            Some((flag, value)) => (flag.to_string(), Some(value.to_string())),
            None => (arg, None),
        };
        let slot = match flag.as_str() {
            "--fg" => &mut palette.fg,
            "--bg" => &mut palette.bg,
            _ => usage_error(&format!("unknown argument '{flag}'")),
        };
        let value = inline_value
            .or_else(|| args.next())
            .unwrap_or_else(|| usage_error(&format!("{flag} needs a color value")));
        *slot = parse_hex(&value)
            .unwrap_or_else(|| usage_error(&format!("invalid color '{value}' for {flag}")));
    }
    palette
}

/// Map an SDL keycode onto the firmware-side button index.
/// Arrows for the d-pad, A=A, S=B, Q=MENU1, W=MENU2.
fn button_index(key: Keycode) -> Option<usize> {
    match key {
        Keycode::Up => Some(0),
        Keycode::Down => Some(1),
        Keycode::Left => Some(2),
        Keycode::Right => Some(3),
        Keycode::A => Some(4),
        Keycode::S => Some(5),
        Keycode::Q => Some(6),
        Keycode::W => Some(7),
        _ => None,
    }
}

fn main() {
    // Before anything else, so `--help` / bad flags exit without touching
    // the save file.
    let palette = parse_args();

    storage::init();

    let renderer = Renderer::new();
    let buttons = Buttons::new([
        ButtonPin::new(0),
        ButtonPin::new(1),
        ButtonPin::new(2),
        ButtonPin::new(3),
        ButtonPin::new(4),
        ButtonPin::new(5),
        ButtonPin::new(6),
        ButtonPin::new(7),
    ]);
    let rng = Rng::new();
    let led = Led::new();
    let wifi = WIFI::new();
    let espnow = EspNowManager::new();

    let mut game = Game::new(renderer, buttons, rng, led, wifi, espnow);

    let mut sim_display = SimulatorDisplay::<Rgb888>::new(Size::new(WIDTH, HEIGHT));
    let output = OutputSettingsBuilder::new().scale(SCALE).build();
    let mut window = Window::new("catode32", &output);
    // Prime the window so `events()` has a valid SDL window to pump from
    // on the first iteration.
    window.update(&sim_display);

    // Buttons that received both KeyDown and KeyUp in a single pump are
    // held pressed across one tick and released afterwards, so brief taps
    // that land entirely within a frame still register.
    let mut pending_release = [false; 8];

    'outer: loop {
        let frame_start = Instant::now();

        let mut pressed_this_frame = [false; 8];
        let mut released_this_frame = [false; 8];

        for ev in window.events() {
            match ev {
                SimulatorEvent::Quit => break 'outer,
                SimulatorEvent::KeyDown { keycode, .. } => {
                    if keycode == Keycode::Escape {
                        break 'outer;
                    }
                    if let Some(idx) = button_index(keycode) {
                        pressed_this_frame[idx] = true;
                    }
                }
                SimulatorEvent::KeyUp { keycode, .. } => {
                    if let Some(idx) = button_index(keycode) {
                        released_this_frame[idx] = true;
                    }
                }
                _ => {}
            }
        }

        for idx in 0..8 {
            if pressed_this_frame[idx] {
                set_desktop_button(idx, true);
                if released_this_frame[idx] {
                    pending_release[idx] = true;
                }
            } else if released_this_frame[idx] {
                set_desktop_button(idx, false);
            }
        }

        game.tick();

        for idx in 0..8 {
            if pending_release[idx] {
                set_desktop_button(idx, false);
                pending_release[idx] = false;
            }
        }

        blit(game.renderer(), &mut sim_display, &palette);
        window.update(&sim_display);

        let elapsed = frame_start.elapsed();
        let target = Duration::from_millis(FRAME_MS);
        if elapsed < target {
            thread::sleep(target - elapsed);
        }
    }

    // ESC or closing the window: save before exiting.
    game.save();
}

fn blit(renderer: &Renderer, target: &mut SimulatorDisplay<Rgb888>, palette: &Palette) {
    let fb = renderer.framebuffer();
    let invert = fb.invert();
    let fg = palette.fg;

    let _ = target.clear(palette.bg);
    let pixels = (0..HEIGHT as i32).flat_map(|y| {
        (0..WIDTH as i32).filter_map(move |x| {
            let lit = fb.pixel(x as usize, y as usize) ^ invert;
            if lit {
                Some(Pixel(Point::new(x, y), fg))
            } else {
                None
            }
        })
    });
    let _ = target.draw_iter(pixels);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_accepts_valid_colors() {
        assert_eq!(parse_hex("23a5cc"), Some(Rgb888::new(35, 165, 204)));
        assert_eq!(parse_hex("#23A5CC"), Some(Rgb888::new(35, 165, 204)));
        assert_eq!(parse_hex("0a0A0a"), Some(Rgb888::new(10, 10, 10)));
        assert_eq!(parse_hex("#ffffff"), Some(Rgb888::new(255, 255, 255)));
    }

    #[test]
    fn parse_hex_rejects_invalid_colors() {
        assert_eq!(parse_hex(""), None);
        assert_eq!(parse_hex("#"), None);
        assert_eq!(parse_hex("fff"), None);
        assert_eq!(parse_hex("1234567"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
        assert_eq!(parse_hex("##123456"), None);
        assert_eq!(parse_hex("+12345"), None);
    }
}
