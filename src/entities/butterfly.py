import random

from entities.entity import Entity
from assets.nature import BUTTERFLY1


class ButterflyEntity(Entity):
    """A butterfly that flies around with simple movement AI."""

    def __init__(self, x, y):
        super().__init__(x, y)

        # Animation state
        self.anim_counter = 0.0
        self.anim_speed = 8  # frames per second

        # Movement state
        self.vx = 0.5
        self.vy = 0.3

        # Bounds for movement
        self.bounds_left = 10
        self.bounds_right = 110
        self.bounds_top = 10
        self.bounds_bottom = 45

        # AI timer for direction changes
        self.direction_timer = 0.0
        self.direction_interval = 2.0

    def update(self, dt):
        """Update animation and movement."""
        # Update animation
        frame_count = len(BUTTERFLY1["frames"])
        self.anim_counter = (self.anim_counter + dt * self.anim_speed) % frame_count

        # Update AI movement timer
        self.direction_timer += dt
        if self.direction_timer >= self.direction_interval:
            self.direction_timer = 0.0
            self._pick_new_direction()

        # Apply velocity
        self.x += self.vx * dt * 20
        self.y += self.vy * dt * 20

        # Bounce off bounds
        if self.x < self.bounds_left:
            self.x = self.bounds_left
            self.vx = abs(self.vx)
        elif self.x > self.bounds_right:
            self.x = self.bounds_right
            self.vx = -abs(self.vx)

        if self.y < self.bounds_top:
            self.y = self.bounds_top
            self.vy = abs(self.vy)
        elif self.y > self.bounds_bottom:
            self.y = self.bounds_bottom
            self.vy = -abs(self.vy)

    def _pick_new_direction(self):
        """Pick a new random direction."""
        # Randomly change vertical direction
        if random.random() < 0.5:
            self.vy = random.uniform(-0.5, 0.5)

        # Only sometimes change horizontal direction
        if random.random() < 0.3:
            self.vx = random.uniform(-0.5, 0.5)

        # Randomize next direction change interval
        self.direction_interval = random.uniform(1.0, 3.0)

    def draw(self, renderer, camera_offset=0):
        """Draw the butterfly.

        Args:
            renderer: the renderer to draw with
            camera_offset: horizontal camera offset to subtract from x position
        """
        if not self.visible:
            return

        frame = int(self.anim_counter) % len(BUTTERFLY1["frames"])
        renderer.draw_sprite_obj(BUTTERFLY1, int(self.x) - camera_offset, int(self.y), frame=frame)
