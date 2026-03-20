"""Eating behavior for the character entity."""

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
        # Meals
        "chicken":    {"stats": {"fullness": 55, "energy": 5}, "eating_speed": 0.5},
        "turkey":     {"stats": {"fullness": 50, "energy": 3, "fitness": 3}, "eating_speed": 0.4},
        "salmon":     {"stats": {"fullness": 50, "energy": 5, "fitness": 2}, "eating_speed": 0.45},
        "tuna":       {"stats": {"fullness": 45, "energy": 4, "affection": 3}, "eating_speed": 0.45},
        "shrimp":     {"stats": {"fullness": 30, "energy": 2, "affection": 4, "playfulness": 2}, "eating_speed": 0.5},
        "kibble":     {"stats": {"fullness": 45, "energy": 2, "affection": 2}, "eating_speed": 0.45},
        # Hunted / special
        "caught_snack": {"stats": {"fullness": 20}, "eating_speed": 0.5},
        # Snacks
        "chew_stick": {"stats": {"fullness": 8, "comfort": 3, "playfulness": 2}, "eating_speed": 1.0},
        "nugget":     {"stats": {"fullness": 12, "affection": 2}, "eating_speed": 1.25},
        "cream":      {"stats": {"fullness": 8, "comfort": 5, "affection": 2}, "eating_speed": 1.5},
        "milk":       {"stats": {"fullness": 10, "energy": 3, "comfort": 3}, "eating_speed": 1.5},
        "fish_bite":  {"stats": {"fullness": 4, "affection": 3}, "eating_speed": 1.25},
    }
    DEFAULT_FOOD_CONFIG = {"stats": {"fullness": 8}, "eating_speed": 0.4}

    FOOD_OFFSET_X = 34  # Horizontal offset of food from character anchor

    def __init__(self, character):
        super().__init__(character)

        self._food_sprite = None
        self._food_frame = 0.0
        self._food_y_progress = 0.0  # 0 = above screen, 1 = ground level
        self._food_type = None

        self.eating_speed = 0.4   # Food frames per second during eating phase (set per food)
        self.lower_duration = 1.0  # Time for food to lower
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
        bonus = dict(config["stats"])
        return self.apply_location_bonus(context, bonus)

    def apply_location_bonus(self, context, bonus):
        if context.last_main_scene == 'kitchen':
            self._character.play_bursts()
            for stat in ('fullness', 'energy'):
                if stat in bonus:
                    bonus[stat] = bonus[stat] * 1.2
        return bonus

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
