//! Manages device sleep modes for power saving.
//!
//! Currently implements only the "basic" mode:
//!
//!   - After `DIM_TIMEOUT` of no button activity, dim the display; the
//!     next press restores it (and still acts as a normal press).
//!   - After `SLEEP_TIMEOUT` of no button activity, power off the display.
//!   - Keep simulating while invisible so needs/time advance: the game is
//!     stepped at a fixed `SLEEP_STEP_MS`, but in batches that run once per
//!     `SLEEP_SIM_INTERVAL` (see "Power" below).
//!   - Any button press wakes the device; the wake press is consumed so it
//!     does not register as a game action.
//!   - On wake, set `ctx.pending_wake_greeting` so the pet greets the
//!     returning player on its next behavior pick, and signal the active
//!     scene's behavior manager to wind down quickly so the greeting fires
//!     promptly rather than at the next natural completion.
//!
//! Power: while the screen is off the chip spends the gaps between batches
//! in true light sleep (timer + button GPIO wake). Each batch replays the
//! elapsed time as `SLEEP_STEP_MS` sub-steps. The sim isn't dt-invariant
//! (behavior phases drop leftover time, walkers overshoot their targets),
//! so one big step would not be equivalent. Batching only saves the fixed
//! per-wake cost of leaving and re-entering light sleep. If the radio is up
//! (scan, ESP-NOW) it falls back to the scheduler idle loop and steps as
//! soon as each step is due, since light sleep would stall the radio.
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

/// Run every whole `SLEEP_STEP_MS` step between `sim_time` and now, advancing
/// `sim_time` by the steps taken. Leftover time under one step carries to the
/// next call. Capped at `MAX_CATCH_UP_STEPS`.
fn catch_up(
    ctx: &mut GameContext,
    buttons: &mut Buttons,
    scene_manager: &mut SceneManager,
    time_system: &mut TimeSystem,
    sim_time: &mut Instant,
) {
    let step = Duration::from_millis(SLEEP_STEP_MS);
    let mut steps = 0;
    while sim_time.elapsed() >= step {
        if steps == MAX_CATCH_UP_STEPS {
            println!("[Sleep] Catch-up backlog dropped");
            *sim_time = Instant::now();
            break;
        }
        watchdog::feed();
        let dt = (SLEEP_STEP_MS as f32) / 1000.0 * ctx.time_speed;
        time_system.advance(ctx, dt);
        scene_manager.sleep_update(ctx, buttons, dt);
        *sim_time = *sim_time + step;
        steps += 1;
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

/// Simulation step while in basic sleep (real time, before `time_speed`).
pub const SLEEP_STEP_MS: u64 = 500;

/// How often the chip leaves light sleep to run a batch of sim steps. The
/// screen stays off; this is not a wake.
pub const SLEEP_SIM_INTERVAL: Duration = Duration::from_secs(15);

/// Most steps one batch will run. A batch normally covers one
/// `SLEEP_SIM_INTERVAL` (30 steps); anything past this is dropped rather
/// than stalling the wake on a huge backlog.
const MAX_CATCH_UP_STEPS: u32 = 120;

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

        // Sim time simulated so far. Advanced in exact steps (never reset
        // to `now()`) so no fraction of a step is lost between batches.
        let mut sim_time = Instant::now();
        loop {
            watchdog::feed();
            if buttons.any_pressed() {
                break;
            }

            // Light sleep stalls the radio, so only when neither WiFi nor
            // Bluetooth is up.
            if ctx.radio_users == 0 && !crate::ble::status().is_active() {
                if sim_time.elapsed() >= SLEEP_SIM_INTERVAL {
                    catch_up(ctx, buttons, scene_manager, time_system, &mut sim_time);
                }
                // Sleep through to the next batch unless a button wakes
                // us first.
                buttons.arm_wakeup();
                power::light_sleep_until(sim_time + SLEEP_SIM_INTERVAL);
                buttons.disarm_wakeup();
                // A quick tap can be released before we read the pins.
                if buttons.caused_wakeup() {
                    break;
                }
            } else {
                catch_up(ctx, buttons, scene_manager, time_system, &mut sim_time);
                time::delay(WAKE_POLL);
            }
        }

        // Simulate the partial interval since the last batch, so the time
        // before the press isn't lost.
        catch_up(ctx, buttons, scene_manager, time_system, &mut sim_time);

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
