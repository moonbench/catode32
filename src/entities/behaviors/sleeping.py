"""Sleeping behavior for energy recovery."""

import math
import random
from entities.behaviors.base import BaseBehavior


class SleepingBehavior(BaseBehavior):
    """Pet sleeps to recover energy.

    Phases:
    1. settling - Pet settles into sleeping position
    2. sleeping - Main sleep phase, energy recovers
    3. waking - Pet stretches and wakes up
    """

    NAME = "sleeping"

    @classmethod
    def can_trigger(cls, context):
        trigger = context.energy < 40

        if not trigger:
            print("Skipping sleeping. Energy: %6.4f" % context.energy)

        return trigger
    
    @classmethod
    def get_priority(cls, context):
        return random.uniform(5, max(5, context.energy * 1.5))

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": 45,
        "focus": 35,
        "comfort": 5,
        "playfulness": 5,
        "fullness": -5,

        # Medium changers
        "curiosity": 0.25,

        # Slow changers
        "fitness": -0.1,
    }

    # Sleep pose options
    SLEEP_POSES = [
        "sleeping.side.sploot",
        "sleeping.side.modest",
        "sleeping.side.crossed",
    ]

    def next(self, context):
        from entities.behaviors.stretching import StretchingBehavior
        return StretchingBehavior

    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        if context.fullness > 60:
            bonus["energy"] = bonus.get("energy", 0) + 10
        return bonus

    def __init__(self, character):
        """Initialize the sleeping behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.considering_duration = 1.0
        self.settle_duration = 2.5
        self.sleep_duration = 45.0
        self.wake_duration = 5.0

        self._sleep_pose = None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._sleep_pose = random.choice(self.SLEEP_POSES)
        self.sleep_duration = random.randint(40, 100)
        self._phase = "considering"
        self._character.set_pose("sitting.side.looking_down")

    def update(self, dt):
        """Update sleep phases.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "considering":
            if self._phase_timer >= self.considering_duration:
                self._phase = "settling"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "sleeping"
                self._phase_timer = 0.0
                self._character.set_pose(self._sleep_pose)

        elif self._phase == "sleeping":
            # Update progress
            self._progress = min(1.0, self._phase_timer / self.sleep_duration)

            if self._phase_timer >= self.sleep_duration:
                self._phase = "waking"
                self._phase_timer = 0.0
                # Stay in sleep pose briefly while "waking"

        elif self._phase == "waking":
            if self._phase_timer >= self.wake_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        """Draw animated Z's above the pet when sleeping.

        Args:
            renderer: The renderer to draw with.
            char_x: Character's x position on screen.
            char_y: Character's y position.
            mirror: If True, character is facing right.
        """
        if not self._active or self._phase != "sleeping":
            return

        # Animation parameters
        num_zs = 4
        base_x = char_x + (20 if mirror else -20)
        base_y = char_y - 35
        wave_speed = 3.0
        wave_amplitude = 3
        z_spacing_x = 8
        z_spacing_y = -2

        for i in range(num_zs):
            # Each Z has a phase offset for the wave effect
            phase_offset = i * 0.8
            wave_offset = math.sin(self._phase_timer * wave_speed - phase_offset) * wave_amplitude

            # Position each Z slightly higher and to the side
            x = int(base_x + i * z_spacing_x)
            y = int(base_y + i * z_spacing_y + wave_offset)

            renderer.draw_text("z", x, y)
