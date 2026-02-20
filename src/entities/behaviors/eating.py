"""Eating behavior for the character entity."""


class EatingBehavior:
    """Manages the eating animation sequence for a character.

    Phases:
    1. lowering - Bowl lowers to ground, character stands happy
    2. pre_eating - Brief pause, character leans forward neutral
    3. eating - Character eats, bowl empties through frames
    4. post_eating - Brief pause, character leans forward neutral
    5. Complete - Return to original pose
    """

    NAME = "eating"

    # Poses used during the eating sequence
    POSES = {
        "standing.side.happy",
        "leaning_forward.side.neutral",
        "leaning_forward.side.eating",
    }

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
        self._character = character

        # State
        self._active = False
        self._phase = None  # "lowering", "pre_eating", "eating", "post_eating"
        self._phase_timer = 0.0
        self._bowl_sprite = None
        self._bowl_frame = 0.0
        self._bowl_y_progress = 0.0  # 0 = start (above), 1 = ground level
        self._pose_before = None
        self._on_complete = None
        self._meal_type = None

        # Timing configuration
        self.eating_speed = 0.4  # Bowl frames per second during eating phase
        self.lower_duration = 0.5  # Time for bowl to lower
        self.pause_duration = 1.5  # Pre/post eating pause

    @property
    def active(self):
        """Return True if currently in an eating sequence."""
        return self._active

    @property
    def progress(self):
        """Return eating progress from 0.0 to 1.0."""
        if not self._active or not self._bowl_sprite:
            return 0.0
        num_frames = len(self._bowl_sprite["frames"])
        return min(1.0, self._bowl_frame / num_frames)

    @property
    def phase(self):
        """Return the current phase name."""
        return self._phase

    def mark_triggered(self, current_time):
        """Mark this behavior as triggered (for manager compatibility)."""
        pass  # Eating doesn't use cooldown tracking

    def apply_stat_effects(self, context, dt):
        """Apply per-frame stat changes (for manager compatibility).

        Eating doesn't apply gradual stat effects - stats are applied
        once on completion via _apply_meal_stats().
        """
        pass

    def start(self, bowl_sprite, meal_type, on_complete=None):
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
        self._bowl_sprite = bowl_sprite
        self._bowl_frame = 0.0
        self._bowl_y_progress = 0.0
        self._pose_before = self._character.pose_name
        self._on_complete = on_complete
        self._meal_type = meal_type
        self._character.set_pose("standing.side.happy")

    def stop(self, completed=True):
        """End the eating state.

        Args:
            completed: If True, eating finished naturally. If False, it was
                       interrupted (e.g., by another action changing the pose).
        """
        if not self._active:
            return

        self._active = False
        self._phase = None
        self._phase_timer = 0.0
        self._bowl_y_progress = 1.0  # Ensure bowl is at ground level

        # Apply stat changes if eating completed naturally
        if completed:
            self._apply_meal_stats()

        # Only restore previous pose if eating completed naturally
        if self._pose_before and completed:
            self._character.set_pose(self._pose_before)

        self._pose_before = None
        self._bowl_sprite = None
        self._meal_type = None

        callback = self._on_complete
        self._on_complete = None
        if callback:
            callback()

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
