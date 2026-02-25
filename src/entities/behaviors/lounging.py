"""Lounging behavior - comfortable resting between activities."""

import random
from entities.behaviors.base import BaseBehavior


class LoungeingBehavior(BaseBehavior):
    """Pet lounges comfortably.

    A relaxed resting state more restful than idle. Reached from idle
    when nothing more urgent triggers, and from kneading.

    Phases:
    1. settling - Pet gets comfortable
    2. lounging - Main lounge
    3. rousing  - Brief rouse before returning to activity
    """

    NAME = "lounging"

    PRIORITY = 90  # Low priority — comfortable fallback

    STAT_EFFECTS = {"comfort": -0.1, "energy": -0.05}
    COMPLETION_BONUS = {}

    @classmethod
    def can_trigger(cls, context):
        return True  # Always eligible — acts as a comfortable alternative to idle

    def __init__(self, character):
        super().__init__(character)

        self.settle_duration = 1.0
        self.lounge_duration = 20.0
        self.rouse_duration = 1.0

    def next(self, context):
        if random.random() < 0.3:
            from entities.behaviors.kneading import KneadingBehavior
            return KneadingBehavior
        return None  # -> idle

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "settling"
        self._character.set_pose("sitting.side.looking_down")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "lounging"
                self._character.set_pose("laying.side.neutral")
                self._phase_timer = 0.0

        elif self._phase == "lounging":
            self._progress = min(1.0, self._phase_timer / self.lounge_duration)
            if self._phase_timer >= self.lounge_duration:
                self._phase = "rousing"
                self._character.set_pose("leaning_forward.side.stretch")
                self._phase_timer = 0.0

        elif self._phase == "rousing":
            if self._phase_timer >= self.rouse_duration:
                self.stop(completed=True)
