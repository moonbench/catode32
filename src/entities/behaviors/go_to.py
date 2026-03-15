"""Go-to behavior - walks the character to a specific x position.

A reusable primitive for directed movement. Used for scene exits (walk off
the left edge), and future actions like walking to a bed or food bowl.

start() kwargs:
    target_x      - destination x coordinate (default: scene_x_min)
    speed         - pixels per second (default: 12)
    pending_scene - if set, assigns context.pending_scene on natural completion,
                    which scene_manager picks up to trigger a scene transition
    next_behavior - behavior name to chain to after arrival (default: None → idle)
    next_kwargs   - kwargs dict passed to the chained behavior
"""

from entities.behaviors.base import BaseBehavior


class GoToBehavior(BaseBehavior):
    NAME = "go_to"
    COMPLETION_BONUS = {}

    def __init__(self, character):
        super().__init__(character)
        self._target_x = 0
        self._speed = 12
        self._direction = -1
        self._pending_scene = None
        self._next_behavior = None
        self._next_kwargs = {}

    def start(self, target_x=None, speed=12, pending_scene=None,
              next_behavior=None, next_kwargs=None, on_complete=None):
        if self._active:
            return
        super().start(on_complete)

        ctx = self._character.context
        if target_x is None:
            target_x = getattr(ctx, 'scene_x_min', 10)

        self._target_x = float(target_x)
        self._speed = speed
        self._pending_scene = pending_scene
        self._next_behavior = next_behavior
        self._next_kwargs = next_kwargs or {}

        self._direction = -1 if self._target_x < self._character.x else 1
        self._character.mirror = self._direction > 0
        self._character.set_pose("walking.side.neutral")
        self._phase = "walking"

    def next(self, context):
        if self._pending_scene:
            context.pending_scene = self._pending_scene
        if self._next_behavior:
            return (self._next_behavior, self._next_kwargs)
        return None

    def update(self, dt):
        if not self._active:
            return

        self._character.x += self._direction * self._speed * dt

        arrived = (
            (self._direction < 0 and self._character.x <= self._target_x) or
            (self._direction > 0 and self._character.x >= self._target_x)
        )
        if arrived:
            self._character.x = self._target_x
            self.stop(completed=True)
