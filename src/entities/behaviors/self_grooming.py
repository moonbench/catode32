"""Self grooming behavior - pet cleans itself with focused attention."""

from entities.behaviors.base import BaseBehavior


class SelfGroomingBehavior(BaseBehavior):
    """Pet settles in for a dedicated grooming session.

    Phases:
    1. preparing  - Pet finds a good spot and settles
    2. grooming   - Focused licking and washing
    3. finishing  - Satisfied shake-out, all done
    """

    NAME = "self_grooming"

    COMPLETION_BONUS = {
        # Rapid changers
        "energy": -0.75,
        "comfort": 0.5,
        "focus": -0.5,

        # Medium changers
        "cleanliness": 15,
        "fulfillment": 0.05,
    }

    def __init__(self, character):
        super().__init__(character)
        self.prepare_duration = 1.0
        self.groom_duration = 12.0
        self.finish_duration = 1.5

    def next(self, context):
        return None  # -> idle

    def start(self, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._phase = "preparing"
        self._character.set_pose("sitting.side.neutral")

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == "preparing":
            if self._phase_timer >= self.prepare_duration:
                self._phase = "grooming"
                self._phase_timer = 0.0
                self._character.set_pose("sitting_licking.side.licking_leg")

        elif self._phase == "grooming":
            self._progress = min(1.0, self._phase_timer / self.groom_duration)
            if self._phase_timer >= self.groom_duration:
                self._phase = "finishing"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.happy")

        elif self._phase == "finishing":
            if self._phase_timer >= self.finish_duration:
                self.stop(completed=True)
