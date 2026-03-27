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


class Game:
    def __init__(self):
        print("==> Virtual Pet Starting...")

        self.renderer = Renderer()
        show_splash(self.renderer)
        del sys.modules['splash']

        self.input = InputHandler()
        self.context = GameContext()
        self.context.load()

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

        # Collect frequently to limit heap fragmentation.
        # Trigger after every ~40KB of allocations rather than waiting for OOM.
        import gc as _gc
        _gc.threshold(24000)
        del _gc

        self.last_frame_time = time.ticks_ms()
        # Simulated time rate: game minutes per real second (full day = 24 real minutes)
        self.time_system.game_minutes_per_second = 1.0

    def run(self):
        print("==> Starting game loop...")

        while True:
            current_time = time.ticks_ms()
            delta_time = time.ticks_diff(current_time, self.last_frame_time)
            dt = delta_time / 1000.0 * self.context.time_speed

            self.scene_manager.handle_input()
            self.time_system.advance(dt, self.context.environment, self.weather_system)

            if self.espnow_handler:
                self.espnow_handler.dispatch()
                self.espnow_handler.update(dt)

            self.scene_manager.update(dt)

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
