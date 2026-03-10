"""Pacing behavior - restless back-and-forth when the pet is unsettled."""

import random
from entities.behaviors.base import BaseBehavior


class PacingBehavior(BaseBehavior):
    """Pet burns off restless anxiety by pacing.

    Triggered when at least one core emotional need is low (comfort,
    fulfillment, or affection) and the pet lacks and serenity
    to sit with it. Provides mild comfort relief and channels unspent energy
    into mischievousness.

    After pacing, whether the pet vocalizes or just resigns itself to idle
    depends on personality. An immature and social cat is far more likely
    to demand attention out loud; a mature, independent cat just sulks off.

    Transition probability to vocalizing:
        p = (1 - maturity/100) * (sociability/100)

    Phases:
    1. starting  - Pet gets up, restless energy building
    2. pacing    - Back and forth movement
    3. stopping  - Settles, for now
    """

    NAME = "pacing"

    COMPLETION_BONUS = {
        # Medium changers
        "cleanliness": -0.1,
        "fulfillment": -0.035,
        "affection": -0.02,
        "comfort": -1.5,

        # Slow changers
        "fitness": 0.01,
        "loyalty": -0.01,

        # Extra slow changers
        "mischievousness": 0.005,
    }

    def __init__(self, character):
        super().__init__(character)
        self.start_duration = 1.0
        self.pace_duration = 10.0
        self.stop_duration = 1.5
        self.pace_speed = 20  # pixels per second

        self._pace_direction = 1
        self._dir_change_timer = 0.0
        self._dir_change_interval = 4.0

    def next(self, context):
        maturity = getattr(context, 'maturity', 50) / 100.0
        sociability = getattr(context, 'sociability', 50) / 100.0
        p_vocalize = (1.0 - maturity) * sociability
        if random.random() < p_vocalize:
            return 'vocalizing'

        # Sulk if emotionally depleted and luck doesn't favor recovery
        if (getattr(context, 'fulfillment', 50) < 40 and
                getattr(context, 'affection', 50) < 40 and
                random.random() < 0.5):
            return 'sulking'

        # Act out if the pet is immature, devious, and still has energy for it
        if (getattr(context, 'mischievousness', 50) > 30 and
                getattr(context, 'maturity', 50) < 40 and
                getattr(context, 'playfulness', 50) > 60 and
                getattr(context, 'energy', 50) > 50 and
                random.random() < 0.5):
            return 'mischief'

        # Retreat if scared, depleted, and out of coping resources
        if (getattr(context, 'courage', 50) < 60 and
                getattr(context, 'affection', 50) < 60 and
                getattr(context, 'energy', 50) < 60 and
                random.random() < 0.4):
            return 'hiding'

        return None

    def _apply_direction(self):
        """Sync character mirror state with current pace direction."""
        self._character.mirror = self._pace_direction > 0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "starting"
        self._pace_direction = random.choice([-1, 1])
        self._dir_change_timer = 0.0
        self._dir_change_interval = random.uniform(3.0, 6.0)
        self.pace_duration = random.randint(10, 45)
        self._apply_direction()
        self._character.set_pose("sitting.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "starting":
            if self._phase_timer >= self.start_duration:
                self._phase = "pacing"
                self._phase_timer = 0.0
                self._character.set_pose("walking.side.neutral")

        elif self._phase == "pacing":
            context = self._character.context
            x_min = getattr(context, 'scene_x_min', 10) + 20
            x_max = getattr(context, 'scene_x_max', 118) - 20

            # Move the character
            self._character.x += self._pace_direction * self.pace_speed * dt

            # Bounce at bounds
            if self._character.x <= x_min:
                self._character.x = x_min
                self._pace_direction = 1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(3.0, 6.0)
                self._apply_direction()
            elif self._character.x >= x_max:
                self._character.x = x_max
                self._pace_direction = -1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(3.0, 6.0)
                self._apply_direction()

            # Occasional mid-pace direction changes
            self._dir_change_timer += dt
            if self._dir_change_timer >= self._dir_change_interval:
                self._pace_direction *= -1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(3.0, 6.0)
                self._apply_direction()

            self._progress = min(1.0, self._phase_timer / self.pace_duration)
            if self._phase_timer >= self.pace_duration:
                self._phase = "stopping"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.aloof")

        elif self._phase == "stopping":
            if self._phase_timer >= self.stop_duration:
                self.stop(completed=True)
