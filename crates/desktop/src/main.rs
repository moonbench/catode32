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

/// "Lit pixel" tint.
const PIXEL_ON: Rgb888 = Rgb888::new(35, 165, 204);
/// Background (a near-black charcoal, not pure 0,0,0 so the off pixels read
/// as "screen, not void").
const PIXEL_OFF: Rgb888 = Rgb888::new(10, 10, 10);

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

        blit(game.renderer(), &mut sim_display);
        window.update(&sim_display);

        let elapsed = frame_start.elapsed();
        let target = Duration::from_millis(FRAME_MS);
        if elapsed < target {
            thread::sleep(target - elapsed);
        }
    }
}

fn blit(renderer: &Renderer, target: &mut SimulatorDisplay<Rgb888>) {
    let fb = renderer.framebuffer();
    let invert = fb.invert();

    let _ = target.clear(PIXEL_OFF);
    let pixels = (0..HEIGHT as i32).flat_map(|y| {
        (0..WIDTH as i32).filter_map(move |x| {
            let lit = fb.pixel(x as usize, y as usize) ^ invert;
            if lit {
                Some(Pixel(Point::new(x, y), PIXEL_ON))
            } else {
                None
            }
        })
    });
    let _ = target.draw_iter(pixels);
}
