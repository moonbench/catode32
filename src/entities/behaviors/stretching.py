"""Stretching behavior for comfort and vigor."""

from entities.behaviors.base import BaseBehavior


class StretchingBehavior(BaseBehavior):
    """Pet stretches for comfort and vigor.

    Phases:
    1. preparing - Pet prepares to stretch
    2. stretching - Main stretch
    3. relaxing - Pet relaxes after stretch
    """

    NAME = "stretching"

    # Trigger when comfort is low
    TRIGGER_STAT = "comfort"
    TRIGGER_THRESHOLD = 40
    TRIGGER_BELOW = True
    PRIORITY = 50

    # Stretching improves comfort and vigor
    STAT_EFFECTS = {"comfort": 1.5}
    COMPLETION_BONUS = {"comfort": 15}

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
        """Begin stretching.

        Args:
            on_complete: Optional callback when stretch finishes.
        """
        if self._active:
            return

        self._active = True
        self._phase = "preparing"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

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
