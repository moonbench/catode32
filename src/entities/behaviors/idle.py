"""Idle behavior - default state when no other behavior is active."""

import random
from entities.behaviors.base import BaseBehavior


class IdleBehavior(BaseBehavior):
    """Default idle behavior — runs when nothing else is active.

    Runs for CHECK_INTERVAL seconds, then completes naturally. On completion,
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

    TRIGGER_STAT = None
    PRIORITY = 100  # Lowest priority — only used as fallback

    STAT_EFFECTS = {"curiosity": 0.1, "energy": -0.1, "fullness": -0.1, "affection": -0.05}
    COMPLETION_BONUS = {}

    CHECK_INTERVAL = 15.0  # How long each idle cycle runs before checking next()

    def __init__(self, character):
        super().__init__(character)
        self.min_pose_duration = 10.0
        self.max_pose_duration = 30.0
        self._time_until_pose_change = 0.0
        self._current_idle_pose = None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "idling"
        self._pick_new_pose()

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt
        self._time_until_pose_change -= dt

        if self._time_until_pose_change <= 0:
            self._pick_new_pose()

        if self._phase_timer >= self.CHECK_INTERVAL:
            self.stop(completed=True)

    def next(self, context):
        """Scan auto-triggerable behaviors and return the highest priority one.

        Returns a behavior class if something should trigger, or None to restart idle.
        """
        if not context:
            return None

        from entities.behaviors.sleeping import SleepingBehavior
        from entities.behaviors.napping import NappingBehavior
        from entities.behaviors.playing import PlayingBehavior
        from entities.behaviors.zoomies import ZoomiesBehavior
        from entities.behaviors.vocalizing import VocalizingBehavior
        from entities.behaviors.investigating import InvestigatingBehavior
        from entities.behaviors.observing import ObservingBehavior
        from entities.behaviors.stretching import StretchingBehavior
        from entities.behaviors.self_grooming import SelfGroomingBehavior
        from entities.behaviors.lounging import LoungeingBehavior

        candidates = []
        for cls in (SleepingBehavior, NappingBehavior, ZoomiesBehavior, VocalizingBehavior, PlayingBehavior, InvestigatingBehavior, ObservingBehavior, SelfGroomingBehavior, StretchingBehavior, LoungeingBehavior):
            if cls.can_trigger(context):
                candidates.append(cls)

        if not candidates:
            return None

        best_priority = min(cls.get_priority(context) for cls in candidates)
        top = [cls for cls in candidates if cls.get_priority(context) == best_priority]
        return random.choice(top)

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
