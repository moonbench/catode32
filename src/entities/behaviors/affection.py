"""Affection behavior for kiss and pets interactions."""

import math
import random
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
            "loyalty": 1.0,
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
            "loyalty": 1.0,
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
            "loyalty": 1.0,
            "mischievousness": 0.1,
            "courage": 0.4,
        },
    },
}


class AffectionBehavior(BaseBehavior):
    """Handles kiss and pets interactions.

    Single "reacting" phase that displays a bubble and reverts pose on completion.
    When the cat rejects affection, it holds a laying pose for the full duration
    with no heart bubble, and the hand is drawn lower to match. Stats are awarded
    at 0.5x so the player can still gradually build affection.
    """

    NAME = "affection"

    # Laying-only poses for rejection (hand position adjusts to match).
    REJECTION_POSES = (
        "laying.side.neutral2",
        "laying.side.bored",
        "laying.side.annoyed",
        "laying.side.content",
    )

    # Hand y-offset reduction when the cat is laying (tunable).
    REJECTION_HAND_Y_OFFSET = 30

    REJECTION_STAT_MULTIPLIER = 0.5

    _REJECTION_THRESHOLDS = {
        "affection":   25,
        "comfort":     30,
        "sociability": 25,
        "courage":     20,
    }

    @classmethod
    def _rejection_chance(cls, context):
        complement = 1.0
        for stat, threshold in cls._REJECTION_THRESHOLDS.items():
            val = getattr(context, stat, 100)
            if val < threshold:
                deficit = (threshold - val) / threshold
                complement *= (1.0 - deficit)
        return 1.0 - complement

    def __init__(self, character):
        super().__init__(character)
        self._bubble = None
        self._duration = 8.0
        self._variant = "pets"
        self._rejecting = False

    def get_completion_bonus(self, context):
        bonus = dict(VARIANTS[self._variant].get("stats", {}))
        fed_factor = max(0.0, (context.fullness - 90) / 10.0)
        if fed_factor > 0:
            bonus["affection"] = bonus.get("affection", 0) + 2 * fed_factor
            bonus["loyalty"] = bonus.get("loyalty", 0) + 0.2 * fed_factor
            bonus["fulfillment"] = bonus.get("fulfillment", 0) + 0.5 * fed_factor
        if getattr(context, 'in_familiar_location', False):
            bonus['affection'] = bonus.get('affection', 0) * 1.2
            bonus['serenity'] = bonus.get('serenity', 0) + 0.5
        else:
            bonus['affection'] = bonus.get('affection', 0) * 0.85
        ph = getattr(context, 'scene_plant_health', 0)
        if ph != 0:
            bonus['affection'] = bonus.get('affection', 0) + ph * 0.1
            bonus['comfort'] = bonus.get('comfort', 0) + ph * 0.1
            bonus['fulfillment'] = bonus.get('fulfillment', 0) + ph * 0.05
        if self._rejecting:
            bonus = {k: v * self.REJECTION_STAT_MULTIPLIER for k, v in bonus.items()}
        return bonus

    def start(self, variant=None, on_complete=None):
        if self._active:
            return
        self._variant = variant if variant in VARIANTS else "pets"
        config = VARIANTS[self._variant]
        super().start(on_complete)
        self._phase = "reacting"
        self._duration = config["duration"]

        context = self._character.context
        if context:
            self._rejecting = random.random() < self._rejection_chance(context)
        else:
            self._rejecting = False

        if self._rejecting:
            self._bubble = None
            self._character.set_pose(random.choice(self.REJECTION_POSES))
        else:
            self._bubble = config["bubble"]
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
        if self._active and self._bubble:
            draw_bubble(renderer, self._bubble, char_x, char_y, self._progress, mirror)

        hand_y_adjust = self.REJECTION_HAND_Y_OFFSET if self._rejecting else 0

        if self._active and self._variant == "pets":
            sweep_speed = 1.2
            raw = (self._phase_timer * sweep_speed) % 2.0
            t = raw if raw <= 1.0 else 2.0 - raw  # 0 -> 1 -> 0

            arc_span = 30
            base_height = 50 - hand_y_adjust
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
            base_height = 52 - hand_y_adjust
            vertical_range = 4

            offset = int(jitter_span * (t - 0.5))
            hand_x = int(char_x + offset if mirror else char_x - offset) - HAND_SCRATCH["width"] // 2
            hand_y = int(char_y - base_height + vertical_range * t)

            frame_count = len(HAND_SCRATCH["frames"])
            frame_speed = HAND_SCRATCH.get("speed", 6)
            frame = int(self._phase_timer * frame_speed) % frame_count

            renderer.draw_sprite_obj(HAND_SCRATCH, hand_x, hand_y, frame=frame, mirror_h=mirror)
