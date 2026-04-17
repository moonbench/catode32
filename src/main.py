import sys
import time
import config

from input import InputHandler
from renderer import Renderer
from context import GameContext
from scene_manager import SceneManager
from weather_system import WeatherSystem
from time_system import TimeSystem
from splash import show_splash

if config.WIFI_ENABLED:
    from espnow_manager import EspNowManager
    from espnow_handler import EspNowHandler
    from visit_manager import VisitManager


class Game:
    def __init__(self):
        print("==> Virtual Pet Starting...")

        self.renderer = Renderer()
        show_splash(self.renderer)
        del sys.modules['splash']

        self.input = InputHandler()
        self.context = GameContext()
        self.context.load()
        self.context.input = self.input  # expose input to behaviors

        if config.WIFI_ENABLED:
            espnow = EspNowManager()
            self.context.espnow = espnow
        else:
            espnow = None

        # Scan WiFi at boot while memory is cleanest (splash is already showing)
        if config.WIFI_ENABLED:
            try:
                import wifi_tracker
                wifi_tracker.scan_now(self.context)
                del sys.modules['wifi_tracker']
            except Exception as e:
                print("[Boot] WiFi scan failed: " + str(e))

        self.scene_manager = SceneManager(self.context, self.renderer, self.input)
        self.scene_manager.change_scene_by_name('inside')

        self.weather_system = WeatherSystem()
        if 'weather' not in self.context.environment:
            self.weather_system.init_environment(self.context.environment, self.context.pet_seed)

        self.time_system = TimeSystem()
        self.time_system.update_moon_phase(self.context.environment)

        self.espnow_handler = EspNowHandler(espnow, self.scene_manager) if espnow else None
        self.visit_manager = VisitManager(self.context, self.scene_manager) if espnow else None
        self.context.visit_manager = self.visit_manager

        # Collect frequently to limit heap fragmentation.
        # Trigger after every ~40KB of allocations rather than waiting for OOM.
        import gc as _gc
        _gc.threshold(24000)
        del _gc

        self.last_frame_time = time.ticks_ms()
        # Simulated time rate: game minutes per real second (full day = 24 real minutes)
        self.time_system.game_minutes_per_second = 1.0

        if config.SLEEP_MODE:
            from sleep_manager import SleepManager
            self.sleep_manager = SleepManager(self.input, self.renderer)
        else:
            self.sleep_manager = None
        self._sleep_pending = False   # True while transition-out is playing pre-sleep
        self._woke_from_sleep = False  # True on the first frame after waking

    def _on_sleep_midpoint(self):
        """Called at the transition-out midpoint: enter sleep, then let transition-in play on wake."""
        # Deactivate the transition so scene updates work normally inside the sleep loop.
        # start_in_only() will re-activate it after the device wakes.
        self.scene_manager.transitions.active = False

        self.sleep_manager.enter_sleep(self._sleep_update)

        # If the pet navigated to a different location during sleep, switch scenes
        # now while the screen is still black so the transition-in reveals the
        # correct scene directly.
        self.scene_manager.apply_pending_scene_after_sleep()

        # Kick off the reveal transition and reset housekeeping state
        self.scene_manager.transitions.start_in_only()
        self.scene_manager.reset_idle_timer()
        self._sleep_pending = False
        self._woke_from_sleep = True

    def _sleep_update(self, dt):
        """Minimal game tick called ~SLEEP_FPS times per second while sleeping."""
        dt_scaled = dt * self.context.time_speed
        self.time_system.advance(dt_scaled, self.context.environment, self.weather_system)
        self.scene_manager.sleep_update(dt_scaled)
        if self.espnow_handler:
            self.espnow_handler.dispatch()
            self.espnow_handler.update(dt_scaled)

    def run(self):
        print("==> Starting game loop...")

        while True:
            # After waking from sleep the elapsed time since last_frame_time spans
            # the entire sleep duration.  Reset it to one nominal frame so that
            # time_system doesn't get a massive dt on the first awake frame
            # (the sleep loop already advanced time correctly via _sleep_update).
            if self._woke_from_sleep:
                self._woke_from_sleep = False
                self.last_frame_time = time.ticks_ms() - config.FRAME_TIME_MS

            current_time = time.ticks_ms()
            delta_time = time.ticks_diff(current_time, self.last_frame_time)
            dt = delta_time / 1000.0 * self.context.time_speed

            self.scene_manager.handle_input()

            # Track button activity for the sleep inactivity timer
            if self.sleep_manager and self.input.any_button_pressed():
                self.sleep_manager.notify_activity()

            self.time_system.advance(dt, self.context.environment, self.weather_system)

            if self.espnow_handler:
                self.espnow_handler.dispatch()
                self.espnow_handler.update(dt)

            self.scene_manager.update(dt)

            if self.visit_manager:
                self.visit_manager.update(dt)

            try:
                self.scene_manager.draw()
                if self.espnow_handler:
                    self.espnow_handler.draw(self.renderer)
                self.renderer.show()
            except OSError as e:
                if e.errno == 19:  # ENODEV - display disconnected
                    print("==! Display disconnected, attempting reinit...")
                    time.sleep_ms(500)
                    self.renderer.reinit()
                else:
                    raise

            self.last_frame_time = current_time

            frame_time = time.ticks_diff(time.ticks_ms(), current_time)
            if frame_time < config.FRAME_TIME_MS:
                time.sleep_ms(config.FRAME_TIME_MS - frame_time)

            # Begin sleep if inactive long enough — but not during a visit or
            # while another transition is already running.
            if (self.sleep_manager
                    and not self._sleep_pending
                    and not self.scene_manager.transitions.active
                    and getattr(self.context, 'visit', None) is None
                    and self.sleep_manager.should_sleep()):
                self._sleep_pending = True
                self.scene_manager.transitions.start(on_midpoint=self._on_sleep_midpoint)


def main():
    try:
        game = Game()
        game.run()
    except KeyboardInterrupt:
        print("== Interrupted ==")
    except Exception as e:
        print(f"==! Error: {e}")
        sys.print_exception(e)


if __name__ == "__main__":
    main()
