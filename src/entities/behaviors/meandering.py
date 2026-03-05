"""Pacing behavior - restless back-and-forth when the pet is unsettled."""

import random
from entities.behaviors.base import BaseBehavior


class MeanderingBehavior(BaseBehavior):
    NAME = "meandering"

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -0.5,
        "fullness": -1,
        "playfulness": -0.8,
        "comfort": -1,

        # Medium changes
        "intelligence": -0.01,

        # Slow changers
        "fitness": 0.1
    }

    @classmethod
    def can_trigger(cls, context):
        trigger = context.energy > 20

        if not trigger:
            failures = []
            if context.energy <= 20:
                failures.append("Energy: %6.4f" % context.comfort)
            print("Skipping meandering. " + ", ".join(failures))

        return trigger

    def __init__(self, character):
        super().__init__(character)
        self.start_duration = 1.0
        self.pace_duration = 45
        self.stop_duration = 1.5
        self.pace_speed = 8  # pixels per second

        self._pace_direction = 1
        self._dir_change_timer = 0.0
        self._dir_change_interval = 8.0

    def _apply_direction(self):
        """Sync character mirror state with current pace direction."""
        self._character.mirror = self._pace_direction > 0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "starting"
        self._pace_direction = random.choice([-1, 1])
        self._dir_change_timer = 0.0
        self._dir_change_interval = random.uniform(3.0, 20.0)
        self.pace_duration = random.randint(20, 45)
        self._apply_direction()
        self._character.set_pose("sitting.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "starting":
            if self._phase_timer >= self.start_duration:
                self._phase = "pacing"
                self._phase_timer = 0.0
                self._character.set_pose("walking.side.neutral")

        elif self._phase == "pacing":
            context = self._character.context
            x_min = getattr(context, 'scene_x_min', 10) + 20
            x_max = getattr(context, 'scene_x_max', 118) - 20

            # Move the character
            self._character.x += self._pace_direction * self.pace_speed * dt

            # Bounce at bounds
            if self._character.x <= x_min:
                self._character.x = x_min
                self._pace_direction = 1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(3.0, 20.0)
                self._apply_direction()
            elif self._character.x >= x_max:
                self._character.x = x_max
                self._pace_direction = -1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(3.0, 20.0)
                self._apply_direction()

            # Occasional mid-pace direction changes
            self._dir_change_timer += dt
            if self._dir_change_timer >= self._dir_change_interval:
                self._pace_direction *= -1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(3.0, 20.0)
                self._apply_direction()

            self._progress = min(1.0, self._phase_timer / self.pace_duration)
            if self._phase_timer >= self.pace_duration:
                self._phase = "stopping"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.aloof")

        elif self._phase == "stopping":
            if self._phase_timer >= self.stop_duration:
                self.stop(completed=True)
