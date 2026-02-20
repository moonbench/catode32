"""Idle behavior - default state when no other behavior is active."""

import random
from entities.behaviors.base import BaseBehavior


class IdleBehavior(BaseBehavior):
    """Default idle behavior with random pose selection.

    This behavior runs indefinitely when no other behavior is active.
    It periodically changes between idle-appropriate poses for variety
    and slowly builds curiosity over time.
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

    # Idle is always available as the fallback
    TRIGGER_STAT = None
    PRIORITY = 100  # Lowest priority - only when nothing else triggers
    COOLDOWN = 0.0  # No cooldown

    # Slowly build curiosity while idle
    STAT_EFFECTS = {"curiosity": 0.1}
    COMPLETION_BONUS = {}  # No completion bonus - idle doesn't really "complete"

    def __init__(self, character):
        """Initialize the idle behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # How long to stay in one pose before considering a change
        self.min_pose_duration = 10.0
        self.max_pose_duration = 60.0
        self._time_until_pose_change = 0.0
        self._current_idle_pose = None

    def start(self, on_complete=None):
        """Begin idling.

        Args:
            on_complete: Optional callback (rarely used for idle).
        """
        if self._active:
            return

        self._active = True
        self._phase = "idling"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

        # Pick initial pose and duration
        self._pick_new_pose()

    def stop(self, completed=True):
        """Stop idling.

        Args:
            completed: Whether idle ended naturally (rarely True for idle).
        """
        if not self._active:
            return

        self._active = False
        self._current_idle_pose = None

        # Don't restore pose - let the next behavior set its own pose
        self._pose_before = None

        callback = self._on_complete
        self._on_complete = None
        if callback:
            callback(completed, self._progress)

    def update(self, dt):
        """Update idle state, potentially changing poses.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt
        self._time_until_pose_change -= dt

        # Time for a new pose?
        if self._time_until_pose_change <= 0:
            self._pick_new_pose()

    def _pick_new_pose(self):
        """Select a new random idle pose and reset the timer."""
        poses = list(self.POSES)

        # Try to pick a different pose than current
        if self._current_idle_pose and len(poses) > 1:
            poses = [p for p in poses if p != self._current_idle_pose]

        self._current_idle_pose = random.choice(poses)
        self._character.set_pose(self._current_idle_pose)

        # Random duration until next change
        self._time_until_pose_change = random.uniform(
            self.min_pose_duration,
            self.max_pose_duration
        )

    def can_trigger(self, context, current_time):
        """Idle can always trigger as a fallback.

        The manager handles idle specially - it's started when nothing
        else is active, not through the normal triggering system.
        """
        return True
