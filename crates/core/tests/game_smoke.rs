//! End-to-end smoke test: build a `Game` with desktop platform handles,
//! tick it a few times, and confirm something landed in the framebuffer.
//! This is the cheapest possible regression net against "I cfg-gated a
//! module wrong and the desktop build no longer reaches scene::draw".

#![cfg(feature = "desktop")]

use std::env;
use std::fs;

use catode32_core::{
    peer_manager::PeerManager,
    game::Game,
    input::{ButtonPin, Buttons},
    led::Led,
    platform::{radio::WIFI, rng::Rng},
    render::Renderer,
    storage,
};

#[test]
fn ten_ticks_paints_pixels() {
    // Move CWD into a temp dir so we don't touch a real save file in the
    // repo root, then start fresh by erasing whatever's there.
    let prev = env::current_dir().expect("get cwd");
    let mut tmp = env::temp_dir();
    tmp.push(format!(
        "catode32-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ));
    fs::create_dir_all(&tmp).expect("create tmp");
    env::set_current_dir(&tmp).expect("set cwd");

    let _restore = scopeguard(|| {
        let _ = env::set_current_dir(&prev);
        let _ = fs::remove_dir_all(&tmp);
    });

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
    let mut game = Game::new(
        renderer,
        buttons,
        Rng::new(),
        Led::new(),
        WIFI::new(),
        PeerManager::new(),
    );

    for _ in 0..10 {
        game.tick();
    }

    // Some scene drew at least one pixel, proves the renderer reached
    // the framebuffer through the full pipeline.
    let fb = game.renderer().framebuffer();
    let mut any_lit = false;
    'outer: for y in 0..64 {
        for x in 0..128 {
            if fb.pixel(x, y) {
                any_lit = true;
                break 'outer;
            }
        }
    }
    assert!(any_lit, "expected at least one pixel set after 10 ticks");
}

/// Minimal Drop guard so we restore CWD even if the test panics.
struct ScopeGuard<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Drop for ScopeGuard<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

fn scopeguard<F: FnOnce()>(f: F) -> ScopeGuard<F> {
    ScopeGuard(Some(f))
}
