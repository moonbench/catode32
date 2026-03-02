"""Playing behavior for energetic fun."""

import math
import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble
from assets.items import YARN_BALL


# Variant configurations
VARIANTS = {
    "toy": {
        "bubble": "exclaim",
        "stats": {"playfulness": -2, "energy": -5, "focus": -1},
    },
    "throw_stick": {
        "bubble": "star",
        "stats": {"playfulness": -2, "energy": -10, "focus": -1},
    },
    "ball": {
        "stats": {"playfulness": -3, "energy": -8, "focus": -2},
        "passes": 4,  # number of half-passes (direction changes) before pouncing
    },
}

# Shared pounce constants (reusable across play variants)
POUNCE_SLIDE_SPEED = 28       # pixels per second during the leap slide
POUNCE_SLIDE_DURATION = 0.9   # seconds the slide lasts

# Ball variant constants
BALL_ROLL_SPEED = 25           # pixels per second
BALL_ROLL_RANGE = 28           # max horizontal offset left/right from cat center
BALL_Y_OFFSET = 8              # pixels above cat's y anchor
BALL_CATCH_DURATION = 1.5      # seconds of celebration after catching


def _compute_eye_frame(ball_offset_x, mirror):
    """Map ball horizontal offset from cat to eye frame index 0-4.

    For the CHAR_EYES_FRONT_LOOKAROUND sprite (non-mirrored):
      Frame 0 = looking right, Frame 2 = center, Frame 4 = looking left.
    When mirror=True the sprite is flipped, so we invert the mapping so the
    rendered gaze direction still follows the ball on screen.

    Args:
        ball_offset_x: Ball x minus cat x (positive = ball to the right).
        mirror: Whether the character sprite is currently mirrored.

    Returns:
        Integer frame index 0-4.
    """
    t = max(-1.0, min(1.0, ball_offset_x / BALL_ROLL_RANGE))
    # Non-mirrored: right(t=1)→frame 0, center→frame 2, left(t=-1)→frame 4
    # Mirrored: sprite is flipped, so invert t so gaze matches screen position
    if mirror:
        t = -t
    return max(0, min(4, round(2 - t * 2)))


class PlayingBehavior(BaseBehavior):
    """Pet plays energetically.

    Default variants (toy / throw_stick) phases:
    1. excited  - Pet reacts with a speech bubble
    2. playing  - Active play animation
    3. tired    - Pet winds down

    Ball variant phases:
    1. watching  - Yarn ball rolls back and forth; cat tracks it with its eyes
    2. pouncing  - Cat leaps toward the stopped ball (pose + forward slide)
    3. catching  - Brief celebration after landing
    """

    NAME = "playing"

    @classmethod
    def can_trigger(cls, context):
        return context.playfulness >= 40

    @classmethod
    def get_priority(cls, context):
        return random.uniform(100 - context.playfulness * 1.5, context.playfulness * 1.5)

    def get_completion_bonus(self, context):
        return dict(VARIANTS[self._variant].get("stats", {}))

    def __init__(self, character):
        super().__init__(character)

        # Default variant timing
        self.excited_duration = 1.0
        self.play_duration = 5.0
        self.tired_duration = 1.0

        # Active variant
        self._variant = "toy"
        self._bubble = None

        # Ball variant state
        self._ball_offset_x = 0.0   # horizontal offset from character.x (world coords)
        self._ball_rotation = 0.0   # current rotation in degrees
        self._ball_direction = 1    # 1 = rolling right, -1 = rolling left
        self._ball_passes_left = 4

        # Shared pounce state
        self._pounce_direction = 1

        # Eye frame override — exposed as a property and read by CharacterEntity.draw()
        self._eye_frame_override = None

    @property
    def eye_frame_override(self):
        return self._eye_frame_override

    # ------------------------------------------------------------------
    # Scene helpers
    # ------------------------------------------------------------------

    def _get_scene_bounds(self):
        context = self._character.context
        x_min = getattr(context, 'scene_x_min', 10) + 15
        x_max = getattr(context, 'scene_x_max', 118) - 15
        return x_min, x_max

    # ------------------------------------------------------------------
    # Start / stop
    # ------------------------------------------------------------------

    def start(self, variant=None, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._variant = variant if variant in VARIANTS else "toy"
        self._eye_frame_override = None

        if self._variant == "ball":
            self._start_ball()
        else:
            config = VARIANTS[self._variant]
            self._bubble = config.get("bubble")
            self._phase = "excited"
            self._character.set_pose("sitting.side.happy")

    def _start_ball(self):
        """Initialise the ball variant state and enter the watching phase."""
        config = VARIANTS["ball"]
        self._ball_passes_left = config.get("passes", 4)
        self._ball_offset_x = BALL_ROLL_RANGE   # start on the right side
        self._ball_rotation = 0.0
        self._ball_direction = -1               # roll left first
        self._eye_frame_override = _compute_eye_frame(
            self._ball_offset_x, self._character.mirror
        )
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        if not self._active:
            return
        self._phase_timer += dt

        if self._variant == "ball":
            self._update_ball(dt)
        else:
            self._update_default(dt)

    # --- Default (toy / throw_stick) ---

    def _update_default(self, dt):
        if self._phase == "excited":
            if self._phase_timer >= self.excited_duration:
                self._phase = "playing"
                self._phase_timer = 0.0
                self._bubble = None
                self._character.set_pose("sitting_silly.side.happy")

        elif self._phase == "playing":
            self._progress = min(1.0, self._phase_timer / self.play_duration)
            if self._phase_timer >= self.play_duration:
                self._phase = "tired"
                self._phase_timer = 0.0
                self._character.set_pose("sitting.side.neutral")

        elif self._phase == "tired":
            if self._phase_timer >= self.tired_duration:
                self.stop(completed=True)

    # --- Ball variant ---

    def _update_ball(self, dt):
        if self._phase == "watching":
            self._update_ball_rolling(dt)
        elif self._phase == "pouncing":
            self._update_pounce(dt)
        elif self._phase == "catching":
            if self._phase_timer >= BALL_CATCH_DURATION:
                self._progress = 1.0
                self.stop(completed=True)

    def _update_ball_rolling(self, dt):
        """Advance the ball and update eye tracking each frame."""
        self._ball_offset_x += self._ball_direction * BALL_ROLL_SPEED * dt

        # Rotate proportional to distance rolled (d / r * 180/pi degrees)
        ball_radius = YARN_BALL["width"] / 2.0
        angle_delta = (self._ball_direction * BALL_ROLL_SPEED * dt
                       / ball_radius * (180.0 / math.pi))
        self._ball_rotation = (self._ball_rotation + angle_delta) % 360.0

        # Update eye tracking
        self._eye_frame_override = _compute_eye_frame(
            self._ball_offset_x, self._character.mirror
        )

        # Check if ball reached a boundary
        if self._ball_direction > 0 and self._ball_offset_x >= BALL_ROLL_RANGE:
            self._ball_offset_x = BALL_ROLL_RANGE
            self._ball_passes_left -= 1
            if self._ball_passes_left <= 0:
                self._begin_pounce()
            else:
                self._ball_direction = -1

        elif self._ball_direction < 0 and self._ball_offset_x <= -BALL_ROLL_RANGE:
            self._ball_offset_x = -BALL_ROLL_RANGE
            self._ball_passes_left -= 1
            if self._ball_passes_left <= 0:
                self._begin_pounce()
            else:
                self._ball_direction = 1

        # Track overall progress (watching counts as 0-90% to leave room for pounce)
        total = VARIANTS["ball"].get("passes", 4)
        done = total - self._ball_passes_left
        self._progress = min(0.9, done / total)

    # ------------------------------------------------------------------
    # Shared pounce helpers — reusable for other play variants
    # ------------------------------------------------------------------

    def _begin_pounce(self):
        """Transition into the pouncing phase (reusable for any play variant).

        Turns the cat to face the ball's side, sets the pounce pose, and
        releases the eye-tracking override so the side-facing pose looks correct.
        """
        self._pounce_direction = 1 if self._ball_offset_x >= 0 else -1
        self._character.mirror = self._pounce_direction > 0
        self._character.set_pose("leaning_forward.side.pounce")
        self._eye_frame_override = None
        self._phase = "pouncing"
        self._phase_timer = 0.0

    def _update_pounce(self, dt):
        """Slide the cat forward during the pounce (reusable for any play variant)."""
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt

        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._phase = "catching"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    # ------------------------------------------------------------------
    # Draw
    # ------------------------------------------------------------------

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active:
            return

        if self._variant == "ball":
            self._draw_ball(renderer, char_x, char_y)
        elif self._bubble and self._phase == "excited":
            progress = min(1.0, self._phase_timer / self.excited_duration)
            draw_bubble(renderer, self._bubble, char_x, char_y, progress, mirror)

    def _draw_ball(self, renderer, char_x, char_y):
        """Draw the rolling yarn ball (visible during watching and pouncing phases)."""
        if self._phase not in ("watching", "pouncing"):
            return

        hw = YARN_BALL["width"] // 2
        hh = YARN_BALL["height"] // 2
        # char_x is already the screen x (world x minus camera offset), so the
        # ball's screen x is simply char_x plus its offset from the cat.
        ball_x = char_x + int(self._ball_offset_x) - hw
        ball_y = char_y - BALL_Y_OFFSET - hh

        renderer.draw_sprite_obj(
            YARN_BALL,
            ball_x,
            ball_y,
            frame=0,
            rotate=int(self._ball_rotation),
        )
