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
        "cleanliness": -0.8,
        "intelligence": -0.025,

        # Slow changers
        "fitness": -0.1,
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

        hungry_factor = max(0.0, (30 - context.fullness) / 30.0)
        fed_factor = max(0.0, (context.fullness - 90) / 10.0)
        if hungry_factor > 0:
            bonus["focus"] = bonus.get("focus", 0) - 2 * hungry_factor
            bonus["serenity"] = bonus.get("serenity", 0) - 0.75 * hungry_factor
        if fed_factor > 0:
            bonus["focus"] = bonus.get("focus", 0) + 1.5 * fed_factor
            bonus["serenity"] = bonus.get("serenity", 0) + 0.75 * fed_factor
            bonus["fulfillment"] = bonus.get("fulfillment", 0) + 0.25 * fed_factor

        return self.apply_location_bonus(context, bonus)

    def apply_location_bonus(self, context, bonus):
        scene = context.last_main_scene
        weather = context.environment.get('weather', 'Clear')
        if scene == 'bedroom':
            bonus['energy'] = bonus.get('energy', 0) * 1.2
            bonus['comfort'] = bonus.get('comfort', 0) * 1.2
            self._character.play_bursts()
        if scene in ('outside', 'treehouse') and weather in ('Rain', 'Storm', 'Snow'):
            bonus['comfort'] = bonus.get('comfort', 0) - 7
        if getattr(context, 'in_familiar_location', False):
            bonus['serenity'] = bonus.get('serenity', 0) + 1.5
        else:
            bonus['serenity'] = bonus.get('serenity', 0) - 1
            bonus['comfort'] = bonus.get('comfort', 0) * 0.9
        if context.meteor_shower_happening:
            bonus['serenity'] = bonus.get('serenity', 0) + 1.5
            bonus['fulfillment'] = bonus.get('fulfillment', 0) + 0.75
        if getattr(context, 'in_cat_bed', False):
            bonus['energy'] = bonus.get('energy', 0) * 1.15
            bonus['comfort'] = bonus.get('comfort', 0) + 5
            bonus['serenity'] = bonus.get('serenity', 0) + 1.5
        ph = getattr(context, 'scene_plant_health', 0)
        if ph != 0:
            bonus['serenity'] = bonus.get('serenity', 0) + ph * 0.1
            bonus['comfort'] = bonus.get('comfort', 0) + ph * 0.1
        return bonus
    
    def __init__(self, character):
        super().__init__(character)

        self.settle_duration = random.uniform(1.0, 5.0)
        self.nap_duration = random.uniform(30.0, 120.0)
        self.wake_duration = random.uniform(1.0, 5.0)

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
                if self._character.context:
                    self._character.context.save_if_needed()

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
