"""Kneading biscuits behavior - rhythmic pawing when content after a stretch."""

import random
from entities.behaviors.base import BaseBehavior


class KneadingBehavior(BaseBehavior):
    """Pet kneads rhythmically after a satisfying stretch.

    A calm, self-soothing behavior. Chains back to stretching,
    which has a 50% chance of returning here — creating a short
    but naturally-terminating comfort loop.

    Phases:
    1. kneading - Rhythmic pawing
    2. settling  - Pet winds down and stills
    """

    NAME = "kneading"

    STAT_EFFECTS = {
        "comfort": 0.2,
        "cleanliness": 0.05,
        "serenity": 0.01,
    }
    COMPLETION_BONUS = {
        "focus": -1,
        "comfort": 2,
        "serenity": 0.1,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, 100 - context.serenity))

    def __init__(self, character):
        super().__init__(character)

        self.knead_duration = 16.0
        self.settle_duration = 1.5

    def next(self, context):
        if random.random() < 0.5:
            from entities.behaviors.stretching import StretchingBehavior
            return StretchingBehavior
        from entities.behaviors.lounging import LoungeingBehavior
        return LoungeingBehavior

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "kneading"
        self._character.set_pose("kneading.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "kneading":
            self._progress = min(1.0, self._phase_timer / self.knead_duration)

            if self._phase_timer >= self.knead_duration:
                self._phase = "settling"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self.stop(completed=True)
