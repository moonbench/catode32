"""Zoomies behavior - explosive burst of chaotic energy."""

import random
from entities.behaviors.base import BaseBehavior


class ZoomiesBehavior(BaseBehavior):
    """Pet suddenly sprints around for no apparent reason.

    Requires both high energy and high playfulness to trigger. Burns
    through both rapidly — a natural self-limiting burst.

    Phases:
    1. winding_up  - Pet gets the wild look in its eyes
    2. zooming     - Full chaotic sprint
    3. collapsing  - Sudden stop, pet catches its breath
    """

    NAME = "zoomies"

    STAT_EFFECTS = {
        "fitness": 0.1,
        "energy": -0.1,
        "playfulness": -0.1,
        "cleanliness": -0.1
    }
    COMPLETION_BONUS = {
        "energy": -10,
        "playfulness": -5,
        "fitness": 1,
        "cleanliness": -1
    }

    @classmethod
    def can_trigger(cls, context):
        return context.energy > 40 and context.playfulness > 40
    
    @classmethod
    def get_priority(cls, context):
        return random.uniform(100 - context.playfulness, context.playfulness * 1.5)

    def __init__(self, character):
        super().__init__(character)

        self.windup_duration = 1.0
        self.zoom_duration = 8.0
        self.collapse_duration = 2.0

    def next(self, context):
        if random.random() < 0.2:
            from entities.behaviors.vocalizing import VocalizingBehavior
            return VocalizingBehavior
        return None  # -> idle (exhausted)

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "winding_up"
        self._character.set_pose("sitting.side.happy")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "winding_up":
            if self._phase_timer >= self.windup_duration:
                self._phase = "zooming"
                self._phase_timer = 0.0
                self._character.set_pose("sitting_silly.side.happy")

        elif self._phase == "zooming":
            self._progress = min(1.0, self._phase_timer / self.zoom_duration)
            if self._phase_timer >= self.zoom_duration:
                self._phase = "collapsing"
                self._phase_timer = 0.0
                self._character.set_pose("sleeping.side.sploot")

        elif self._phase == "collapsing":
            if self._phase_timer >= self.collapse_duration:
                self.stop(completed=True)
