"""Mischief behavior - pet acts out with chaotic, scheming energy."""

import random
from entities.behaviors.base import BaseBehavior


class MischiefBehavior(BaseBehavior):
    """Pet channels pent-up restlessness into deliberate troublemaking.

    Only reachable from pacing when the pet is immature, devious, and
    brimming with energy. Getting away with it makes them worse. Goes
    back to pacing when done — whether satisfied or just gearing up
    for more.

    Phases:
    1. plotting   - Scheming crouch, deciding on a target
    2. mischief   - Full chaos: running punctuated by brief kneading bursts
    3. satisfied  - Smug, entirely unrepentant
    """

    NAME = "mischief"

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -2,
        "focus": -1,
        "playfulness": -2,

        # Medium changers
        "maturity": -1.5,
        "grace": -0.5,
        "sociability": -0.2,
        "independence": -0.15,
        "resilience": -0.05,
        "affection": -0.05,

        # Extra slow changers
        "dignity": -0.05,
        "mischievousness": 0.03,
        "loyalty": -0.2,
    }

    @classmethod
    def get_priority(cls, context):
        return random.uniform(20, max(20, (200 - context.mischievousness - context.playfulness) * 0.5))

    def __init__(self, character):
        super().__init__(character)
        self.plot_duration = 1.5
        self.mischief_duration = 8.0
        self.satisfy_duration = 1.5
        self.zoom_speed = 50

        self._zoom_direction = 1
        self._dir_change_timer = 0.0
        self._dir_change_interval = 1.5
        self._mischief_sub = "running"
        self._sub_timer = 0.0
        self._sub_duration = 0.0

    def next(self, context):
        # Retreat if the pet's nerve broke and it's now depleted
        if (getattr(context, 'courage', 50) < 60 and
                getattr(context, 'affection', 50) < 40 and
                getattr(context, 'resilience', 50) < 40 and
                getattr(context, 'energy', 50) < 40 and
                random.random() < 0.4):
            return 'hiding'
        return 'pacing'

    def _apply_direction(self):
        """Sync character mirror state with current run direction."""
        self._character.mirror = self._zoom_direction > 0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "plotting"
        self._zoom_direction = random.choice([-1, 1])
        self._dir_change_timer = 0.0
        self._dir_change_interval = random.uniform(1.0, 2.5)
        self._mischief_sub = "running"
        self._sub_timer = 0.0
        self._sub_duration = 0.0
        self._apply_direction()
        self._character.set_pose("leaning_forward.side.pounce")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "plotting":
            if self._phase_timer >= self.plot_duration:
                self._phase = "mischief"
                self._phase_timer = 0.0
                self._mischief_sub = "running"
                self._sub_timer = 0.0
                self._sub_duration = random.uniform(1.5, 3.0)
                self._character.set_pose("running.side.angry")
                self._apply_direction()

        elif self._phase == "mischief":
            context = self._character.context
            x_min = getattr(context, 'scene_x_min', 10) + 20
            x_max = getattr(context, 'scene_x_max', 118) - 20

            self._sub_timer += dt

            if self._mischief_sub == "running":
                # Move character
                self._character.x += self._zoom_direction * self.zoom_speed * dt

                # Bounce at bounds
                if self._character.x <= x_min:
                    self._character.x = x_min
                    self._zoom_direction = 1
                    self._dir_change_timer = 0.0
                    self._dir_change_interval = random.uniform(1.0, 2.5)
                    self._apply_direction()
                elif self._character.x >= x_max:
                    self._character.x = x_max
                    self._zoom_direction = -1
                    self._dir_change_timer = 0.0
                    self._dir_change_interval = random.uniform(1.0, 2.5)
                    self._apply_direction()

                # Random mid-run direction changes
                self._dir_change_timer += dt
                if self._dir_change_timer >= self._dir_change_interval:
                    self._zoom_direction *= -1
                    self._dir_change_timer = 0.0
                    self._dir_change_interval = random.uniform(1.0, 2.5)
                    self._apply_direction()

                # Drop into a kneading pause
                if self._sub_timer >= self._sub_duration:
                    self._mischief_sub = "kneading"
                    self._sub_timer = 0.0
                    self._sub_duration = random.uniform(0.4, 0.8)
                    self._character.set_pose("kneading.side.angry")

            elif self._mischief_sub == "kneading":
                if self._sub_timer >= self._sub_duration:
                    self._mischief_sub = "running"
                    self._sub_timer = 0.0
                    self._sub_duration = random.uniform(1.5, 3.0)
                    self._character.set_pose("running.side.angry")
                    self._apply_direction()

            self._progress = min(1.0, self._phase_timer / self.mischief_duration)
            if self._phase_timer >= self.mischief_duration:
                self._phase = "satisfied"
                self._phase_timer = 0.0
                self._character.set_pose("sitting_silly.side.annoyed")

        elif self._phase == "satisfied":
            if self._phase_timer >= self.satisfy_duration:
                self.stop(completed=True)
