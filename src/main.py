import time
import config

from input import InputHandler
from renderer import Renderer
from context import GameContext
from scene_manager import SceneManager
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

        # Setup the scene manager (imports all scenes during init)
        self.scene_manager = SceneManager(
            self.context,
            self.renderer,
            self.input,
        )

        self.scene_manager.change_scene_by_name('normal')

        # Prepare to start rendering
        self.last_frame_time = time.ticks_ms()
    
    def run(self):
        print("==> Starting game loop...")

        while True:
            #  Calculate frame timing
            current_time = time.ticks_ms()
            delta_time = time.ticks_diff(current_time, self.last_frame_time)

            # Handle inputs
            self.scene_manager.handle_input()
            
            # Update game logic
            self.scene_manager.update(delta_time / 1000.0)
            
            # Render frame
            self.scene_manager.draw()
            
            # Update timing
            self.last_frame_time = current_time
            
            # Frame rate limiting
            frame_time = time.ticks_diff(time.ticks_ms(), current_time)
            if frame_time < config.FRAME_TIME_MS:
                time.sleep_ms(config.FRAME_TIME_MS - frame_time)

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
