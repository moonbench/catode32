import time
import config

from input import InputHandler
from renderer import Renderer
from context import GameContext
from scene_manager import SceneManager
from weather_system import WeatherSystem
from assets.boot_img import STRETCH_CAT1

class Game:
    def __init__(self):
        print("==> Virtual Pet Starting...")

        # Setup shared resources
        self.renderer = Renderer()

        # Show boot screen immediately
        self._show_boot_screen()

        self.input = InputHandler()
        self.context = GameContext()
        self.context.load()

        # Setup the scene manager (imports all scenes during init)
        self.scene_manager = SceneManager(
            self.context,
            self.renderer,
            self.input,
        )

        self.scene_manager.change_scene_by_name('inside')

        self.weather_system = WeatherSystem()

        # Prepare to start rendering
        self.last_frame_time = time.ticks_ms()
        self._time_accumulator = 0.0
        # Simulated time rate: game minutes per real second (full day = 24 real minutes)
        self._game_minutes_per_second = 1.0
    
    def run(self):
        print("==> Starting game loop...")

        while True:
            #  Calculate frame timing
            current_time = time.ticks_ms()
            delta_time = time.ticks_diff(current_time, self.last_frame_time)

            # Handle inputs
            self.scene_manager.handle_input()
            
            dt = delta_time / 1000.0 * self.context.time_speed
            self._advance_time(dt)

            # Update game logic
            self.scene_manager.update(dt)
            
            # Render frame
            try:
                self.scene_manager.draw()
            except OSError as e:
                if e.errno == 19:  # ENODEV - display disconnected
                    print(f"==! Display disconnected, attempting reinit...")
                    time.sleep_ms(500)
                    self.renderer.reinit()
                else:
                    raise
            
            # Update timing
            self.last_frame_time = current_time
            
            # Frame rate limiting
            frame_time = time.ticks_diff(time.ticks_ms(), current_time)
            if frame_time < config.FRAME_TIME_MS:
                time.sleep_ms(config.FRAME_TIME_MS - frame_time)

    def _advance_time(self, dt):
        """Advance simulated time of day. Replace body with RTC read when hardware is available."""
        self._time_accumulator += dt * self._game_minutes_per_second
        if self._time_accumulator >= 1.0:
            mins = int(self._time_accumulator)
            self._time_accumulator -= mins
            env = self.context.environment
            total_minutes = env.get('time_minutes', 0) + mins
            old_hours = env.get('time_hours', 12)
            new_hours_raw = old_hours + total_minutes // 60
            env['time_hours'] = new_hours_raw % 24
            env['time_minutes'] = total_minutes % 60
            if new_hours_raw >= 24:
                new_day = env.get('day_number', 0) + (new_hours_raw // 24)
                env['day_number'] = new_day
                _MOON_PHASES = ("New", "Wax Cres", "1st Qtr", "Wax Gib",
                                "Full", "Wan Gib", "3rd Qtr", "Wan Cres")
                env['moon_phase'] = _MOON_PHASES[(new_day // 6 + 2) % 8]
            self.weather_system.update(mins, env)

    def _show_boot_screen(self):
        self.renderer.clear()
        # Center sprite (23x30) and text on 128x64 display
        sprite_x = (config.DISPLAY_WIDTH - STRETCH_CAT1["width"]) // 2
        sprite_y = 10
        self.renderer.draw_sprite_obj(STRETCH_CAT1, sprite_x, sprite_y)
        # "Loading..." is 10 chars * 8px = 80px wide
        text_x = (config.DISPLAY_WIDTH - 80) // 2
        text_y = sprite_y + STRETCH_CAT1["height"] + 6
        self.renderer.draw_text("Loading...", text_x, text_y)
        self.renderer.show()

def main():
    try:
        game = Game()
        game.run()
    except KeyboardInterrupt:
        print("== Interrupted ==")
    except Exception as e:
        print(f"==! Error: {e}")
        import sys
        sys.print_exception(e)


if __name__ == "__main__":
    main()
