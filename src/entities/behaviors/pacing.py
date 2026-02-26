"""Pacing behavior - restless back-and-forth when the pet is unsettled."""

import random
from entities.behaviors.base import BaseBehavior


class PacingBehavior(BaseBehavior):
    """Pet burns off restless anxiety by pacing.

    Triggered when at least one core emotional need is low (comfort,
    fulfillment, or affection) and the pet lacks the patience and serenity
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
        "cleanliness": -2,
        "fulfillment": -2,
        "comfort": 1.25,
        "charisma": -0.13,
        "mischievousness": 0.38,
        "patience": 0.88,
    }

    @classmethod
    def can_trigger(cls, context):
        trigger = context.comfort < 65 and (context.patience < 60 or context.serenity < 60)

        if not trigger:
            failures = []
            if context.comfort >= 65:
                failures.append("Comfort: %6.4f" % context.comfort)
            if context.patience >= 60 and context.serenity >= 60:
                failures.append("Patience: %6.4f" % context.patience)
                failures.append("Serenity: %6.4f" % context.serenity)
            print("Skipping pacing. " + ", ".join(failures))

        return trigger

    @classmethod
    def get_priority(cls, context):
        worst = min(context.comfort, context.patience, context.serenity)
        return random.uniform(10, max(10, 100 - (100 - worst) * 0.8))

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
            from entities.behaviors.vocalizing import VocalizingBehavior
            return VocalizingBehavior

        # Sulk if emotionally depleted and luck doesn't favor recovery
        if (getattr(context, 'fulfillment', 50) < 40 and
                getattr(context, 'resilience', 50) < 50 and
                getattr(context, 'affection', 50) < 40 and
                random.random() < 0.5):
            from entities.behaviors.sulking import SulkingBehavior
            return SulkingBehavior

        # Act out if the pet is immature, devious, and still has energy for it
        if (getattr(context, 'mischievousness', 50) > 30 and
                getattr(context, 'craftiness', 50) > 40 and
                getattr(context, 'maturity', 50) < 40 and
                getattr(context, 'playfulness', 50) > 60 and
                getattr(context, 'energy', 50) > 50 and
                random.random() < 0.5):
            from entities.behaviors.mischief import MischiefBehavior
            return MischiefBehavior

        # Retreat if scared, depleted, and out of coping resources
        if (getattr(context, 'courage', 50) < 60 and
                getattr(context, 'affection', 50) < 60 and
                getattr(context, 'resilience', 50) < 60 and
                getattr(context, 'energy', 50) < 60 and
                random.random() < 0.4):
            from entities.behaviors.hiding import HidingBehavior
            return HidingBehavior

        return None  # -> idle

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
