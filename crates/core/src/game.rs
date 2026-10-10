use crate::platform::{
    persist, watchdog,
    power::software_reset,
    radio::WIFI,
    rng::Rng,
    time::{Duration, Instant},
};

use crate::{
    context::{GameContext, PowerAction},
    espnow_manager::EspNowManager,
    input::Buttons,
    led::Led,
    render::Renderer,
    save,
    scene::{SceneId, SceneManager},
    sleep_manager::SleepManager,
    time_system::TimeSystem,
    transition::{TransitionManager, TransitionStep},
    wifi_tracker,
};
// Both the WiFi peripheral stash and the ESP-NOW manager live on
// `GameContext`. The wifi controller itself only exists between
// `radio::acquire` / `radio::release` calls.

/// Boots allowed to resume an interrupted scene change before giving up.
const MAX_RESUME_ATTEMPTS: u8 = 2;

const FPS: u64 = 12;
const FRAME_TIME_MS: u64 = 1000 / FPS;

pub struct Game {
    renderer: Renderer,
    buttons: Buttons,
    context: GameContext,
    scene_manager: SceneManager,
    time_system: TimeSystem,
    sleep_manager: SleepManager,
    transition: TransitionManager,
    /// Scene swap waiting on the current transition's midpoint.
    pending_scene: Option<SceneId>,
    /// True while a transition-out is playing pre-sleep.
    sleep_pending: bool,
    /// True while a transition-out is playing before deep sleep. Deep sleep
    /// is one-way (device resets on wake), so there is no `in_only` reveal
    /// to play afterwards.
    deep_sleep_pending: bool,
    /// True on the first frame after a sleep-wake transition was started, so
    /// the frame-timer can be reset and we don't apply a huge dt spike.
    just_woke: bool,
    last_dt_ms: u64,
    /// Real-time instant at which the next wifi scan becomes due.
    /// `None` means "scan ASAP" (used on boot). The controller itself
    /// lives on `GameContext` so the ESP-NOW manager and scenes can
    /// share it.
    wifi_next_scan: Option<Instant>,
    /// Promoted from a local in `run()` so `tick()` can compute dt across
    /// calls. Initialised lazily on the first tick.
    last_frame: Option<Instant>,
    /// True when this boot resumed a scene from a crash-resume intent. The
    /// intent is cleared after the first completed tick.
    resumed_intent: bool,
}

impl Game {
    pub fn new(
        renderer: Renderer,
        buttons: Buttons,
        rng: Rng,
        led: Led,
        wifi_peripheral: WIFI<'static>,
        espnow: EspNowManager,
    ) -> Self {
        let mut context = GameContext::new(led);
        // Seed the behavior RNG from the hardware peripheral so each boot's
        // behavior choices differ until/unless a save provides a seed.
        let seed = rng.random();
        context.rng = if seed == 0 { 1 } else { seed };
        context.hw_rng = rng;
        context.espnow = Some(espnow);
        context.wifi_peripheral = Some(wifi_peripheral);
        let (reset_label, brownout) = persist::reset_reason();
        println!("[Boot] Reset reason: {}", reset_label);
        context.last_reset_reason = reset_label;
        context.last_crash = persist::crash_take();
        if let Some(crash) = &context.last_crash {
            println!("[Boot] Previous reset was a panic: {}", crash.msg);
            for pc in crash.pcs.iter() {
                println!("[Boot]   at 0x{:08x}", pc);
            }
        }

        let loaded = save::has_save() && save::load(&mut context);

        // Resume a scene change that a reset interrupted. Capped so a scene
        // that crashes on entry gets at most MAX_RESUME_ATTEMPTS boots before
        // we fall back to the normal start scene.
        let mut resumed = false;
        let start = match persist::intent_peek() {
            Some((id, attempts)) if loaded && attempts < MAX_RESUME_ATTEMPTS => {
                match SceneId::from_u8(id).filter(|s| s.resumable()) {
                    Some(scene) => {
                        println!("[Boot] Resuming {:?} (attempt {})", scene, attempts + 1);
                        persist::intent_rearm(id, attempts + 1);
                        resumed = true;
                        scene
                    }
                    None => SceneId::Inside,
                }
            }
            Some((id, attempts)) => {
                println!("[Boot] Dropping intent {} after {} attempt(s)", id, attempts);
                if loaded { SceneId::Inside } else { SceneId::Adoption }
            }
            None if loaded => SceneId::Inside,
            None => SceneId::Adoption,
        };
        if !resumed {
            persist::intent_clear();
        }

        // No clock survives a reset, so a save written just before a scan
        // means "the last scan was at this save's moment": wait a full
        // interval. A brownout also defers, in case it hit during that save
        // before the marker was committed.
        let wifi_next_scan = if context.scan_started || brownout {
            println!("[Boot] Deferring first wifi scan");
            Some(Instant::now() + wifi_tracker::SCAN_INTERVAL)
        } else {
            None
        };
        context.scan_started = false;

        let scene_manager = SceneManager::new(&mut context, start);
        Self {
            renderer,
            buttons,
            context,
            scene_manager,
            time_system: TimeSystem::new(),
            sleep_manager: SleepManager::new(),
            transition: TransitionManager::new(),
            pending_scene: None,
            sleep_pending: false,
            deep_sleep_pending: false,
            just_woke: false,
            last_dt_ms: 0,
            wifi_next_scan,
            last_frame: None,
            resumed_intent: resumed,
        }
    }

    /// Run a single frame: input snapshot, update, draw, post-frame
    /// bookkeeping. Does **not** pace the frame; the caller decides when
    /// to invoke the next tick (the firmware busy-waits to 12 FPS in
    /// `run()`; the desktop binary uses the simulator window's vsync /
    /// sleep instead).
    pub fn tick(&mut self) {
        let frame_start = Instant::now();
        let last_frame = self.last_frame.unwrap_or(frame_start);
        let elapsed_ms = (frame_start.duration_since_epoch()
            - last_frame.duration_since_epoch())
        .as_millis();
        self.last_frame = Some(frame_start);
        self.last_dt_ms = elapsed_ms;

        let dt = elapsed_ms as f32 / 1000.0;

        // Reset the inactivity timer whenever the player is touching a button.
        if self.buttons.any_pressed() {
            self.sleep_manager.notify_activity();
        }

        // Pull any inbound ESP-NOW frames from the driver into our inbox
        // before scene update so a scene-side dispatcher can drain them.
        // No-op when the manager has not been started by a social scene.
        if let Some(espnow) = self.context.espnow.as_mut() {
            espnow.poll();
        }

        self.update(dt);

        // Debug-scene-requested scan. Runs immediately (no transition
        // cover) since the player is staring at a debug screen and
        // explicitly asked for fresh data.
        if self.context.wifi_scan_requested {
            self.context.wifi_scan_requested = false;
            // No pre-scan save here: repeated debug presses would mean
            // repeated flash writes. The hourly clock still restarts.
            self.wifi_next_scan = Some(Instant::now() + wifi_tracker::SCAN_INTERVAL);
            wifi_tracker::scan_now(&mut self.context);
        }

        self.draw();

        if let Some(action) = self.context.pending_power.take() {
            self.handle_power_action(action);
        }

        // Begin a sleep transition if idle long enough. The actual sleep
        // happens at the transition midpoint inside `update`; the
        // in-only reveal then plays automatically on wake.
        if !self.transition.is_active()
            && !self.sleep_pending
            && self.sleep_manager.should_sleep()
        {
            self.sleep_pending = true;
            self.transition.start();
        }

        // Sleep ran inside `update` at the transition midpoint. Reset
        // the frame timer so the next iteration doesn't see the entire
        // sleep duration as one dt.
        if self.just_woke {
            self.just_woke = false;
            self.last_frame = Some(Instant::now());
        }

        // A resumed scene survived a full frame, so the intent is done.
        if self.resumed_intent {
            self.resumed_intent = false;
            persist::intent_clear();
        }
    }

    /// Borrow the renderer so the desktop binary can read its framebuffer
    /// between ticks.
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// Persist the current state, e.g. before the desktop simulator quits.
    /// Skipped while the adoption scene is up, so quitting mid-adoption
    /// doesn't write a nameless pet that would skip adoption next launch.
    pub fn save(&mut self) -> bool {
        if self.scene_manager.current_id() == SceneId::Adoption {
            return false;
        }
        save::save(&mut self.context)
    }

    pub fn run(&mut self) -> ! {
        loop {
            let frame_start = Instant::now();
            watchdog::feed();
            self.tick();
            let frame_used = frame_start.elapsed().as_millis();
            if frame_used < FRAME_TIME_MS {
                let wait_start = Instant::now();
                let remaining = Duration::from_millis(FRAME_TIME_MS - frame_used);
                while wait_start.elapsed() < remaining {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        // Single point of `time_speed` scaling for the whole frame — every
        // subsystem downstream (time-of-day, sky, behaviors, animations,
        // transitions, minigames) sees the scaled dt. Mirrors the
        // MicroPython main loop; do not re-scale inside subsystems.
        let dt = dt * self.context.time_speed;
        self.time_system.advance(&mut self.context, dt);
        let requested = self
            .scene_manager
            .update(&mut self.context, &mut self.buttons, dt);

        // Stash a scene swap behind the transition. Requests that arrive
        // while another transition is already running are dropped.
        if !self.transition.is_active() {
            if let Some(next) = requested {
                self.pending_scene = Some(next);
                self.transition.start();
            }
        }

        match self.transition.update(dt) {
            TransitionStep::Midpoint => {
                // Scene swap at midpoint also doubles as the cover for a
                // wifi scan. The screen is fully black, the scan takes
                // ~1-3 s, and the player just sees a slightly longer fade.
                // Triggered on any transition that's also swapping a scene
                // (so menu-only transitions don't pay the cost).
                if let Some(next) = self.pending_scene.take() {
                    // Record the intent in RTC RAM (no flash write) so a
                    // reset during the swap or the scan resumes into `next`.
                    if next.resumable() {
                        persist::intent_set(next.to_u8());
                    }
                    self.scene_manager.swap_to(&mut self.context, next);
                    self.maybe_scan_wifi();
                    persist::intent_clear();
                }
                if self.sleep_pending {
                    self.sleep_pending = false;
                    self.sleep_manager.enter_sleep(
                        &mut self.renderer,
                        &mut self.buttons,
                        &mut self.context,
                        &mut self.scene_manager,
                        &mut self.time_system,
                    );
                    // Replace the auto-advanced `In` phase with a fresh
                    // in-only reveal so the wake fade plays from full black.
                    self.transition.start_in_only();
                    self.just_woke = true;
                }
                if self.deep_sleep_pending {
                    // One-way: enter_deep_sleep never returns. The screen is
                    // already fully black from the out phase, so no reveal
                    // is needed (and would never play, device resets on wake).
                    self.enter_deep_sleep();
                }
            }
            TransitionStep::Active | TransitionStep::Inactive => {}
        }
    }

    /// Run a wifi scan if one is due. Called only from the transition
    /// midpoint so the ~1-3 s blocking scan is hidden behind a black screen.
    /// `wifi_next_scan = None` means "scan immediately" (a boot whose save
    /// was not taken right before a scan).
    fn maybe_scan_wifi(&mut self) {
        // No `ctx.wifi` check anymore: the controller doesn't exist at
        // rest. `scan_now` does its own acquire/release.
        let now = Instant::now();
        let due = match self.wifi_next_scan {
            None => true,
            Some(t) => now >= t,
        };
        if !due {
            return;
        }
        // Push the next scan out before scanning, success or not, so a
        // failing or resetting scan can't be retried on every swap.
        self.wifi_next_scan = Some(now + wifi_tracker::SCAN_INTERVAL);

        // The pre-scan save doubles as the hourly save (it restamps
        // `last_save_time`, holding off `save_if_needed`). Its
        // `scan_started` marker tells the next boot a scan just began.
        // Nothing is written after the scan; results ride along with the
        // next save.
        self.context.scan_started = true;
        save::save(&mut self.context);
        self.context.scan_started = false;

        wifi_tracker::scan_now(&mut self.context);
    }

    fn draw(&mut self) {
        self.renderer.clear();
        // Baseline; scenes that want lightning override this within their draw.
        self.renderer.set_invert(false);
        self.scene_manager
            .draw(&self.context, &mut self.renderer, self.last_dt_ms);
        self.transition.draw(&mut self.renderer);
        self.renderer.flush();
    }

    fn handle_power_action(&mut self, action: PowerAction) {
        match action {
            PowerAction::Reboot => {
                println!("[Power] Software reset");
                // Persist state before the user-initiated reset so progress
                // isn't lost. The reboot itself is the explicit action; the
                // save is incidental.
                save::save(&mut self.context);
                software_reset();
            }
            PowerAction::LightSleep => {
                // Desktop has no light sleep. The basic-sleep loop
                // wouldn't yield back to the SDL pump and the simulator
                // would freeze. Swallow the action so the debug menu's
                // "Sleep" button just no-ops.
                #[cfg(feature = "desktop")]
                {
                    println!("[Power] Light sleep ignored on desktop");
                }
                #[cfg(not(feature = "desktop"))]
                {
                    println!("[Power] Light sleep (basic mode)");
                    // Defer to the transition path so the screen fades out
                    // before the sleep loop blocks.
                    if !self.transition.is_active() && !self.sleep_pending {
                        self.sleep_pending = true;
                        self.transition.start();
                    }
                }
            }
            PowerAction::DeepSleep => {
                // Defer to the transition path so the screen fades out
                // before the (one-way) deep sleep. If a transition is
                // already active, fall through to an immediate cut.
                if !self.transition.is_active()
                    && !self.sleep_pending
                    && !self.deep_sleep_pending
                {
                    self.deep_sleep_pending = true;
                    self.transition.start();
                } else {
                    self.enter_deep_sleep();
                }
            }
        }
    }

    /// One-way deep sleep. Wakes the device only on a falling edge on
    /// GPIO0/1/2/3 (A/B/Menu1/Menu2 buttons). Direction buttons live on
    /// GPIO14/18/19/20, outside the C6's LP-IO domain, so they cannot
    /// serve as wake sources. The device resets on wake.
    ///
    /// Desktop builds have no deep-sleep equivalent; the simulator exits
    /// the process instead.
    ///
    /// TODO(c3): the wake pins are the C6 mapping. Needs hardware to verify:
    /// * On the C3 only GPIO0-5 can wake the chip. GPIO0-3 are the D-pad,
    ///   A/B are GPIO4/5, and Menu1/Menu2 (GPIO10/11) can't wake it at all.
    ///   Wake set probably becomes A/B only, chosen per board.
    /// * Once the C3 build compiles, re-check it for warnings.
    #[cfg(not(feature = "desktop"))]
    fn enter_deep_sleep(&mut self) -> ! {
        use esp_hal::{
            gpio::{Event, Input, InputConfig, Pull, WakeupConfig},
            peripherals::{GPIO0, GPIO1, GPIO2, GPIO3, LPWR},
            rtc_cntl::sleep::{LowPower, RtcSleepConfig},
        };

        use crate::{input::Button, sleep_manager::wait_buttons_stable_released_mask};

        const DEEP_WAKE_BUTTONS: [Button; 4] =
            [Button::A, Button::B, Button::Menu1, Button::Menu2];

        println!("[Power] Entering deep sleep");
        self.renderer.clear();
        self.renderer.flush();
        self.renderer.power_off();

        // Wait for the wake-capable buttons to be released and stable,
        // otherwise the level=Low wake trigger fires immediately on either
        // the still-held press or its release bounce.
        wait_buttons_stable_released_mask(
            &self.buttons,
            &DEEP_WAKE_BUTTONS,
            Duration::from_millis(500),
        );

        // Safety: deep sleep is one-way; on wake the device boots from reset
        // and the old `Input` wrappers in `Buttons` will never be used again.
        let input_config = InputConfig::default().with_pull(Pull::Up);
        let mut pins = [
            Input::new(unsafe { GPIO0::steal() }, input_config),
            Input::new(unsafe { GPIO1::steal() }, input_config),
            Input::new(unsafe { GPIO2::steal() }, input_config),
            Input::new(unsafe { GPIO3::steal() }, input_config),
        ];

        // Sleep entry copies these pull-ups onto the LP-IO pads before it
        // holds them, so the level=Low trigger doesn't see a floating pin.
        let wake_config = WakeupConfig::default().with_low_power_path(true);
        for pin in &mut pins {
            pin.listen(Event::LowLevel);
            pin.apply_wakeup_config(&wake_config).unwrap();
        }

        watchdog::stop();
        LowPower::new(unsafe { LPWR::steal() }).sleep_deep(RtcSleepConfig::deep());
    }

    #[cfg(feature = "desktop")]
    fn enter_deep_sleep(&mut self) -> ! {
        println!("[Power] Deep sleep, exiting simulator");
        self.renderer.clear();
        self.renderer.flush();
        std::process::exit(0)
    }
}
