//! Manages device sleep modes for power saving.
//!
//! Currently implements only the "basic" mode:
//!
//!   - After `DIM_TIMEOUT` of no button activity, dim the display; the
//!     next press restores it (and still acts as a normal press).
//!   - After `SLEEP_TIMEOUT` of no button activity, power off the display.
//!   - Continue ticking the game at `SLEEP_FPS` so needs/time advance while
//!     invisible.
//!   - Any button press wakes the device; the wake press is consumed so it
//!     does not register as a game action.
//!   - On wake, set `ctx.pending_wake_greeting` so the pet greets the
//!     returning player on its next behavior pick, and signal the active
//!     scene's behavior manager to wind down quickly so the greeting fires
//!     promptly rather than at the next natural completion.
//!
//! While the screen is off the chip spends the gaps between slow ticks in
//! true light sleep (timer + button GPIO wake), so the idle current is a
//! fraction of a running CPU. If the radio is up (scan, ESP-NOW) it falls
//! back to the scheduler idle loop, since light sleep would stall the
//! radio.
//!
//! TODO(deep_sleep): implement a "deep" mode for basic auto-sleep (the
//! Power debug menu already has a one-way deep sleep).

use crate::platform::time::{self, Duration, Instant};
use crate::platform::{power, watchdog};

use crate::{
    context::GameContext,
    input::{Button, Buttons},
    render::Renderer,
    scene::SceneManager,
    time_system::TimeSystem,
};

/// Spin until none of the given buttons (or `Button::ALL` if `mask` is empty)
/// has been pressed for `stable` continuously. Any press resets the timer.
pub fn wait_buttons_stable_released(buttons: &Buttons, stable: Duration) {
    wait_buttons_stable_released_mask(buttons, &Button::ALL, stable);
}

pub fn wait_buttons_stable_released_mask(
    buttons: &Buttons,
    watch: &[Button],
    stable: Duration,
) {
    let mut released_since = Instant::now();
    loop {
        // A held button can keep this spinning indefinitely.
        watchdog::feed();
        let any_pressed = watch.iter().any(|&b| buttons.is_pressed(b));
        if any_pressed {
            released_since = Instant::now();
        } else if released_since.elapsed() >= stable {
            return;
        }
        time::delay(STABLE_POLL);
    }
}

/// Inactivity before the display dims.
pub const DIM_TIMEOUT: Duration = Duration::from_secs(60);

/// Inactivity before sleeping.
pub const SLEEP_TIMEOUT: Duration = Duration::from_secs(300);

/// Button poll interval while waiting for a stable release.
const STABLE_POLL: Duration = Duration::from_millis(10);

/// Button poll interval while asleep with the radio up (no light sleep).
/// Short enough that a tap wakes the device; the CPU idles between polls.
const WAKE_POLL: Duration = Duration::from_millis(50);

/// Game update rate while in basic sleep.
pub const SLEEP_FPS: u64 = 2;
pub const SLEEP_FRAME_TIME_MS: u64 = 1000 / SLEEP_FPS;

pub struct SleepManager {
    sleeping: bool,
    dimmed: bool,
    last_activity: Instant,
}

impl SleepManager {
    pub fn new() -> Self {
        Self {
            sleeping: false,
            dimmed: false,
            last_activity: Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub fn is_sleeping(&self) -> bool {
        self.sleeping
    }

    /// Reset the inactivity timer. Call from the main loop on any button press.
    pub fn notify_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Dim the display once idle for `DIM_TIMEOUT`, restore it on activity.
    /// Call once per frame, after `notify_activity`. Only talks to the panel
    /// when the state changes.
    pub fn update_dim(&mut self, renderer: &mut Renderer) {
        let dim = self.last_activity.elapsed() >= DIM_TIMEOUT;
        if dim != self.dimmed {
            self.dimmed = dim;
            renderer.set_dimmed(dim);
        }
    }

    /// True when the inactivity timeout has elapsed and sleep is appropriate.
    ///
    /// Desktop builds never auto-sleep. The basic-sleep path runs an
    /// internal `while !buttons.any_pressed()` loop that doesn't yield
    /// back to the desktop binary's SDL event pump, so the simulator
    /// would freeze instead of sleeping.
    pub fn should_sleep(&self) -> bool {
        #[cfg(feature = "desktop")]
        {
            return false;
        }
        #[cfg(not(feature = "desktop"))]
        {
            if self.sleeping {
                return false;
            }
            self.last_activity.elapsed() >= SLEEP_TIMEOUT
        }
    }

    /// Block in basic sleep until any button is pressed.
    ///
    /// The display is powered off for the duration; the game keeps ticking
    /// at `SLEEP_FPS` so needs and time still advance while the screen is
    /// dark. On wake we restore the display, consume the wake press, and
    /// flag a pending greeting.
    pub fn enter_sleep(
        &mut self,
        renderer: &mut Renderer,
        buttons: &mut Buttons,
        ctx: &mut GameContext,
        scene_manager: &mut SceneManager,
        time_system: &mut TimeSystem,
    ) {
        println!("[Sleep] Entering basic sleep");
        self.sleeping = true;
        renderer.power_off();

        // Wait until all buttons have been released for a stable period
        // before arming the wake check. Without this, the A-press that
        // triggered an explicit sleep (or its release bounce) is read as
        // an instant wake.
        wait_buttons_stable_released(buttons, Duration::from_millis(500));

        let mut last_tick = Instant::now();
        loop {
            watchdog::feed();
            if buttons.any_pressed() {
                break;
            }
            let elapsed_ms = last_tick.elapsed().as_millis();
            if elapsed_ms >= SLEEP_FRAME_TIME_MS {
                last_tick = Instant::now();
                let dt = (elapsed_ms as f32) / 1000.0 * ctx.time_speed;
                time_system.advance(ctx, dt);
                scene_manager.sleep_update(ctx, buttons, dt);
            }

            if ctx.radio_users == 0 {
                // Sleep through to the next slow tick unless a button
                // wakes us first.
                let next_tick = last_tick + Duration::from_millis(SLEEP_FRAME_TIME_MS);
                buttons.arm_wakeup();
                power::light_sleep_until(next_tick);
                buttons.disarm_wakeup();
                // A quick tap can be released before we read the pins.
                if buttons.caused_wakeup() {
                    break;
                }
            } else {
                time::delay(WAKE_POLL);
            }
        }

        println!("[Sleep] Waking from basic sleep");
        self.sleeping = false;
        self.last_activity = Instant::now();
        // Undim before the panel comes back so the first frame is bright.
        self.update_dim(renderer);
        renderer.power_on();
        // Drop the wake press so it doesn't fire a game action.
        buttons.consume_all();
        // Greet the returning player on the next behavior pick.
        ctx.pending_wake_greeting = true;
        // Tell the active behavior to wind down fast. Sleeping / napping
        // either honor this (rouse in ~3s) or veto the greeting and stay
        // asleep based on serenity (matches the MicroPython port).
        scene_manager.mark_behavior_almost_done(ctx);
    }
}
