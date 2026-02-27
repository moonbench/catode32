"""Eating behavior for the character entity."""

import random
from entities.behaviors.base import BaseBehavior


class EatingBehavior(BaseBehavior):
    """Manages the eating animation sequence for a character.

    Phases:
    1. lowering    - Food lowers to ground, character stands happy
    2. pre_eating  - Brief pause, character leans forward neutral
    3. eating      - Character eats, food sprite advances through frames
    4. post_eating - Brief pause, character leans forward neutral
    5. Complete    - Return to original pose
    """

    NAME = "eating"

    # Config for each food type: stat effects and how fast it's eaten
    FOOD_CONFIG = {
        "chicken": {"stats": {"fullness": 55, "energy": 5, "appetite": 0.1}, "eating_speed": 0.3},
        "fish":    {"stats": {"fullness": 45, "energy": 2, "affection": 3, "appetite": 0.1}, "eating_speed": 0.35},
        "treat":   {"stats": {"fullness": 5, "affection": 1, "appetite": 0.1}, "eating_speed": 1.5},
    }
    DEFAULT_FOOD_CONFIG = {"stats": {"fullness": 8}, "eating_speed": 0.4}

    FOOD_OFFSET_X = 30  # Horizontal offset of food from character anchor

    @classmethod
    def get_priority(cls, context):
        return random.uniform(5, max(5, context.fullness * 0.15))

    def __init__(self, character):
        super().__init__(character)

        self._food_sprite = None
        self._food_frame = 0.0
        self._food_y_progress = 0.0  # 0 = above screen, 1 = ground level
        self._food_type = None

        self.eating_speed = 0.4   # Food frames per second during eating phase (set per food)
        self.lower_duration = 0.5  # Time for food to lower
        self.pause_duration = 1.5  # Pre/post eating pause

    @property
    def progress(self):
        """Return eating progress from 0.0 to 1.0."""
        if not self._active or not self._food_sprite:
            return 0.0
        num_frames = len(self._food_sprite["frames"])
        return min(1.0, self._food_frame / num_frames)

    def start(self, food_sprite=None, food_type=None, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "lowering"
        self._food_sprite = food_sprite
        self._food_frame = 0.0
        self._food_y_progress = 0.0
        self._food_type = food_type

        config = self.FOOD_CONFIG.get(food_type, self.DEFAULT_FOOD_CONFIG)
        self.eating_speed = config["eating_speed"]

        self._character.set_pose("standing.side.happy")

    def get_completion_bonus(self, context):
        config = self.FOOD_CONFIG.get(self._food_type, self.DEFAULT_FOOD_CONFIG)
        return config["stats"]

    def stop(self, completed=True):
        if not self._active:
            return

        self._food_y_progress = 1.0
        self._food_sprite = None
        super().stop(completed=completed)
        self._food_type = None

    def update(self, dt):
        if not self._active or not self._food_sprite:
            return

        phase = self._phase
        self._phase_timer += dt

        if phase == "lowering":
            progress = self._phase_timer / self.lower_duration
            self._food_y_progress = min(progress, 1.0)
            if progress >= 1.0:
                self._phase = "pre_eating"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif phase == "pre_eating":
            if self._phase_timer >= self.pause_duration:
                self._phase = "eating"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.eating")

        elif phase == "eating":
            num_frames = len(self._food_sprite["frames"])
            self._food_frame += dt * self.eating_speed
            self._progress = min(1.0, self._food_frame / num_frames)
            if self._food_frame >= num_frames:
                self._phase = "post_eating"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif phase == "post_eating":
            if self._phase_timer >= self.pause_duration:
                self.stop()

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or not self._food_sprite:
            return

        food_width = self._food_sprite["width"]
        food_height = self._food_sprite["height"]

        ground_y = int(char_y) - food_height
        start_y = ground_y - 40
        food_y = int(start_y + (ground_y - start_y) * self._food_y_progress)

        if mirror:
            food_x = int(char_x) + self.FOOD_OFFSET_X - food_width // 2
        else:
            food_x = int(char_x) - self.FOOD_OFFSET_X - food_width // 2

        food_frame = min(int(self._food_frame), len(self._food_sprite["frames"]) - 1)
        renderer.draw_sprite_obj(self._food_sprite, food_x, food_y, frame=food_frame)
