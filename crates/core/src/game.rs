use crate::transport::Transport;
use crate::platform::{
    persist, watchdog,
    power::software_reset,
    radio::WIFI,
    rng::Rng,
    time::{Duration, Instant},
};

use crate::{
    catch_up,
    context::{GameContext, PowerAction},
    peer_manager::PeerManager,
    input::Buttons,
    led::Led,
    render::Renderer,
    save,
    scene::{SceneId, SceneManager},
    ui::confirm::{Confirm, ConfirmResult},
    sleep_manager::SleepManager,
    time_system::TimeSystem,
    transition::{TransitionManager, TransitionStep},
    location_tracker,
};
// Both the WiFi peripheral stash and the peer manager live on
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
    /// lives on `GameContext` so the peer manager and scenes can
    /// share it.
    location_next_scan: Option<Instant>,
    /// Promoted from a local in `run()` so `tick()` can compute dt across
    /// calls. Initialised lazily on the first tick.
    last_frame: Option<Instant>,
    /// True when this boot resumed a scene from a crash-resume intent. The
    /// intent is cleared after the first completed tick.
    resumed_intent: bool,
    /// Numeric-comparison pairing prompt (BLE only), drawn over any scene.
    pair_confirm: Confirm,
}

impl Game {
    pub fn new(
        renderer: Renderer,
        buttons: Buttons,
        rng: Rng,
        led: Led,
        radio_peripheral: WIFI<'static>,
        peers: PeerManager,
    ) -> Self {
        let mut slot = core::mem::MaybeUninit::<Game>::uninit();
        Self::init(&mut slot, renderer, buttons, rng, led, radio_peripheral, peers);
        // SAFETY: `init` fully initialised the slot.
        unsafe { slot.assume_init() }
    }

    /// Build the game directly inside `slot` so firmware can keep the 23 KB
    /// struct in static memory instead of copying it through stack frames.
    #[inline(never)]
    pub fn init<'a>(
        slot: &'a mut core::mem::MaybeUninit<Game>,
        renderer: Renderer,
        buttons: Buttons,
        rng: Rng,
        led: Led,
        radio_peripheral: WIFI<'static>,
        peers: PeerManager,
    ) -> &'a mut Game {
        let mut context = GameContext::new(led);
        // Seed the behavior RNG from the hardware peripheral so each boot's
        // behavior choices differ until/unless a save provides a seed.
        let seed = rng.random();
        context.rng = if seed == 0 { 1 } else { seed };
        context.hw_rng = rng;
        context.peers = Some(peers);
        context.radio_peripheral = Some(radio_peripheral);
        let (reset_label, brownout) = persist::reset_reason();
        println!("[Boot] Reset reason: {}", reset_label);
        context.last_reset_reason = reset_label;
        println!("[DBG] before crash_take");
        context.last_crash = persist::crash_take();
        println!("[DBG] after crash_take");
        if let Some(crash) = &context.last_crash {
            println!("[Boot] Previous reset was a panic: {}", crash.msg);
            for pc in crash.pcs.iter() {
                println!("[Boot]   at 0x{:08x}", pc);
            }
        }

        let hs = save::has_save();
        println!("[DBG] has_save = {}", hs);
        let loaded = hs && save::load(&mut context);
        println!("[DBG] loaded = {}", loaded);

        // A valid sleep stamp means this boot is a wake from deep sleep.
        // Land on the home scene and catch the pet up on the time asleep.
        let mut time_system = TimeSystem::new();
        let slept_us = persist::sleep_record_take().filter(|_| loaded);
        if let Some(start_us) = slept_us {
            let elapsed_s = persist::rtc_now_us().saturating_sub(start_us) as f32 / 1.0e6;
            println!("[Boot] Woke from deep sleep after {}s", elapsed_s as u32);
            time_system.advance(&mut context, elapsed_s);
            context.apply_stat_changes(&catch_up::catch_up(elapsed_s));
            context.pending_wake_greeting = true;
            persist::intent_clear();
        }

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
        let location_next_scan = if context.scan_started || brownout {
            println!("[Boot] Deferring first wifi scan");
            Some(Instant::now() + location_tracker::SCAN_INTERVAL)
        } else {
            None
        };
        context.scan_started = false;

        let p = slot.as_mut_ptr();
        // SAFETY: every field of Game is written exactly once before `assume_init_mut`.
        unsafe {
            let scene_manager = core::ptr::addr_of_mut!((*p).scene_manager);
            scene_manager.write(SceneManager::new(&mut context, start));
            core::ptr::addr_of_mut!((*p).renderer).write(renderer);
            core::ptr::addr_of_mut!((*p).buttons).write(buttons);
            core::ptr::addr_of_mut!((*p).context).write(context);
            core::ptr::addr_of_mut!((*p).time_system).write(time_system);
            core::ptr::addr_of_mut!((*p).sleep_manager).write(SleepManager::new());
            core::ptr::addr_of_mut!((*p).transition).write(TransitionManager::new());
            core::ptr::addr_of_mut!((*p).pending_scene).write(None);
            core::ptr::addr_of_mut!((*p).sleep_pending).write(false);
            core::ptr::addr_of_mut!((*p).deep_sleep_pending).write(false);
            core::ptr::addr_of_mut!((*p).just_woke).write(false);
            core::ptr::addr_of_mut!((*p).last_dt_ms).write(0);
            core::ptr::addr_of_mut!((*p).location_next_scan).write(location_next_scan);
            core::ptr::addr_of_mut!((*p).last_frame).write(None);
            core::ptr::addr_of_mut!((*p).resumed_intent).write(resumed);
            core::ptr::addr_of_mut!((*p).pair_confirm).write(Confirm::new());
            slot.assume_init_mut()
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

        // Pull any inbound peer frames from the driver into our inbox
        // before scene update so a scene-side dispatcher can drain them.
        // No-op when the manager has not been started by a social scene.
        if let Some(peers) = self.context.peers.as_mut() {
            peers.poll();
        }
        self.update_pairing_prompt();

        self.update(dt);

        // Debug-scene-requested scan. Runs immediately (no transition
        // cover) since the player is staring at a debug screen and
        // explicitly asked for fresh data.
        if self.context.location_scan_requested {
            self.context.location_scan_requested = false;
            // No pre-scan save here: repeated debug presses would mean
            // repeated flash writes. The hourly clock still restarts.
            self.location_next_scan = Some(Instant::now() + location_tracker::SCAN_INTERVAL);
            location_tracker::scan_now(&mut self.context);
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
        crate::platform::stackmark::report("run entry");
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
                    let go_deep = self.sleep_manager.enter_sleep(
                        &mut self.renderer,
                        &mut self.buttons,
                        &mut self.context,
                        &mut self.scene_manager,
                        &mut self.time_system,
                    );
                    if go_deep {
                        self.enter_deep_sleep();
                    }
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

    /// Show the pairing code from the transport and route A/B to it, hiding
    /// input from the scenes while the prompt is up.
    fn update_pairing_prompt(&mut self) {
        let code = self.context.peers.as_ref().and_then(|p| p.pairing_code());
        match code {
            Some(c) if !self.pair_confirm.is_open() => {
                use core::fmt::Write as _;
                let mut text: heapless::String<32> = heapless::String::new();
                let _ = write!(text, "Pair? {:06}", c);
                self.pair_confirm.open(text.as_str());
            }
            None if self.pair_confirm.is_open() => self.pair_confirm.close(),
            _ => {}
        }
        if self.pair_confirm.is_open() {
            let accept = match self.pair_confirm.handle_input(&mut self.buttons) {
                ConfirmResult::Confirmed => Some(true),
                ConfirmResult::Cancelled => Some(false),
                ConfirmResult::Pending => None,
            };
            if let Some(a) = accept {
                if let Some(p) = self.context.peers.as_mut() {
                    p.answer_pairing(a);
                }
            }
            self.buttons.consume_all();
        }
    }

    /// Run a wifi scan if one is due. Called only from the transition
    /// midpoint so the ~1-3 s blocking scan is hidden behind a black screen.
    /// `location_next_scan = None` means "scan immediately" (a boot whose save
    /// was not taken right before a scan).
    fn maybe_scan_wifi(&mut self) {
        // No `ctx.wifi` check anymore: the controller doesn't exist at
        // rest. `scan_now` does its own acquire/release.
        let now = Instant::now();
        let due = match self.location_next_scan {
            None => true,
            Some(t) => now >= t,
        };
        if !due {
            return;
        }
        // Push the next scan out before scanning, success or not, so a
        // failing or resetting scan can't be retried on every swap.
        self.location_next_scan = Some(now + location_tracker::SCAN_INTERVAL);

        // The pre-scan save doubles as the hourly save (it restamps
        // `last_save_time`, holding off `save_if_needed`). Its
        // `scan_started` marker tells the next boot a scan just began.
        // Nothing is written after the scan; results ride along with the
        // next save.
        self.context.scan_started = true;
        save::save(&mut self.context);
        self.context.scan_started = false;

        location_tracker::scan_now(&mut self.context);
    }

    fn draw(&mut self) {
        self.renderer.clear();
        // Baseline; scenes that want lightning override this within their draw.
        self.renderer.set_invert(false);
        self.scene_manager
            .draw(&self.context, &mut self.renderer, self.last_dt_ms);
        self.transition.draw(&mut self.renderer);
        self.pair_confirm.draw(&mut self.renderer);
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

    /// One-way deep sleep. The device resets on wake. Which buttons can wake
    /// it depends on the chip; see [`crate::sleep_pins`].
    ///
    /// Desktop builds have no deep-sleep equivalent; the simulator exits
    /// the process instead.
    #[cfg(not(feature = "desktop"))]
    fn enter_deep_sleep(&mut self) -> ! {
        use esp_hal::{
            peripherals::LPWR,
            rtc_cntl::sleep::{LowPower, RtcSleepConfig},
        };

        use crate::{
            sleep_manager::wait_buttons_stable_released_mask,
            sleep_pins::{arm_deep_wake, DEEP_WAKE_BUTTONS},
        };

        println!("[Power] Entering deep sleep");
        save::save(&mut self.context);
        persist::sleep_record_set(persist::rtc_now_us());
        self.renderer.clear();
        self.renderer.flush();
        self.renderer.power_off();

        // Wait for the wake-capable buttons to be released and stable,
        // otherwise the level=Low wake trigger fires immediately on either
        // the still-held press or its release bounce.
        wait_buttons_stable_released_mask(
            &self.buttons,
            DEEP_WAKE_BUTTONS,
            Duration::from_millis(500),
        );

        // Safety: deep sleep is one-way; on wake the device boots from reset
        // and the old `Input` wrappers in `Buttons` will never be used again.
        let _wake_pins = unsafe { arm_deep_wake() };

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
