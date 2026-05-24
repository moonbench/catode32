"""Being groomed behavior - player brushes or combs the pet."""

import math
import random
from entities.behaviors.base import BaseBehavior
from assets.items import HAIR_BRUSH
from ui import draw_bubble


class BeingGroomedBehavior(BaseBehavior):
    """Player-initiated grooming session — brushing, combing, or tidying up.

    Builds cleanliness, affection, and sociability.
    Softens focus and mischievousness while it's happening.
    May chain into self grooming if the pet is still feeling a little scruffy.

    Phases:
    1. accepting  - Pet adjusts and lets the player start
    2. enjoying   - Relaxed, purring, showing a heart bubble
    3. satisfied  - Content shake-out, all done

    Rejection:
    - Cat holds a laying pose for the full duration; brush still draws at normal
      height. Half stat rewards so grooming an unhappy cat still helps slowly.
    """

    NAME = "being_groomed"

    COMPLETION_BONUS = {
        # Rapid changers
        "focus": -1,
        "playfulness": 2.5,
        "comfort": 2,

        # Medium changers
        "cleanliness": 20,
        "affection": 2,
        "sociability": 2.5,
        "fulfillment": 2.5,
        "maturity": 0.25,

        # Slow changers
        "serenity": 1,

        # Extra slow changers
        "mischievousness": -0.1,
        "courage": 0.15,
    }

    REJECTION_POSES = (
        "laying.side.neutral2",
        "laying.side.bored",
        "laying.side.annoyed",
        "laying.side.content",
    )

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
        self._rejecting = False
        self.accept_duration = 1.5
        self.enjoy_duration = 16.0
        self.satisfy_duration = 1.5

    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        bonus = self.apply_location_bonus(context, bonus)
        if self._rejecting:
            bonus = {k: v * self.REJECTION_STAT_MULTIPLIER for k, v in bonus.items()}
        return bonus

    def _on_first_complete(self, milestones):
        milestones['groomed'] = True

    def apply_location_bonus(self, context, bonus):
        if getattr(context, 'in_familiar_location', False):
            bonus['affection'] = bonus.get('affection', 0) * 1.2
            bonus['serenity'] = bonus.get('serenity', 0) + 0.5
        else:
            bonus['serenity'] = bonus.get('serenity', 0) * 0.7
        return bonus

    def next(self, context):
        if self._rejecting:
            return None
        if context.cleanliness < 70 and context.energy > 30 and random.random() < 0.4:
            return 'self_grooming'
        return None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self.enjoy_duration = random.randint(10, 30)

        context = self._character.context
        if context:
            self._rejecting = random.random() < self._rejection_chance(context)
        else:
            self._rejecting = False

        if self._rejecting:
            self._phase = "enjoying"
            self._character.set_pose(random.choice(self.REJECTION_POSES))
            return

        self._phase = "accepting"
        self._character.set_pose("leaning_forward.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "accepting":
            if self._phase_timer >= self.accept_duration:
                self._phase = "enjoying"
                self._phase_timer = 0.0
                self._character.set_pose("laying.side.bliss")

        elif self._phase == "enjoying":
            self._progress = min(1.0, self._phase_timer / self.enjoy_duration)
            if self._phase_timer >= self.enjoy_duration:
                if self._rejecting:
                    self.stop(completed=True)
                    return
                self._phase = "satisfied"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.happy")
                self._character.play_bursts()

        elif self._phase == "satisfied":
            if self._phase_timer >= self.satisfy_duration:
                self._character.play_bursts()
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or self._phase != "enjoying":
            return

        if not self._rejecting:
            bubble_start = self.enjoy_duration / 2
            bubble_end = bubble_start + 5.0
            if bubble_start <= self._phase_timer < bubble_end:
                bubble_progress = (self._phase_timer - bubble_start) / 5.0
                draw_bubble(renderer, "heart", char_x, char_y, bubble_progress, mirror)

        # Brush arc: triangle-wave sweep, parabolic arc lift at the peak
        sweep_speed = 0.7  # sweeps per second
        raw = (self._phase_timer * sweep_speed) % 2.0
        t = raw if raw <= 1.0 else 2.0 - raw  # 0 -> 1 -> 0

        arc_span = 32        # horizontal travel in pixels
        base_height = 30     # pixels above char_y
        arc_lift = 6         # extra rise at arc midpoint

        offset = int(arc_span * (t - 0.5))
        brush_x = int(char_x + offset if mirror else char_x - offset) - HAIR_BRUSH["width"] // 2
        brush_y = int(char_y - base_height + arc_lift * math.sin(math.pi * t))

        renderer.draw_sprite_obj(HAIR_BRUSH, brush_x, brush_y, mirror_h=mirror)
