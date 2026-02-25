"""Hunting behavior - pet locks onto prey and pounces."""

import random
from entities.behaviors.base import BaseBehavior


class HuntingBehavior(BaseBehavior):
    """Pet enters full predator mode — stalking, pouncing, catching.

    Requires the full suite of hunter stats to be high: energy, playfulness,
    focus, curiosity, and independence all need to be elevated at once. When
    everything lines up the pet spots something irresistible and goes for it.

    On completion the hunt either pays off (chains to eating with caught prey)
    or the prey escapes and the pet slinks back to idle.

    Phases:
    1. stalking  - Slow deliberate creep toward target
    2. pouncing  - The explosive leap
    3. catching  - Landing and pinning the catch
    """

    NAME = "hunting"

    STAT_EFFECTS = {
        "energy": -0.1,
        "comfort": -0.1,
        "playfulness": -0.1,
        "cleanliness": -0.1,
        "charisma": -0.02,
        "serenity": -0.03,
        "patience": -0.04,
    }
    COMPLETION_BONUS = {
        "fulfillment": 5,
        "independence": 5,
        "intelligence": 5,
        "resilience": 5,
        "fitness": 1,
        "craftiness": 0.2,
        "mischievousness": 0.2,
        "energy": -5,
        "cleanliness": -5,
    }

    @classmethod
    def can_trigger(cls, context):
        return context.energy > 60 and context.playfulness > 40 and context.curiosity > 40
    
    @classmethod
    def get_priority(cls, context):
        upper = max(20, (100 - context.energy) * 0.7 + (100 - context.playfulness) * 0.5)
        return random.uniform(20, upper)

    def __init__(self, character):
        super().__init__(character)
        self.stalk_duration = 10.0
        self.pounce_duration = 1.5
        self.catch_duration = 2.0

    def next(self, context):
        roll = random.random()
        if roll < 0.5:
            from entities.behaviors.eating import EatingBehavior
            from assets.items import FISH1
            return (EatingBehavior, {"food_sprite": FISH1, "meal_type": "fish"})
        elif roll < 0.75:
            from entities.behaviors.gift_bringing import GiftBringingBehavior
            from assets.items import FISH1
            return (GiftBringingBehavior, {"gift_sprite": FISH1})
        return None  # -> idle (prey escaped)

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "stalking"
        self._character.set_pose("sitting.side.aloof")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "stalking":
            self._progress = min(1.0, self._phase_timer / self.stalk_duration)
            if self._phase_timer >= self.stalk_duration:
                self._phase = "pouncing"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.forward.happy")

        elif self._phase == "pouncing":
            if self._phase_timer >= self.pounce_duration:
                self._phase = "catching"
                self._phase_timer = 0.0
                self._character.set_pose("sitting_silly.side.happy")

        elif self._phase == "catching":
            if self._phase_timer >= self.catch_duration:
                self.stop(completed=True)
