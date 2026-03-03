"""Idle behavior - default state when no other behavior is active."""

import random
from entities.behaviors.base import BaseBehavior


class IdleBehavior(BaseBehavior):
    """Default idle behavior — runs when nothing else is active.

    Runs for a while, then completes naturally. On completion,
    next() scans auto-triggerable behaviors and transitions to whichever one
    has the highest priority and a satisfied trigger condition. If nothing
    qualifies, next() returns None and the base class restarts a fresh
    IdleBehavior.
    """

    NAME = "idle"
    POSES = {
        "sitting.side.neutral",
        "sitting.side.happy",
        "sitting.side.aloof",
        "sitting.forward.neutral",
        "sitting.forward.happy",
        "sitting.forward.aloof",
        "standing.side.neutral",
        "standing.side.happy",
    }

    COMPLETION_BONUS = {
        # Rapid changers
        "fullness": -0.15,
        "energy": -0.2,
        "comfort": -0.3,
        "playfulness": 0.2,
        "focus": -0.6,

        # Medium changers
        "fulfillment": -0.1,
        "affection": -0.05,
        "curiosity": 0.025,
        "cleanliness": -0.2,

        # Slow changers
        "fitness": -0.03,
        "appetite": -0.01,
        "craftiness": -0.01,
        "serenity": 0.01,
        "patience": -0.02,
    }

    def __init__(self, character):
        super().__init__(character)
        self.min_pose_duration = 15.0
        self.max_pose_duration = 60.0
        self._time_until_pose_change = 0.0
        self._current_idle_pose = None
        self._idle_for = 30.0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "idling"
        self._pick_new_pose()
        self._idle_for = random.uniform(self.min_pose_duration, self.max_pose_duration)

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt
        self._time_until_pose_change -= dt

        if self._time_until_pose_change <= 0:
            self._pick_new_pose()

        self._progress = min(1.0, self._phase_timer / self._idle_for)

        if self._phase_timer >= self._idle_for:
            self.stop(completed=True)

    def next(self, context):
        # Auto-selection is handled by BehaviorManager._auto_select().
        return None

    def _pick_new_pose(self):
        """Select a new random idle pose and reset the timer."""
        poses = list(self.POSES)
        if self._current_idle_pose and len(poses) > 1:
            poses = [p for p in poses if p != self._current_idle_pose]

        self._current_idle_pose = random.choice(poses)
        self._character.set_pose(self._current_idle_pose)

        self._time_until_pose_change = random.uniform(
            self.min_pose_duration,
            self.max_pose_duration
        )
