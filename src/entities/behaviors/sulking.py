"""Sulking behavior - pet withdraws and broods after pacing doesn't help."""

import random
from lang import t
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


class SulkingBehavior(BaseBehavior):
    """Pet retreats into itself for a quiet, withdrawn sulk.

    Only reachable from pacing when the pet is emotionally depleted —
    low fulfillment and affection at once. Sitting alone and stewing
    provides a small comfort, and lets curiosity drift in to fill the
    void. Goes back to pacing
    when it's done.

    Phases:
    1. settling  - Finds a spot and curls inward
    2. sulking   - Still and withdrawn
    3. emerging  - Slowly rejoins the world
    """

    NAME = "sulking"

    BUBBLE_DURATION = 3.5

    COMPLETION_BONUS = {
        # Rapid changers
        "comfort": 0.2,

        # Medium changers
        "affection": -0.025,
        "maturity": -0.025,
        "sociability": -0.1,

        # Slow changers
        "loyalty": -0.02,

        # Extra slow changers
        "courage": -0.005,
    }

    def get_completion_bonus(self, context):
        bonus = dict(super().get_completion_bonus(context))
        hungry_factor = max(0.0, (30 - context.fullness) / 30.0)
        if hungry_factor > 0:
            bonus["loyalty"] = bonus.get("loyalty", 0) - 0.05 * hungry_factor
            bonus["affection"] = bonus.get("affection", 0) - 0.05 * hungry_factor
            bonus["serenity"] = bonus.get("serenity", 0) - 0.75 * hungry_factor
            bonus["fulfillment"] = bonus.get("fulfillment", 0) - 0.05 * hungry_factor
        return bonus

    def __init__(self, character):
        super().__init__(character)
        self.settle_duration = random.uniform(1.0, 5.0)
        self.sulk_duration = random.uniform(20.0, 45.0)
        self.emerge_duration = random.uniform(1.0, 5.0)
        self._bubble_trigger_time = 0.0
        self._bubble_timer = None
        self._sulk_pose = "laying.side.bored"
        self._sulk_cause = "lonely"

    @classmethod
    def _pick_sulk_cause(cls, context):
        """Return the bubble icon for the dominant unmet need."""
        if context is None:
            return "lonely"
        needs = [
            (context.fullness,    "hunger"),
            (context.comfort,     "discomfort"),
            (context.fulfillment, "bored"),
            (context.affection,   "lonely"),
        ]
        _, icon = min(needs, key=lambda x: x[0])
        return icon

    @classmethod
    def _hint_text(cls, cause, context):
        """Return a one-time hint for an unlearned care action, or None."""
        ms = getattr(context, 'milestones', None)
        if ms is None:
            return None
        if cause == 'hunger' and not ms.get('fed'):
            has_food = any(v > 0 for v in context.food_stock.values())
            if has_food:
                return t("Your pet is hungry!\nYou should feed them a meal or a snack.")
            if not ms.get('store'):
                return t("Your pet is hungry but has no food!\nVisit the store to buy them something.\nPlay minigames to earn more coins.")
        elif cause == 'bored' and not ms.get('played'):
            return t("Your pet is bored.\nYou should play with them.")
        elif cause == 'lonely' and not ms.get('petted'):
            return t("Your pet is lonely.\nYou should play with them or give them some attention.")
        return None

    def _pick_sulk_pose(self):
        ctx = self._character.context
        if ctx is None:
            return "laying.side.bored"
        distress = (max(0, 50 - ctx.fullness) + max(0, 50 - ctx.affection) +
                    max(0, 50 - ctx.comfort) + max(0, 50 - ctx.fulfillment)) / 200.0
        w_bored = max(0.0, 1.0 - distress * 2.0)
        w_sulking = 1.0
        w_sulking2 = max(0.0, distress * 2.0 - 0.5)
        r = random.uniform(0, w_bored + w_sulking + w_sulking2)
        if r < w_bored:
            return "laying.side.bored"
        r -= w_bored
        if r < w_sulking:
            return "laying.side.sulking"
        return "laying.side.sulking2"

    def next(self, context):
        return 'pacing'

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "settling"
        self._sulk_pose = self._pick_sulk_pose()
        self._sulk_cause = self._pick_sulk_cause(self._character.context)
        self._character.set_pose("sitting.side.aloof")
        self._bubble_trigger_time = random.uniform(self.sulk_duration * 0.2, self.sulk_duration * 0.7)
        self._bubble_timer = None

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self._phase = "sulking"
                self._phase_timer = 0.0
                self._character.set_pose(self._sulk_pose)
                ctx = self._character.context
                if ctx and not getattr(ctx, 'pending_popup', None):
                    hint = self._hint_text(self._sulk_cause, ctx)
                    if hint:
                        ctx.pending_popup = hint

        elif self._phase == "sulking":
            self._progress = min(1.0, self._phase_timer / self.sulk_duration)

            if self._bubble_timer is None and self._phase_timer >= self._bubble_trigger_time:
                self._bubble_timer = 0.0
            if self._bubble_timer is not None and self._bubble_timer < self.BUBBLE_DURATION:
                self._bubble_timer += dt

            if self._phase_timer >= self.sulk_duration:
                self._phase = "emerging"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "emerging":
            if self._phase_timer >= self.emerge_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or self._phase != "sulking":
            return
        if self._bubble_timer is None or self._bubble_timer >= self.BUBBLE_DURATION:
            return
        progress = self._bubble_timer / self.BUBBLE_DURATION
        draw_bubble(renderer, self._sulk_cause, char_x, char_y, progress, mirror)
