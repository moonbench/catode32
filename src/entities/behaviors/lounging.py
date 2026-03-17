"""Lounging behavior - comfortable resting between activities."""

import random
from entities.behaviors.base import BaseBehavior


class LoungeingBehavior(BaseBehavior):
    """Pet lounges comfortably.

    A relaxed resting state more restful than idle. Reached from idle
    when nothing more urgent triggers, and from kneading.

    Phases:
    1. settling - Pet gets comfortable
    2. lounging - Main lounge
    3. rousing  - Brief rouse before returning to activity
    """

    NAME = "lounging"

    COMPLETION_BONUS = {
        # Rapid changers
        "fullness": -0.025,
        "energy": -0.2,
        "comfort": 1,
        "focus": -0.05,
        "playfulness": -0.05,
        
        # Medium changers
        "fulfillment": -0.02,
        "sociability": -0.025,
        "intelligence": -0.005,
        "maturity": 0.02,

        # Slow changers
        "fitness": -0.025,
    }

    LOUNGE_POSES = [
        "laying.side.neutral",
        "laying.side.neutral2",
        "laying.side.aloof",
        "laying.side.happy",
    ]

    def __init__(self, character):
        super().__init__(character)

        self.settle_duration = random.uniform(4.0, 10.0)
        self.lounge_duration = random.uniform(30.0, 120.0)
        self.rouse_duration = random.uniform(1.0, 5.0)
        self._lounge_pose = random.choice(self.LOUNGE_POSES)

    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        return self.apply_location_bonus(context, bonus)

    def apply_location_bonus(self, context, bonus):
        scene = context.last_main_scene
        weather = context.environment.get('weather', 'Clear')
        if scene in ('inside', 'outside', 'treehouse'):
            bonus['comfort'] = bonus.get('comfort', 0) * 1.3
        if scene in ('outside', 'treehouse') and weather in ('Rain', 'Storm', 'Snow'):
            bonus['comfort'] = bonus.get('comfort', 0) - 6
        return bonus

    def next(self, context):
        # Low serenity -> more likely to knead (restless, not fully settled)
        kneading_p = (100 - context.serenity) * 0.35  # 0% at serenity=100, 15% at serenity=0
        if random.random() * 100 < kneading_p:
            return 'kneading'
        # Low energy -> more likely to nap
        napping_p = (100 - context.energy) * 0.45  # 0% at energy=100, 15% at energy=0
        if random.random() * 100 < napping_p:
            return 'napping'
        return None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "settling"
        self._character.set_pose("kneading.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "lounging"
                self._character.set_pose(self._lounge_pose)
                self._phase_timer = 0.0

        elif self._phase == "lounging":
            self._progress = min(1.0, self._phase_timer / self.lounge_duration)
            if self._phase_timer >= self.lounge_duration:
                self._phase = "rousing"
                self._character.set_pose("leaning_forward.side.stretch")
                self._phase_timer = 0.0

        elif self._phase == "rousing":
            if self._phase_timer >= self.rouse_duration:
                self.stop(completed=True)
