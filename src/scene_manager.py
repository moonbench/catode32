# scene_manager.py - Manages scene transitions and lifecycle

import gc
import time
import config
from menu import Menu, MenuItem
from settings import Settings, SettingItem
from transitions import TransitionManager
from ui import OverlayManager
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

        # Overlay management (menus, settings, dialogs)
        self.overlays = OverlayManager()
        self.big_menu = Menu(renderer, input_handler)
        self.settings = Settings(renderer, input_handler)

        # Import and register all scenes upfront (after boot screen is showing)
        self._scene_classes = self._load_all_scenes()

        # Transition manager
        self.transitions = TransitionManager(
            renderer,
            transition_type=config.TRANSITION_TYPE,
            duration=config.TRANSITION_DURATION
        )
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
        if self.transitions.active:
            return

        # If no current scene, switch immediately (initial load)
        if self.current_scene is None:
            self._perform_scene_switch(scene_class)
            return

        # Store pending scene and start transition
        self.pending_scene_class = scene_class
        self.transitions.start(on_midpoint=self._on_transition_midpoint)

    def _on_transition_midpoint(self):
        """Called at transition midpoint to perform the scene switch."""
        if self.pending_scene_class:
            self._perform_scene_switch(self.pending_scene_class)
            self.pending_scene_class = None

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
        if self.transitions.update(dt):
            return  # Don't update scene during transition

        # Update current scene
        if self.current_scene:
            result = self.current_scene.update(dt)
            if result and result[0] == 'change_scene':
                self.change_scene(result[1])
    
    def draw(self):
        """Draw current scene and transition overlay"""
        # If an overlay is active, draw it instead of the scene
        if self.overlays.draw():
            self.renderer.show()
            return

        if self.current_scene:
            self.current_scene.draw()

        # Draw transition overlay if active
        self.transitions.draw()

        self.renderer.show()
    
    def handle_input(self):
        """Handle input for current scene"""
        # Block input during transitions
        if self.transitions.active:
            return

        # Route input to active overlay if any
        if self.overlays.handle_input():
            return

        # Open big menu on menu1 button
        if self.input.was_just_pressed('menu1'):
            self._open_big_menu()
            return

        if self.current_scene:
            result = self.current_scene.handle_input()
            if result and result[0] == 'change_scene':
                self.change_scene(result[1])

    def _open_big_menu(self):
        """Open the big menu as an overlay."""
        self.big_menu.open(self._build_big_menu_items())
        self.overlays.push(self.big_menu, on_result=self._on_big_menu_result)

    def _on_big_menu_result(self, result, metadata):
        """Handle big menu result."""
        if result == 'closed':
            return
        self._handle_big_menu_action(result)

    def _on_settings_result(self, result, metadata):
        """Handle settings overlay result."""
        settings_type = metadata.get('settings_type')
        if settings_type == 'environment':
            self.context.environment = result
        # Return to big menu after settings
        self._open_big_menu()

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

        # Environment settings
        items.append(MenuItem("Environment", icon=SUN_ICON, action=('settings', 'environment')))

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

        elif action_type == 'settings':
            settings_name = action[1]
            if settings_name == 'environment':
                self._open_environment_settings()
    
    def _open_environment_settings(self):
        """Open the environment settings screen"""
        # Get current values from context, with defaults
        env = getattr(self.context, 'environment', {})

        items = [
            SettingItem(
                "Time", "time_of_day",
                options=["Dawn", "Morning", "Noon", "Afternoon", "Evening", "Dusk", "Night"],
                value=env.get('time_of_day', "Noon")
            ),
            SettingItem(
                "Season", "season",
                options=["Spring", "Summer", "Fall", "Winter"],
                value=env.get('season', "Summer")
            ),
            SettingItem(
                "Moon", "moon_phase",
                options=["New", "Wax Cres", "1st Qtr", "Wax Gib",
                         "Full", "Wan Gib", "3rd Qtr", "Wan Cres"],
                value=env.get('moon_phase', "Full")
            ),
            SettingItem(
                "Weather", "weather",
                options=["Clear", "Cloudy", "Overcast", "Rain", "Storm", "Snow", "Windy"],
                value=env.get('weather', "Clear")
            ),
        ]

        self.settings.open(items, transition=False)
        self.overlays.push(
            self.settings,
            on_result=self._on_settings_result,
            metadata={'settings_type': 'environment'}
        )

    def unload_all(self):
        """Unload all cached scenes - call this on shutdown"""
        for scene_name, scene in self.scene_cache.items():
            print(f"Unloading scene: {scene_name}")
            scene.unload()
        self.scene_cache.clear()
        self.scene_access_order.clear()
