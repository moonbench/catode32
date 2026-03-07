"""Startled behavior - sudden fright reaction."""

import random
from entities.behaviors.base import BaseBehavior
from assets.icons import EXCLAIM

_EXCLAIM_RISE_DURATION = 1.5   # seconds to reach full height
_EXCLAIM_RISE_AMOUNT = 15      # pixels risen at peak
_EXCLAIM_WOBBLE_INTERVAL = 0.2 # seconds between rotation jumps
_EXCLAIM_ANGLES = (-5, 0, 5)


class StartledBehavior(BaseBehavior):
    """Pet is suddenly startled by something.

    Triggered randomly, with lower courage making it more likely.
    After the shock wears off, the pet either retreats
    to idle or goes to investigate, depending on courage and curiosity.

    Phases:
    1. startled - Frozen in shock (5-10 seconds)
    2. recovering - Brief wind-down before transitioning
    """

    NAME = "startled"

    @classmethod
    def can_trigger(cls, context):
        p = 0.15 * (1 - context.courage / 100)
        trigger = random.random() < p

        if not trigger:
            print("Skipping startled. p=%.3f, Courage %6.4f" % (p, context.courage))

        return trigger

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, context.courage * 1.2))

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -1.5,
        "comfort": -5,

        # Medium changers
        "curiosity": 1,

        # Extra slow changers
        "courage": -0.2,
    }

    def __init__(self, character):
        super().__init__(character)

        self.startled_duration = random.uniform(5.0, 10.0)
        self.recover_duration = 1.0
        self._exclaim_wobble_timer = 0.0
        self._exclaim_angle = 0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "startled"
        self._character.set_pose("sitting.forward.shocked")
        self._exclaim_wobble_timer = 0.0
        self._exclaim_angle = 0

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "startled":
            self._progress = min(1.0, self._phase_timer / self.startled_duration)

            self._exclaim_wobble_timer += dt
            if self._exclaim_wobble_timer >= _EXCLAIM_WOBBLE_INTERVAL:
                self._exclaim_wobble_timer -= _EXCLAIM_WOBBLE_INTERVAL
                self._exclaim_angle = random.choice(_EXCLAIM_ANGLES)

            if self._phase_timer >= self.startled_duration:
                self._phase = "recovering"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.neutral")

        elif self._phase == "recovering":
            if self._phase_timer >= self.recover_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or self._phase != "startled":
            return

        if self._phase_timer > _EXCLAIM_RISE_DURATION:
            return
        
        rise_t = min(1.0, self._phase_timer / _EXCLAIM_RISE_DURATION)
        rise_offset = int(rise_t * _EXCLAIM_RISE_AMOUNT)

        exclaim_y = char_y - 40 - rise_offset

        if mirror:
            exclaim_x = char_x + 16
        else:
            exclaim_x = char_x - EXCLAIM["width"] - 16

        renderer.draw_sprite_obj(
            EXCLAIM,
            exclaim_x,
            exclaim_y,
            rotate=self._exclaim_angle,
        )

    def next(self, context):
        # Higher courage and curiosity both push toward investigating
        p_investigate = (context.curiosity + context.courage) / 200.0
        if random.random() < p_investigate:
            return 'investigating'
        return None
