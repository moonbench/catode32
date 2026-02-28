"""Investigating behavior for curious exploration."""

import random
from entities.behaviors.base import BaseBehavior
from assets.icons import QUESTION_MARK


class InvestigatingBehavior(BaseBehavior):
    """Pet investigates something out of curiosity.

    Phases:
    1. approaching - Pet moves toward target
    2. sniffing - Pet sniffs/examines
    3. reacting - Pet reacts (satisfied curiosity)
    """

    NAME = "investigating"

    @classmethod
    def can_trigger(cls, context):
        trigger = context.curiosity >= 40

        if not trigger:
            print("Skipping investigating. Curiosity: %6.4f" % context.curiosity)

        return trigger

    @classmethod
    def get_priority(cls, context):
        return random.uniform(10, max(10, 100 - context.curiosity))

    COMPLETION_BONUS = {
        # Medium changers
        "curiosity": -0.05,
        "maturity": 0.3,
        "fulfillment": 0.05,

        # Extra slow changers
        "dignity": 0.02,
    }

    def __init__(self, character):
        """Initialize the investigating behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.approach_duration = 1.0
        self.sniff_duration = 13.0
        self.react_duration = 1.5

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "approaching"
        self._character.set_pose("sitting.side.looking_down")

    def update(self, dt):
        """Update investigation phases.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "approaching":
            if self._phase_timer >= self.approach_duration:
                self._phase = "sniffing"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif self._phase == "sniffing":
            self._progress = min(1.0, self._phase_timer / self.sniff_duration)

            if self._phase_timer >= self.sniff_duration:
                self._phase = "reacting"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.looking_down")

        elif self._phase == "reacting":
            if self._phase_timer >= self.react_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or self._phase == "reacting":
            return

        qmark_y = char_y - 42

        if mirror:
            qmark_x = char_x + 18
        else:
            qmark_x = char_x - QUESTION_MARK["width"] - 18

        renderer.draw_sprite_obj(QUESTION_MARK, qmark_x, qmark_y)

    def next(self, context):
        if random.random() < 0.3:
            from entities.behaviors.observing import ObservingBehavior
            return ObservingBehavior
        return None  # -> idle
