"""Affection behavior for kiss and pets interactions."""

from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


# Variant configurations
VARIANTS = {
    "kiss": {
        "pose": "sitting.side.happy",
        "bubble": "heart",
        "duration": 2.5,
        "stats": {"affection": 10},
    },
    "pets": {
        "pose": "sitting_silly.side.happy",
        "bubble": "heart",
        "duration": 2.0,
        "stats": {"affection": 5},
    },
}


class AffectionBehavior(BaseBehavior):
    """Handles kiss and pets interactions.

    Single "reacting" phase that displays a bubble and reverts pose on completion.
    """

    NAME = "affection"

    # Never auto-triggered - always manual from menu
    TRIGGER_STAT = None
    PRIORITY = 5
    COOLDOWN = 0.0

    def __init__(self, character):
        """Initialize the affection behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)
        self._bubble = None
        self._duration = 8.0

    def start(self, variant=None, context=None, on_complete=None):
        """Begin the affection reaction.

        Args:
            variant: "kiss" or "pets"
            context: GameContext to apply stats to
            on_complete: Optional callback when reaction finishes.
        """
        if self._active:
            return

        config = VARIANTS.get(variant, VARIANTS["pets"])

        self._active = True
        self._phase = "reacting"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

        self._bubble = config["bubble"]
        self._duration = config["duration"]

        # Set reaction pose
        self._character.set_pose(config["pose"])

        # Apply stats immediately
        if context:
            for stat, delta in config.get("stats", {}).items():
                current = getattr(context, stat, 0)
                new_value = max(0, min(100, current + delta))
                setattr(context, stat, new_value)

    def update(self, dt):
        """Update the reaction.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt
        self._progress = min(1.0, self._phase_timer / self._duration)

        if self._phase_timer >= self._duration:
            self.stop(completed=True)

    def stop(self, completed=True):
        """End the reaction."""
        self._bubble = None
        super().stop(completed=completed)

    def draw(self, renderer, char_x, char_y, mirror=False):
        """Draw the speech bubble.

        Args:
            renderer: The renderer to draw with.
            char_x: Character's x position on screen.
            char_y: Character's y position.
            mirror: If True, character is facing right.
        """
        if self._active and self._bubble:
            draw_bubble(renderer, self._bubble, char_x, char_y, self._progress, mirror)
