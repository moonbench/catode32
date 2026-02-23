"""Snacking behavior for snack and treat interactions."""

from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


# Variant configurations
VARIANTS = {
    "snack": {
        "pose": "sitting.forward.happy",
        "bubble": "heart",
        "duration": 2.0,
        "stats": {"fullness": 10},
    },
    "treat": {
        "pose": "sitting.side.happy",
        "bubble": "heart",
        "duration": 2.0,
        "stats": {"fullness": 5, "affection": 3},
    },
}


class SnackingBehavior(BaseBehavior):
    """Handles snack and treat interactions.

    Single "reacting" phase that displays a bubble and reverts pose on completion.
    """

    NAME = "snacking"

    # Never auto-triggered - always manual from menu
    TRIGGER_STAT = None
    PRIORITY = 5

    def __init__(self, character):
        """Initialize the snacking behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)
        self._bubble = None
        self._duration = 2.0

    def start(self, variant=None, on_complete=None):
        """Begin the snacking reaction.

        Args:
            variant: "snack" or "treat"
            on_complete: Optional callback when reaction finishes.
        """
        if self._active:
            return

        config = VARIANTS.get(variant, VARIANTS["snack"])

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
        context = self._character.context
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
