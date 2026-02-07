# scene_manager.py - Manages scene transitions and lifecycle

import gc
import time
from menu import Menu, MenuItem
from assets.icons import WRENCH_ICON, TREES_ICON, HOUSE_ICON


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
        self.max_cached_scenes = 2  # Limit cached scenes for memory

        # Big menu (menu1) - consistent across all scenes
        self.big_menu = Menu(renderer, input_handler)
        self.big_menu_active = False
        self._scene_classes = {}  # Registered scene classes for menu navigation
        
    def register_scene(self, name, scene_class):
        """Register a scene class for menu navigation"""
        self._scene_classes[name] = scene_class

    def change_scene(self, scene_class):
        self.next_scene_class = scene_class
        
        if self.next_scene_class is None:
            return
            
        # Exit current scene
        if self.current_scene:
            self.current_scene.exit()
            
        # Check if we have this scene cached
        scene_name = self.next_scene_class.__name__
        
        if scene_name in self.scene_cache:
            # Reuse cached scene
            print(f"Reusing cached scene: {scene_name}")
            self.current_scene = self.scene_cache[scene_name]
        else:
            # Create new scene instance
            print(f"Creating new scene: {scene_name}")
            self.current_scene = self.next_scene_class(
                self.context, self.renderer, self.input
            )
            self.current_scene.load()
            
            # Add to cache
            self.scene_cache[scene_name] = self.current_scene
            
            # Check cache size and clean if needed
            self._manage_cache()
        
        # Enter the new scene
        self.current_scene.enter()
        self.next_scene_class = None
        
    def _manage_cache(self):
        """Remove old scenes if cache is too large"""
        if len(self.scene_cache) > self.max_cached_scenes:
            # Find the oldest scene that isn't current
            for scene_name, scene in list(self.scene_cache.items()):
                if scene != self.current_scene:
                    print(f"Unloading cached scene: {scene_name}")
                    scene.unload()
                    del self.scene_cache[scene_name]
                    break
    
    def update(self, dt):
        """Update current scene"""
        
        # Update current scene
        if self.current_scene:
            result = self.current_scene.update(dt)
            if result and result[0] == 'change_scene':
                self.change_scene(result[1])
    
    def draw(self):
        """Draw current scene"""
        if self.big_menu_active:
            self.big_menu.draw()
            return

        if self.current_scene:
            self.current_scene.draw()
    
    def handle_input(self):
        """Handle input for current scene"""
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
            items.append(MenuItem("Go outside", icon=TREES_ICON, action=('scene', 'outside')))

        # Debug option
        if 'debug' in self._scene_classes:
            items.append(MenuItem("Debug", icon=WRENCH_ICON, action=('scene', 'debug')))

        return items

    def _handle_big_menu_action(self, action):
        """Handle big menu selection"""
        if not action:
            return

        action_type = action[0]

        if action_type == 'scene':
            scene_name = action[1]
            if scene_name in self._scene_classes:
                self.change_scene(self._scene_classes[scene_name])
    
    def unload_all(self):
        """Unload all cached scenes - call this on shutdown"""
        for scene_name, scene in self.scene_cache.items():
            print(f"Unloading scene: {scene_name}")
            scene.unload()
        self.scene_cache.clear()
