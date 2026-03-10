"""Gift bringing behavior - pet proudly presents a found treasure."""

from entities.behaviors.base import BaseBehavior


class GiftBringingBehavior(BaseBehavior):
    """Pet delivers a gift and sits proudly beside it.

    Typically chained from hunting when the pet decides to share the catch
    rather than eat it. Accepts any item sprite — fish, toys, found objects.
    The gift lowers into place as the pet arrives, then the pet holds the pose
    until the moment passes.

    Phases:
    1. approaching  - Pet arrives, gift lowers to ground
    2. presenting   - Sitting tall and proud beside the gift
    3. satisfied    - Wind-down, mission accomplished
    """

    NAME = "gift_bringing"

    COMPLETION_BONUS = {
        # Medium changers
        "sociability": 0.5,
        "affection": 0.25,

        # Extra slow changers
        "loyalty": 0.05,
    }

    GIFT_OFFSET_X = 30

    def __init__(self, character):
        super().__init__(character)
        self._gift_sprite = None
        self._gift_y_progress = 0.0
        self.approach_duration = 1.5
        self.present_duration = 8.0
        self.satisfy_duration = 1.5

    def next(self, context):
        return None  # -> idle

    def start(self, gift_sprite=None, on_complete=None):
        if self._active:
            return
        from assets.items import MOUSE_TOY
        super().start(on_complete)
        self._phase = "approaching"
        self._gift_sprite = gift_sprite or MOUSE_TOY
        self._gift_y_progress = 0.0
        self._character.set_pose("sitting.side.happy")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "approaching":
            self._gift_y_progress = min(1.0, self._phase_timer / self.approach_duration)
            if self._phase_timer >= self.approach_duration:
                self._phase = "presenting"
                self._phase_timer = 0.0
                self._character.set_pose("yelling.forward.lift_and_yell")

        elif self._phase == "presenting":
            self._progress = min(1.0, self._phase_timer / self.present_duration)
            if self._phase_timer >= self.present_duration:
                self._phase = "satisfied"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.happy")
                self._character.play_bursts()

        elif self._phase == "satisfied":
            if self._phase_timer >= self.satisfy_duration:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active or not self._gift_sprite:
            return

        gift_width = self._gift_sprite["width"]
        gift_height = self._gift_sprite["height"]

        ground_y = int(char_y) - gift_height
        start_y = ground_y - 40
        gift_y = int(start_y + (ground_y - start_y) * self._gift_y_progress)

        if mirror:
            gift_x = int(char_x) + self.GIFT_OFFSET_X - gift_width // 2
        else:
            gift_x = int(char_x) - self.GIFT_OFFSET_X - gift_width // 2

        renderer.draw_sprite_obj(self._gift_sprite, gift_x, gift_y, frame=0)
