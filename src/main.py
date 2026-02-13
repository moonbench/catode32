import time
import config

from input import InputHandler
from renderer import Renderer
from context import GameContext
from scene_manager import SceneManager
from scenes.normal import NormalScene
from scenes.outside import OutsideScene
from scenes.debug import DebugScene
from scenes.stats import StatsScene
from scenes.zoomies import ZoomiesScene
from scenes.maze import MazeScene

class Game:
    def __init__(self):
        print("==> Virtual Pet Starting...")

        # Setup shared resources
        self.renderer = Renderer()
        self.input = InputHandler()
        self.context = GameContext()

        # Setup the scene manager and first scene
        self.scene_manager = SceneManager(
            self.context,
            self.renderer,
            self.input,
        )

        # Register scenes for big menu navigation
        self.scene_manager.register_scene('normal', NormalScene)
        self.scene_manager.register_scene('outside', OutsideScene)
        self.scene_manager.register_scene('stats', StatsScene)
        self.scene_manager.register_scene('zoomies', ZoomiesScene)
        self.scene_manager.register_scene('maze', MazeScene)
        self.scene_manager.register_scene('debug', DebugScene)

        self.scene_manager.change_scene(NormalScene)

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


# """
# main.py - Main game loop and entry point
# """

# import time
# import config
# from input import InputHandler
# from renderer import Renderer
# from character import Character

# class Game:
#     """Main game class that manages the game loop"""
    
#     def __init__(self):
#         """Initialize game systems"""
#         print("Initializing Virtual Pet...")
        
#         # Initialize subsystems
#         self.input = InputHandler()
#         self.renderer = Renderer()
        
#         # Create character at center of screen
#         self.character = Character()
        
#         # Game state
#         self.running = True
#         self.paused = False
#         self.debug_mode = False
        
#         # Frame timing
#         self.last_frame_time = time.ticks_ms()
#         self.frame_count = 0
#         self.fps = 0
#         self.fps_update_time = time.ticks_ms()
        
#         print("Initialization complete!")
#         print("Use D-pad to move, A to toggle debug, B to pause")
    
#     def update(self, delta_time):
#         """
#         Update game logic
#         delta_time: time since last frame in milliseconds
#         """
#         # Check for pause toggle
#         if self.input.was_just_pressed('b'):
#             self.paused = not self.paused
#             print("Paused" if self.paused else "Resumed")
        
#         # Check for debug toggle
#         if self.input.was_just_pressed('a'):
#             self.debug_mode = not self.debug_mode
#             print("Debug mode:", "ON" if self.debug_mode else "OFF")
        
#         # Don't update game logic if paused
#         if self.paused:
#             return
        
#         # Get movement input from D-pad
#         dx, dy = self.input.get_direction()
        
#         # Move character
#         if dx != 0 or dy != 0:
#             self.character.move(dx, dy)
    
#     def render(self):
#         """Render the current game state"""
#         # Clear screen
#         self.renderer.clear()
        
#         # Draw character
#         self.renderer.draw_character(self.character)
        
#         # Draw debug info if enabled
#         if self.debug_mode:
#             x, y = self.character.get_position()
#             debug_info = {
#                 'X': int(x),
#                 'Y': int(y),
#                 'FPS': int(self.fps),
#                 'Dir': self.character.facing
#             }
#             self.renderer.draw_debug_info(debug_info, 0)
        
#         # Draw pause indicator
#         if self.paused:
#             self.renderer.draw_text("PAUSED", 45, 28)
        
#         # Update display
#         self.renderer.show()
    
#     def calculate_fps(self):
#         """Calculate and update FPS counter"""
#         self.frame_count += 1
#         current_time = time.ticks_ms()
#         time_diff = time.ticks_diff(current_time, self.fps_update_time)
        
#         # Update FPS every second
#         if time_diff >= 1000:
#             self.fps = self.frame_count * 1000 / time_diff
#             self.frame_count = 0
#             self.fps_update_time = current_time
    
#     def run(self):
#         """Main game loop"""
#         print("\n=== Game Started ===")
#         print("Controls:")
#         print("  D-pad: Move character")
#         print("  A: Toggle debug info")
#         print("  B: Pause/Unpause")
#         print("  Ctrl+C: Quit")
#         print("====================\n")
        
#         try:
#             while self.running:
#                 # Calculate frame timing
#                 current_time = time.ticks_ms()
#                 delta_time = time.ticks_diff(current_time, self.last_frame_time)
                
#                 # Update game logic
#                 self.update(delta_time)
                
#                 # Render frame
#                 self.render()
                
#                 # Update timing
#                 self.last_frame_time = current_time
#                 self.calculate_fps()
                
#                 # Frame rate limiting
#                 frame_time = time.ticks_diff(time.ticks_ms(), current_time)
#                 if frame_time < config.FRAME_TIME_MS:
#                     time.sleep_ms(config.FRAME_TIME_MS - frame_time)
        
#         except KeyboardInterrupt:
#             print("\n\n=== Game Stopped ===")
#             self.cleanup()
    
#     def cleanup(self):
#         """Clean up resources before exit"""
#         self.renderer.clear()
#         self.renderer.draw_text("Goodbye!", 35, 28)
#         self.renderer.show()
#         time.sleep(1)
#         self.renderer.clear()
#         self.renderer.show()
#         print("Cleanup complete")

# # Entry point
# if __name__ == '__main__':
#     game = Game()
#     game.run()