"""Playing behavior for energetic fun."""

import math
import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble
from assets.items import YARN_BALL, MOUSE_TOY, HAND_SCRATCH, BUBBLE_WAND, BUBBLE1, BUBBLE2, BUBBLE_POP


# Variant configurations
VARIANTS = {
    "string": {
        "stats": {"playfulness": -8, "energy": -3, "focus": -1, "fitness": 1.5, "fulfillment": 1.5, "courage": 0.4},
    },
    "feather": {
        "stats": {"playfulness": -6, "energy": -5, "focus": -1, "fitness": 1.5, "fulfillment": 1.5, "courage": 0.4},
    },
    "ball": {
        "stats": {"playfulness": -8, "energy": -4, "focus": -1, "fitness": 1.5, "fulfillment": 1.5, "courage": 0.4},
    },
    "mouse": {
        "stats": {"playfulness": -8, "energy": -4, "focus": -1, "fitness": 1.5, "fulfillment": 1.5, "courage": 0.4},
    },
    "hand": {
        "stats": {"playfulness": -6, "energy": -3, "focus": -1, "fitness": 1.0, "fulfillment": 1.0, "courage": 0.3},
    },
    "laser": {
        "stats": {"playfulness": -6, "energy": -3, "focus": -1, "fitness": 1.5, "fulfillment": 1.5, "courage": 0.4},
    },
    "bubbles": {
        "stats": {"playfulness": -6, "energy": -3, "focus": -1, "fitness": 1.5, "fulfillment": 1.5, "courage": 0.4},
    },
}

# Shared pounce constants (reusable across play variants)
POUNCE_SLIDE_SPEED = 28       # pixels per second during the leap slide
POUNCE_SLIDE_DURATION = 0.9   # seconds the slide lasts

# Session constants
PLAY_MIN_DURATION = 15.0       # seconds before a B-button exit counts as completed

# Shared pounce constants
POUNCE_RECOVER_DURATION = 0.8  # seconds the cat sits happy between pounces
POUNCE_CATCH_DURATION = 1.5    # seconds of celebration after the final pounce
POUNCE_COUNT_MIN = 4           # fewest pounces per session
POUNCE_COUNT_MAX = 8           # most pounces per session

# Shared toy movement bounds — toys travel between screen edges minus this margin
TOY_SCREEN_MARGIN = 8

# Ball variant constants
BALL_PUSH_FORCE = 140          # pixels/s² acceleration when player holds a direction
BALL_MAX_SPEED = 65            # max ball speed in pixels per second
BALL_FRICTION = 0.20           # fraction of speed retained per second (lower = stops faster)
BALL_BOUNCE_DAMPING = 0.55     # fraction of speed kept after hitting a boundary
BALL_ROLL_RANGE = 60           # used only to normalise eye-gaze offset (not movement bounds)
BALL_Y_OFFSET = 8              # pixels above cat's y anchor
MOUSE_Y_OFFSET = 4             # pixels above cat's y anchor (sits lower, on the floor)
HAND_Y_OFFSET = 6              # pixels above cat's y anchor
HAND_ANIM_STEP = 10            # pixels of travel before toggling between open/closed frames
HAND_PUSH_FORCE = 300          # pixels/s² acceleration when player holds a direction
HAND_MAX_SPEED = 90            # max hand speed in pixels per second
HAND_FRICTION = 0.08           # fraction of speed retained per second (stops quickly)
HAND_BOUNCE_DAMPING = 0.20     # fraction of speed kept after hitting a boundary
BALL_POUNCE_DELAY_MIN = 2.5   # minimum seconds before each pounce
BALL_POUNCE_DELAY_MAX = 6.0   # maximum seconds before each pounce

# Laser variant constants
LASER_WOBBLE_AMPLITUDE = 8     # pixels of auto-oscillation around user-controlled position
LASER_WOBBLE_SPEED = 2.5       # radians per second for the wobble sine wave
LASER_USER_SPEED = 50          # pixels per second when player holds left/right
LASER_Y_OFFSET = 1             # pixels above cat's y anchor
LASER_POUNCE_DELAY_MIN = 2.0   # minimum seconds before each pounce
LASER_POUNCE_DELAY_MAX = 5.0   # maximum seconds before each pounce
LASER_DOT_RADIUS = 2           # radius in pixels → 5×5 filled circle
LASER_LINE_TOP_Y = -64         # y coordinate of the off-screen line origin

# String variant constants
STRING_SEGMENTS = 8            # number of rope nodes (anchor + 6 free nodes)
STRING_SEG_LEN_TOP = 20       # rest length of the topmost segment in pixels
STRING_SEG_LEN_BOT = 4        # rest length of the bottommost segment in pixels
STRING_GRAVITY = 120           # pixels per second² downward pull on each node
STRING_DAMPING = 0.45          # velocity damping per second (lower = more sluggish)
STRING_ITERATIONS = 3          # constraint solver passes per frame
STRING_ANCHOR_SPEED = 60       # pixels per second for player-driven anchor movement
STRING_ANCHOR_Y = -70          # screen-y offset from char_y → anchor sits just above screen
STRING_POUNCE_DELAY_MIN = 2.0
STRING_POUNCE_DELAY_MAX = 8.0

# Feather tip constants (used when variant == "feather")
FEATHER_SEGMENTS = 8   # fewer rope nodes so feather hangs shorter
FEATHER_WIDTH = 2      # perpendicular vane width in pixels

# Bubble wand constants
WAND_PUSH_FORCE = 200          # pixels/s² when player holds a direction
WAND_MAX_SPEED = 80            # max wand speed in pixels per second
WAND_FRICTION = 0.15           # fraction of speed retained per second (lower = stops faster)
WAND_BOUNCE_DAMPING = 0.4      # fraction of speed kept after hitting a boundary
WAND_RANGE = 60                # kept only for pounce target sampling — movement uses TOY_SCREEN_MARGIN
WAND_SCREEN_TOP = 8            # y of the wand sprite's top edge (fixed screen position)
WAND_POUNCE_DELAY_MIN = 2.5
WAND_POUNCE_DELAY_MAX = 6.5
BUBBLE_MAX = 16                # max simultaneous bubbles on screen
BUBBLE_SPAWN_DIST = 20         # wand travel (px) between bubble spawns
BUBBLE_SPAWN_SPEED_MIN = 12    # minimum wand speed (px/s) required to spawn bubbles
BUBBLE_FALL_SPEED = 9          # pixels per second downward drift
BUBBLE_DRIFT_SPEED = 2.5       # max horizontal drift speed in pixels per second
BUBBLE_POP_FPS = 7.0           # pop animation playback speed in frames per second
BUBBLE_POP_DURATION = 4 / BUBBLE_POP_FPS   # total pop animation duration in seconds

# Surprised poses the cat cycles through while bubbles are floating
BUBBLE_SURPRISED_POSES = (
    "leaning_forward.side.crazy",
    "playful.forward.wowed",
    "sitting.forward.shocked",
    "standing.side.crazy",
)


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

    Default variants phases:
    1. excited  - Pet reacts with a speech bubble
    2. playing  - Active play animation
    3. tired    - Pet winds down

    Ball variant phases:
    1. watching  - Yarn ball rolls back and forth; cat tracks it with its eyes
    2. pouncing  - Cat leaps toward the stopped ball (pose + forward slide)
    3. catching  - Brief celebration after landing

    Rejection phase (any variant):
    1. rejecting - Cat ignores the toy; holds a disinterested pose, then meanders away
    """

    NAME = "playing"

    # Poses used when the cat is not in the mood to play.
    REJECTION_POSES = (
        "standing.side.neutral_looking_down",
        "sitting.side.looking_down",
        "laying.side.neutral2",
        "laying.side.bored",
        "sitting_silly.side.neutral",
        "standing.side.annoyed",
        "laying.side.annoyed",
        "laying.side.content",
        "sitting_licking.side.licking_leg",
    )

    # Minimum stat values below which rejection becomes increasingly likely.
    _REJECTION_THRESHOLDS = {
        "energy":       35,
        "playfulness":  35,
        "fullness":     25,
        "comfort":      30,
        "focus":        25,
        "curiosity":    25,
        "affection":    30,
        "sociability":  25,
        "courage":      25,
    }

    @classmethod
    def _rejection_chance(cls, context):
        """Return probability (0.0-1.0) that the cat refuses to play.

        Uses a probabilistic OR across all stat deficits: each stat below its
        threshold contributes independently, and multiple low stats compound
        upward without any single one being suppressed.
        """
        complement = 1.0
        for stat, threshold in cls._REJECTION_THRESHOLDS.items():
            val = getattr(context, stat, 100)
            if val < threshold:
                deficit = (threshold - val) / threshold
                complement *= (1.0 - deficit)
        return 1.0 - complement

    def get_completion_bonus(self, context):
        if self._rejecting:
            return {}
        bonus = dict(VARIANTS[self._variant].get("stats", {}))
        bonus = self.apply_location_bonus(context, bonus)
        if self._variant == getattr(context, 'fav_toy', None):
            for stat in ('fitness', 'fulfillment', 'courage', 'loyalty'):
                if stat in bonus:
                    bonus[stat] *= 1.2
        elif self._variant == getattr(context, 'least_fav_toy', None):
            for stat in ('fitness', 'fulfillment', 'courage', 'loyalty'):
                if stat in bonus:
                    bonus[stat] *= 0.85
        return bonus

    def apply_location_bonus(self, context, bonus):
        if context.last_main_scene in ('outside', 'treehouse', 'inside'):
            # Better play locations: reduce energy and playfulness costs by 25%
            for stat in ('energy', 'playfulness'):
                if stat in bonus:
                    bonus[stat] = bonus[stat] * 0.75
            bonus['fitness'] = bonus.get('fitness', 0) + 1.0
        bonus['loyalty'] = bonus.get('loyalty', 0) + 0.5
        return bonus

    def next(self, context):
        if self._rejecting:
            return 'meandering'
        return None

    def __init__(self, character):
        super().__init__(character)
        self._rejecting = False
        self._rejection_timeout = 15.0

        # Default variant timing
        self.excited_duration = random.uniform(1.0, 3.0)
        self.play_duration = random.uniform(5.0, 20.0)
        self.tired_duration = random.uniform(1.0, 10.0)

        # Active variant
        self._variant = "string"
        self._bubble = None

        # Shared screen-space cache — updated every draw() so update() can read it
        self._play_char_x = 64
        self._play_char_y = 40

        # Ball variant state  (absolute screen-space X)
        self._ball_x = 64.0
        self._ball_vel_x = 0.0
        self._ball_rotation = 0.0    # current rotation in degrees (drives frame selection)

        # Mouse variant state  (absolute screen-space X)
        self._mouse_x = 64.0
        self._mouse_vel_x = 0.0
        self._mouse_facing_right = False

        # Hand variant state  (absolute screen-space X)
        self._hand_x = 64.0
        self._hand_vel_x = 0.0
        self._hand_facing_right = False
        self._hand_anim_dist = 0.0   # accumulated travel distance for frame selection

        # Laser variant state  (absolute screen-space X)
        self._laser_x = 64.0         # final dot position (base + wobble)
        self._laser_base_x = 64.0    # player-controlled base position
        self._laser_wobble_phase = 0.0
        self._laser_line_x_top = 64

        # Bubbles variant state  (absolute screen-space X)
        self._wand_x = 64.0
        self._wand_vel_x = 0.0
        self._wand_facing_right = True
        self._wand_spawn_dist = 0.0
        self._bubbles = []
        self._bubble_pose_timer = 0.0
        self._had_bubbles = False

        # String/feather variant state — all positions are screen-space floats
        # _str_px/py: current positions; _str_ox/oy: positions from previous frame
        self._str_px = [0.0] * STRING_SEGMENTS
        self._str_py = [0.0] * STRING_SEGMENTS
        self._str_ox = [0.0] * STRING_SEGMENTS
        self._str_oy = [0.0] * STRING_SEGMENTS
        self._str_anchor_x = 0.0       # screen-x of the fixed anchor node
        self._str_node_count = STRING_SEGMENTS  # active node count (fewer for feather)
        self._str_seg_lens = []        # per-segment rest lengths (tapered top→bottom)
        self._str_needs_init = True

        # Shared pounce state (only one variant active at a time)
        self._pounce_direction = 1
        self._pounce_timer = 0.0       # countdown to next pounce
        self._pounces_total = 3        # total pounces this session
        self._pounces_done = 0         # pounces completed so far

        # Session timer — used to decide if B-button exit earns a reward
        self._session_timer = 0.0

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

    def _on_first_complete(self, milestones):
        if not self._rejecting:
            milestones['played'] = True

    def stop(self, completed=True):
        if completed and not self._rejecting:
            context = self._character.context
            if context:
                for toy in context.inventory.get("toys", []):
                    if toy.get("variant") == self._variant:
                        toy["durability"] = max(0, toy.get("durability", 1) - 1)
                        print(f"[Playing] {self._variant} durability now {toy['durability']}")
                        break
        super().stop(completed)

    def start(self, variant=None, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._variant = variant if variant in VARIANTS else "string"
        self._eye_frame_override = None
        self._session_timer = 0.0

        context = self._character.context
        if context:
            chance = self._rejection_chance(context)
            self._rejecting = random.random() < chance
        else:
            self._rejecting = False

        if self._variant == "ball":
            self._start_ball()
        elif self._variant == "mouse":
            self._start_mouse()
        elif self._variant == "hand":
            self._start_hand()
        elif self._variant == "laser":
            self._start_laser()
        elif self._variant in ("string", "feather"):
            self._start_string()
        elif self._variant == "bubbles":
            self._start_bubbles()
        else:
            config = VARIANTS[self._variant]
            self._bubble = config.get("bubble")
            self._phase = "excited"
            self._character.set_pose("sitting.side.happy")

        if self._rejecting:
            self._character.set_pose(random.choice(self.REJECTION_POSES))
            self._rejection_timeout = random.uniform(10.0, 20.0)

    def _start_laser(self):
        """Initialise the laser variant state and enter the watching phase."""
        self._laser_base_x = float(self._play_char_x)
        self._laser_x = float(self._play_char_x)
        self._laser_wobble_phase = 0.0
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX)
        self._laser_line_x_top = random.randint(20, 108)
        self._eye_frame_override = _compute_eye_frame(0.0, self._character.mirror)
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_ball(self):
        """Initialise the ball variant state and enter the watching phase."""
        self._ball_x = float(self._play_char_x)
        self._ball_vel_x = 0.0
        self._ball_rotation = 0.0
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)
        self._eye_frame_override = _compute_eye_frame(0.0, self._character.mirror)
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_mouse(self):
        """Initialise the mouse toy variant state and enter the watching phase."""
        self._mouse_x = float(self._play_char_x)
        self._mouse_vel_x = 0.0
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)
        self._eye_frame_override = _compute_eye_frame(0.0, self._character.mirror)
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_hand(self):
        """Initialise the hand toy variant state and enter the watching phase."""
        self._hand_x = float(self._play_char_x)
        self._hand_vel_x = 0.0
        self._hand_facing_right = False
        self._hand_anim_dist = 0.0
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)
        self._eye_frame_override = _compute_eye_frame(0.0, self._character.mirror)
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_string(self):
        """Initialise the dangling string and enter the watching phase."""
        self._str_node_count = FEATHER_SEGMENTS if self._variant == "feather" else STRING_SEGMENTS
        n_segs = self._str_node_count - 1
        self._str_seg_lens = [
            STRING_SEG_LEN_TOP + (STRING_SEG_LEN_BOT - STRING_SEG_LEN_TOP) * (i / max(n_segs - 1, 1))
            for i in range(n_segs)
        ]
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX)
        # Nodes are placed in a straight vertical line; we don't know screen pos
        # yet so we use (0, 0) as placeholder — _update_string will snap them on
        # the first frame using the real char_x/char_y passed to draw().
        # We store a flag so the first update initialises positions.
        self._str_needs_init = True
        self._str_anchor_x = 0.0
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_bubbles(self):
        """Initialise the bubble wand state and enter the watching phase."""
        self._wand_x = float(self._play_char_x)
        self._wand_vel_x = 0.0
        self._wand_facing_right = True
        self._wand_spawn_dist = 0.0
        self._bubbles = []
        self._had_bubbles = False
        self._bubble_pose_timer = 0.0
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(WAND_POUNCE_DELAY_MIN, WAND_POUNCE_DELAY_MAX)
        self._phase = "watching"
        self._character.set_pose("sitting.forward.neutral")

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        if not self._active:
            return
        self._phase_timer += dt
        self._session_timer += dt

        if self._rejecting and self._session_timer >= self._rejection_timeout:
            self.stop(completed=True)
            return

        # B button ends the session early; reward given only if enough time has passed
        inp = getattr(self._character.context, 'input', None)
        if inp and inp.was_just_pressed('b'):
            if self._session_timer < PLAY_MIN_DURATION:
                self._progress = 0.0  # no reward for cutting it short
            self.stop(completed=True)
            return

        if self._variant == "ball":
            self._update_ball(dt)
        elif self._variant == "mouse":
            self._update_mouse(dt)
        elif self._variant == "hand":
            self._update_hand(dt)
        elif self._variant == "laser":
            self._update_laser(dt)
        elif self._variant in ("string", "feather"):
            self._update_string(dt)
        elif self._variant == "bubbles":
            self._update_bubbles(dt)
        else:
            self._update_default(dt)

    # --- Default ---

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
                self._character.play_bursts()
                self.stop(completed=True)

    # --- Ball variant ---

    def _update_ball(self, dt):
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._ball_vel_x -= BALL_PUSH_FORCE * dt
            if inp.is_pressed('right'):
                self._ball_vel_x += BALL_PUSH_FORCE * dt
            if self._ball_vel_x > BALL_MAX_SPEED:
                self._ball_vel_x = BALL_MAX_SPEED
            elif self._ball_vel_x < -BALL_MAX_SPEED:
                self._ball_vel_x = -BALL_MAX_SPEED
        self._ball_vel_x *= BALL_FRICTION ** dt
        self._ball_x += self._ball_vel_x * dt
        ball_radius = YARN_BALL["width"] / 2.0
        angle_delta = self._ball_vel_x * dt / ball_radius * (180.0 / math.pi)
        self._ball_rotation = (self._ball_rotation + angle_delta) % 360.0
        lo = TOY_SCREEN_MARGIN
        hi = 128 - TOY_SCREEN_MARGIN
        if self._ball_x >= hi:
            self._ball_x = hi
            self._ball_vel_x = -abs(self._ball_vel_x) * BALL_BOUNCE_DAMPING
        elif self._ball_x <= lo:
            self._ball_x = lo
            self._ball_vel_x = abs(self._ball_vel_x) * BALL_BOUNCE_DAMPING
        self._eye_frame_override = _compute_eye_frame(
            self._ball_x - self._play_char_x, self._character.mirror
        )

        if self._phase == "watching":
            self._update_ball_rolling(dt)
        elif self._phase == "pouncing":
            self._update_ball_pounce(dt)
        elif self._phase == "recovering":
            self._update_recovering(dt, BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX, self._ball_x - self._play_char_x)
        elif self._phase == "catching":
            self._update_catching(dt)

    def _update_ball_rolling(self, dt):
        """Count down to the next pounce."""
        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                self._begin_pounce(self._ball_x - self._play_char_x)
                return
            self._pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

        self._progress = self._pounces_done / self._pounces_total

    def _update_ball_pounce(self, dt):
        """Slide the cat toward the ball; ball stays at its absolute screen position."""
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._ball_vel_x = 0.0
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")


    # --- Mouse variant ---

    def _update_mouse(self, dt):
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._mouse_vel_x -= BALL_PUSH_FORCE * dt
            if inp.is_pressed('right'):
                self._mouse_vel_x += BALL_PUSH_FORCE * dt
            if self._mouse_vel_x > BALL_MAX_SPEED:
                self._mouse_vel_x = BALL_MAX_SPEED
            elif self._mouse_vel_x < -BALL_MAX_SPEED:
                self._mouse_vel_x = -BALL_MAX_SPEED
        self._mouse_vel_x *= BALL_FRICTION ** dt
        self._mouse_x += self._mouse_vel_x * dt
        if abs(self._mouse_vel_x) > 2.0:
            self._mouse_facing_right = self._mouse_vel_x > 0
        lo = TOY_SCREEN_MARGIN
        hi = 128 - TOY_SCREEN_MARGIN
        if self._mouse_x >= hi:
            self._mouse_x = hi
            self._mouse_vel_x = -abs(self._mouse_vel_x) * BALL_BOUNCE_DAMPING
        elif self._mouse_x <= lo:
            self._mouse_x = lo
            self._mouse_vel_x = abs(self._mouse_vel_x) * BALL_BOUNCE_DAMPING
        self._eye_frame_override = _compute_eye_frame(
            self._mouse_x - self._play_char_x, self._character.mirror
        )

        if self._phase == "watching":
            self._update_mouse_rolling(dt)
        elif self._phase == "pouncing":
            self._update_mouse_pounce(dt)
        elif self._phase == "recovering":
            self._update_recovering(dt, BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX, self._mouse_x - self._play_char_x)
        elif self._phase == "catching":
            self._update_catching(dt)

    def _update_mouse_rolling(self, dt):
        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                self._begin_pounce(self._mouse_x - self._play_char_x)
                return
            self._pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)
        self._progress = self._pounces_done / self._pounces_total

    def _update_mouse_pounce(self, dt):
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._mouse_vel_x = 0.0
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    # --- Hand variant ---

    def _update_hand(self, dt):
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._hand_vel_x -= HAND_PUSH_FORCE * dt
            if inp.is_pressed('right'):
                self._hand_vel_x += HAND_PUSH_FORCE * dt
            if self._hand_vel_x > HAND_MAX_SPEED:
                self._hand_vel_x = HAND_MAX_SPEED
            elif self._hand_vel_x < -HAND_MAX_SPEED:
                self._hand_vel_x = -HAND_MAX_SPEED
        self._hand_vel_x *= HAND_FRICTION ** dt
        self._hand_x += self._hand_vel_x * dt
        self._hand_anim_dist += abs(self._hand_vel_x) * dt
        if abs(self._hand_vel_x) > 2.0:
            self._hand_facing_right = self._hand_vel_x > 0
        lo = TOY_SCREEN_MARGIN
        hi = 128 - TOY_SCREEN_MARGIN
        if self._hand_x >= hi:
            self._hand_x = hi
            self._hand_vel_x = -abs(self._hand_vel_x) * HAND_BOUNCE_DAMPING
        elif self._hand_x <= lo:
            self._hand_x = lo
            self._hand_vel_x = abs(self._hand_vel_x) * HAND_BOUNCE_DAMPING
        self._eye_frame_override = _compute_eye_frame(
            self._hand_x - self._play_char_x, self._character.mirror
        )

        if self._phase == "watching":
            self._update_hand_rolling(dt)
        elif self._phase == "pouncing":
            self._update_hand_pounce(dt)
        elif self._phase == "recovering":
            self._update_recovering(dt, BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX, self._hand_x - self._play_char_x)
        elif self._phase == "catching":
            self._update_catching(dt)

    def _update_hand_rolling(self, dt):
        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                self._begin_pounce(self._hand_x - self._play_char_x)
                return
            self._pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)
        self._progress = self._pounces_done / self._pounces_total

    def _update_hand_pounce(self, dt):
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._hand_vel_x = 0.0
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    # --- String / feather variant ---

    def _update_string(self, dt):
        if self._phase == "watching":
            self._update_string_physics(dt)
        elif self._phase == "pouncing":
            self._update_string_pounce(dt)
        elif self._phase == "recovering":
            self._update_string_recovering(dt)
        elif self._phase == "catching":
            self._update_catching(dt)

    def _str_init_positions(self, anchor_sx, anchor_sy, floor_y):
        """Place nodes in a gentle curve then pre-simulate to a settled state."""
        # Spawn with a sine curve so the string looks naturally draped rather than
        # snapping from a rigid straight line.  Amplitude tapers toward the tip.
        n = self._str_node_count
        curve_amp = random.uniform(4.0, 9.0)
        curve_dir = random.choice((-1, 1))
        cumulative_y = 0.0
        for i in range(n):
            t = i / max(n - 1, 1)
            x_off = curve_dir * curve_amp * math.sin(t * math.pi)
            self._str_px[i] = anchor_sx + x_off
            self._str_py[i] = anchor_sy + cumulative_y
            self._str_ox[i] = self._str_px[i]
            self._str_oy[i] = self._str_py[i]
            if i < n - 1:
                cumulative_y += self._str_seg_lens[i]
        self._str_anchor_x = anchor_sx

        # Run several settling steps so the string appears hanging correctly from frame 1
        settle_dt = 1.0 / 12.0
        for _ in range(30):
            damp = STRING_DAMPING ** settle_dt
            for i in range(1, n):
                px, py = self._str_px[i], self._str_py[i]
                ox, oy = self._str_ox[i], self._str_oy[i]
                vx = (px - ox) * damp
                vy = (py - oy) * damp + STRING_GRAVITY * settle_dt * settle_dt
                self._str_ox[i] = px
                self._str_oy[i] = py
                self._str_px[i] = px + vx
                self._str_py[i] = py + vy
            self._str_ox[0] = self._str_px[0]
            self._str_oy[0] = self._str_py[0]
            self._str_px[0] = anchor_sx
            self._str_py[0] = anchor_sy
            for _ in range(STRING_ITERATIONS):
                for i in range(n - 1):
                    ax, ay = self._str_px[i], self._str_py[i]
                    bx, by = self._str_px[i + 1], self._str_py[i + 1]
                    ddx = bx - ax
                    ddy = by - ay
                    dist = math.sqrt(ddx * ddx + ddy * ddy)
                    if dist < 0.001:
                        dist = 0.001
                    correction = (dist - self._str_seg_lens[i]) / dist * 0.5
                    cx_ = ddx * correction
                    cy_ = ddy * correction
                    if i == 0:
                        self._str_px[i + 1] -= cx_ * 2
                        self._str_py[i + 1] -= cy_ * 2
                    else:
                        self._str_px[i] += cx_
                        self._str_py[i] += cy_
                        self._str_px[i + 1] -= cx_
                        self._str_py[i + 1] -= cy_
            # Floor clamp after constraints
            for i in range(1, n):
                if self._str_py[i] > floor_y:
                    self._str_py[i] = floor_y
                    self._str_oy[i] = floor_y

        self._str_needs_init = False

    def _update_string_physics(self, dt):
        """Verlet integration + distance constraints for the dangling string."""
        # We need the current screen position of the cat.  The behavior receives
        # it via draw() but update() runs before draw() each frame.  We cache the
        # last known screen-x from draw() in self._str_last_char_x/y.
        char_x = getattr(self, '_str_last_char_x', None)
        char_y = getattr(self, '_str_last_char_y', None)
        if char_x is None:
            return  # draw() hasn't run yet; skip until positions are known

        if self._str_needs_init:
            anchor_sy = char_y + STRING_ANCHOR_Y
            self._str_init_positions(char_x, anchor_sy, char_y)

        # Move anchor based on player input; bounds are screen edges
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._str_anchor_x -= STRING_ANCHOR_SPEED * dt
            if inp.is_pressed('right'):
                self._str_anchor_x += STRING_ANCHOR_SPEED * dt
        if self._str_anchor_x < TOY_SCREEN_MARGIN:
            self._str_anchor_x = TOY_SCREEN_MARGIN
        elif self._str_anchor_x > 128 - TOY_SCREEN_MARGIN:
            self._str_anchor_x = 128 - TOY_SCREEN_MARGIN

        anchor_sy = char_y + STRING_ANCHOR_Y

        n = self._str_node_count

        # Verlet integrate all free nodes (skip index 0 = anchor)
        damp = STRING_DAMPING ** dt
        for i in range(1, n):
            px, py = self._str_px[i], self._str_py[i]
            ox, oy = self._str_ox[i], self._str_oy[i]
            # velocity ≈ (current - previous), then damp and add gravity
            vx = (px - ox) * damp
            vy = (py - oy) * damp + STRING_GRAVITY * dt * dt
            self._str_ox[i] = px
            self._str_oy[i] = py
            self._str_px[i] = px + vx
            self._str_py[i] = py + vy

        # Fix the anchor node
        self._str_ox[0] = self._str_px[0]
        self._str_oy[0] = self._str_py[0]
        self._str_px[0] = self._str_anchor_x
        self._str_py[0] = anchor_sy

        # Constraint solver: enforce segment lengths
        for _ in range(STRING_ITERATIONS):
            for i in range(n - 1):
                ax, ay = self._str_px[i], self._str_py[i]
                bx, by = self._str_px[i + 1], self._str_py[i + 1]
                dx = bx - ax
                dy = by - ay
                dist = math.sqrt(dx * dx + dy * dy)
                if dist < 0.001:
                    dist = 0.001
                correction = (dist - self._str_seg_lens[i]) / dist * 0.5
                cx_ = dx * correction
                cy_ = dy * correction
                if i == 0:
                    # Anchor is fixed; push only the child
                    self._str_px[i + 1] -= cx_ * 2
                    self._str_py[i + 1] -= cy_ * 2
                else:
                    self._str_px[i] += cx_
                    self._str_py[i] += cy_
                    self._str_px[i + 1] -= cx_
                    self._str_py[i + 1] -= cy_

        # Floor clamp after constraints — zeroing velocity at floor prevents bouncing
        for i in range(1, n):
            if self._str_py[i] > char_y:
                self._str_py[i] = char_y
                self._str_oy[i] = char_y

        # Eye tracking toward the tip
        tip_sx = self._str_px[n - 1]
        tip_offset = tip_sx - char_x
        self._eye_frame_override = _compute_eye_frame(tip_offset, self._character.mirror)

        # Pounce countdown — only when actively watching (not during pounce/recover)
        if self._phase == "watching":
            self._pounce_timer -= dt
            if self._pounce_timer <= 0:
                if not self._rejecting:
                    self._pounces_done += 1
                    self._begin_pounce(self._str_px[n - 1] - char_x)
                    return
                self._pounce_timer = random.uniform(STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX)
            self._progress = self._pounces_done / self._pounces_total

    def _update_string_pounce(self, dt):
        """Slide the cat forward; string physics keep running so it trails naturally."""
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        # Keep physics alive (uses cached char positions from last draw)
        self._update_string_physics(dt)

        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    def _update_string_recovering(self, dt):
        """Brief celebration; string physics keep running."""
        self._update_string_physics(dt)
        if self._phase_timer >= POUNCE_RECOVER_DURATION:
            if self._pounces_done >= self._pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._pounce_timer = random.uniform(
                    STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX
                )
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("playful.forward.wowed")

    # --- Laser variant ---

    def _update_laser(self, dt):
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._laser_base_x -= LASER_USER_SPEED * dt
            if inp.is_pressed('right'):
                self._laser_base_x += LASER_USER_SPEED * dt
        lo = TOY_SCREEN_MARGIN
        hi = 128 - TOY_SCREEN_MARGIN
        if self._laser_base_x < lo:
            self._laser_base_x = lo
        elif self._laser_base_x > hi:
            self._laser_base_x = hi

        self._laser_wobble_phase += LASER_WOBBLE_SPEED * dt
        self._laser_x = self._laser_base_x + LASER_WOBBLE_AMPLITUDE * math.sin(self._laser_wobble_phase)
        self._eye_frame_override = _compute_eye_frame(
            self._laser_x - self._play_char_x, self._character.mirror
        )

        if self._phase == "watching":
            self._update_laser_rolling(dt)
        elif self._phase == "pouncing":
            self._update_laser_pounce(dt)
        elif self._phase == "recovering":
            self._update_recovering(dt, LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX, self._laser_x - self._play_char_x)
        elif self._phase == "catching":
            self._update_catching(dt)

    def _update_laser_rolling(self, dt):
        """Count down to the next pounce."""
        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                self._begin_pounce(self._laser_x - self._play_char_x)
                return
            self._pounce_timer = random.uniform(LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX)

        self._progress = self._pounces_done / self._pounces_total

    def _update_laser_pounce(self, dt):
        """Slide the cat toward the laser; dot stays at its absolute screen position."""
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    # --- Bubbles variant ---

    def _update_bubbles(self, dt):
        # Bubble particles update every frame regardless of phase
        self._update_bubble_particles(dt, self._play_char_y)

        # During catching the wand is gone; just wait for remaining bubbles to clear
        if self._phase == "catching":
            self._update_bubbles_catching(dt)
            return

        # Wand input and physics always run — player can move it at any time
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._wand_vel_x -= WAND_PUSH_FORCE * dt
            if inp.is_pressed('right'):
                self._wand_vel_x += WAND_PUSH_FORCE * dt
            if self._wand_vel_x > WAND_MAX_SPEED:
                self._wand_vel_x = WAND_MAX_SPEED
            elif self._wand_vel_x < -WAND_MAX_SPEED:
                self._wand_vel_x = -WAND_MAX_SPEED
        self._wand_vel_x *= WAND_FRICTION ** dt
        if abs(self._wand_vel_x) > 2.0:
            self._wand_facing_right = self._wand_vel_x > 0
        self._wand_x += self._wand_vel_x * dt
        lo = TOY_SCREEN_MARGIN
        hi = 128 - TOY_SCREEN_MARGIN
        if self._wand_x >= hi:
            self._wand_x = hi
            self._wand_vel_x = -abs(self._wand_vel_x) * WAND_BOUNCE_DAMPING
        elif self._wand_x <= lo:
            self._wand_x = lo
            self._wand_vel_x = abs(self._wand_vel_x) * WAND_BOUNCE_DAMPING

        # Spawn bubbles proportional to wand speed (cap enforced by BUBBLE_MAX)
        speed = abs(self._wand_vel_x)
        if speed >= BUBBLE_SPAWN_SPEED_MIN and len(self._bubbles) < BUBBLE_MAX:
            self._wand_spawn_dist += speed * dt
            while self._wand_spawn_dist >= BUBBLE_SPAWN_DIST and len(self._bubbles) < BUBBLE_MAX:
                self._wand_spawn_dist -= BUBBLE_SPAWN_DIST
                drift = random.uniform(-BUBBLE_DRIFT_SPEED, BUBBLE_DRIFT_SPEED)
                size = random.randint(0, 1)
                wy = float(WAND_SCREEN_TOP + BUBBLE_WAND["height"] // 2)
                self._bubbles.append([size, self._wand_x, wy, drift, -1.0])

        if self._phase == "watching":
            self._update_bubbles_watching(dt)
        elif self._phase == "pouncing":
            self._update_bubbles_pounce(dt)
        elif self._phase == "recovering":
            self._update_bubbles_recovering(dt)

    def _update_bubble_particles(self, dt, ground_y):
        """Advance bubble positions, start pops on ground contact, remove finished ones."""
        i = 0
        while i < len(self._bubbles):
            b = self._bubbles[i]
            if b[4] < 0:  # floating
                b[2] += BUBBLE_FALL_SPEED * dt
                b[1] += b[3] * dt
                if b[2] >= ground_y:
                    b[2] = float(ground_y)
                    b[4] = 0.0  # begin pop animation
            else:  # popping
                b[4] += dt
                if b[4] >= BUBBLE_POP_DURATION:
                    self._bubbles.pop(i)
                    continue
            i += 1

    def _update_bubbles_watching(self, dt):
        # Manage surprised/idle pose transitions
        has_bubbles = bool(self._bubbles)
        if has_bubbles and not self._had_bubbles:
            self._character.set_pose(random.choice(BUBBLE_SURPRISED_POSES))
            self._had_bubbles = True
            self._bubble_pose_timer = random.uniform(1.5, 3.0)
        elif not has_bubbles and self._had_bubbles:
            self._character.set_pose("sitting.forward.neutral")
            self._had_bubbles = False
        elif has_bubbles:
            self._bubble_pose_timer -= dt
            if self._bubble_pose_timer <= 0:
                self._character.set_pose(random.choice(BUBBLE_SURPRISED_POSES))
                self._bubble_pose_timer = random.uniform(1.5, 3.0)

        # Pounce countdown
        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                target = random.uniform(-WAND_RANGE * 0.7, WAND_RANGE * 0.7)
                self._begin_pounce(target)
                return
            self._pounce_timer = random.uniform(WAND_POUNCE_DELAY_MIN, WAND_POUNCE_DELAY_MAX)

        self._progress = self._pounces_done / self._pounces_total

    def _update_bubbles_pounce(self, dt):
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    def _update_bubbles_recovering(self, dt):
        if self._phase_timer >= POUNCE_RECOVER_DURATION:
            if self._pounces_done >= self._pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._pounce_timer = random.uniform(WAND_POUNCE_DELAY_MIN, WAND_POUNCE_DELAY_MAX)
                self._phase = "watching"
                self._phase_timer = 0.0
                if self._bubbles:
                    self._character.set_pose(random.choice(BUBBLE_SURPRISED_POSES))
                    self._had_bubbles = True
                    self._bubble_pose_timer = random.uniform(1.5, 3.0)
                else:
                    self._character.set_pose("sitting.forward.neutral")
                    self._had_bubbles = False

    def _update_bubbles_catching(self, dt):
        """Wait for the celebration timer and all bubbles to clear before stopping."""
        if self._phase_timer >= POUNCE_CATCH_DURATION and not self._bubbles:
            self._progress = 1.0
            self._character.play_bursts()
            self.stop(completed=True)

    # ------------------------------------------------------------------
    # Shared pounce helpers
    # ------------------------------------------------------------------

    def _begin_pounce(self, offset_x):
        """Face the target, set pounce pose, and enter the pouncing phase."""
        self._pounce_direction = 1 if offset_x >= 0 else -1
        self._character.mirror = self._pounce_direction > 0
        self._character.set_pose("leaning_forward.side.pounce")
        self._eye_frame_override = None
        self._phase = "pouncing"
        self._phase_timer = 0.0

    def _update_recovering(self, dt, delay_min, delay_max, offset_x):
        """Brief celebration; then return to watching or end the session."""
        if self._phase_timer >= POUNCE_RECOVER_DURATION:
            if self._pounces_done >= self._pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._pounce_timer = random.uniform(delay_min, delay_max)
                self._eye_frame_override = _compute_eye_frame(offset_x, self._character.mirror)
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("playful.forward.wowed")

    def _update_catching(self, dt):
        """Final celebration phase shared by all interactive variants."""
        if self._phase_timer >= POUNCE_CATCH_DURATION:
            self._progress = 1.0
            self._character.play_bursts()
            self.stop(completed=True)

    # ------------------------------------------------------------------
    # Draw
    # ------------------------------------------------------------------

    def draw(self, renderer, char_x, char_y, mirror=False):
        if not self._active:
            return

        if self._variant == "ball":
            self._draw_ball(renderer, char_x, char_y)
        elif self._variant == "mouse":
            self._draw_mouse(renderer, char_x, char_y)
        elif self._variant == "hand":
            self._draw_hand(renderer, char_x, char_y)
        elif self._variant == "laser":
            self._draw_laser(renderer, char_x, char_y)
        elif self._variant in ("string", "feather"):
            self._draw_string(renderer, char_x, char_y)
        elif self._variant == "bubbles":
            self._draw_bubbles(renderer, char_x, char_y)
        elif self._bubble and self._phase == "excited":
            progress = min(1.0, self._phase_timer / self.excited_duration)
            draw_bubble(renderer, self._bubble, char_x, char_y, progress, mirror)

    def _draw_ball(self, renderer, char_x, char_y):
        """Draw the rolling yarn ball (visible in all active phases)."""
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering"):
            return

        hw = YARN_BALL["width"] // 2
        hh = YARN_BALL["height"] // 2
        ball_x = int(self._ball_x) - hw
        ball_y = char_y - BALL_Y_OFFSET - hh

        # Map rotation to the nearest pre-baked 90° frame (0°/90°/180°/270°)
        frame = int(self._ball_rotation // 90) % 4

        renderer.draw_sprite_obj(YARN_BALL, ball_x, ball_y, frame=frame)

    def _draw_mouse(self, renderer, char_x, char_y):
        """Draw the mouse toy (no rotation animation)."""
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering"):
            return
        hw = MOUSE_TOY["width"] // 2
        hh = MOUSE_TOY["height"] // 2
        mouse_x = int(self._mouse_x) - hw
        mouse_y = char_y - MOUSE_Y_OFFSET - hh
        renderer.draw_sprite_obj(MOUSE_TOY, mouse_x, mouse_y, mirror_h=self._mouse_facing_right)

    def _draw_hand(self, renderer, char_x, char_y):
        """Draw the hand toy, alternating open/closed frames based on distance traveled."""
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering"):
            return
        hw = HAND_SCRATCH["width"] // 2
        hh = HAND_SCRATCH["height"] // 2
        hand_x = int(self._hand_x) - hw
        hand_y = char_y - HAND_Y_OFFSET - hh
        frame = int(self._hand_anim_dist / HAND_ANIM_STEP) % 2
        renderer.draw_sprite_obj(HAND_SCRATCH, hand_x, hand_y, frame=frame, mirror_h=self._hand_facing_right)

    def _draw_laser(self, renderer, char_x, char_y):
        """Draw the laser dot and beam line (always together while visible)."""
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering"):
            return

        dot_x = int(self._laser_x)
        dot_y = char_y - LASER_Y_OFFSET

        # Draw the beam line in all visible phases
        renderer.draw_line(
            self._laser_line_x_top, LASER_LINE_TOP_Y,
            dot_x, dot_y,
        )

        # Draw the 5×5 laser dot as a filled circle
        renderer.draw_circle(dot_x, dot_y, LASER_DOT_RADIUS, filled=True)

    def _draw_string(self, renderer, char_x, char_y):
        """Draw the dangling string as connected line segments.

        Also caches the current screen position so _update_string_physics() can
        use real coordinates without needing them passed through update().
        """
        if self._phase not in ("watching", "pouncing", "recovering"):
            return

        # Cache positions for the physics update and shared eye tracking
        self._play_char_x = char_x
        self._play_char_y = char_y
        self._str_last_char_x = char_x
        self._str_last_char_y = char_y

        if self._str_needs_init:
            return  # positions not ready yet

        n = self._str_node_count

        if self._variant == "feather":
            # Draw all segments except the last, then draw the feather which IS the last segment
            for i in range(n - 2):
                renderer.draw_line(
                    int(self._str_px[i]), int(self._str_py[i]),
                    int(self._str_px[i + 1]), int(self._str_py[i + 1]),
                )
            self._draw_feather_tip(
                renderer,
                self._str_px[n - 2], self._str_py[n - 2],
                self._str_px[n - 1], self._str_py[n - 1],
            )
        else:
            # Draw all segments plus a small dot at the tip
            for i in range(n - 1):
                renderer.draw_line(
                    int(self._str_px[i]), int(self._str_py[i]),
                    int(self._str_px[i + 1]), int(self._str_py[i + 1]),
                )
            tx = int(self._str_px[n - 1])
            ty = int(self._str_py[n - 1])
            renderer.draw_circle(tx, ty, 1, filled=True)

    def _draw_feather_tip(self, renderer, base_x, base_y, tip_x, tip_y):
        """Draw a feather vane at tip_x/y in the quill direction.

        Exact geometry from reference (45° case, f=quill, n=perpendicular):
          Outline:
            A(0,0) → B(L,0)           spine
            C(1.036L, 0.33W) → D(0.964L, W)  tip cap
            D(0.964L, W) → E(0.32L, W)       far edge (parallel to spine)
            E(0.32L, W) → F(0.21L, 0)        base cap
          Fill (black, drawn first):
            (0.32L,0.33W) → (0.964L,0.33W)   at 1/3 width
            (0.36L,0.67W) → (0.93L, 0.67W)   at 2/3 width
        """
        ddx = tip_x - base_x
        ddy = tip_y - base_y
        mag = math.sqrt(ddx * ddx + ddy * ddy)
        if mag < 0.001:
            return
        fx = ddx / mag
        fy = ddy / mag
        nx = fy        # right-hand perpendicular
        ny = -fx

        # L = actual segment length.
        # All fractions derived directly from the reference pixel coordinates.
        L = mag
        W = float(FEATHER_WIDTH)

        def p(fl, wl):
            return int(base_x + fx*fl + nx*wl), int(base_y + fy*fl + ny*wl)

        # Exact fractions from reference (spine = 14√2, W = 3/√2):
        # A=(0,0)  B=(14,14)  C=(15,14)  D=(15,12)  E=(6,3)  F=(3,3)
        # forward fractions of L=14√2:  B=1.0, C=1.036, D=0.964, E=0.321, F=0.214
        # perp fractions of W=3/√2:     C=0.33, D=1.0, E=1.0, F=0.0
        A = p(0,        0)
        B = p(L,        0)
        C = p(L*1.036,  W*0.33)
        D = p(L*0.964,  W)
        E = p(L*0.321,  W)
        F = p(L*0.214,  0)

        # Fill 1: (5,4)→(14,13) = p(L*0.321, W*0.33) → p(L*0.964, W*0.33)
        # Fill 2: (6,4)→(14,12) = p(L*0.357, W*0.67) → p(L*0.929, W*0.67)
        # Fill 3: F→E covers the base-cap triangle (E and F share the same w-strip)
        G = p(L*0.321, W*0.33);  H = p(L*0.964, W*0.33)
        I = p(L*0.357, W*0.67);  J = p(L*0.929, W*0.67)
        renderer.draw_line(F[0], F[1], G[0], G[1], 0)   # base triangle diagonal
        renderer.draw_line(G[0], G[1], H[0], H[1], 0)   # fill at 1/3 width
        renderer.draw_line(I[0], I[1], J[0], J[1], 0)   # fill at 2/3 width

        # 4 white outline lines
        renderer.draw_line(A[0], A[1], B[0], B[1])   # spine
        renderer.draw_line(C[0], C[1], D[0], D[1])   # tip cap
        renderer.draw_line(D[0], D[1], E[0], E[1])   # far edge
        renderer.draw_line(E[0], E[1], F[0], F[1])   # base cap

    def _draw_bubbles(self, renderer, char_x, char_y):
        """Draw the wand and all floating/popping bubbles; cache char pos for update()."""
        self._play_char_x = char_x
        self._play_char_y = char_y

        if self._phase not in ("watching", "pouncing", "recovering", "catching"):
            return

        # Wand — visible in all non-catching phases
        if self._phase != "catching":
            hw = BUBBLE_WAND["width"] // 2
            wx = int(self._wand_x) - hw
            renderer.draw_sprite_obj(BUBBLE_WAND, wx, WAND_SCREEN_TOP, mirror_h=not self._wand_facing_right)

        # Bubbles — floating outlines and pop bursts
        pw = BUBBLE_POP["width"] // 2
        ph = BUBBLE_POP["height"] // 2
        for b in self._bubbles:
            bx = int(b[1])
            by = int(b[2])
            if b[4] < 0:  # floating
                if b[0] == 0:
                    renderer.draw_sprite_obj(BUBBLE1, bx - 3, by - 3)
                else:
                    renderer.draw_sprite_obj(BUBBLE2, bx - 4, by - 4)
            else:  # popping
                frame = min(3, int(b[4] * BUBBLE_POP_FPS))
                renderer.draw_sprite_obj(BUBBLE_POP, bx - pw, by - ph, frame=frame)
