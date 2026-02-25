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

    STAT_EFFECTS = {
        "curiosity": 0.1,
        "energy": -0.1,
        "fullness": -0.1,
        "cleanliness": -0.04,
        "comfort": -0.02
    }
    COMPLETION_BONUS = {
        "playfulness": 2,
        "craftiness": -0.05,
        "appetite": -0.002,
        "affection": -0.05,
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

        if self._phase_timer >= self._idle_for:
            self.stop(completed=True)

    def next(self, context):
        """Scan auto-triggerable behaviors and return the highest priority one.

        Returns a behavior class if something should trigger, or None to restart idle.
        """
        if not context:
            return None
        
        # Sometimes, just stay idle
        if random.uniform(0, context.serenity) > 50:
            print("Just staying idle")
            return None

        from entities.behaviors.sleeping import SleepingBehavior
        from entities.behaviors.napping import NappingBehavior
        from entities.behaviors.playing import PlayingBehavior
        from entities.behaviors.zoomies import ZoomiesBehavior
        from entities.behaviors.vocalizing import VocalizingBehavior
        from entities.behaviors.hunting import HuntingBehavior
        from entities.behaviors.investigating import InvestigatingBehavior
        from entities.behaviors.observing import ObservingBehavior
        from entities.behaviors.stretching import StretchingBehavior
        from entities.behaviors.self_grooming import SelfGroomingBehavior
        from entities.behaviors.pacing import PacingBehavior
        from entities.behaviors.lounging import LoungeingBehavior
        from entities.behaviors.startled import StartledBehavior

        candidates = []
        for cls in (SleepingBehavior, NappingBehavior, ZoomiesBehavior, VocalizingBehavior, HuntingBehavior, PlayingBehavior, InvestigatingBehavior, ObservingBehavior, SelfGroomingBehavior, StretchingBehavior, PacingBehavior, LoungeingBehavior, StartledBehavior):
            if cls.can_trigger(context):
                candidates.append(cls)

        if not candidates:
            return None

        priorities = {cls: cls.get_priority(context) for cls in candidates}

        print("Idle change priorities:")
        for cls in sorted(candidates, key=lambda c: priorities[c]):
            print(f"  {cls.NAME}: priority={priorities[cls]}")

        best_priority = min(priorities.values())
        top = [cls for cls, p in priorities.items() if p == best_priority]
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
