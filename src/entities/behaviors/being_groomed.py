"""Being groomed behavior - player brushes or combs the pet."""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


class BeingGroomedBehavior(BaseBehavior):
    """Player-initiated grooming session — brushing, combing, or tidying up.

    Builds cleanliness, affection, patience, grace, and sociability.
    Softens independence, focus, and mischievousness while it's happening.
    May chain into self grooming if the pet is still feeling a little scruffy.

    Phases:
    1. accepting  - Pet adjusts and lets the player start
    2. enjoying   - Relaxed, purring, showing a heart bubble
    3. satisfied  - Content shake-out, all done
    """

    NAME = "being_groomed"

    COMPLETION_BONUS = {
        "cleanliness": 20,
        "affection": 10,
        "grace": 5,
        "sociability": 3,
        "independence": -5,
        "mischievousness": -0.1,
        "patience": -5.5,
        "focus": -3.3,
        "maturity": -0.1,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(3, max(3, context.cleanliness * 0.1))

    def __init__(self, character):
        super().__init__(character)
        self.accept_duration = 1.5
        self.enjoy_duration = 8.0
        self.satisfy_duration = 1.5

    def next(self, context):
        from entities.behaviors.self_grooming import SelfGroomingBehavior
        if SelfGroomingBehavior.can_trigger(context) and random.random() < 0.4:
            return SelfGroomingBehavior
        return None  # -> idle

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "accepting"
        self._character.set_pose("sitting.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "accepting":
            if self._phase_timer >= self.accept_duration:
                self._phase = "enjoying"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.aloof")

        elif self._phase == "enjoying":
            self._progress = min(1.0, self._phase_timer / self.enjoy_duration)
            if self._phase_timer >= self.enjoy_duration:
                self._phase = "satisfied"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.happy")

        elif self._phase == "satisfied":
            if self._phase_timer >= self.satisfy_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if self._active and self._phase == "enjoying":
            draw_bubble(renderer, "heart", char_x, char_y, self._progress, mirror)
