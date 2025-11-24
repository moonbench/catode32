# scene_manager.py - Manages scene transitions and lifecycle

import gc
import time

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
        if self.current_scene:
            self.current_scene.draw()
    
    def handle_input(self):
        """Handle input for current scene"""
        
        if self.current_scene:
            result = self.current_scene.handle_input()
            if result and result[0] == 'change_scene':
                self.change_scene(result[1])
    
    def unload_all(self):
        """Unload all cached scenes - call this on shutdown"""
        for scene_name, scene in self.scene_cache.items():
            print(f"Unloading scene: {scene_name}")
            scene.unload()
        self.scene_cache.clear()
