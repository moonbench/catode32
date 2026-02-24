"""Training behavior - player-led exercise and skill session."""

import random
from entities.behaviors.base import BaseBehavior


class TrainingBehavior(BaseBehavior):
    """A structured training session with the player.

    Builds physical and mental stats over time. The pet finishes either
    energized enough to play or content to rest — their call.

    Phases:
    1. warming_up   - Gets into position and focused
    2. training     - The active session
    3. cooling_down - Wraps up, catches breath
    """

    NAME = "training"

    TRIGGER_STAT = None  # Always player-triggered
    PRIORITY = 5

    STAT_EFFECTS = {
        "intelligence": 0.2,
        "fitness": 0.3,
        "resilience": 0.2,
        "sociability": 0.1,
        "fulfillment": 0.2,
        "patience": 0.2,
        "loyalty": 0.2,
        "courage": 0.2,
        "energy": -0.5,
        "focus": -0.3,
    }
    COMPLETION_BONUS = {
        "intelligence": 8,
        "fitness": 10,
        "resilience": 8,
        "sociability": 5,
        "fulfillment": 8,
        "patience": 5,
        "loyalty": 8,
        "courage": 8,
        "energy": -15,
        "focus": -10,
    }

    def __init__(self, character):
        super().__init__(character)
        self.warmup_duration = 2.0
        self.train_duration = 12.0
        self.cooldown_duration = 2.0

    def next(self, context):
        if random.random() < 0.5:
            from entities.behaviors.playing import PlayingBehavior
            return PlayingBehavior
        return None  # -> idle

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "warming_up"
        self._character.set_pose("standing.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "warming_up":
            if self._phase_timer >= self.warmup_duration:
                self._phase = "training"
                self._phase_timer = 0.0
                self._character.set_pose("standing.side.happy")

        elif self._phase == "training":
            self._progress = min(1.0, self._phase_timer / self.train_duration)
            if self._phase_timer >= self.train_duration:
                self._phase = "cooling_down"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "cooling_down":
            if self._phase_timer >= self.cooldown_duration:
                self.stop(completed=True)
