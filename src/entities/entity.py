class Entity:
    """Base class for all game entities."""

    def __init__(self, x, y):
        self.x = x
        self.y = y
        self.visible = True

    def update(self, dt):
        """Update entity logic. Override in subclasses."""
        pass

    def draw(self, renderer, camera_offset=0):
        """Draw the entity. Override in subclasses.

        Args:
            renderer: the renderer to draw with
            camera_offset: horizontal camera offset to subtract from x position
        """
        pass
