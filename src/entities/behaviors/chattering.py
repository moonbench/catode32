"""Chattering behavior - excited jaw-clicking at prey through the window."""

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

    TRIGGER_STAT = None  # Only reached from observing
    PRIORITY = 40

    STAT_EFFECTS = {"curiosity": -0.5, "playfulness": 0.5}
    COMPLETION_BONUS = {"curiosity": -5, "playfulness": 5}

    def __init__(self, character):
        super().__init__(character)

        self.chatter_duration = 4.0
        self.settle_duration = 1.0

    def next(self, context):
        from entities.behaviors.observing import ObservingBehavior
        return ObservingBehavior

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "chattering"
        self._character.set_pose("sitting.forward.aloof")

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
