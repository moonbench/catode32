# environment.py - World environment with parallax scrolling and camera

import config

# Layer identifiers
LAYER_BACKGROUND = 'background'  # 0.3x parallax
LAYER_MIDGROUND = 'midground'    # 0.6x parallax
LAYER_FOREGROUND = 'foreground'  # 1.0x parallax

# Parallax factors for each layer
PARALLAX_FACTORS = {
    LAYER_BACKGROUND: 0.3,
    LAYER_MIDGROUND: 0.6,
    LAYER_FOREGROUND: 1.0,
}


class Environment:
    """Manages world rendering with parallax scrolling and camera panning"""

    def __init__(self, world_width=128):
        self.world_width = world_width
        self.camera_x = 0

        # Parallax layers - each contains list of objects
        # Object format: {"sprite": sprite_obj, "x": world_x, "y": y}
        # Or animated: {"sprite": sprite_obj, "x": world_x, "y": y, "animate": callable}
        self.layers = {
            LAYER_BACKGROUND: [],
            LAYER_MIDGROUND: [],
            LAYER_FOREGROUND: [],
        }

        # Entities that need update() called and draw with foreground parallax
        self.entities = []

        # Custom draw functions for procedural elements (grass, ground lines, etc.)
        # Format: {"draw": callable(renderer, camera_x), "layer": layer_name}
        self.custom_draws = []

    def add_object(self, layer, sprite, x, y, **kwargs):
        """Add a static sprite object to a layer

        Args:
            layer: LAYER_BACKGROUND, LAYER_MIDGROUND, or LAYER_FOREGROUND
            sprite: sprite dict with 'width', 'height', 'frames' keys
            x: world x position
            y: screen y position (y doesn't scroll)
            **kwargs: additional draw options (rotate, mirror_h, etc.)
        """
        obj = {"sprite": sprite, "x": x, "y": y}
        obj.update(kwargs)
        self.layers[layer].append(obj)

    def add_entity(self, entity):
        """Add an entity that gets update() called and draws with foreground"""
        self.entities.append(entity)

    def add_custom_draw(self, layer, draw_func):
        """Add a custom draw function for procedural elements

        Args:
            layer: which layer to draw in
            draw_func: callable(renderer, camera_x, parallax_factor)
        """
        self.custom_draws.append({"draw": draw_func, "layer": layer})

    def pan(self, dx):
        """Move camera by dx pixels, clamped to world bounds"""
        max_camera = max(0, self.world_width - config.DISPLAY_WIDTH)
        self.camera_x = max(0, min(max_camera, self.camera_x + dx))

    def set_camera(self, x):
        """Set camera position directly, clamped to bounds"""
        max_camera = max(0, self.world_width - config.DISPLAY_WIDTH)
        self.camera_x = max(0, min(max_camera, x))

    def update(self, dt):
        """Update all entities"""
        for entity in self.entities:
            entity.update(dt)

    def draw(self, renderer):
        """Draw all layers with parallax offset"""
        # Draw layers in order: background, midground, foreground
        for layer_name in [LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND]:
            parallax = PARALLAX_FACTORS[layer_name]
            camera_offset = int(self.camera_x * parallax)

            # Draw custom elements for this layer
            for custom in self.custom_draws:
                if custom["layer"] == layer_name:
                    custom["draw"](renderer, self.camera_x, parallax)

            # Draw static objects in this layer
            for obj in self.layers[layer_name]:
                screen_x = int(obj["x"] - camera_offset)

                # Skip if completely off-screen
                sprite = obj["sprite"]
                if screen_x + sprite["width"] < 0 or screen_x >= config.DISPLAY_WIDTH:
                    continue

                # Build draw kwargs from object
                draw_kwargs = {
                    k: v for k, v in obj.items()
                    if k not in ("sprite", "x", "y")
                }

                renderer.draw_sprite_obj(
                    sprite,
                    screen_x,
                    int(obj["y"]),
                    **draw_kwargs
                )

        # Draw entities (with foreground parallax)
        camera_offset = int(self.camera_x * PARALLAX_FACTORS[LAYER_FOREGROUND])
        for entity in self.entities:
            entity.draw(renderer, camera_offset=camera_offset)

    def clear(self):
        """Remove all objects and entities"""
        for layer in self.layers.values():
            layer.clear()
        self.entities.clear()
        self.custom_draws.clear()
