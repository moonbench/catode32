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

    COMPLETION_BONUS = {
        # Rapid changers
        "fullness": -0.1,
        "energy": -0.3,
        "comfort": -0.2,
        
        # Medium changers
        "fulfillment": -0.15,
        "sociability": -0.05,

        # Slow changers
        "fitness": -0.2,
        "patience": 1,
    }

    @classmethod
    def can_trigger(cls, context):
        trigger = context.focus > 30 and context.serenity > 30

        if not trigger:
            failures = []
            if context.focus <= 30:
                failures.append("Focus: %6.2f" % context.focus)
            if context.serenity <= 30:
                failures.append("Serenity: %6.2f" % context.serenity)
            print("Skipping lounging. " + ", ".join(failures))

        return trigger
    
    @classmethod
    def get_priority(cls, context):
        return 100 - random.uniform(context.serenity, context.serenity * 2.0)

    LOUNGE_POSES = [
        "laying.side.neutral",
        "laying.side.neutral2",
        "laying.side.aloof",
        "laying.side.happy",
    ]

    def __init__(self, character):
        super().__init__(character)

        self.settle_duration = random.uniform(4.0, 10.0)
        self.lounge_duration = random.uniform(30.0, 120.0)
        self.rouse_duration = random.uniform(1.0, 5.0)
        self._lounge_pose = random.choice(self.LOUNGE_POSES)

    def next(self, context):
        # Inline KneadingBehavior.get_priority: random.uniform(20, max(20, 100 - serenity))
        kneading_p = random.uniform(20, max(20, 100 - context.serenity))
        if random.random() * 100 < kneading_p:
            return 'kneading'
        # Inline NappingBehavior.get_priority: random.uniform(10, max(10, energy * 2))
        napping_p = random.uniform(10, max(10, context.energy * 2))
        if random.random() * 100 < napping_p:
            return 'napping'
        return None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "settling"
        self._character.set_pose("kneading.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "lounging"
                self._character.set_pose(self._lounge_pose)
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
