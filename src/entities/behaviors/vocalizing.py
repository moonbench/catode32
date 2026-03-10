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
        "energy": -0.75,
        "comfort": -0.5,

        # Slow changers
        "serenity": -0.015,
    }

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
