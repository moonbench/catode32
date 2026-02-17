# Reaction definitions and manager for pet interactions

from assets.effects import (
    SPEECH_BUBBLE,
    BUBBLE_HEART,
    BUBBLE_QUESTION,
    BUBBLE_EXCLAIM,
    BUBBLE_NOTE,
    BUBBLE_STAR,
)

# Map bubble names to sprites
BUBBLE_SPRITES = {
    "heart": BUBBLE_HEART,
    "question": BUBBLE_QUESTION,
    "exclaim": BUBBLE_EXCLAIM,
    "note": BUBBLE_NOTE,
    "star": BUBBLE_STAR,
}

# Reaction definitions
# Each reaction specifies: pose to switch to, bubble content, and duration
REACTIONS = {
    "kiss": {
        "pose": "sitting.forward.happy",
        "bubble": "heart",
        "duration": 2.5,
    },
    "pets": {
        "pose": "sitting.forward.content",
        "bubble": "note",
        "duration": 2.0,
    },
    "psst": {
        "pose": "sitting.forward.aloof",
        "bubble": "question",
        "duration": 1.5,
    },
    "snack": {
        "pose": "sitting.forward.happy",
        "bubble": "heart",
        "duration": 2.0,
    },
    "toy": {
        "pose": "sitting.forward.happy",
        "bubble": "exclaim",
        "duration": 2.0,
    },
    # Outside-specific reactions
    "point_bird": {
        "pose": "sitting.side.aloof",
        "bubble": "exclaim",
        "duration": 2.0,
    },
    "throw_stick": {
        "pose": "sitting.side.happy",
        "bubble": "star",
        "duration": 2.5,
    },
}


class ReactionManager:
    """Manages reaction animations for pet interactions."""

    def __init__(self):
        self.timer = 0
        self.duration = 0
        self.bubble = None

    @property
    def active(self):
        """Return True if a reaction is currently playing."""
        return self.bubble is not None

    def trigger(self, reaction_key, character):
        """Trigger a reaction animation.

        Args:
            reaction_key: Key into REACTIONS dict (e.g., "kiss", "pets")
            character: CharacterEntity to set pose on
        """
        reaction = REACTIONS.get(reaction_key)
        if reaction:
            character.set_pose(reaction["pose"])
            self.bubble = reaction["bubble"]
            self.duration = reaction["duration"]
            self.timer = reaction["duration"]

    def update(self, dt, character, default_pose):
        """Update reaction timer and revert pose when done.

        Args:
            dt: Delta time in seconds
            character: CharacterEntity to revert pose on
            default_pose: Pose name to revert to when reaction ends
        """
        if self.timer > 0:
            self.timer -= dt
            if self.timer <= 0:
                character.set_pose(default_pose)
                self.bubble = None

    def draw(self, renderer, char_screen_x, char_y, mirror=False):
        """Draw the reaction bubble if active.

        Args:
            renderer: Renderer to draw with
            char_screen_x: Character's x position on screen (after camera offset)
            char_y: Character's y position
            mirror: If True, position bubble on right side and mirror bubble sprite
        """
        if not self.bubble:
            return

        # Calculate animation progress (0 at start, 1 at end)
        progress = 0
        if self.duration > 0:
            progress = 1 - (self.timer / self.duration)

        # Position bubble relative to character's head
        # Drift upward as progress increases
        drift_amount = 10
        bubble_y = int(char_y) - 45 - int(progress * drift_amount)

        if mirror:
            # Position bubble to the right of the character
            bubble_x = char_screen_x + 15
        else:
            # Position bubble to the left of the character
            bubble_x = char_screen_x - SPEECH_BUBBLE["width"] - 15

        # Draw bubble frame (mirrored if needed so tail points correct direction)
        renderer.draw_sprite_obj(SPEECH_BUBBLE, bubble_x, bubble_y, mirror_h=mirror)

        # Draw content sprite centered inside bubble (inverted)
        content_sprite = BUBBLE_SPRITES.get(self.bubble)
        if content_sprite:
            content_x = bubble_x + 4
            content_y = bubble_y + 2
            renderer.draw_sprite_obj(
                content_sprite, content_x, content_y,
                invert=True, transparent=True, transparent_color=1
            )
