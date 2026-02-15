# scene_manager.py - Manages scene transitions and lifecycle

import gc
import time
import config
from menu import Menu, MenuItem
from assets.icons import WRENCH_ICON, SUN_ICON, HOUSE_ICON, STATS_ICON, MINIGAME_ICONS, MINIGAMES_ICON


class SceneManager:
    """Manages scene loading, unloading, and transitions"""
    
    def __init__(self, context, renderer, input_handler):
        self.context = context
        self.renderer = renderer
        self.input = input_handler

        self.current_scene = None
        self.next_scene_class = None

        # Track loaded scenes for memory management
        self.scene_cache = {}
        self.scene_access_order = []  # Track LRU order explicitly
        self.max_cached_scenes = 2  # Limit cached scenes for memory

        # Big menu (menu1) - consistent across all scenes
        self.big_menu = Menu(renderer, input_handler)
        self.big_menu_active = False

        # Import and register all scenes upfront (after boot screen is showing)
        self._scene_classes = self._load_all_scenes()

        # Transition state
        self.transition_active = False
        self.transition_type = config.TRANSITION_TYPE
        self.transition_duration = config.TRANSITION_DURATION
        self.transition_progress = 0.0
        self.transition_phase = 'out'  # 'out' (closing) or 'in' (opening)
        self.pending_scene_class = None

    def _load_all_scenes(self):
        """Import all scene modules and return name-to-class mapping."""
        from scenes.normal import NormalScene
        from scenes.outside import OutsideScene
        from scenes.stats import StatsScene
        from scenes.zoomies import ZoomiesScene
        from scenes.maze import MazeScene
        from scenes.breakout import BreakoutScene
        from scenes.tictactoe import TicTacToeScene
        from scenes.debug_context import DebugContextScene
        from scenes.debug_memory import DebugMemoryScene
        from scenes.debug_poses import DebugPosesScene

        return {
            'normal': NormalScene,
            'outside': OutsideScene,
            'stats': StatsScene,
            'zoomies': ZoomiesScene,
            'maze': MazeScene,
            'breakout': BreakoutScene,
            'tictactoe': TicTacToeScene,
            'debug_context': DebugContextScene,
            'debug_memory': DebugMemoryScene,
            'debug_poses': DebugPosesScene,
        }

    def _get_scene_class(self, name):
        """Return a scene class by name."""
        return self._scene_classes.get(name)

    def change_scene_by_name(self, name):
        """Change scene using registered name"""
        scene_class = self._get_scene_class(name)
        if scene_class:
            self.change_scene(scene_class)

    def change_scene(self, scene_class):
        """Start a transition to a new scene"""
        if scene_class is None:
            return

        # Don't start a new transition if one is already active
        if self.transition_active:
            return

        # If no current scene, switch immediately (initial load)
        if self.current_scene is None:
            self._perform_scene_switch(scene_class)
            return

        # Start transition-out phase
        self.transition_active = True
        self.transition_phase = 'out'
        self.transition_progress = 0.0
        self.pending_scene_class = scene_class

    def _perform_scene_switch(self, scene_class):
        """Actually switch to a new scene (called at transition midpoint)"""
        # Exit current scene
        if self.current_scene:
            self.current_scene.exit()

        # Check if we have this scene cached
        scene_name = scene_class.__name__

        if scene_name in self.scene_cache:
            # Reuse cached scene
            print(f"Reusing cached scene: {scene_name}")
            self.current_scene = self.scene_cache[scene_name]
            # Move to end of access order (most recently used)
            self.scene_access_order.remove(scene_name)
            self.scene_access_order.append(scene_name)
        else:
            # Create new scene instance
            print(f"Creating new scene: {scene_name}")
            self.current_scene = scene_class(
                self.context, self.renderer, self.input
            )
            self.current_scene.load()

            # Add to cache and access order
            self.scene_cache[scene_name] = self.current_scene
            self.scene_access_order.append(scene_name)

            # Check cache size and clean if needed
            self._manage_cache()

        # Enter the new scene
        self.current_scene.enter()
        
    def _manage_cache(self):
        """Remove old scenes if cache is too large"""
        while len(self.scene_cache) > self.max_cached_scenes:
            # Remove the least recently used scene (first in access order)
            oldest_name = self.scene_access_order.pop(0)
            print(f"Unloading cached scene: {oldest_name}")
            self.scene_cache[oldest_name].unload()
            del self.scene_cache[oldest_name]
    
    def update(self, dt):
        """Update current scene and transitions"""

        # Handle transition animation
        if self.transition_active:
            self._update_transition(dt)
            return  # Don't update scene during transition

        # Update current scene
        if self.current_scene:
            result = self.current_scene.update(dt)
            if result and result[0] == 'change_scene':
                self.change_scene(result[1])

    def _update_transition(self, dt):
        """Advance transition animation"""
        # Cap dt to prevent jumps after slow scene loads
        dt = min(dt, self.transition_duration * 0.5)
        # Advance progress
        self.transition_progress += dt / self.transition_duration

        if self.transition_progress >= 1.0:
            self.transition_progress = 1.0

            if self.transition_phase == 'out':
                # Transition-out complete, switch scenes and start transition-in
                self._perform_scene_switch(self.pending_scene_class)
                self.pending_scene_class = None
                self.transition_phase = 'in'
                self.transition_progress = 0.0
            else:
                # Transition-in complete, end transition
                self.transition_active = False
                self.transition_progress = 0.0
    
    def draw(self):
        """Draw current scene and transition overlay"""
        if self.big_menu_active:
            self.big_menu.draw()
            self.renderer.show()
            return

        if self.current_scene:
            self.current_scene.draw()

        # Draw transition overlay if active
        if self.transition_active:
            self._draw_transition()

        self.renderer.show()

    def _draw_transition(self):
        """Draw the transition effect overlay"""
        # Calculate effective progress (inverted for 'in' phase)
        if self.transition_phase == 'out':
            progress = self.transition_progress
        else:
            progress = 1.0 - self.transition_progress

        # Draw the appropriate transition type
        if self.transition_type == 'fade':
            self.renderer.draw_transition_fade(progress)
        elif self.transition_type == 'wipe':
            direction = 'right' if self.transition_phase == 'out' else 'left'
            self.renderer.draw_transition_wipe(progress, direction)
        elif self.transition_type == 'iris':
            self.renderer.draw_transition_iris(progress)
    
    def handle_input(self):
        """Handle input for current scene"""
        # Block input during transitions
        if self.transition_active:
            return

        # Handle big menu input when active
        if self.big_menu_active:
            result = self.big_menu.handle_input()
            if result == 'closed':
                self.big_menu_active = False
            elif result is not None:
                self.big_menu_active = False
                self._handle_big_menu_action(result)
            return

        # Open big menu on menu1 button
        if self.input.was_just_pressed('menu1'):
            self.big_menu_active = True
            self.big_menu.open(self._build_big_menu_items())
            return

        if self.current_scene:
            result = self.current_scene.handle_input()
            if result and result[0] == 'change_scene':
                self.change_scene(result[1])

    def _build_big_menu_items(self):
        """Build the big menu items"""
        items = []

        # Location options
        if 'normal' in self._scene_classes:
            items.append(MenuItem("Go inside", icon=HOUSE_ICON, action=('scene', 'normal')))
        if 'outside' in self._scene_classes:
            items.append(MenuItem("Go outside", icon=SUN_ICON, action=('scene', 'outside')))

        # Stats page
        if 'stats' in self._scene_classes:
            items.append(MenuItem("Pet stats", icon=STATS_ICON, action=('scene', 'stats')))

        # Minigames submenu
        minigame_items = []
        if 'zoomies' in self._scene_classes:
            minigame_items.append(MenuItem("Zoomies", icon=MINIGAME_ICONS.get("Zoomies"), action=('scene', 'zoomies')))
        if 'maze' in self._scene_classes:
            minigame_items.append(MenuItem("Maze", icon=MINIGAME_ICONS.get("Maze"), action=('scene', 'maze')))
        if 'breakout' in self._scene_classes:
            minigame_items.append(MenuItem("Breakout", icon=MINIGAME_ICONS.get("Breakout"), action=('scene', 'breakout')))
        if 'tictactoe' in self._scene_classes:
            minigame_items.append(MenuItem("TicTacToe", icon=MINIGAME_ICONS.get("TicTacToe"), action=('scene', 'tictactoe')))
        if minigame_items:
            items.append(MenuItem("Minigames", icon=MINIGAMES_ICON, submenu=minigame_items))

        # Debug submenu
        debug_items = []
        if 'debug_context' in self._scene_classes:
            debug_items.append(MenuItem("Context", icon=WRENCH_ICON, action=('scene', 'debug_context')))
        if 'debug_memory' in self._scene_classes:
            debug_items.append(MenuItem("Memory", icon=WRENCH_ICON, action=('scene', 'debug_memory')))
        if 'debug_poses' in self._scene_classes:
            debug_items.append(MenuItem("Poses", icon=WRENCH_ICON, action=('scene', 'debug_poses')))
        if debug_items:
            items.append(MenuItem("Debug", icon=WRENCH_ICON, submenu=debug_items))

        return items

    def _handle_big_menu_action(self, action):
        """Handle big menu selection"""
        if not action:
            return

        action_type = action[0]

        if action_type == 'scene':
            scene_name = action[1]
            scene_class = self._get_scene_class(scene_name)
            if scene_class:
                self.change_scene(scene_class)
    
    def unload_all(self):
        """Unload all cached scenes - call this on shutdown"""
        for scene_name, scene in self.scene_cache.items():
            print(f"Unloading scene: {scene_name}")
            scene.unload()
        self.scene_cache.clear()
        self.scene_access_order.clear()
