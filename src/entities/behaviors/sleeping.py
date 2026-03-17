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

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": 35,
        "focus": 9,
        "comfort": 10,
        "playfulness": 15,
        "fullness": -5,

        # Medium changers
        "curiosity": 0.2,
        "cleanliness": -4,
        "intelligence": -0.05,

        # Slow changers
        "fitness": -0.5,
    }

    # Sleep pose options
    SLEEP_POSES = [
        "sleeping.side.sploot",
        "sleeping.side.modest",
        "sleeping.side.crossed",
    ]

    def next(self, context):
        return 'stretching'


    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        if context.fullness > 60:
            bonus["energy"] = bonus.get("energy", 0) + 12

        if context.playfulness > 75:
            bonus["playfulness"] = bonus.get("playfulness", 0) / 2

        if context.focus > 75:
            bonus["focus"] = bonus.get("focus", 0) / 2
        elif context.focus < 30:
            bonus["focus"] = bonus.get("focus", 0) * 3.0
        return self.apply_location_bonus(context, bonus)

    def apply_location_bonus(self, context, bonus):
        scene = context.last_main_scene
        weather = context.environment.get('weather', 'Clear')
        if scene == 'bedroom':
            bonus['energy'] = bonus.get('energy', 0) * 1.3
            bonus['comfort'] = bonus.get('comfort', 0) * 1.25
        if scene in ('outside', 'treehouse') and weather in ('Rain', 'Storm', 'Snow'):
            bonus['comfort'] = bonus.get('comfort', 0) - 10
        return bonus

    def __init__(self, character):
        """Initialize the sleeping behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Phase durations
        self.considering_duration = random.uniform(1.0, 4.0)
        self.settle_duration = random.uniform(1.0, 10.0)
        self.sleep_duration = random.randint(120, 360)
        self.wake_duration = random.uniform(1.0, 10.0)

        self._sleep_pose = None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._sleep_pose = random.choice(self.SLEEP_POSES)
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
                if self._character.context:
                    self._character.context.save_if_needed()

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
