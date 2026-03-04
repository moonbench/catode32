"""Vocalizing behavior - pet meows, yowls, or chirps with energy."""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


class VocalizingBehavior(BaseBehavior):
    """Pet breaks into a vocal outburst, either joyful or to express an unmet need.

    Happy vocalizing requires high energy and playfulness.
    Need-based vocalizing triggers when fullness, comfort, fulfillment, or
    affection drop below NEED_THRESHOLD — the more critical the need, the
    higher the priority.  The speech bubble icon reflects the dominant need.

    Phases:
    1. winding_up  - Pet gears up, shifts pose
    2. vocalizing  - Active vocal display with speech bubble
    3. settling    - Calms down after the outburst
    """

    NAME = "vocalizing"

    NEED_THRESHOLD = 35

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -1,

        # Slow changers
        "serenity": -0.025,
    }

    @classmethod
    def can_trigger(cls, context):
        happy = context.energy > 35 and context.playfulness > 35
        needs_unmet = (
            context.fullness < cls.NEED_THRESHOLD
            or context.comfort < cls.NEED_THRESHOLD
            or context.fulfillment < cls.NEED_THRESHOLD
            or context.affection < cls.NEED_THRESHOLD
        )

        trigger = happy or needs_unmet

        if not trigger:
            failures = []
            if context.energy <= 35:
                failures.append("Energy: %6.4f" % context.energy)
            if context.playfulness <= 35:
                failures.append("Playfulness: %6.4f" % context.playfulness)
            print("Skipping vocalizing. " + ", ".join(failures))

        return trigger

    @classmethod
    def get_priority(cls, context):
        urgency = max(
            cls.NEED_THRESHOLD - context.fullness,
            cls.NEED_THRESHOLD - context.comfort,
            cls.NEED_THRESHOLD - context.fulfillment,
            cls.NEED_THRESHOLD - context.affection,
            0,
        )
        if urgency > 0:
            return max(10, 65 - urgency * 2)
        return random.uniform(10, max(10, (200 - context.energy - context.playfulness) * 0.6))

    @classmethod
    def _pick_icon(cls, context):
        needs = [
            (context.fullness, "hunger"),
            (context.comfort, "discomfort"),
            (context.fulfillment, "bored"),
            (context.affection, "lonely"),
        ]
        worst_stat, worst_icon = min(needs, key=lambda x: x[0])
        if worst_stat < cls.NEED_THRESHOLD:
            return worst_icon
        return "exclaim"

    def __init__(self, character):
        super().__init__(character)
        self.windup_duration = 1.0
        self.vocalize_duration = 6.0
        self.settle_duration = 1.5
        self._vocalize_icon = "exclaim"

    def next(self, context):
        if random.random() < 0.2:
            return 'zoomies'
        return None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "winding_up"
        self._character.set_pose("sitting.forward.neutral")
        self._vocalize_icon = self._pick_icon(self._character.context)
        self.vocalize_duration = random.randint(6, 15)

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "winding_up":
            if self._phase_timer >= self.windup_duration:
                self._phase = "vocalizing"
                self._phase_timer = 0.0
                self._character.set_pose("yelling.forward.lift_and_yell")

        elif self._phase == "vocalizing":
            self._progress = min(1.0, self._phase_timer / self.vocalize_duration)
            if self._phase_timer >= self.vocalize_duration:
                self._phase = "settling"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if self._active and self._phase == "vocalizing":
            draw_bubble(renderer, self._vocalize_icon, char_x, char_y, self._progress, mirror)
