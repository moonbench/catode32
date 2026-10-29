#![no_std]
#![no_main]

mod panic;

use catode32_core::{
    board, peer_manager::PeerManager, game::Game, platform::watchdog, render::Renderer,
    storage,
};
use esp_hal::{clock::CpuClock, main, timer::timg::TimerGroup};
use esp_println::println;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    // Heap for the wifi stack. esp-radio (the wifi driver) and esp-rtos (the
    // scheduler it needs) both allocate from a global heap. 64 KB is the size
    // the esp-rs maintainers recommend for scan-only wifi; it holds the
    // driver's permanent buffers plus transient per-scan AP lists with
    // comfortable headroom. Nothing else in this firmware allocates.
    // The H2 has less RAM than the C3/C6, and the game state (a ~16 KB scene
    // moved by value during startup) needs a ~90 KB stack. With a 64 KB heap
    // the stack was ~64 KB and overflowed silently, so use a smaller heap.
    catode32_core::platform::stackmark::paint();
    #[cfg(feature = "h2")]
    esp_alloc::heap_allocator!(size: 56 * 1024);
    #[cfg(not(feature = "h2"))]
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    // `init` disables every watchdog. Arm ours before anything that could
    // hang (flash, display, radio scheduler); the game loop feeds it.
    watchdog::start();
    let board = board::init(peripherals);
    storage::init(board.flash);
    let renderer = Renderer::new(board.i2c);

    println!("catode32 v0.13.0 behavior framework");

    // Start the preemptive scheduler. The radio stack drives its event loop
    // on a task scheduled by esp-rtos, so the scheduler must be running
    // before `WifiController::new`. Our main game loop runs on top of this
    // as the "main" task.
    let timg0 = TimerGroup::new(board.timg0);
    esp_rtos::start(timg0.timer0, board.sw_int0);

    // No wifi init at boot. The radio is brought up on demand by
    // `radio::acquire` and dropped again on the last release. The
    // peripheral and an empty peer manager are stashed on the game
    // context so the first acquire can do the actual `wifi::new`.
    static mut GAME: core::mem::MaybeUninit<Game> = core::mem::MaybeUninit::uninit();
    // SAFETY: `main` runs once; GAME is only ever touched here.
    let slot = unsafe { &mut *core::ptr::addr_of_mut!(GAME) };
    Game::init(
        slot,
        renderer,
        board.buttons,
        board.rng,
        board.led,
        board.radio,
        PeerManager::new(),
    )
    .run();
}
