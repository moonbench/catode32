"""Sulking behavior - pet withdraws and broods after pacing doesn't help."""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


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

    BUBBLE_DURATION = 3.5

    COMPLETION_BONUS = {
        # Rapid changers
        "comfort": 0.25,

        # Medium changers
        "independence": -0.05,
        "curiosity": 0.05,
        "affection": -1,
        "maturity": -0.2,
        "sociability": -0.2,
        "resilience": -0.05,

        # Slow changers
        "loyalty": -0.05,

        # Extra slow changers
        "dignity": -0.05,
        "courage": -0.2,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(10, max(20, (context.fulfillment + context.affection) * 0.45))

    def __init__(self, character):
        super().__init__(character)
        self.settle_duration = 1.0
        self.sulk_duration = 20.0
        self.emerge_duration = 1.5
        self._bubble_trigger_time = 0.0
        self._bubble_timer = None

    def next(self, context):
        return 'pacing'

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "settling"
        self._character.set_pose("sitting.side.aloof")
        self._bubble_trigger_time = random.uniform(self.sulk_duration * 0.2, self.sulk_duration * 0.7)
        self._bubble_timer = None

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "sulking"
                self._phase_timer = 0.0
                self._character.set_pose("laying.side.bored")

        elif self._phase == "sulking":
            self._progress = min(1.0, self._phase_timer / self.sulk_duration)

            if self._bubble_timer is None and self._phase_timer >= self._bubble_trigger_time:
                self._bubble_timer = 0.0
            if self._bubble_timer is not None and self._bubble_timer < self.BUBBLE_DURATION:
                self._bubble_timer += dt

            if self._phase_timer >= self.sulk_duration:
                self._phase = "emerging"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "emerging":
            if self._phase_timer >= self.emerge_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or self._phase != "sulking":
            return
        if self._bubble_timer is None or self._bubble_timer >= self.BUBBLE_DURATION:
            return
        progress = self._bubble_timer / self.BUBBLE_DURATION
        draw_bubble(renderer, "lonely", char_x, char_y, progress, mirror)
