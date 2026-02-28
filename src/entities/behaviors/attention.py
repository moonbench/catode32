"""Attention behavior for psst and point_bird interactions."""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


# Variant configurations
VARIANTS = {
    "psst": {
        "pose": "sitting.forward.aloof",
        "bubble": "question",
        "duration": 4.5,
        "stats": {"curiosity": 2},
    },
    "point_bird": {
        "pose": "sitting.side.aloof",
        "bubble": "exclaim",
        "duration": 5.0,
        "stats": {"curiosity": 3},
    },
}


class AttentionBehavior(BaseBehavior):
    """Handles psst and point_bird interactions.

    Single "reacting" phase that displays a bubble and reverts pose on completion.
    """

    NAME = "attention"

    @classmethod
    def get_priority(cls, context):
        return random.uniform(2, max(2, (100 - context.curiosity) * 0.1))

    def __init__(self, character):
        """Initialize the attention behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)
        self._bubble = None
        self._duration = 2.0
        self._variant = "psst"

    def get_completion_bonus(self, context):
        return dict(VARIANTS[self._variant].get("stats", {}))

    def next(self, context):
        if self._variant == "point_bird" and context:
            from entities.behaviors.chattering import ChatteringBehavior
            chance = 0.25 * ((context.playfulness + context.curiosity) / 100)
            if random.random() < chance:
                return ChatteringBehavior
        return None

    def start(self, variant=None, on_complete=None):
        if self._active:
            return
        self._variant = variant if variant in VARIANTS else "psst"
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
