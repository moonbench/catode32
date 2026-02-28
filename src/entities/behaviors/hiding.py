"""Hiding behavior - pet tucks away somewhere quiet until it feels better."""

import random
from entities.behaviors.base import BaseBehavior


class HidingBehavior(BaseBehavior):
    """Pet disappears to a safe spot to recover.

    Triggered from pacing or mischief when the pet is scared, depleted,
    and out of energy to keep acting out. Time alone rebuilds comfort,
    patience, and independence. Returns to idle when ready to rejoin the world.

    Phases:
    1. finding_spot  - Pet looks around and retreats
    2. hiding        - Still, tucked away, recovering quietly
    3. emerging      - Cautiously comes back out
    """

    NAME = "hiding"

    COMPLETION_BONUS = {
        # Rapid changers
        "comfort": 1,

        # Medium changers
        "independence": -0.05,
        "sociability": -0.2,
        "resilience": -0.02,
        "affection": -0.1,

        # Slow changers
        "patience": 0.75,
        "charisma": -0.2,
        "craftiness": 0.01,
        "craftiness": -0.02,

        # Extra slow changers
        "courage": -0.3,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(15, max(15, (context.courage + context.resilience) * 0.5))

    def __init__(self, character):
        super().__init__(character)
        self.find_duration = 1.5
        self.hide_duration = 12.0
        self.emerge_duration = 1.5

    def next(self, context):
        return None  # -> idle

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "finding_spot"
        self._character.set_pose("laying.side.content")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "finding_spot":
            if self._phase_timer >= self.find_duration:
                self._phase = "hiding"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.aloof")

        elif self._phase == "hiding":
            self._progress = min(1.0, self._phase_timer / self.hide_duration)
            if self._phase_timer >= self.hide_duration:
                self._phase = "emerging"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.neutral")

        elif self._phase == "emerging":
            if self._phase_timer >= self.emerge_duration:
                self.stop(completed=True)
