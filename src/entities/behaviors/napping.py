"""Napping behavior - a short, lighter sleep for mid-day energy recovery."""

import math
import random
from entities.behaviors.base import BaseBehavior


class NappingBehavior(BaseBehavior):
    """Pet takes a short nap to recover energy and focus.

    Lighter and shorter than full sleep. Triggers earlier (higher energy
    threshold) so the pet catches a quick rest before becoming truly exhausted.

    Phases:
    1. settling - Pet curls up
    2. napping  - Main nap, energy and focus recover
    3. waking   - Brief rouse before returning to activity
    """

    NAME = "napping"

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": 22,
        "focus": 6,
        "playfulness": 13,
        "fullness": -1,
        "comfort": 4,

        # Medium changers
        "curiosity": 0.1,
        "cleanliness": -2,
        "intelligence": -0.025,

        # Slow changers
        "fitness": -0.25,
    }

    NAP_POSES = [
        "sleeping.side.modest",
        "sleeping.side.crossed",
    ]

    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        if context.fullness > 60:
            bonus["energy"] = bonus.get("energy", 0) + 4

        if context.playfulness > 75:
            bonus["playfulness"] = bonus.get("playfulness", 0) / 2
        
        if context.focus > 75:
            bonus["focus"] = bonus.get("focus", 0) / 2
        elif context.focus < 25:
            bonus["focus"] = bonus.get("focus", 0) * 2.0
        return bonus
    
    def __init__(self, character):
        super().__init__(character)

        self.settle_duration = 1.5
        self.nap_duration = 12.0
        self.wake_duration = 2.0

        self._nap_pose = None

    def next(self, context):
        return 'stretching'

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._nap_pose = random.choice(self.NAP_POSES)
        self.nap_duration = random.randint(12, 45)
        self._phase = "settling"
        self._character.set_pose("sitting.side.looking_down")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "napping"
                self._phase_timer = 0.0
                self._character.set_pose(self._nap_pose)

        elif self._phase == "napping":
            self._progress = min(1.0, self._phase_timer / self.nap_duration)

            if self._phase_timer >= self.nap_duration:
                self._phase = "waking"
                self._phase_timer = 0.0

        elif self._phase == "waking":
            if self._phase_timer >= self.wake_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        """Draw a single small z while napping."""
        if not self._active or self._phase != "napping":
            return

        base_x = char_x + (18 if mirror else -18)
        base_y = char_y - 28
        wave_offset = math.sin(self._phase_timer * 2.5) * 2

        renderer.draw_text("z", int(base_x), int(base_y + wave_offset))
