"""Observing behavior - passive, watchful curiosity."""

import random
from entities.behaviors.base import BaseBehavior


class ObservingBehavior(BaseBehavior):
    """Pet watches something with calm, focused attention.

    A quieter counterpart to investigating — the pet notices something
    and simply stares rather than actively sniffing it out. Can escalate
    to investigating or dissolve back to idle.

    Phases:
    1. noticing       - Pet spots something and orients toward it
    2. watching       - Sustained stillness, eyes locked on target
    3. losing_interest - Attention fades before returning to activity
    """

    NAME = "observing"

    @classmethod
    def can_trigger(cls, context):
        return context.curiosity >= 60

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, 100 - context.curiosity))

    STAT_EFFECTS = {
        "curiosity": -0.1
    }
    COMPLETION_BONUS = {
        "curiosity": -2,
        "fulfillment": 1
    }

    def __init__(self, character):
        super().__init__(character)

        self.notice_duration = 1.0
        self.watch_duration = 8.0
        self.lose_interest_duration = 1.5

    def next(self, context):
        # Chatter if playful enough — stat-gated and probabilistic
        if context and getattr(context, 'playfulness', 0) > 60:
            if random.random() < 0.4:
                from entities.behaviors.chattering import ChatteringBehavior
                return ChatteringBehavior
        if random.random() < 0.5:
            from entities.behaviors.investigating import InvestigatingBehavior
            return InvestigatingBehavior
        return None  # -> idle

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "noticing"
        self._character.set_pose("sitting.side.aloof")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "noticing":
            if self._phase_timer >= self.notice_duration:
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.aloof")

        elif self._phase == "watching":
            self._progress = min(1.0, self._phase_timer / self.watch_duration)
            if self._phase_timer >= self.watch_duration:
                self._phase = "losing_interest"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "losing_interest":
            if self._phase_timer >= self.lose_interest_duration:
                self.stop(completed=True)
