"""Affection behavior for kiss and pets interactions."""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


# Variant configurations
VARIANTS = {
    "kiss": {
        "pose": "sitting.side.happy",
        "bubble": "heart",
        "duration": 6.5,
        "stats": {
            "affection": 5,
            "fulfillment": 2,
            "comfort": 2,
            "focus": 2,
            "fulfillment": 0.5,
            "sociability": 0.5,
            "routine": 0.5,
            "serenity": 0.2,
            "loyalty": 0.2,
            "mischievousness": -0.1,
        },
    },
    "pets": {
        "pose": "sitting_silly.side.happy",
        "bubble": "heart",
        "duration": 6.0,
        "stats": {
            "affection": 2.5,
            "fulfillment": 1,
            "comfort": 2,
            "focus": 1,
            "playfulness": 1,
            "fulfillment": 0.5,
            "sociability": 0.5,
            "routine": 0.5,
            "serenity": 0.2,
            "loyalty": 0.2,
            "mischievousness": -0.1,
        },
    },
}


class AffectionBehavior(BaseBehavior):
    """Handles kiss and pets interactions.

    Single "reacting" phase that displays a bubble and reverts pose on completion.
    """

    NAME = "affection"

    @classmethod
    def get_priority(cls, context):
        return random.uniform(2, max(2, (100 - context.affection) * 0.1))

    def __init__(self, character):
        """Initialize the affection behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)
        self._bubble = None
        self._duration = 8.0
        self._variant = "pets"

    def get_completion_bonus(self, context):
        return dict(VARIANTS[self._variant].get("stats", {}))

    def start(self, variant=None, on_complete=None):
        if self._active:
            return
        self._variant = variant if variant in VARIANTS else "pets"
        config = VARIANTS[self._variant]
        super().start(on_complete)
        self._phase = "reacting"
        self._bubble = config["bubble"]
        self._duration = config["duration"]

        # Set reaction pose
        self._character.set_pose(config["pose"])

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
