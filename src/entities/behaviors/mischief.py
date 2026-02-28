"""Mischief behavior - pet acts out with chaotic, scheming energy."""

import random
from entities.behaviors.base import BaseBehavior


class MischiefBehavior(BaseBehavior):
    """Pet channels pent-up restlessness into deliberate troublemaking.

    Only reachable from pacing when the pet is immature, devious, and
    brimming with energy. Getting away with it makes them worse. Goes
    back to pacing when done — whether satisfied or just gearing up
    for more.

    Phases:
    1. plotting   - Scheming look, deciding on a target
    2. mischief   - Full chaos mode
    3. satisfied  - Smug, entirely unrepentant
    """

    NAME = "mischief"

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -2,
        "focus": -2,
        "playfulness": -2,

        # Medium changers
        "maturity": -1.5,
        "grace": -0.5,
        "sociability": -0.2,
        "independence": -0.15,
        "resilience": -0.05,
        "affection": -0.05,

        # Extra slow changers
        "dignity": -0.05,
        "mischievousness": 0.03,
        "loyalty": -0.2,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, (200 - context.mischievousness - context.playfulness) * 0.5))

    def __init__(self, character):
        super().__init__(character)
        self.plot_duration = 1.5
        self.mischief_duration = 8.0
        self.satisfy_duration = 1.5

    def next(self, context):
        # Retreat if the pet's nerve broke and it's now depleted
        if (getattr(context, 'courage', 50) < 60 and
                getattr(context, 'affection', 50) < 40 and
                getattr(context, 'resilience', 50) < 40 and
                getattr(context, 'energy', 50) < 40 and
                random.random() < 0.4):
            from entities.behaviors.hiding import HidingBehavior
            return HidingBehavior
        from entities.behaviors.pacing import PacingBehavior
        return PacingBehavior  # -> back to pacing

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "plotting"
        self._character.set_pose("sitting.side.aloof")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "plotting":
            if self._phase_timer >= self.plot_duration:
                self._phase = "mischief"
                self._phase_timer = 0.0
                self._character.set_pose("sitting_silly.side.happy")

        elif self._phase == "mischief":
            self._progress = min(1.0, self._phase_timer / self.mischief_duration)
            if self._phase_timer >= self.mischief_duration:
                self._phase = "satisfied"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.happy")

        elif self._phase == "satisfied":
            if self._phase_timer >= self.satisfy_duration:
                self.stop(completed=True)
