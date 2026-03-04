"""Hunting behavior - pet locks onto prey and pounces."""

import math
import random
from entities.behaviors.base import BaseBehavior


class HuntingBehavior(BaseBehavior):
    """Pet enters full predator mode — stalking, darting, pouncing, catching.

    Requires the full suite of hunter stats to be high: energy, playfulness,
    focus, curiosity, and independence all need to be elevated at once. When
    everything lines up the pet spots something irresistible and goes for it.

    On completion the hunt either pays off (chains to eating with caught prey)
    or the prey escapes and the pet slinks back to idle.

    Phases:
    1. stalking  - Slow deliberate observation of prey
    2. darting   - Explosive sprint(s) toward the prey; 1-2 quick dashes
    3. pouncing  - The flying leap with a forward slide
    4. missing   - Frustrated pause after a botched pounce (0-2 times)
    5. catching  - Landing and pinning the catch
    """

    NAME = "hunting"

    COMPLETION_BONUS = {
        # Rapid changers
        "fullness": -2,
        "energy": -3,
        "comfort": -1,
        "playfulness": -0.75,

        # Medium changers
        "fulfillment": 0.25,
        "independence": 0.15,
        "intelligence": 0.05,
        "resilience": 0.1,
        "cleanliness": -0.5,

        # Slow changers
        "fitness": 0.1,
        "craftiness": 0.05,
        "serenity": -0.05,
        "patience": -0.05,
        "charisma": -0.05,

        # Extra slow changers
        "mischievousness": 0.02,
    }

    @classmethod
    def can_trigger(cls, context):
        # Survival instinct: a starving pet will hunt regardless of mood
        if context.fullness < 15 and context.energy > 20:
            return True

        trigger = context.energy > 20 and context.playfulness > 20

        if not trigger:
            failures = []
            if context.energy <= 20:
                failures.append("Energy: %6.4f" % context.energy)
            if context.playfulness <= 20:
                failures.append("Playfulness: %6.4f" % context.playfulness)
            print("Skipping hunting. " + ", ".join(failures))

        return trigger

    @classmethod
    def get_priority(cls, context):
        # Survival instinct: starvation overrides normal priority calculation
        if context.fullness < 15 and context.energy > 20:
            return random.uniform(5, 15)
        upper = max(20, (100 - context.energy) * 0.7 + (100 - context.playfulness) * 0.5)
        hunger_bonus = (100 - context.fullness) * 0.3
        return random.uniform(10, max(10, upper - hunger_bonus * 0.5))

    def __init__(self, character):
        super().__init__(character)
        self.stalk_duration = 3.0
        self.dart_speed = 55          # pixels per second
        self.pounce_slide_speed = 28  # pixels per second during forward slide
        self.pounce_slide_duration = 0.9
        self.miss_pause_duration = 1.2
        self.catch_duration = 2.0

        self._prey_x = 0.0
        self._dart_target_x = 0.0
        self._dart_direction = 1
        self._darts_remaining = 1
        self._miss_count = 0
        self._max_misses = 0

    def next(self, context):
        # Sigmoid centered at fullness=30: near-zero above 40, rapid rise through 40-30
        eating_chance = 0.9 / (1 + math.exp(0.2 * (context.fullness - 30)))
        roll = random.random()
        if roll < eating_chance:
            from assets.items import MOUSE_TOY
            return ('eating', {"food_sprite": MOUSE_TOY, "food_type": "caught_snack"})
        elif roll < eating_chance + 0.25:
            from assets.items import FISH1
            return ('gift_bringing', {"gift_sprite": FISH1})
        return None

    def _get_scene_bounds(self):
        context = self._character.context
        x_min = getattr(context, 'scene_x_min', 10) + 15
        x_max = getattr(context, 'scene_x_max', 118) - 15
        return x_min, x_max

    def _pick_prey_location(self):
        """Pick a random prey position on screen."""
        x_min, x_max = self._get_scene_bounds()
        self._prey_x = random.uniform(x_min, x_max)

    def _set_next_dart_target(self):
        """Set the destination for the next sprint segment.

        If multiple darts are planned, the first is a feinting dash to the
        opposite side of the screen from the prey to build momentum. The
        final dart always goes straight at the prey.
        """
        x_min, x_max = self._get_scene_bounds()

        if self._darts_remaining > 1:
            # Feinting dash: run to the opposite half of the screen from prey
            midpoint = (x_min + x_max) / 2
            if self._prey_x >= midpoint:
                self._dart_target_x = random.uniform(x_min, midpoint)
            else:
                self._dart_target_x = random.uniform(midpoint, x_max)
        else:
            # Final sprint: go straight at the prey
            self._dart_target_x = max(x_min, min(x_max, self._prey_x))

        self._dart_direction = 1 if self._dart_target_x > self._character.x else -1
        self._character.mirror = self._dart_direction > 0

    def _begin_dart_phase(self):
        """Transition into the darting phase with 1-2 fresh sprint segments."""
        self._darts_remaining = random.randint(1, 2)
        self._set_next_dart_target()
        self._phase = "darting"
        self._phase_timer = 0.0
        self._character.set_pose("running.side.angry")

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._miss_count = 0
        # Weighted: 50% no misses, 35% one miss, 15% two misses
        self._max_misses = random.choice([0]*10 + [1]*7 + [2]*3)
        self._pick_prey_location()
        self._phase = "stalking"
        self._character.set_pose("sitting.side.aloof")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "stalking":
            self._progress = min(1.0, self._phase_timer / self.stalk_duration)
            if self._phase_timer >= self.stalk_duration:
                self._begin_dart_phase()

        elif self._phase == "darting":
            self._character.x += self._dart_direction * self.dart_speed * dt

            # Arrived at dart target?
            if self._dart_direction > 0:
                arrived = self._character.x >= self._dart_target_x
            else:
                arrived = self._character.x <= self._dart_target_x

            if arrived:
                self._character.x = self._dart_target_x
                self._darts_remaining -= 1

                if self._darts_remaining > 0:
                    # More darts before pouncing
                    self._set_next_dart_target()
                else:
                    # All darts done — pounce!
                    self._phase = "pouncing"
                    self._phase_timer = 0.0
                    self._character.set_pose("leaning_forward.side.pounce")

        elif self._phase == "pouncing":
            # Slide forward during the pounce leap
            self._character.x += self._dart_direction * self.pounce_slide_speed * dt

            if self._phase_timer >= self.pounce_slide_duration:
                x_min, x_max = self._get_scene_bounds()
                self._character.x = max(x_min, min(x_max, self._character.x))

                if self._miss_count < self._max_misses:
                    # Missed — frustration pause, then try again
                    self._miss_count += 1
                    self._pick_prey_location()
                    self._phase = "missing"
                    self._phase_timer = 0.0
                    self._character.set_pose("sitting.side.aloof")
                else:
                    # Got it!
                    self._phase = "catching"
                    self._phase_timer = 0.0
                    self._character.set_pose("sitting_silly.side.happy")

        elif self._phase == "missing":
            if self._phase_timer >= self.miss_pause_duration:
                self._begin_dart_phase()

        elif self._phase == "catching":
            if self._phase_timer >= self.catch_duration:
                self.stop(completed=True)
