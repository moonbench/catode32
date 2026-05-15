# scene_manager.py - Manages scene transitions and lifecycle

import gc
import sys
import config
from lang import t
from menu import Menu, MenuItem
from transitions import TransitionManager
from ui import OverlayManager
from assets.icons import WRENCH_ICON, SUN_ICON, HOUSE_ICON, STATS_ICON, MINIGAME_ICONS, MINIGAMES_ICON, CAT_ICON, TREES_ICON, MEAL_ICON, POWER_ICON, CREDITS_ICON, STORE_ICON, WIFI_ICON, FISH_ICON


class SceneManager:
    """Manages scene loading, unloading, and transitions"""

    _IDLE_TIMEOUT = 300.0  # seconds of inactivity before reverting to main scene

    # Modules that survive every scene transition once loaded.
    # Includes heavy asset data and the shared infrastructure that every
    # location scene depends on — purging and reimporting these each transition
    # is pure churn with no memory benefit.
    _PINNED_MODULES = frozenset({
        # Asset data
        'assets.character',     # 109 KB sprite data
        'assets.effects',
        'assets.items',
        'assets.plants',
        'assets.furniture',
        'assets.nature',
        # Sky / environment
        'sky',
        'environment',
        'clock',
        # Scene base classes
        'scene',
        'scenes',               # namespace package, registered on any scenes.* import
        'scenes.main_scene',
        'scenes.vacation_scene',
        # Entity / behavior system
        'entities',
        'entities.entity',
        'entities.character',
        'entities.behaviors',
        'entities.behaviors.base',
        'entities.behaviors.idle',
        'behavior_manager',
        # Plant / gardening system
        'plant_system',
        'plant_renderer',
        'gardening_ui',
    })

    def __init__(self, context, renderer, input_handler):
        self.context = context
        self.renderer = renderer
        self.input = input_handler

        self.current_scene = None
        self.next_scene_class = None

        # Overlay management (menus, settings, dialogs)
        self.overlays = OverlayManager()
        self.big_menu = Menu(renderer, input_handler)

        # Scene registry: name -> (module_path, class_name)
        # Scenes are lazy-loaded when first accessed
        self._scene_registry = self._build_scene_registry()

        # Transition manager
        self.transitions = TransitionManager(renderer, duration=config.TRANSITION_DURATION)
        self.pending_scene_class = None
        self.pending_scene_name = None

        # Idle timeout tracking
        self._idle_timer = 0.0

        # Snapshot of sys.modules after all core infrastructure is loaded.
        # Anything not in this set is fair game to purge on scene exit.
        self._baseline_modules = frozenset(sys.modules)

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
            'lightsout': ('scenes.lightsout', 'LightsOutScene'),
            'pipes': ('scenes.pipes', 'PipeScene'),
            'platformer': ('scenes.platformer', 'PlatformerScene'),
            'debug_context': ('scenes.debug_context', 'DebugContextScene'),
            'debug_memory': ('scenes.debug_memory', 'DebugMemoryScene'),
            'debug_poses': ('scenes.debug_poses', 'DebugPosesScene'),
            'debug_behaviors': ('scenes.debug_behaviors', 'DebugBehaviorsScene'),
            'debug_plants':    ('scenes.debug_plants',    'DebugPlantsScene'),
            'debug_led': ('scenes.debug_led', 'DebugLedScene'),
            'debug_power': ('scenes.debug_power', 'DebugPowerScene'),
            'debug_stats': ('scenes.debug_stats', 'DebugStatsScene'),
            'debug_wifi': ('scenes.debug_wifi', 'DebugWifiScene'),
            'debug_bluetooth': ('scenes.debug_bluetooth', 'DebugBluetoothScene'),
            'debug_espnow': ('scenes.debug_espnow', 'DebugEspnowScene'),
            'credits': ('scenes.credits', 'CreditsScene'),
            'environment_settings': ('scenes.environment_settings', 'EnvironmentSettingsScene'),
            'time_settings': ('scenes.time_settings', 'TimeSettingsScene'),
            'forecast': ('scenes.forecast', 'ForecastScene'),
            'store': ('scenes.store', 'StoreScene'),
            'social': ('scenes.social', 'SocialScene'),
            'pet_info': ('scenes.pet_info', 'PetInfoScene'),
            'vacation_park':     ('scenes.vacation_park',     'VacationParkScene'),
            'vacation_forest':   ('scenes.vacation_forest',   'VacationForestScene'),
            'vacation_aquarium': ('scenes.vacation_aquarium', 'VacationAquariumScene'),
            'vacation_beach':    ('scenes.vacation_beach',    'VacationBeachScene'),
        }

    def _get_scene_class(self, name):
        """Return a scene class by name, importing it lazily if needed."""
        if name not in self._scene_registry:
            return None
        module_path, class_name = self._scene_registry[name]
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

    def change_scene_by_name(self, name):
        """Change scene using registered name, deferring the import to the transition midpoint."""
        if name not in self._scene_registry:
            return

        # Don't start a new transition if one is already active
        if self.transitions.active:
            return

        # Track intent in memory for crash handler
        self.context.pending_intent = name

        # If no current scene, import and switch immediately (initial load)
        if self.current_scene is None:
            scene_class = self._get_scene_class(name)
            if scene_class:
                self._perform_scene_switch(scene_class)
            return

        # Store pending name and start transition; import deferred to midpoint
        self.pending_scene_name = name
        self.transitions.start(on_midpoint=self._on_transition_midpoint)

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
        if self.pending_scene_name:
            name = self.pending_scene_name
            self.pending_scene_name = None
            self._perform_scene_switch(scene_name=name)
        elif self.pending_scene_class:
            scene_class = self.pending_scene_class
            self.pending_scene_class = None
            self._perform_scene_switch(scene_class=scene_class)

    def _perform_scene_switch(self, scene_class=None, scene_name=None):
        """Actually switch to a new scene (called at transition midpoint)"""
        # Exit and unload current scene first to maximise free heap before import
        if self.current_scene:
            self.current_scene.exit()
            self.current_scene.unload()
            scene_class_name = type(self.current_scene).__name__
            for reg_name, (_, class_name) in self._scene_registry.items():
                if class_name == scene_class_name:
                    self._unload_scene_module(reg_name)
                    break
            self.current_scene = None
            self._purge_scene_modules()  # includes gc.collect()

        # Import new scene module now, with maximum free heap
        if scene_name and scene_class is None:
            scene_class = self._get_scene_class(scene_name)

        if not scene_class:
            return

        # Create new scene instance
        print(f"Creating new scene: {scene_class.__name__}")
        self.current_scene = scene_class(self.context, self.renderer, self.input)
        self.current_scene.load()
        self.current_scene.enter()

        # Scene entered successfully — clear crash-resume intent
        self.context.pending_intent = None
        try:
            import uos
            uos.remove('/intent.json')
        except OSError:
            pass

    def _purge_scene_modules(self):
        """Remove any modules loaded after startup, except pinned asset modules."""
        to_remove = [
            mod for mod in sys.modules
            if mod not in self._baseline_modules and mod not in self._PINNED_MODULES
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

        # Idle timeout: after inactivity, close overlays and/or revert to main scene
        self._idle_timer += dt
        if self._idle_timer >= self._IDLE_TIMEOUT:
            self._idle_timer = 0.0
            if self.overlays.active:
                self.overlays.clear()
            if (getattr(self.current_scene, 'SCENE_NAME', None) is None
                    and not getattr(self.current_scene, 'IS_VACATION', False)):
                self.change_scene_by_name(self.context.last_main_scene)

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
        """Draw current scene and transition overlay into the frame buffer.

        Does NOT call renderer.show() — the caller is responsible for
        presenting the buffer after any additional overlays are drawn.
        """
        self.renderer.clear()

        # If an overlay is active, draw it instead of the scene
        if self.overlays.draw():
            return

        if self.current_scene:
            self.current_scene.draw()

        # Draw transition overlay if active
        self.transitions.draw()
    
    def handle_input(self):
        """Handle input for current scene"""
        # Any button activity resets the idle timer
        if self.input.any_button_pressed():
            self._idle_timer = 0.0

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
        on_vacation = getattr(self.current_scene, 'IS_VACATION', False)
        vac_confirm = "End vacation?" if on_vacation else None

        # Stats page
        items.append(MenuItem(t("Pet stats"), icon=STATS_ICON, action=('scene', 'stats')))

        # Location options
        location_items = []
        location_items.append(MenuItem(t("Living Room"), icon=HOUSE_ICON, action=('scene', 'inside'), confirm=vac_confirm))
        location_items.append(MenuItem(t("Bedroom"), icon=HOUSE_ICON, action=('scene', 'bedroom'), confirm=vac_confirm))
        location_items.append(MenuItem(t("Kitchen"), icon=MEAL_ICON, action=('scene', 'kitchen'), confirm=vac_confirm))
        location_items.append(MenuItem(t("Outside"), icon=SUN_ICON, action=('scene', 'outside'), confirm=vac_confirm))
        location_items.append(MenuItem(t("Treehouse"), icon=TREES_ICON, action=('scene', 'treehouse'), confirm=vac_confirm))
        items.append(MenuItem(t("Locations"), icon=HOUSE_ICON, submenu=location_items))

        # Minigames submenu
        minigame_items = []
        minigame_items.append(MenuItem(t("Zoomies"), icon=MINIGAME_ICONS.get(t("Zoomies")), action=('scene', 'zoomies'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Breakout"), icon=MINIGAME_ICONS.get(t("Breakout")), action=('scene', 'breakout'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Snake"), icon=MINIGAME_ICONS.get(t("Snake")), action=('scene', 'snake'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Hunter"), icon=MINIGAME_ICONS.get(t("Prowl")), action=('scene', 'platformer'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Memory"), icon=MINIGAME_ICONS.get(t("Memory")), action=('scene', 'memory'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Maze"), icon=MINIGAME_ICONS.get(t("Maze")), action=('scene', 'maze'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("TicTacToe"), icon=MINIGAME_ICONS.get(t("TicTacToe")), action=('scene', 'tictactoe'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Hanjie"), icon=MINIGAME_ICONS.get(t("Hanjie")), action=('scene', 'hanjie'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Lights Out"), icon=MINIGAME_ICONS.get("LightsOut"), action=('scene', 'lightsout'), confirm=vac_confirm))
        minigame_items.append(MenuItem(t("Pipes"), icon=MINIGAME_ICONS.get(t("Pipes")), action=('scene', 'pipes'), confirm=vac_confirm))
        items.append(MenuItem(t("Minigames"), icon=MINIGAMES_ICON, submenu=minigame_items))
        
        # Store
        items.append(MenuItem(t("Store"), icon=STORE_ICON, action=('scene', 'store')))

        # Weather forecast
        items.append(MenuItem(t("Forecast"), icon=SUN_ICON, action=('scene', 'forecast')))

        # Social / playdate
        items.append(MenuItem(t("Social"), icon=CAT_ICON, action=('scene', 'social')))

        # Pet info
        items.append(MenuItem(t("Pet info"), icon=CAT_ICON, action=('scene', 'pet_info')))

        # Debug submenu (gated on config flag)
        if config.SHOW_DEBUG_MENUS:
            debug_items = []
            debug_items.append(MenuItem(t("Environment"), icon=SUN_ICON, action=('scene', 'environment_settings')))
            debug_items.append(MenuItem(t("Poses"), icon=CAT_ICON, action=('scene', 'debug_poses')))
            debug_items.append(MenuItem(t("Behaviors"), icon=CAT_ICON, action=('scene', 'debug_behaviors')))
            debug_items.append(MenuItem(t("Stats"), icon=CAT_ICON, action=('scene', 'debug_stats')))
            debug_items.append(MenuItem(t("Plants"), icon=TREES_ICON, action=('scene', 'debug_plants')))

            vacation_items = [
                MenuItem(t("Park"), icon=TREES_ICON, action=('scene', 'vacation_park')),
                MenuItem(t("Forest"), icon=TREES_ICON, action=('scene', 'vacation_forest')),
                MenuItem(t("Aquarium"), icon=FISH_ICON, action=('scene', 'vacation_aquarium')),
                MenuItem(t("Beach"), icon=SUN_ICON, action=('scene', 'vacation_beach')),
            ]
            debug_items.append(MenuItem(t("Vacations"), icon=SUN_ICON, submenu=vacation_items))
            debug_items.append(MenuItem(t("Time Speed"), icon=WRENCH_ICON, action=('scene', 'time_settings')))
            debug_items.append(MenuItem(t("Mem. Usage"), icon=WRENCH_ICON, action=('scene', 'debug_memory')))
            debug_items.append(MenuItem(t("RGB LED"), icon=WRENCH_ICON, action=('scene', 'debug_led')))
            debug_items.append(MenuItem(t("Power"), icon=POWER_ICON, action=('scene', 'debug_power')))

            context_save_items = []
            context_save_items.append(MenuItem("Context", icon=WRENCH_ICON, action=('scene', 'debug_context')))
            context_save_items.append(MenuItem(t("Save now"), icon=WRENCH_ICON, action=('context', 'save'), confirm=t("Save and reboot?")))
            context_save_items.append(MenuItem(t("Reset stats"), icon=WRENCH_ICON, action=('context', 'reset'), confirm=t("Reset all stats to defaults?")))
            debug_items.append(MenuItem("Context", icon=WRENCH_ICON, submenu=context_save_items))

            wireless_items = []
            wireless_items.append(MenuItem(t("Wifi"), icon=WIFI_ICON, action=('scene', 'debug_wifi')))
            wireless_items.append(MenuItem(t("ESP-NOW"), icon=WIFI_ICON, action=('scene', 'debug_espnow')))
            debug_items.append(MenuItem(t("Wireless"), icon=WIFI_ICON, submenu=wireless_items))

            items.append(MenuItem(t("Debug"), icon=WRENCH_ICON, submenu=debug_items))

        items.append(MenuItem(t("Credits"), icon=CREDITS_ICON, action=('scene', 'credits')))

        return items

    def _handle_big_menu_action(self, action):
        """Handle big menu selection"""
        if not action:
            return

        action_type = action[0]

        if action_type == 'scene':
            self.change_scene_by_name(action[1])
        elif action_type == 'context':
            if action[1] == 'save':
                self.context.save()
            elif action[1] == 'reset':
                self.context.reset()

    def sleep_update(self, dt):
        """Minimal scene update for use during basic sleep mode.

        Updates the current scene's logic (keeping behaviors and needs ticking)
        without advancing transitions, triggering idle timeout, or processing
        pending scene-change requests.  Scene-change results from behaviors are
        intentionally ignored so the device doesn't silently switch scenes while
        the screen is off.
        """
        if self.current_scene:
            self.current_scene.update(dt)

    def apply_pending_scene_after_sleep(self):
        """If a scene change was queued during sleep, apply it immediately (no transition).

        Called after waking but before the transition-in starts, so the reveal
        animation shows the correct scene directly rather than the stale one.
        """
        if self.context.pending_scene:
            name = self.context.pending_scene
            self.context.pending_scene = None
            self._perform_scene_switch(scene_name=name)

    def reset_idle_timer(self):
        """Reset the inactivity timer (e.g. immediately after waking from sleep)."""
        self._idle_timer = 0.0

    def unload_all(self):
        """Unload current scene - call this on shutdown"""
        if self.current_scene:
            self.current_scene.unload()
            self.current_scene = None
