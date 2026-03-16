# scene_manager.py - Manages scene transitions and lifecycle

import gc
import sys
import config
from menu import Menu, MenuItem
from transitions import TransitionManager
from ui import OverlayManager
from assets.icons import WRENCH_ICON, SUN_ICON, HOUSE_ICON, STATS_ICON, MINIGAME_ICONS, MINIGAMES_ICON, CAT_ICON, TREES_ICON, MEAL_ICON, POWER_ICON, CREDITS_ICON


class SceneManager:
    """Manages scene loading, unloading, and transitions"""

    # Modules always kept in memory regardless of which scenes are active.
    # These are imported by core modules (ui, scene_manager, entity behaviors).
    _GLOBAL_MODULES_TO_KEEP = {
        'assets.icons',
        'assets.effects',
        'assets.character',
        'assets.items',
    }

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

        # Scene registry: name -> (module_path, class_name)
        # Scenes are lazy-loaded when first accessed
        self._scene_registry = self._build_scene_registry()

        # Transition manager
        self.transitions = TransitionManager(
            renderer,
            transition_type=config.TRANSITION_TYPE,
            duration=config.TRANSITION_DURATION
        )
        self.pending_scene_class = None

        # Accumulates MODULES_TO_KEEP from every scene ever instantiated,
        # so we know which modules are "scene-specific" and can purge them
        # when no cached scene claims them.
        self._known_scene_modules = set()

    def _build_scene_registry(self):
        """Build registry of scene names to (module_path, class_name) tuples.

        Scenes are NOT imported here - just registered for lazy loading.
        """
        return {
            'inside': ('scenes.inside', 'InsideScene'),
            'outside': ('scenes.outside', 'OutsideScene'),
            'bedroom': ('scenes.bedroom', 'BedroomScene'),
            'kitchen': ('scenes.kitchen', 'KitchenScene'),
            'treehouse': ('scenes.treehouse', 'TreehouseScene'),
            'stats': ('scenes.stats', 'StatsScene'),
            'zoomies': ('scenes.zoomies', 'ZoomiesScene'),
            'maze': ('scenes.maze', 'MazeScene'),
            'breakout': ('scenes.breakout', 'BreakoutScene'),
            'tictactoe': ('scenes.tictactoe', 'TicTacToeScene'),
            'snake': ('scenes.snake', 'SnakeScene'),
            'memory': ('scenes.memory', 'MemoryScene'),
            'hanjie': ('scenes.hanjie', 'HanjieScene'),
            'debug_context': ('scenes.debug_context', 'DebugContextScene'),
            'debug_memory': ('scenes.debug_memory', 'DebugMemoryScene'),
            'debug_poses': ('scenes.debug_poses', 'DebugPosesScene'),
            'debug_behaviors': ('scenes.debug_behaviors', 'DebugBehaviorsScene'),
            'debug_led': ('scenes.debug_led', 'DebugLedScene'),
            'debug_power': ('scenes.debug_power', 'DebugPowerScene'),
            'debug_stats': ('scenes.debug_stats', 'DebugStatsScene'),
            'credits': ('scenes.credits', 'CreditsScene'),
            'environment_settings': ('scenes.environment_settings', 'EnvironmentSettingsScene'),
            'time_settings': ('scenes.time_settings', 'TimeSettingsScene'),
            'forecast': ('scenes.forecast', 'ForecastScene'),
        }

    def _get_scene_class(self, name):
        """Return a scene class by name, importing it lazily if needed."""
        if name not in self._scene_registry:
            return None

        module_path, class_name = self._scene_registry[name]

        # Import the module (or get from cache if already loaded)
        module = __import__(module_path, None, None, [class_name])
        return getattr(module, class_name)

    def _unload_scene_module(self, scene_name):
        """Unload a scene's module from sys.modules to free memory."""
        if scene_name not in self._scene_registry:
            return

        module_path, _ = self._scene_registry[scene_name]

        if module_path in sys.modules:
            print(f"Unloading module: {module_path}")
            del sys.modules[module_path]
            gc.collect()

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

            # Track which modules this scene declares as scene-specific
            self._known_scene_modules.update(scene_class.MODULES_TO_KEEP)

            # Add to cache and access order
            self.scene_cache[scene_name] = self.current_scene
            self.scene_access_order.append(scene_name)

            # Check cache size and clean if needed
            self._manage_cache()

        # Enter the new scene
        self.current_scene.enter()
        
    def _manage_cache(self):
        """Remove old scenes if cache is too large"""
        evicted = False
        while len(self.scene_cache) > self.max_cached_scenes:
            # Remove the least recently used scene (first in access order)
            oldest_class_name = self.scene_access_order.pop(0)
            print(f"Unloading cached scene: {oldest_class_name}")
            self.scene_cache[oldest_class_name].unload()
            del self.scene_cache[oldest_class_name]

            # Also unload the module to free memory
            # Find the registry name for this class
            for reg_name, (_, class_name) in self._scene_registry.items():
                if class_name == oldest_class_name:
                    self._unload_scene_module(reg_name)
                    break
            evicted = True

        if evicted:
            self._purge_unused_scene_modules()

    def _purge_unused_scene_modules(self):
        """Remove scene-specific modules from sys.modules not needed by any cached scene."""
        keep = set(self._GLOBAL_MODULES_TO_KEEP)
        for scene in self.scene_cache.values():
            keep.update(type(scene).MODULES_TO_KEEP)

        to_remove = [
            mod for mod in sys.modules
            if mod not in keep and (
                mod.startswith('assets.') or mod in self._known_scene_modules
            )
        ]
        for mod_name in to_remove:
            print(f"Purging module: {mod_name}")
            del sys.modules[mod_name]
        if to_remove:
            gc.collect()
    
    def _handle_scene_change(self, scene_ref):
        """Handle a scene change request. scene_ref can be a name (str) or class."""
        if isinstance(scene_ref, str):
            if scene_ref == 'last_main':
                scene_ref = self.context.last_main_scene
            self.change_scene_by_name(scene_ref)
        else:
            self.change_scene(scene_ref)

    def update(self, dt):
        """Update current scene and transitions"""

        # Handle transition animation
        if self.transitions.update(dt):
            return  # Don't update scene during transition

        # Update current scene
        if self.current_scene:
            result = self.current_scene.update(dt)
            if result and result[0] == 'change_scene':
                self._handle_scene_change(result[1])

        # Check for a scene change requested by a behavior (e.g. go_to on arrival)
        if self.context.pending_scene:
            pending = self.context.pending_scene
            self.context.pending_scene = None
            self.change_scene_by_name(pending)
    
    def draw(self):
        """Draw current scene and transition overlay"""
        self.renderer.clear()

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
                self._handle_scene_change(result[1])

    def _open_big_menu(self):
        """Open the big menu as an overlay."""
        self.big_menu.open(self._build_big_menu_items())
        self.overlays.push(self.big_menu, on_result=self._on_big_menu_result)

    def _on_big_menu_result(self, result, metadata):
        """Handle big menu result."""
        if result == 'closed':
            return
        self._handle_big_menu_action(result)

    def _build_big_menu_items(self):
        """Build the big menu items"""
        items = []

        # Stats page
        items.append(MenuItem("Pet stats", icon=STATS_ICON, action=('scene', 'stats')))

        # Weather forecast
        items.append(MenuItem("Forecast", icon=SUN_ICON, action=('scene', 'forecast')))

        # Location options
        location_items = []
        location_items.append(MenuItem("Living Room", icon=HOUSE_ICON, action=('scene', 'inside')))
        location_items.append(MenuItem("Bedroom", icon=HOUSE_ICON, action=('scene', 'bedroom')))
        location_items.append(MenuItem("Kitchen", icon=MEAL_ICON, action=('scene', 'kitchen')))
        location_items.append(MenuItem("Outside", icon=SUN_ICON, action=('scene', 'outside')))
        location_items.append(MenuItem("Treehouse", icon=TREES_ICON, action=('scene', 'treehouse')))
        items.append(MenuItem("Locations", icon=HOUSE_ICON, submenu=location_items))

        # Minigames submenu
        minigame_items = []
        minigame_items.append(MenuItem("Zoomies", icon=MINIGAME_ICONS.get("Zoomies"), action=('scene', 'zoomies')))
        minigame_items.append(MenuItem("Breakout", icon=MINIGAME_ICONS.get("Breakout"), action=('scene', 'breakout')))
        minigame_items.append(MenuItem("Snake", icon=MINIGAME_ICONS.get("Snake"), action=('scene', 'snake')))
        minigame_items.append(MenuItem("Memory", icon=MINIGAME_ICONS.get("Memory"), action=('scene', 'memory')))
        minigame_items.append(MenuItem("Maze", icon=MINIGAME_ICONS.get("Maze"), action=('scene', 'maze')))
        minigame_items.append(MenuItem("TicTacToe", icon=MINIGAME_ICONS.get("TicTacToe"), action=('scene', 'tictactoe')))
        minigame_items.append(MenuItem("Hanjie", icon=MINIGAME_ICONS.get("Hanjie"), action=('scene', 'hanjie')))
        items.append(MenuItem("Minigames", icon=MINIGAMES_ICON, submenu=minigame_items))
        
        # Debug submenu
        debug_items = []
        debug_items.append(MenuItem("Environment", icon=SUN_ICON, action=('scene', 'environment_settings')))
        debug_items.append(MenuItem("Poses", icon=CAT_ICON, action=('scene', 'debug_poses')))
        debug_items.append(MenuItem("Behaviors", icon=CAT_ICON, action=('scene', 'debug_behaviors')))
        debug_items.append(MenuItem("Stats", icon=CAT_ICON, action=('scene', 'debug_stats')))
        debug_items.append(MenuItem("Time Speed", icon=WRENCH_ICON, action=('scene', 'time_settings')))
        debug_items.append(MenuItem("Memory", icon=WRENCH_ICON, action=('scene', 'debug_memory')))
        debug_items.append(MenuItem("RGB LED", icon=WRENCH_ICON, action=('scene', 'debug_led')))
        debug_items.append(MenuItem("Power", icon=POWER_ICON, action=('scene', 'debug_power')))

        context_save_items = []
        context_save_items.append(MenuItem("Context", icon=WRENCH_ICON, action=('scene', 'debug_context')))
        context_save_items.append(MenuItem("Save now", icon=WRENCH_ICON, action=('context', 'save'), confirm="Save and reboot?"))
        context_save_items.append(MenuItem("Reset stats", icon=WRENCH_ICON, action=('context', 'reset'), confirm="Reset all stats to defaults?"))
        debug_items.append(MenuItem("Context", icon=WRENCH_ICON, submenu=context_save_items))

        debug_items.append(MenuItem("Credits", icon=CREDITS_ICON, action=('scene', 'credits')))

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
        elif action_type == 'context':
            if action[1] == 'save':
                self.context.save()
            elif action[1] == 'reset':
                self.context.reset()

    def unload_all(self):
        """Unload all cached scenes - call this on shutdown"""
        for scene_name, scene in self.scene_cache.items():
            print(f"Unloading scene: {scene_name}")
            scene.unload()
        self.scene_cache.clear()
        self.scene_access_order.clear()
