"""Playing behavior for energetic fun."""

from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


# Trigger configurations for toy/throw_stick
TRIGGERS = {
    "toy": {
        "bubble": "exclaim",
        "stats": {"playfulness": 15, "stimulation": 1, "energy": -5, "focus": -1},
    },
    "throw_stick": {
        "bubble": "star",
        "stats": {"playfulness": 15, "energy": -10, "focus": -1},
    },
}


class PlayingBehavior(BaseBehavior):
    """Pet plays energetically.

    Phases:
    1. excited - Pet gets excited
    2. playing - Active play
    3. tired - Pet winds down
    """

    NAME = "playing"

    # Trigger when playfulness is high
    TRIGGER_STAT = "playfulness"
    TRIGGER_THRESHOLD = 70
    TRIGGER_BELOW = False  # Trigger when ABOVE threshold
    PRIORITY = 30
    COOLDOWN = 60.0

    # Playing costs energy but satisfies playfulness
    STAT_EFFECTS = {"playfulness": -2.0, "energy": -0.5, "stimulation": 1.0}
    COMPLETION_BONUS = {"playfulness": -25, "fulfillment": 10}

    def __init__(self, character):
        """Initialize the playing behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.excited_duration = 1.0
        self.play_duration = 5.0
        self.tired_duration = 1.0

        # Trigger-specific state
        self._trigger_type = None
        self._bubble = None

    def start(self, trigger=None, context=None, on_complete=None):
        """Begin playing.

        Args:
            trigger: Optional trigger type ("toy" or "throw_stick") for instant stats/bubble.
            context: GameContext to apply stats to (if trigger specified).
            on_complete: Optional callback when play finishes.
        """
        if self._active:
            return

        self._active = True
        self._phase = "excited"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

        # Handle trigger-specific setup
        self._trigger_type = trigger
        if trigger and trigger in TRIGGERS:
            config = TRIGGERS[trigger]
            self._bubble = config["bubble"]

            # Apply instant stats
            if context:
                for stat, delta in config.get("stats", {}).items():
                    current = getattr(context, stat, 0)
                    new_value = max(0, min(100, current + delta))
                    setattr(context, stat, new_value)
        else:
            self._bubble = None

        self._character.set_pose("sitting.side.happy")

    def update(self, dt):
        """Update play phases.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "excited":
            if self._phase_timer >= self.excited_duration:
                self._phase = "playing"
                self._phase_timer = 0.0
                self._bubble = None  # Clear bubble after excited phase
                self._character.set_pose("sitting_silly.side.happy")

        elif self._phase == "playing":
            self._progress = min(1.0, self._phase_timer / self.play_duration)

            if self._phase_timer >= self.play_duration:
                self._phase = "tired"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "tired":
            if self._phase_timer >= self.tired_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        """Draw the speech bubble during excited phase.

        Args:
            renderer: The renderer to draw with.
            char_x: Character's x position on screen.
            char_y: Character's y position.
            mirror: If True, character is facing right.
        """
        if self._active and self._bubble and self._phase == "excited":
            progress = min(1.0, self._phase_timer / self.excited_duration)
            draw_bubble(renderer, self._bubble, char_x, char_y, progress, mirror)
