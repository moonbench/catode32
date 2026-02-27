"""Sulking behavior - pet withdraws and broods after pacing doesn't help."""

import random
from entities.behaviors.base import BaseBehavior


class SulkingBehavior(BaseBehavior):
    """Pet retreats into itself for a quiet, withdrawn sulk.

    Only reachable from pacing when the pet is emotionally depleted —
    low fulfillment, affection, and resilience all at once. Sitting alone
    and stewing provides a small comfort, builds independence quietly,
    and lets curiosity drift in to fill the void. Goes back to pacing
    when it's done.

    Phases:
    1. settling  - Finds a spot and curls inward
    2. sulking   - Still and withdrawn
    3. emerging  - Slowly rejoins the world
    """

    NAME = "sulking"

    COMPLETION_BONUS = {
        "comfort": 2,
        "independence": 5,
        "curiosity": 0.5,
        "courage": -0.2,
        "affection": -1,
        "maturity": -0.2,
        "dignity": -0.05,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, (context.fulfillment + context.affection) * 0.5))

    def __init__(self, character):
        super().__init__(character)
        self.settle_duration = 1.0
        self.sulk_duration = 8.0
        self.emerge_duration = 1.5

    def next(self, context):
        from entities.behaviors.pacing import PacingBehavior
        return PacingBehavior  # -> back to pacing

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "settling"
        self._character.set_pose("sitting.side.aloof")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "sulking"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.aloof")

        elif self._phase == "sulking":
            self._progress = min(1.0, self._phase_timer / self.sulk_duration)
            if self._phase_timer >= self.sulk_duration:
                self._phase = "emerging"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "emerging":
            if self._phase_timer >= self.emerge_duration:
                self.stop(completed=True)
