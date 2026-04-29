import random
import math

from entities.entity import Entity
from assets.nature import (FROG_IDLE, FROG_MID, FROG_LEAP,
                            GRASSHOPPER_IDLE, GRASSHOPPER_MID, GRASSHOPPER_LEAP)

GROUND_Y = 64

_SPRITES = {
    'frog':        [FROG_IDLE, FROG_MID, FROG_LEAP],
    'grasshopper': [GRASSHOPPER_IDLE, GRASSHOPPER_MID, GRASSHOPPER_LEAP],
}

_ARC_HEIGHT = {
    'frog': 7,
    'grasshopper': 4,
}

_HOP_DURATION = 0.45    # seconds per single hop
_HOP_DISTANCE = 22      # pixels travelled per hop
_HOPS_MIN = 1
_HOPS_MAX = 3
_IDLE_MIN = 2.0
_IDLE_MAX = 7.0

_WORLD_LEFT  = -30
_WORLD_RIGHT = 286


class JumperEntity(Entity):
    """A ground-dwelling critter that hops across the scene.

    Spawns from one world edge facing inward and despawns when it exits
    the other side (or reverses back out). Sets self.despawned = True
    so the scene can remove it and optionally schedule a replacement.
    """

    def __init__(self, variant, x, direction):
        """
        Args:
            variant:   'frog' or 'grasshopper'
            x:         starting world x (typically just off a world edge)
            direction: 1 = moving right, -1 = moving left
        """
        super().__init__(x, GROUND_Y)
        self.variant = variant
        self.direction = direction
        self.despawned = False

        self._sprites = _SPRITES[variant]
        self._arc_height = _ARC_HEIGHT[variant]

        self._hopping = False
        self._hop_progress = 0.0
        self._hops_remaining = 0
        self._idle_timer = random.uniform(0.2, 1.2)
        self._arc_offset = 0.0
        self._pose = 0  # index into self._sprites

    def update(self, dt):
        if self._hopping:
            self._hop_progress += dt / _HOP_DURATION

            if self._hop_progress >= 1.0:
                self._hop_progress = 0.0
                self._arc_offset = 0.0
                self._pose = 0
                self._hops_remaining -= 1
                if self._hops_remaining <= 0:
                    self._hopping = False
                    self._idle_timer = random.uniform(_IDLE_MIN, _IDLE_MAX)
                    if random.random() < 0.25:
                        self.direction = -self.direction
            else:
                p = self._hop_progress
                # Pose selection: idle → mid → leap → mid → idle
                if p < 0.2:
                    self._pose = 0
                elif p < 0.45:
                    self._pose = 1
                elif p < 0.75:
                    self._pose = 2
                elif p < 0.9:
                    self._pose = 1
                else:
                    self._pose = 0

                # Arc only during the airborne portion (pose 1 and 2)
                if 0.2 <= p <= 0.9:
                    arc_p = (p - 0.2) / 0.7
                    self._arc_offset = math.sin(arc_p * math.pi) * self._arc_height
                else:
                    self._arc_offset = 0.0

                self.x += self.direction * (_HOP_DISTANCE / _HOP_DURATION) * dt
        else:
            self._idle_timer -= dt
            if self._idle_timer <= 0:
                self._hopping = True
                self._hop_progress = 0.0
                self._hops_remaining = random.randint(_HOPS_MIN, _HOPS_MAX)

        if self.x < _WORLD_LEFT or self.x > _WORLD_RIGHT:
            self.despawned = True

    def draw(self, renderer, camera_offset=0):
        if not self.visible or self.despawned:
            return

        sprite = self._sprites[self._pose]
        draw_x = int(self.x) - sprite["width"] // 2 - camera_offset
        draw_y = GROUND_Y - sprite["height"] - int(self._arc_offset) - 2
        renderer.draw_sprite_obj(sprite, draw_x, draw_y, mirror_h=(self.direction > 0))
