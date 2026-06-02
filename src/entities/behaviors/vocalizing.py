"""Vocalizing behavior - pet meows, yowls, or chirps with energy."""

import random
from lang import t
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble


class VocalizingBehavior(BaseBehavior):
    """Pet breaks into a vocal outburst, either joyful or to express an unmet need.

    Happy vocalizing requires high energy and playfulness.
    Need-based vocalizing triggers when fullness, comfort, fulfillment, or
    affection drop below NEED_THRESHOLD — the more critical the need, the
    higher the priority.  The speech bubble icon reflects the dominant need.

    Phases:
    1. winding_up  - Pet gears up, shifts pose
    2. vocalizing  - Active vocal display with speech bubble
    3. settling    - Calms down after the outburst
    """

    NAME = "vocalizing"

    NEED_THRESHOLD = 60

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -0.75,
        "comfort": -0.3,

        # Slow changers
        "serenity": -0.015,
    }

    @classmethod
    def _pick_icon(cls, context):
        if getattr(context, 'wants_to_go_home', False):
            print("[vocalizing] wants to go home")
            return "home"
        scene = getattr(context, 'last_main_scene', None)
        if scene in ('outside', 'treehouse'):
            temp = context.environment.get('temperature', 20.0)
            if temp < 2.0:
                print("[vocalizing] temp complaint: cold (%.1fC)" % temp)
                return "cold"
            if temp > 30.0:
                print("[vocalizing] temp complaint: hot (%.1fC)" % temp)
                return "hot"
            weather = context.environment.get('weather', 'Clear')
            if weather in ('Rain', 'Storm'):
                print("[vocalizing] weather complaint: wet")
                return "wet"
            if weather == 'Snow':
                print("[vocalizing] weather complaint: cold")
                return "cold"
        needs = [
            (context.fullness, "hunger"),
            (context.comfort, "discomfort"),
            (context.fulfillment, "bored"),
            (context.affection, "lonely"),
        ]
        for stat, icon in needs:
            print("  %-12s %.2f" % (icon + ":", stat))
        worst_stat, worst_icon = min(needs, key=lambda x: x[0])
        if worst_stat < cls.NEED_THRESHOLD:
            print("[vocalizing] dominant need: %s (%.2f)" % (worst_icon, worst_stat))
            return worst_icon
        print("[vocalizing] no unmet need — exclaim")
        return "exclaim"

    def __init__(self, character):
        super().__init__(character)
        self.windup_duration = random.uniform(1.0, 2.0)
        self.vocalize_duration = random.uniform(6.0, 24.0)
        self.settle_duration = random.uniform(1.0, 3.0)
        self._vocalize_icon = "exclaim"
        self._broadcast_sent = False

    def next(self, context):
        if random.random() < 0.2:
            return 'zoomies'
        return None

    @classmethod
    def _hint_text(cls, icon, context):
        """Return a one-time hint string for unlearned care actions, or None."""
        ms = getattr(context, 'milestones', None)
        if ms is None:
            return None
        if icon == 'hunger' and not ms.get('fed'):
            has_food = any(v > 0 for v in context.food_stock.values())
            if has_food:
                return t("Your pet is hungry!\nYou should feed them a meal or a snack.")
            if not ms.get('store'):
                return t("Your pet is hungry but has no food!\nVisit the store to buy them something.\nPlay minigames to earn more coins.")
        elif icon == 'bored' and not ms.get('played'):
            return t("Your pet is bored.\nYou should play with them.")
        elif icon == 'lonely' and not ms.get('petted'):
            return t("Your pet is lonely.\nYou should play with them or give them some attention.")
        if not ms.get('groomed') and context.cleanliness < 45:
            return t("Your pet is unkempt.\nYou should groom them to help them stay clean.")
        return None

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "winding_up"
        self._broadcast_sent = False
        self._character.set_pose("sitting.forward.neutral")
        self._vocalize_icon = self._pick_icon(self._character.context)
        self.vocalize_duration = random.randint(6, 15)

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "winding_up":
            if self._phase_timer >= self.windup_duration:
                self._phase = "vocalizing"
                self._phase_timer = 0.0
                self._character.set_pose("yelling.forward.lift_and_yell")
                ctx = self._character.context
                if ctx and not getattr(ctx, 'pending_popup', None):
                    hint = self._hint_text(self._vocalize_icon, ctx)
                    if hint:
                        ctx.pending_popup = hint

        elif self._phase == "vocalizing":
            if not self._broadcast_sent:
                ctx = self._character.context
                if ctx and ctx.espnow and ctx.espnow.active:
                    ctx.espnow.send('vocalize', {'i': self._vocalize_icon})
                self._broadcast_sent = True
            self._progress = min(1.0, self._phase_timer / self.vocalize_duration)
            if self._phase_timer >= self.vocalize_duration:
                self._phase = "settling"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "settling":
            if self._phase_timer >= self.settle_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if self._active and self._phase == "vocalizing":
            draw_bubble(renderer, self._vocalize_icon, char_x, char_y, self._progress, mirror)
