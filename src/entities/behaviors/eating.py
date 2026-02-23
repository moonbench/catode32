"""Eating behavior for the character entity."""

from entities.behaviors.base import BaseBehavior


class EatingBehavior(BaseBehavior):
    """Manages the eating animation sequence for a character.

    Phases:
    1. lowering - Bowl lowers to ground, character stands happy
    2. pre_eating - Brief pause, character leans forward neutral
    3. eating - Character eats, bowl empties through frames
    4. post_eating - Brief pause, character leans forward neutral
    5. Complete - Return to original pose
    """

    NAME = "eating"

    # Eating cannot be auto-triggered - requires explicit bowl/meal
    TRIGGER_STAT = None
    PRIORITY = 10  # High priority when manually triggered

    # No per-frame stat effects - stats applied on completion
    STAT_EFFECTS = {}
    COMPLETION_BONUS = {}  # Handled specially via MEAL_STATS

    # Stat effects for each meal type
    MEAL_STATS = {
        "chicken": {"fullness": 30, "energy": 10},
        "fish": {"fullness": 25, "affection": 5},
    }
    DEFAULT_STATS = {"fullness": 20}

    def __init__(self, character):
        """Initialize the eating behavior.

        Args:
            character: The CharacterEntity this behavior belongs to.
        """
        super().__init__(character)

        # Eating-specific state
        self._bowl_sprite = None
        self._bowl_frame = 0.0
        self._bowl_y_progress = 0.0  # 0 = start (above), 1 = ground level
        self._meal_type = None

        # Timing configuration
        self.eating_speed = 0.4  # Bowl frames per second during eating phase
        self.lower_duration = 0.5  # Time for bowl to lower
        self.pause_duration = 1.5  # Pre/post eating pause

    @property
    def progress(self):
        """Return eating progress from 0.0 to 1.0."""
        if not self._active or not self._bowl_sprite:
            return 0.0
        num_frames = len(self._bowl_sprite["frames"])
        return min(1.0, self._bowl_frame / num_frames)

    def start(self, bowl_sprite=None, meal_type=None, on_complete=None):
        """Begin the eating animation sequence.

        Args:
            bowl_sprite: The food bowl sprite dict (with frames)
            meal_type: Type of meal being eaten (e.g., "chicken", "fish")
            on_complete: Optional callback function called when eating finishes.
        """
        if self._active:
            return

        self._active = True
        self._phase = "lowering"
        self._phase_timer = 0.0
        self._progress = 0.0
        self._bowl_sprite = bowl_sprite
        self._bowl_frame = 0.0
        self._bowl_y_progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete
        self._meal_type = meal_type
        self._character.set_pose("standing.side.happy")

    def stop(self, completed=True):
        """End the eating state.

        Applies meal stats if completed naturally, then delegates to the base
        class which handles pose restoration, the scene callback, and chaining
        to the next behavior.

        Args:
            completed: If True, eating finished naturally.
        """
        if not self._active:
            return

        self._bowl_y_progress = 1.0  # Ensure bowl is at ground level

        if completed:
            self._apply_meal_stats()

        # Clear eating-specific state before super() fires the callback,
        # so the scene can safely remove the bowl on cleanup.
        self._bowl_sprite = None
        self._meal_type = None

        super().stop(completed=completed)

    def _apply_meal_stats(self):
        """Apply stat changes for the current meal type."""
        context = getattr(self._character, "context", None)
        if not context or not self._meal_type:
            return

        stats = self.MEAL_STATS.get(self._meal_type, self.DEFAULT_STATS)
        for stat, delta in stats.items():
            current = getattr(context, stat, 0)
            new_value = max(0, min(100, current + delta))
            setattr(context, stat, new_value)

    def update(self, dt):
        """Update the eating sequence.

        Args:
            dt: Delta time in seconds.
        """
        if not self._active or not self._bowl_sprite:
            return

        phase = self._phase
        self._phase_timer += dt

        if phase == "lowering":
            # Bowl lowers to ground while character stands happy
            progress = self._phase_timer / self.lower_duration
            self._bowl_y_progress = min(progress, 1.0)
            if progress >= 1.0:
                self._phase = "pre_eating"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif phase == "pre_eating":
            # Brief pause before eating
            if self._phase_timer >= self.pause_duration:
                self._phase = "eating"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.eating")

        elif phase == "eating":
            # Actual eating - bowl frames advance
            num_frames = len(self._bowl_sprite["frames"])
            self._bowl_frame += dt * self.eating_speed
            if self._bowl_frame >= num_frames:
                self._phase = "post_eating"
                self._phase_timer = 0.0
                self._character.set_pose("leaning_forward.side.neutral")

        elif phase == "post_eating":
            # Brief pause after eating
            if self._phase_timer >= self.pause_duration:
                self.stop()

    def get_bowl_frame(self):
        """Get the current bowl animation frame index.

        Returns:
            Integer frame index (0 to num_frames-1).
        """
        max_frame = 5  # Default for FOOD_BOWL
        if self._bowl_sprite:
            max_frame = len(self._bowl_sprite["frames"]) - 1
        return min(int(self._bowl_frame), max_frame)

    def get_bowl_position(self, char_x, char_y, mirror=False):
        """Get the world position where the food bowl should be drawn.

        Args:
            char_x: Character's x position.
            char_y: Character's y position.
            mirror: If True, position bowl on right side of character.

        Returns:
            (x, y) tuple for bowl position in world coordinates.
        """
        bowl_offset_x = 30
        bowl_width = self._bowl_sprite["width"] if self._bowl_sprite else 22
        bowl_height = self._bowl_sprite["height"] if self._bowl_sprite else 8

        # Ground level Y (where bowl ends up)
        ground_y = int(char_y) - bowl_height
        # Start Y (above the scene)
        start_y = ground_y - 40

        # Interpolate Y based on lowering progress
        bowl_y = int(start_y + (ground_y - start_y) * self._bowl_y_progress)

        if mirror:
            bowl_x = int(char_x) + bowl_offset_x - bowl_width // 2
        else:
            bowl_x = int(char_x) - bowl_offset_x - bowl_width // 2

        return bowl_x, bowl_y
