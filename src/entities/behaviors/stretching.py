"""Stretching behavior for comfort and vigor."""

import random
from entities.behaviors.base import BaseBehavior


class StretchingBehavior(BaseBehavior):
    """Pet stretches for comfort and vigor.

    Phases:
    1. preparing - Pet prepares to stretch
    2. stretching - Main stretch
    3. relaxing - Pet relaxes after stretch
    """

    NAME = "stretching"

    @classmethod
    def can_trigger(cls, context):
        return context.comfort < 40
    
    @classmethod
    def get_priority(cls, context):
        return random.uniform(10, max(10, context.comfort))

    # Stretching improves comfort and vigor
    STAT_EFFECTS = {
        "comfort": 0.1
    }
    COMPLETION_BONUS = {
        "comfort": 4
    }

    def next(self, context):
        if random.random() < 0.2:
            from entities.behaviors.kneading import KneadingBehavior
            return KneadingBehavior
        return None  # -> idle

    def __init__(self, character):
        """Initialize the stretching behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.prepare_duration = 0.5
        self.stretch_duration = 3.0
        self.relax_duration = 1.0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "preparing"
        self._character.set_pose("standing.side.neutral")

    def update(self, dt):
        """Update stretch phases.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "preparing":
            if self._phase_timer >= self.prepare_duration:
                self._phase = "stretching"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.stretch")

        elif self._phase == "stretching":
            self._progress = min(1.0, self._phase_timer / self.stretch_duration)

            if self._phase_timer >= self.stretch_duration:
                self._phase = "relaxing"
                self._phase_timer = 0.0
                self._character.set_pose("standing.side.neutral")

        elif self._phase == "relaxing":
            if self._phase_timer >= self.relax_duration:
                self.stop(completed=True)
