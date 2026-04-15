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

    # Config for each food type: stat effects, eating speed, and appeal.
    # appeal: 0.0 = easily rejected, 1.0 = cat loves it — shifts the fullness
    # threshold at which the cat may refuse food.
    FOOD_CONFIG = {
        # Meals
        "kibble":     {"stats": {"fullness": 45, "energy": 2, "fitness": 2}, "eating_speed": 0.45, "appeal": 0.2},
        "cod":        {"stats": {"fullness": 45, "energy": 2, "affection": 2, "curiosity": 1}, "eating_speed": 0.45, "appeal": 0.4},
        "haddock":    {"stats": {"fullness": 45, "energy": 3, "affection": 2}, "eating_speed": 0.45, "appeal": 0.5},
        "trout":      {"stats": {"fullness": 45, "energy": 3, "affection": 3}, "eating_speed": 0.45, "appeal": 0.6},
        "shrimp":     {"stats": {"fullness": 25, "energy": 2, "affection": 4, "playfulness": 3, "curiosity": 2}, "eating_speed": 0.5, "appeal": 0.65},
        "herring":    {"stats": {"fullness": 45, "energy": 4, "fitness": 2, "affection": 2}, "eating_speed": 0.45, "appeal": 0.5},
        "turkey":     {"stats": {"fullness": 50, "energy": 3, "fitness": 3, "serenity": 3}, "eating_speed": 0.4, "appeal": 0.6},
        "tuna":       {"stats": {"fullness": 40, "energy": 3, "affection": 7, "playfulness": 2, "mischievousness": 1}, "eating_speed": 0.45, "appeal": 0.9},
        "salmon":     {"stats": {"fullness": 50, "energy": 5, "fitness": 4, "affection": 2}, "eating_speed": 0.45, "appeal": 0.8},
        "chicken":    {"stats": {"fullness": 55, "energy": 6, "affection": 4}, "eating_speed": 0.5, "appeal": 0.7},
        "liver":      {"stats": {"fullness": 55, "energy": 5, "fitness": 5, "affection": 3}, "eating_speed": 0.45, "appeal": 0.65},
        "beef":       {"stats": {"fullness": 55, "energy": 4, "fitness": 2, "affection": 4, "comfort": 2}, "eating_speed": 0.45, "appeal": 0.7},
        "lamb":       {"stats": {"fullness": 55, "energy": 3, "affection": 4, "comfort": 4, "serenity": 2}, "eating_speed": 0.45, "appeal": 0.85},

        # Hunted / special (cat caught it — never rejected)
        "caught_snack": {"stats": {"fullness": 20}, "eating_speed": 0.5, "appeal": 1.0},

        # Snacks
        "carrots":    {"stats": {"fullness": 2, "affection": 1}, "eating_speed": 1, "appeal": 0.15},
        "pumpkin":    {"stats": {"fullness": 3, "fitness": 2}, "eating_speed": 1, "appeal": 0.25},
        "treats":     {"stats": {"fullness": 2, "affection": 3, "playfulness": 1}, "eating_speed": 1.25, "appeal": 0.75},
        "fish_bite":  {"stats": {"fullness": 4, "affection": 2, "playfulness": 1, "curiosity": 2}, "eating_speed": 1.25, "appeal": 0.8},
        "eggs":       {"stats": {"fullness": 8, "energy": 3, "fitness": 3}, "eating_speed": 1.25, "appeal": 0.5},
        "nugget":     {"stats": {"fullness": 12, "energy": 3, "affection": 2}, "eating_speed": 1.25, "appeal": 0.55},
        "milk":       {"stats": {"fullness": 8, "affection": 3, "comfort": 5, "mischievousness": 1}, "eating_speed": 1.5, "appeal": 0.7},
        "chew_stick": {"stats": {"fullness": 6, "fitness": 1, "playfulness": 2, "comfort": 3}, "eating_speed": 1.0, "appeal": 0.4},
        "puree":      {"stats": {"fullness": 8, "affection": 4, "comfort": 4, "fulfillment": 2}, "eating_speed": 1.5, "appeal": 0.85},
    }
    DEFAULT_FOOD_CONFIG = {"stats": {"fullness": 8}, "eating_speed": 0.4, "appeal": 0.5}

    # Snack/treat items (from treat pile) get a higher rejection threshold than meals.
    SNACK_TYPES = frozenset((
        "carrots", "pumpkin", "treats", "fish_bite",
        "eggs", "nugget", "milk", "chew_stick", "puree",
    ))

    # Poses the cat may use when inspecting but rejecting food.
    REJECTION_POSES = (
        "standing.side.neutral_looking_down",
        "sitting.side.looking_down",
        "laying.side.neutral2",
        "laying.side.bored",
        "sitting_silly.side.neutral",
    )

    REJECTION_LOOK_DURATION = 4.5  # Seconds cat studies the food before walking away

    FOOD_OFFSET_X = 34  # Horizontal offset of food from character anchor

    def __init__(self, character):
        super().__init__(character)

        self._food_sprite = None
        self._food_frame = 0.0
        self._food_y_progress = 0.0  # 0 = above screen, 1 = ground level
        self._food_type = None
        self._rejecting = False

        self.eating_speed = 0.4   # Food frames per second during eating phase (set per food)
        self.lower_duration = 1.0  # Time for food to lower
        self.pause_duration = 1.5  # Pre/post eating pause

    @classmethod
    def _rejection_chance(cls, food_type, fullness, is_snack=False):
        """Return probability (0.0–1.0) that the cat refuses this food at this fullness.

        Uses a linear ramp from 0% at `start` to 100% at fullness=100.
        The midpoint (50% rejection) is: 76 + appeal*8 for meals,
        86 + appeal*8 for snacks — so meals are rejected more readily.
        """
        config = cls.FOOD_CONFIG.get(food_type, cls.DEFAULT_FOOD_CONFIG)
        appeal = config.get("appeal", 0.5)
        base = 86 if is_snack else 76
        midpoint = base + appeal * 8
        start = 2 * midpoint - 100
        if fullness <= start:
            return 0.0
        return min(1.0, (fullness - start) / (100.0 - start))

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

        context = self._character.context
        if context:
            is_snack = food_type in self.SNACK_TYPES
            chance = self._rejection_chance(food_type, context.fullness, is_snack)
            self._rejecting = random.random() < chance
        else:
            self._rejecting = False

        self._character.set_pose("standing.side.happy")

    def next(self, context):
        if self._rejecting:
            return 'meandering'
        return None

    # Stats that always receive full credit regardless of meal variety.
    _VARIETY_EXEMPT = frozenset(('fullness', 'energy'))

    def get_completion_bonus(self, context):
        if self._rejecting:
            return {}
        config = self.FOOD_CONFIG.get(self._food_type, self.DEFAULT_FOOD_CONFIG)
        bonus = dict(config["stats"])

        recent = context.recent_meals
        repeat_count = sum(1 for m in recent if m == self._food_type)
        if repeat_count > 0:
            appeal = config.get("appeal", 0.5)
            multiplier = max(0.25, 1.0 - repeat_count * 0.18 * (1.0 - appeal))
            for stat in bonus:
                if stat not in self._VARIETY_EXEMPT:
                    bonus[stat] = bonus[stat] * multiplier
            print("[Eating] variety multiplier %.2f (repeat=%d, appeal=%.2f)" % (multiplier, repeat_count, appeal))

        if self._food_type not in self.SNACK_TYPES:
            bonus['loyalty'] = 0.5 if repeat_count == 0 else 0.15

        context.record_meal(self._food_type)
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
                self._phase_timer = 0.0
                if self._rejecting:
                    self._phase = "rejecting"
                    self._character.set_pose(random.choice(self.REJECTION_POSES))
                else:
                    self._phase = "pre_eating"
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

        elif phase == "rejecting":
            if self._phase_timer >= self.REJECTION_LOOK_DURATION:
                self.stop()

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
