"""Zoomies behavior - explosive burst of chaotic energy."""

import random
from entities.behaviors.base import BaseBehavior


class ZoomiesBehavior(BaseBehavior):
    """Pet suddenly sprints around for no apparent reason.

    Requires both high energy and high playfulness to trigger. Burns
    through both rapidly — a natural self-limiting burst.

    Phases:
    1. winding_up  - Pet gets the wild look in its eyes
    2. zooming     - Full chaotic sprint
    3. collapsing  - Sudden stop, pet catches its breath
    """

    NAME = "zoomies"

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -3.5,
        "fullness": -0.5,
        "playfulness": -0.3,

        # Medium changers
        "cleanliness": -0.5,
        "maturity": -0.1,
        "comfort": -0.2,

        # Slow changers
        "fitness": 0.075,
    }

    @classmethod
    def can_trigger(cls, context):
        trigger = context.energy > 40 and context.playfulness > 40

        if not trigger:
            failures = []
            if context.energy <= 40:
                failures.append("Energy: %6.4f" % context.energy)
            if context.playfulness <= 40:
                failures.append("Playfulness: %6.4f" % context.playfulness)
            print("Skipping zoomies. " + ", ".join(failures))

        return trigger
    
    @classmethod
    def get_priority(cls, context):
        return random.uniform(100 - context.playfulness * 1.5, context.playfulness * 1.5)

    def __init__(self, character):
        super().__init__(character)

        self.windup_duration = 1.0
        self.zoom_duration = 8.0
        self.collapse_duration = 2.0
        self.zoom_speed = 50  # pixels per second

        self._zoom_direction = 1
        self._dir_change_timer = 0.0
        self._dir_change_interval = 1.5

    def next(self, context):
        if random.random() < 0.2:
            return 'vocalizing'
        return None

    def _apply_direction(self):
        """Sync character mirror state with current zoom direction."""
        self._character.mirror = self._zoom_direction > 0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "winding_up"
        self._zoom_direction = random.choice([-1, 1])
        self._dir_change_timer = 0.0
        self._dir_change_interval = random.uniform(1.0, 3.0)
        self.zoom_duration = random.randint(20, 45)
        self._apply_direction()
        self._character.set_pose("leaning_forward.side.crazy")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "winding_up":
            if self._phase_timer >= self.windup_duration:
                self._phase = "zooming"
                self._phase_timer = 0.0
                self._character.set_pose("running.side.angry")

        elif self._phase == "zooming":
            context = self._character.context
            x_min = getattr(context, 'scene_x_min', 10) + 20
            x_max = getattr(context, 'scene_x_max', 118) - 20

            # Move the character
            self._character.x += self._zoom_direction * self.zoom_speed * dt

            # Bounce at bounds
            if self._character.x <= x_min:
                self._character.x = x_min
                self._zoom_direction = 1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(1.0, 3.0)
                self._apply_direction()
            elif self._character.x >= x_max:
                self._character.x = x_max
                self._zoom_direction = -1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(1.0, 3.0)
                self._apply_direction()

            # Random mid-run direction changes
            self._dir_change_timer += dt
            if self._dir_change_timer >= self._dir_change_interval:
                self._zoom_direction *= -1
                self._dir_change_timer = 0.0
                self._dir_change_interval = random.uniform(1.0, 3.0)
                self._apply_direction()

            self._progress = min(1.0, self._phase_timer / self.zoom_duration)
            if self._phase_timer >= self.zoom_duration:
                self._phase = "collapsing"
                self._phase_timer = 0.0
                self._character.set_pose("sleeping.side.sploot")

        elif self._phase == "collapsing":
            if self._phase_timer >= self.collapse_duration:
                self.stop(completed=True)
