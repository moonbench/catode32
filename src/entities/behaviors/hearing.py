"""Hearing behavior - cat notices a distant vocalization from another cat.

Triggered externally when an ESP-NOW 'vocalize' message is received.
Not auto-selected.

Phases:
1. noticing  - Cat pauses, shocked pose, question mark bubble near head
2. listening - Cat settles into aloof pose; heard bubble drawn by EspNowHandler HUD
"""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


class HearingBehavior(BaseBehavior):

    NAME = "hearing"

    COMPLETION_BONUS = {
        "sociability": 0.4,
    }

    def __init__(self, character):
        super().__init__(character)
        self._icon = "exclaim"
        self._listen_duration = 3.0

    def start(self, on_complete=None, icon='exclaim'):
        if self._active:
            return
        super().start(on_complete)
        self._icon = icon
        self._listen_duration = random.uniform(2.0, 4.0)
        self._phase = "noticing"
        self._character.set_pose("sitting.forward.shocked")

    def update(self, dt):
        if not self._active:
            return
        self._phase_timer += dt

        if self._phase == "noticing":
            if self._phase_timer >= random.uniform(0.5, 1.0):
                self._phase = "listening"
                self._phase_timer = 0.0
                self._character.set_pose("sitting_silly.side.aloof")

        elif self._phase == "listening":
            self._progress = min(1.0, self._phase_timer / self._listen_duration)
            if self._phase_timer >= self._listen_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active:
            return
        if self._phase == "noticing":
            draw_bubble(renderer, "question", char_x, char_y, 0.0, mirror)

    def next(self, context):
        # Cats with reasonable sociability almost always call back so players
        # can witness the social chain.  Only very unsociable cats stay quiet.
        if context.sociability > 20:
            p = 0.7 + (context.sociability - 20) / 267.0  # ~0.70 at 20, ~0.95 cap at ~87
            p = min(p, 0.95)
            if random.random() < p:
                return 'vocalizing'
        return None
