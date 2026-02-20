"""Sleeping behavior for energy recovery."""

import random
from entities.behaviors.base import BaseBehavior


class SleepingBehavior(BaseBehavior):
    """Pet sleeps to recover energy.

    Phases:
    1. settling - Pet settles into sleeping position
    2. sleeping - Main sleep phase, energy recovers
    3. waking - Pet stretches and wakes up
    """

    NAME = "sleeping"
    POSES = {
        "sitting.side.neutral",  # Settling pose
        "sleeping.side.sploot",
        "sleeping.side.modest",
        "sleeping.side.crossed",
    }

    # Trigger when energy is low
    TRIGGER_STAT = "energy"
    TRIGGER_THRESHOLD = 30
    TRIGGER_BELOW = True
    PRIORITY = 10  # High priority (low number)
    COOLDOWN = 120.0  # 2 minutes between sleeps

    # Stat effects during sleep
    STAT_EFFECTS = {"energy": 2.0, "vigor": 0.5, "comfort": 0.2}
    COMPLETION_BONUS = {"energy": 15, "vigor": 5, "comfort": 10}

    # Sleep pose options
    SLEEP_POSES = [
        "sleeping.side.sploot",
        "sleeping.side.modest",
        "sleeping.side.crossed",
    ]

    def __init__(self, character):
        """Initialize the sleeping behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.settle_duration = 2.5
        self.sleep_duration = 45.0
        self.wake_duration = 5.0

        self._sleep_pose = None

    def start(self, on_complete=None):
        """Begin sleeping.

        Args:
            on_complete: Optional callback when sleep finishes.
        """
        if self._active:
            return

        self._active = True
        self._phase = "settling"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

        # Pick a random sleep pose
        self._sleep_pose = random.choice(self.SLEEP_POSES)

        # Start with settling pose
        self._character.set_pose("sitting.side.neutral")

    def update(self, dt):
        """Update sleep phases.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "sleeping"
                self._phase_timer = 0.0
                self._character.set_pose(self._sleep_pose)

        elif self._phase == "sleeping":
            # Update progress
            self._progress = min(1.0, self._phase_timer / self.sleep_duration)

            if self._phase_timer >= self.sleep_duration:
                self._phase = "waking"
                self._phase_timer = 0.0
                # Stay in sleep pose briefly while "waking"

        elif self._phase == "waking":
            if self._phase_timer >= self.wake_duration:
                self.stop(completed=True)
