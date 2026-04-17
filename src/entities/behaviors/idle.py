"""Idle behavior - default state when no other behavior is active."""

import random
from entities.behaviors.base import BaseBehavior


class IdleBehavior(BaseBehavior):
    """Default idle behavior — runs when nothing else is active.

    Runs for a while, then completes naturally. On completion,
    next() scans auto-triggerable behaviors and transitions to whichever one
    has the highest priority and a satisfied trigger condition. If nothing
    qualifies, next() returns None and the base class restarts a fresh
    IdleBehavior.
    """

    NAME = "idle"

    # Always available regardless of stats
    NEUTRAL_POSES = (
        "sitting.side.neutral",
        "sitting.side.looking_down",
        "sitting.forward.neutral",
        "sitting.forward.sleepy",
        "sitting.forward.content",
        "sitting_silly.side.neutral",
        "standing.side.neutral",
        "standing.side.neutral_looking_down",
    )

    # Added when fullness>=50, comfort>=50, affection>=60, serenity>=60 (all)
    HAPPY_POSES = NEUTRAL_POSES + (
        "standing.side.happy",
        "sitting.forward.aloof",
        "sitting.forward.happy",
        "sitting.side.happy",
        "sitting.side.aloof",
        "sitting_silly.side.happy",
        "sitting_silly.side.aloof",
    )

    # Added when fullness<=25, comfort<=20, affection<=25, or serenity<=15 (any)
    UPSET_POSES = NEUTRAL_POSES + (
        "standing.side.angry",
        "sitting.side.angry",
        "sitting.side.annoyed",
        "sitting_silly.side.annoyed",
        "sitting_silly.side.angry",
    )

    COMPLETION_BONUS = {
        # Rapid changers
        "fullness": -0.05,
        "energy": -0.1,
        "comfort": -0.4,
        "playfulness": -0.05,
        "focus": -0.05,

        # Medium changers
        "fulfillment": -0.02,
        "curiosity": 0.02,
        "cleanliness": -0.06,
        "intelligence": -0.005,

        # Slow changers
        "fitness": -0.015,
        "serenity": 0.0075,
    }

    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        return self.apply_location_bonus(context, bonus)

    def apply_location_bonus(self, context, bonus):
        scene = context.last_main_scene
        weather = context.environment.get('weather', 'Clear')
        if scene in ('outside', 'treehouse') and weather in ('Rain', 'Storm', 'Snow'):
            bonus['comfort'] = bonus.get('comfort', 0) - 5
        if context.meteor_shower_happening:
            bonus['serenity'] = bonus.get('serenity', 0) + 0.5
            bonus['fulfillment'] = bonus.get('fulfillment', 0) + 0.3
            bonus['comfort'] = bonus.get('comfort', 0) + 0.5
        ph = getattr(context, 'scene_plant_health', 0)
        if ph != 0:
            bonus['serenity'] = bonus.get('serenity', 0) + ph * 0.15
            bonus['comfort'] = bonus.get('comfort', 0) + ph * 0.1
        return bonus

    def __init__(self, character):
        super().__init__(character)
        self.min_pose_duration = 15.0
        self.max_pose_duration = 60.0
        self._time_until_pose_change = 0.0
        self._current_idle_pose = None
        self._idle_for = 30.0

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "idling"
        self._pick_new_pose()
        self._idle_for = random.uniform(self.min_pose_duration, self.max_pose_duration * 2.0)

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt
        self._time_until_pose_change -= dt

        if self._time_until_pose_change <= 0:
            self._pick_new_pose()

        self._progress = min(1.0, self._phase_timer / self._idle_for)

        if self._phase_timer >= self._idle_for:
            self.stop(completed=True)

    def next(self, context):
        # Auto-selection is handled by BehaviorManager._auto_select().
        return None

    def _get_pose_set(self):
        ctx = self._character.context
        if ctx is not None:
            if (ctx.fullness >= 50 and ctx.comfort >= 50
                    and ctx.affection >= 60 and ctx.serenity >= 60):
                return self.HAPPY_POSES
            if (ctx.fullness <= 25 or ctx.comfort <= 20
                    or ctx.affection <= 25 or ctx.serenity <= 15):
                return self.UPSET_POSES
        return self.NEUTRAL_POSES

    def _pick_new_pose(self):
        """Select a new random idle pose and reset the timer."""
        poses = list(self._get_pose_set())
        if self._current_idle_pose and len(poses) > 1:
            poses = [p for p in poses if p != self._current_idle_pose]

        self._current_idle_pose = random.choice(poses)
        self._character.set_pose(self._current_idle_pose)

        self._time_until_pose_change = random.uniform(
            self.min_pose_duration,
            self.max_pose_duration
        )
