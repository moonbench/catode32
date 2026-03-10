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

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -3.5,
        "focus": -2,
        "playfulness": -6,

        # Medium changers
        "intelligence": 1.5,
        "sociability": 2,
        "fulfillment": 2.5,
        "courage": 1.75,
        "couriosity": 1,
        "maturity": 0.5,

        # Slow changers
        "fitness": 1,

        # Extra slow changers
        "loyalty": 1.75,
        "mischievousness": -0.5,
    }

    BEGGING_POSES = [
        "begging.side.arm_up",
        "begging.side.arm_up2",
        "begging.side.demanding",
    ]

    def __init__(self, character):
        super().__init__(character)
        self.warmup_duration = 2.0
        self.train_duration = 12.0
        self.cooldown_duration = 5.0
        self._begging_pair = []
        self._begging_index = 0
        self._pose_timer = 0.0
        self._pose_duration = 0.0

    def next(self, context):
        if random.random() < 0.5:
            return 'playing'
        return None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "warming_up"
        idx = random.randint(0, 2)
        offset = random.randint(1, 2)
        self._begging_pair = [
            self.BEGGING_POSES[idx],
            self.BEGGING_POSES[(idx + offset) % 3],
        ]
        self._begging_index = 0
        self._pose_timer = 0.0
        self._pose_duration = random.uniform(1.5, 2.5)
        self._character.set_pose("standing.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "warming_up":
            if self._phase_timer >= self.warmup_duration:
                self._phase = "training"
                self._phase_timer = 0.0
                self._character.set_pose(self._begging_pair[0])

        elif self._phase == "training":
            self._pose_timer += dt
            if self._pose_timer >= self._pose_duration:
                self._begging_index = 1 - self._begging_index
                self._pose_timer = 0.0
                self._pose_duration = random.uniform(1.5, 2.5)
                self._character.set_pose(self._begging_pair[self._begging_index])

            self._progress = min(1.0, self._phase_timer / self.train_duration)
            if self._phase_timer >= self.train_duration:
                self._phase = "cooling_down"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.looking_down")
                self._character.play_bursts()

        elif self._phase == "cooling_down":
            if self._phase_timer >= self.cooldown_duration:
                self.stop(completed=True)

