"""Affection behavior for kiss and pets interactions."""

import math
from entities.behaviors.base import BaseBehavior
from assets.items import HAND, HAND_SCRATCH
from ui import draw_bubble


# Variant configurations
VARIANTS = {
    "kiss": {
        "pose": "sitting.side.happy",
        "bubble": "heart",
        "duration": 2.5,
        "stats": {
            "affection": 8,
            "fulfillment": 2.5,
            "comfort": 5,
            "focus": 3,
            "playfulness": 2,
            "curiosity": 2,
            "sociability": 2,
            "serenity": 0.25,
            "maturity": 0.2,
            "loyalty": 0.5,
            "mischievousness": -0.1,
            "courage": 0.3,
        },
    },
    "pets": {
        "pose": "sitting_silly.side.happy",
        "bubble": "heart",
        "duration": 5.0,
        "stats": {
            "affection": 4,
            "fulfillment": 1.5,
            "comfort": 5,
            "focus": 1,
            "playfulness": 3.5,
            "curiosity": 2,
            "sociability": 1,
            "serenity": 0.25,
            "maturity": 0.2,
            "loyalty": 0.5,
            "mischievousness": -0.1,
            "courage": 0.3,
        },
    },
    "scratching": {
        "pose": "sitting_silly.side.happy",
        "bubble": "heart",
        "duration": 6.0,
        "stats": {
            "affection": 3,
            "fulfillment": 2.0,
            "comfort": 9,
            "focus": 0.5,
            "playfulness": 4.5,
            "curiosity": 1.5,
            "sociability": 1,
            "serenity": 0.5,
            "maturity": 0.1,
            "loyalty": 0.5,
            "mischievousness": 0.1,
            "courage": 0.4,
        },
    },
}


class AffectionBehavior(BaseBehavior):
    """Handles kiss and pets interactions.

    Single "reacting" phase that displays a bubble and reverts pose on completion.
    """

    NAME = "affection"

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
        bonus = dict(VARIANTS[self._variant].get("stats", {}))
        if getattr(context, 'in_familiar_location', False):
            bonus['affection'] = bonus.get('affection', 0) * 1.2
            bonus['serenity'] = bonus.get('serenity', 0) + 0.5  # more open to affection at home
        else:
            bonus['affection'] = bonus.get('affection', 0) * 0.85  # distracted away from home
        return bonus

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
            self._character.play_bursts()
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

        if self._active and self._variant == "pets":
            sweep_speed = 1.2
            raw = (self._phase_timer * sweep_speed) % 2.0
            t = raw if raw <= 1.0 else 2.0 - raw  # 0 -> 1 -> 0

            arc_span = 30
            base_height = 50
            arc_lift = 5

            offset = int(arc_span * (t - 0.5))
            hand_x = int(char_x + offset if mirror else char_x - offset) - HAND["width"] // 2
            hand_y = int(char_y - base_height + arc_lift * math.sin(math.pi * t))

            renderer.draw_sprite_obj(HAND, hand_x, hand_y, mirror_h=mirror)

        if self._active and self._variant == "scratching":
            scratch_speed = 3.5
            raw = (self._phase_timer * scratch_speed) % 2.0
            t = raw if raw <= 1.0 else 2.0 - raw  # 0 -> 1 -> 0

            jitter_span = 8
            base_height = 52
            vertical_range = 4

            offset = int(jitter_span * (t - 0.5))
            hand_x = int(char_x + offset if mirror else char_x - offset) - HAND_SCRATCH["width"] // 2
            hand_y = int(char_y - base_height + vertical_range * t)

            frame_count = len(HAND_SCRATCH["frames"])
            frame_speed = HAND_SCRATCH.get("speed", 6)
            frame = int(self._phase_timer * frame_speed) % frame_count

            renderer.draw_sprite_obj(HAND_SCRATCH, hand_x, hand_y, frame=frame, mirror_h=mirror)
