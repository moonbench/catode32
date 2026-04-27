import random

from entities.entity import Entity
from assets.nature import BUTTERFLY1, MOTH, FIREFLY

_SPRITES = {
    'butterfly': BUTTERFLY1,
    'moth':      MOTH,
    'firefly':   FIREFLY,
}


class FlyerEntity(Entity):
    """A flying critter (butterfly, moth, or firefly) with simple movement AI."""

    def __init__(self, variant, x, y):
        super().__init__(x, y)
        self.variant = variant
        self._sprite = _SPRITES[variant]

        self.anim_counter = 0.0
        self.anim_speed = self._sprite.get("speed", 8)

        self.vx = 0.5
        self.vy = 0.3

        self.bounds_left = 10
        self.bounds_right = 110
        self.bounds_top = 10
        self.bounds_bottom = 45

        self.direction_timer = 0.0
        self.direction_interval = 2.0

    def update(self, dt):
        frame_count = len(self._sprite["frames"])
        self.anim_counter = (self.anim_counter + dt * self.anim_speed) % frame_count

        self.direction_timer += dt
        if self.direction_timer >= self.direction_interval:
            self.direction_timer = 0.0
            self._pick_new_direction()

        self.x += self.vx * dt * 20
        self.y += self.vy * dt * 20

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
        if random.random() < 0.5:
            self.vy = random.uniform(-0.5, 0.5)
        if random.random() < 0.3:
            self.vx = random.uniform(-0.5, 0.5)
        self.direction_interval = random.uniform(1.0, 3.0)

    def draw(self, renderer, camera_offset=0):
        if not self.visible:
            return
        frame = int(self.anim_counter) % len(self._sprite["frames"])
        renderer.draw_sprite_obj(self._sprite, int(self.x) - camera_offset, int(self.y), frame=frame)
