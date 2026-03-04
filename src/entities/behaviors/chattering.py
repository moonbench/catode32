"""Chattering behavior - excited jaw-clicking at prey through the window."""

import random
from entities.behaviors.base import BaseBehavior


class ChatteringBehavior(BaseBehavior):
    """Pet chatters excitedly while fixated on something it can't reach.

    Only reached from observing, when the pet is playful enough to get
    worked up about what it's watching. Returns to observing afterward —
    the target is still there.

    Phases:
    1. chattering - Rapid excited fixation
    2. settling   - Brief wind-down before resuming watch
    """

    NAME = "chattering"

    COMPLETION_BONUS = {
        # Rapid changers
        "playfulness": 1.2,

        # Medium changers
        "curiosity": -0.1,
        "intelligence": -0.02,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, (200 - context.curiosity - context.playfulness) * 0.5))

    def __init__(self, character):
        super().__init__(character)

        self.chatter_duration = 10.0
        self.settle_duration = 1.0

    def next(self, context):
        return 'observing'

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "chattering"
        self._character.set_pose("sitting_silly.side.annoyed")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "chattering":
            self._progress = min(1.0, self._phase_timer / self.chatter_duration)
            if self._phase_timer >= self.chatter_duration:
                self._phase = "settling"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.aloof")

        elif self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or self._phase != "chattering":
            return

        cycle = 1.2
        on_duration = 0.8
        base_x = char_x + (16 if mirror else -36)
        base_y = char_y - 10

        for i in range(3):
            age = (self._phase_timer - i * 0.3) % cycle
            if age < on_duration:
                renderer.draw_text("ek", int(base_x), int(base_y - i * 9))
