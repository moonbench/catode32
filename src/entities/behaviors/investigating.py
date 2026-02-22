"""Investigating behavior for curious exploration."""

from entities.behaviors.base import BaseBehavior


class InvestigatingBehavior(BaseBehavior):
    """Pet investigates something out of curiosity.

    Phases:
    1. approaching - Pet moves toward target
    2. sniffing - Pet sniffs/examines
    3. reacting - Pet reacts (satisfied curiosity)
    """

    NAME = "investigating"

    # Trigger when curiosity is high
    TRIGGER_STAT = "curiosity"
    TRIGGER_THRESHOLD = 70
    TRIGGER_BELOW = False  # Trigger when ABOVE threshold
    PRIORITY = 40
    COOLDOWN = 90.0

    # Investigating satisfies curiosity but adds stimulation
    STAT_EFFECTS = {"curiosity": -1.0, "stimulation": 0.5}
    COMPLETION_BONUS = {"curiosity": -20, "stimulation": 10, "fulfillment": 5}

    def __init__(self, character):
        """Initialize the investigating behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.approach_duration = 1.0
        self.sniff_duration = 3.0
        self.react_duration = 1.5

    def start(self, on_complete=None):
        """Begin investigating.

        Args:
            on_complete: Optional callback when investigation finishes.
        """
        if self._active:
            return

        self._active = True
        self._phase = "approaching"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

        self._character.set_pose("sitting.side.looking_down")

    def update(self, dt):
        """Update investigation phases.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt

        print(f"-->{self._phase}")

        if self._phase == "approaching":
            if self._phase_timer >= self.approach_duration:
                self._phase = "sniffing"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif self._phase == "sniffing":
            self._progress = min(1.0, self._phase_timer / self.sniff_duration)

            if self._phase_timer >= self.sniff_duration:
                self._phase = "reacting"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.looking_down")

        elif self._phase == "reacting":
            if self._phase_timer >= self.react_duration:
                self.stop(completed=True)
