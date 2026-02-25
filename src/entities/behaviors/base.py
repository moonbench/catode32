"""Base class for all character behaviors."""


class BaseBehavior:
    """Abstract base class for all behaviors.

    Each behavior is instantiated on demand, runs to completion, then calls
    next() to determine what behavior to start next. The base stop() handles
    this chaining automatically when completed=True.

    When interrupted by a player action (completed=False), stop() fires any
    scene callback for cleanup but does not chain — the caller is responsible
    for starting the next behavior.
    """

    NAME = "base"

    # Stat effects (override in subclasses)
    STAT_EFFECTS = {}         # e.g., {"energy": 0.5} = +0.5/sec during behavior
    COMPLETION_BONUS = {}     # e.g., {"energy": 10} = +10 on natural completion

    def __init__(self, character):
        """Initialize the behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
                       character.context provides the GameContext.
        """
        self._character = character

        # State
        self._active = False
        self._phase = None
        self._phase_timer = 0.0
        self._pose_before = None
        self._on_complete = None
        self._progress = 0.0  # 0.0 to 1.0

    @property
    def active(self):
        """Return True if this behavior is currently active."""
        return self._active

    @property
    def progress(self):
        """Return the behavior's progress from 0.0 to 1.0."""
        return self._progress

    @property
    def phase(self):
        """Return the current phase name."""
        return self._phase

    @classmethod
    def can_trigger(cls, context):
        """Check if this behavior can be auto-triggered based on stats.

        Args:
            context: The GameContext to check stats from.

        Returns:
            True if this behavior is eligible to trigger.
        """
        return False

    @classmethod
    def get_priority(cls, context):
        """Return the priority of this behavior given the current context.

        Lower values = higher priority. Override for dynamic priority.

        Args:
            context: The GameContext (available for subclass overrides).

        Returns:
            Numeric priority value.
        """
        return 100

    def next(self, context):
        """Return the behavior class to transition to after this one completes.

        Override in subclasses to define chained transitions.
        Return None to fall back to IdleBehavior.

        Args:
            context: The GameContext.

        Returns:
            A behavior class, or None for idle.
        """
        return None

    def start(self, on_complete=None):
        """Begin the behavior.

        Args:
            on_complete: Optional callback(completed, progress) called when
                         the behavior ends, whether naturally or interrupted.
        """
        if self._active:
            return

        print(f"[Behavior started] {self.NAME}")

        self._active = True
        self._phase_timer = 0.0
        self._progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete

    def stop(self, completed=True):
        """End the behavior.

        When completed=True (natural finish):
          - Restores the previous pose
          - Fires the scene callback (if any)
          - Applies completion bonus stats
          - Calls next() and chains to the next behavior (or IdleBehavior)

        When completed=False (player interrupt):
          - Fires the scene callback so the scene can clean up (e.g., remove bowl)
          - Does NOT chain — the caller handles what comes next

        Args:
            completed: True if the behavior finished naturally.
        """
        if not self._active:
            return
        
        print(f"[Behavior stopped] {self.NAME}")

        self._active = False
        self._phase = None
        self._phase_timer = 0.0

        if self._pose_before and completed:
            self._character.set_pose(self._pose_before)
        self._pose_before = None

        callback = self._on_complete
        final_progress = self._progress
        self._on_complete = None

        if callback:
            callback(completed, final_progress)

        if completed:
            context = self._character.context
            if context:
                self.apply_completion_bonus(context, final_progress)

            next_result = self.next(context)
            if isinstance(next_result, tuple):
                next_cls, next_kwargs = next_result
            else:
                next_cls, next_kwargs = next_result, {}

            if next_cls is None:
                from entities.behaviors.idle import IdleBehavior
                next_cls = IdleBehavior

            next_behavior = next_cls(self._character)
            self._character.current_behavior = next_behavior
            next_behavior.start(**next_kwargs)

    def update(self, dt):
        """Update behavior state each frame.

        Override in subclasses to implement phase transitions.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active:
            return
        self._phase_timer += dt

    def draw(self, renderer, char_x, char_y, mirror=False):
        """Draw behavior visual effects (bubbles, particles, etc.).

        Override in subclasses as needed.
        """
        pass

    def apply_stat_effects(self, context, dt):
        """Apply per-frame stat changes while the behavior is active.

        Called each frame by the character's update loop.

        Args:
            context: The GameContext to modify.
            dt: Delta time in seconds.
        """
        for stat, rate in self.STAT_EFFECTS.items():
            current = getattr(context, stat, 0)
            new_value = max(0, min(100, current + rate * dt))
            setattr(context, stat, new_value)

    def apply_completion_bonus(self, context, progress=1.0):
        """Apply completion bonus stats, scaled by how much was completed.

        Args:
            context: The GameContext to modify.
            progress: How much of the behavior was completed (0.0 to 1.0).
        """
        for stat, bonus in self.COMPLETION_BONUS.items():
            current = getattr(context, stat, 0)
            new_value = max(0, min(100, current + bonus * progress))
            setattr(context, stat, new_value)
            print(f">>> {stat}: {new_value}")

