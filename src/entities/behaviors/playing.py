"""Playing behavior for energetic fun."""

import math
import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble
from assets.items import YARN_BALL


# Variant configurations
VARIANTS = {
    "string": {
        "stats": {"playfulness": -8, "energy": -3, "focus": -1},
    },
    "feather": {
        "stats": {"playfulness": -6, "energy": -5, "focus": -1},
    },
    "ball": {
        "stats": {"playfulness": -8, "energy": -4, "focus": -1},
    },
    "laser": {
        "stats": {"playfulness": -6, "energy": -3, "focus": -1},
    },
}

# Shared pounce constants (reusable across play variants)
POUNCE_SLIDE_SPEED = 28       # pixels per second during the leap slide
POUNCE_SLIDE_DURATION = 0.9   # seconds the slide lasts

# Session constants
PLAY_MIN_DURATION = 15.0       # seconds before a B-button exit counts as completed

# Ball variant constants
BALL_PUSH_FORCE = 140          # pixels/s² acceleration when player holds a direction
BALL_MAX_SPEED = 65            # max ball speed in pixels per second
BALL_FRICTION = 0.20           # fraction of speed retained per second (lower = stops faster)
BALL_BOUNCE_DAMPING = 0.55     # fraction of speed kept after hitting a boundary
BALL_ROLL_RANGE = 60           # max horizontal offset left/right from cat center
BALL_Y_OFFSET = 8              # pixels above cat's y anchor
BALL_CATCH_DURATION = 1.5      # seconds of celebration after the final pounce
BALL_RECOVER_DURATION = 0.8   # seconds the cat sits happy between pounces
BALL_POUNCE_DELAY_MIN = 2.5   # minimum seconds before each pounce
BALL_POUNCE_DELAY_MAX = 6.0   # maximum seconds before each pounce
BALL_POUNCE_COUNT_MIN = 4     # fewest pounces per session
BALL_POUNCE_COUNT_MAX = 8     # most pounces per session

# Laser variant constants
LASER_WOBBLE_AMPLITUDE = 8     # pixels of auto-oscillation around user-controlled position
LASER_WOBBLE_SPEED = 2.5       # radians per second for the wobble sine wave
LASER_USER_SPEED = 50          # pixels per second when player holds left/right
LASER_USER_RANGE = 60          # max offset from cat center for player-controlled position
LASER_Y_OFFSET = 1             # pixels above cat's y anchor
LASER_CATCH_DURATION = 1.5     # seconds of celebration after the final pounce
LASER_RECOVER_DURATION = 0.8   # seconds the cat sits happy between pounces
LASER_POUNCE_DELAY_MIN = 2.0   # minimum seconds before each pounce
LASER_POUNCE_DELAY_MAX = 5.0   # maximum seconds before each pounce
LASER_POUNCE_COUNT_MIN = 4     # fewest pounces per session
LASER_POUNCE_COUNT_MAX = 8     # most pounces per session
LASER_DOT_RADIUS = 2           # radius in pixels → 5×5 filled circle
LASER_LINE_TOP_Y = -64         # y coordinate of the off-screen line origin

# String variant constants
STRING_SEGMENTS = 8            # number of rope nodes (anchor + 6 free nodes)
STRING_SEG_LEN = 12           # rest length of each segment in pixels (7×11=77px reach)
STRING_GRAVITY = 120           # pixels per second² downward pull on each node
STRING_DAMPING = 0.45          # velocity damping per second (lower = more sluggish)
STRING_ITERATIONS = 3          # constraint solver passes per frame
STRING_ANCHOR_SPEED = 60       # pixels per second for player-driven anchor movement
STRING_ANCHOR_RANGE = 60       # max horizontal offset from cat center for the anchor
STRING_ANCHOR_Y = -70          # screen-y offset from char_y → anchor sits just above screen
STRING_POUNCE_DELAY_MIN = 2.0
STRING_POUNCE_DELAY_MAX = 8.0
STRING_POUNCE_COUNT_MIN = 4
STRING_POUNCE_COUNT_MAX = 8
STRING_RECOVER_DURATION = 0.8
STRING_CATCH_DURATION = 1.5

# Feather tip constants (used when variant == "feather")
FEATHER_SEGMENTS = 8   # fewer rope nodes so feather hangs shorter
FEATHER_WIDTH = 2      # perpendicular vane width in pixels


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
    """

    NAME = "playing"

    def get_completion_bonus(self, context):
        bonus = dict(VARIANTS[self._variant].get("stats", {}))
        return self.apply_location_bonus(context, bonus)

    def apply_location_bonus(self, context, bonus):
        if context.last_main_scene in ('outside', 'treehouse', 'inside'):
            # Better play locations: reduce energy and playfulness costs by 25%
            for stat in ('energy', 'playfulness'):
                if stat in bonus:
                    bonus[stat] = bonus[stat] * 0.75
            bonus['fitness'] = bonus.get('fitness', 0) + 0.01
        return bonus

    def __init__(self, character):
        super().__init__(character)

        # Default variant timing
        self.excited_duration = random.uniform(1.0, 3.0)
        self.play_duration = random.uniform(5.0, 20.0)
        self.tired_duration = random.uniform(1.0, 10.0)

        # Active variant
        self._variant = "string"
        self._bubble = None

        # Ball variant state
        self._ball_offset_x = 0.0    # horizontal offset from character.x
        self._ball_vel_x = 0.0       # rolling velocity in pixels per second
        self._ball_rotation = 0.0    # current rotation in degrees (drives frame selection)
        self._ball_pounce_timer = 0.0
        self._ball_pounces_total = 3
        self._ball_pounces_done = 0

        # Laser variant state
        self._laser_offset_x = 0.0    # current offset from character.x (wobble + user)
        self._laser_user_x = 0.0      # player-controlled base position
        self._laser_wobble_phase = 0.0 # phase of the auto-oscillation sine wave
        self._laser_pounce_timer = 0.0 # countdown to next pounce (set each watching phase)
        self._laser_pounces_total = 3  # total pounces this session (randomised at start)
        self._laser_pounces_done = 0   # pounces completed so far
        self._laser_line_x_top = 64    # fixed screen-space x for the off-screen line end

        # String/feather variant state — all positions are screen-space floats
        # _str_px/py: current positions; _str_ox/oy: positions from previous frame
        self._str_px = [0.0] * STRING_SEGMENTS
        self._str_py = [0.0] * STRING_SEGMENTS
        self._str_ox = [0.0] * STRING_SEGMENTS
        self._str_oy = [0.0] * STRING_SEGMENTS
        self._str_anchor_x = 0.0       # screen-x of the fixed anchor node
        self._str_node_count = STRING_SEGMENTS  # active node count (fewer for feather)
        self._str_pounce_timer = 0.0
        self._str_pounces_total = 3
        self._str_pounces_done = 0

        # Shared pounce state
        self._pounce_direction = 1

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

    def start(self, variant=None, on_complete=None):
        if self._active:
            return
        super().start(on_complete)
        self._variant = variant if variant in VARIANTS else "string"
        self._eye_frame_override = None
        self._session_timer = 0.0

        if self._variant == "ball":
            self._start_ball()
        elif self._variant == "laser":
            self._start_laser()
        elif self._variant in ("string", "feather"):
            self._start_string()
        else:
            config = VARIANTS[self._variant]
            self._bubble = config.get("bubble")
            self._phase = "excited"
            self._character.set_pose("sitting.side.happy")

    def _start_laser(self):
        """Initialise the laser variant state and enter the watching phase."""
        self._laser_user_x = 0.0
        self._laser_wobble_phase = 0.0
        self._laser_offset_x = 0.0
        self._laser_pounces_total = random.randint(LASER_POUNCE_COUNT_MIN, LASER_POUNCE_COUNT_MAX)
        self._laser_pounces_done = 0
        self._laser_pounce_timer = random.uniform(LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX)
        self._laser_line_x_top = random.randint(20, 108)
        self._eye_frame_override = _compute_eye_frame(
            self._laser_offset_x, self._character.mirror
        )
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_ball(self):
        """Initialise the ball variant state and enter the watching phase."""
        self._ball_offset_x = 0.0
        self._ball_vel_x = 0.0
        self._ball_rotation = 0.0
        self._ball_pounces_total = random.randint(BALL_POUNCE_COUNT_MIN, BALL_POUNCE_COUNT_MAX)
        self._ball_pounces_done = 0
        self._ball_pounce_timer = random.uniform(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)
        self._eye_frame_override = _compute_eye_frame(
            self._ball_offset_x, self._character.mirror
        )
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_string(self):
        """Initialise the dangling string and enter the watching phase."""
        self._str_node_count = FEATHER_SEGMENTS if self._variant == "feather" else STRING_SEGMENTS
        self._str_pounces_total = random.randint(STRING_POUNCE_COUNT_MIN, STRING_POUNCE_COUNT_MAX)
        self._str_pounces_done = 0
        self._str_pounce_timer = random.uniform(STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX)
        # Nodes are placed in a straight vertical line; we don't know screen pos
        # yet so we use (0, 0) as placeholder — _update_string will snap them on
        # the first frame using the real char_x/char_y passed to draw().
        # We store a flag so the first update initialises positions.
        self._str_needs_init = True
        self._str_anchor_x = 0.0
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        if not self._active:
            return
        self._phase_timer += dt
        self._session_timer += dt

        # B button ends the session early; reward given only if enough time has passed
        inp = getattr(self._character.context, 'input', None)
        if inp and inp.was_just_pressed('b'):
            if self._session_timer < PLAY_MIN_DURATION:
                self._progress = 0.0  # no reward for cutting it short
            self.stop(completed=True)
            return

        if self._variant == "ball":
            self._update_ball(dt)
        elif self._variant == "laser":
            self._update_laser(dt)
        elif self._variant in ("string", "feather"):
            self._update_string(dt)
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
        self._ball_offset_x += self._ball_vel_x * dt
        ball_radius = YARN_BALL["width"] / 2.0
        angle_delta = self._ball_vel_x * dt / ball_radius * (180.0 / math.pi)
        self._ball_rotation = (self._ball_rotation + angle_delta) % 360.0
        if self._ball_offset_x >= BALL_ROLL_RANGE:
            self._ball_offset_x = BALL_ROLL_RANGE
            self._ball_vel_x = -abs(self._ball_vel_x) * BALL_BOUNCE_DAMPING
        elif self._ball_offset_x <= -BALL_ROLL_RANGE:
            self._ball_offset_x = -BALL_ROLL_RANGE
            self._ball_vel_x = abs(self._ball_vel_x) * BALL_BOUNCE_DAMPING
        self._eye_frame_override = _compute_eye_frame(
            self._ball_offset_x, self._character.mirror
        )

        if self._phase == "watching":
            self._update_ball_rolling(dt)
        elif self._phase == "pouncing":
            self._update_ball_pounce(dt)
        elif self._phase == "recovering":
            self._update_ball_recovering(dt)
        elif self._phase == "catching":
            if self._phase_timer >= BALL_CATCH_DURATION:
                self._progress = 1.0
                self._character.play_bursts()
                self.stop(completed=True)

    def _update_ball_rolling(self, dt):
        """Count down to the next pounce."""
        self._ball_pounce_timer -= dt
        if self._ball_pounce_timer <= 0:
            self._begin_ball_pounce()
            return

        self._progress = self._ball_pounces_done / self._ball_pounces_total

    def _begin_ball_pounce(self):
        """Start a pounce toward the current ball position."""
        self._ball_pounces_done += 1
        direction = 1 if self._ball_offset_x >= 0 else -1
        self._pounce_direction = direction
        self._character.mirror = direction > 0
        self._character.set_pose("leaning_forward.side.pounce")
        self._eye_frame_override = None
        self._phase = "pouncing"
        self._phase_timer = 0.0

    def _update_ball_pounce(self, dt):
        """Slide the cat toward the ball; keep the ball fixed on screen."""
        slide = self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        self._character.x += slide
        self._ball_offset_x -= slide  # keep ball at same screen position

        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._ball_vel_x = 0.0
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    def _update_ball_recovering(self, dt):
        """Brief celebration pose after each pounce."""
        if self._phase_timer >= BALL_RECOVER_DURATION:
            if self._ball_pounces_done >= self._ball_pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._ball_pounce_timer = random.uniform(
                    BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX
                )
                self._eye_frame_override = _compute_eye_frame(
                    self._ball_offset_x, self._character.mirror
                )
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("playful.forward.wowed")

    # --- String / feather variant ---

    def _update_string(self, dt):
        if self._phase == "watching":
            self._update_string_physics(dt)
        elif self._phase == "pouncing":
            self._update_string_pounce(dt)
        elif self._phase == "recovering":
            self._update_string_recovering(dt)
        elif self._phase == "catching":
            if self._phase_timer >= STRING_CATCH_DURATION:
                self._progress = 1.0
                self._character.play_bursts()
                self.stop(completed=True)

    def _str_init_positions(self, anchor_sx, anchor_sy, floor_y):
        """Place nodes in a gentle curve then pre-simulate to a settled state."""
        # Spawn with a sine curve so the string looks naturally draped rather than
        # snapping from a rigid straight line.  Amplitude tapers toward the tip.
        n = self._str_node_count
        curve_amp = random.uniform(4.0, 9.0)
        curve_dir = random.choice((-1, 1))
        for i in range(n):
            t = i / max(n - 1, 1)
            x_off = curve_dir * curve_amp * math.sin(t * math.pi)
            self._str_px[i] = anchor_sx + x_off
            self._str_py[i] = anchor_sy + i * STRING_SEG_LEN
            self._str_ox[i] = self._str_px[i]
            self._str_oy[i] = self._str_py[i]
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
                    correction = (dist - STRING_SEG_LEN) / dist * 0.5
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

        if getattr(self, '_str_needs_init', True):
            anchor_sy = char_y + STRING_ANCHOR_Y
            self._str_init_positions(char_x, anchor_sy, char_y)

        # Move anchor based on player input
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._str_anchor_x -= STRING_ANCHOR_SPEED * dt
            if inp.is_pressed('right'):
                self._str_anchor_x += STRING_ANCHOR_SPEED * dt
        lo = char_x - STRING_ANCHOR_RANGE
        hi = char_x + STRING_ANCHOR_RANGE
        if self._str_anchor_x < lo:
            self._str_anchor_x = lo
        elif self._str_anchor_x > hi:
            self._str_anchor_x = hi

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
                correction = (dist - STRING_SEG_LEN) / dist * 0.5
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
            self._str_pounce_timer -= dt
            if self._str_pounce_timer <= 0:
                self._begin_string_pounce(char_x)
                return
            self._progress = self._str_pounces_done / self._str_pounces_total

    def _begin_string_pounce(self, char_x):
        """Lunge toward the tip of the string."""
        self._str_pounces_done += 1
        n = self._str_node_count
        tip_sx = self._str_px[n - 1]
        direction = 1 if tip_sx >= char_x else -1
        self._pounce_direction = direction
        self._character.mirror = direction > 0
        self._character.set_pose("leaning_forward.side.pounce")
        self._eye_frame_override = None
        self._phase = "pouncing"
        self._phase_timer = 0.0

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
        if self._phase_timer >= STRING_RECOVER_DURATION:
            if self._str_pounces_done >= self._str_pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._str_pounce_timer = random.uniform(
                    STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX
                )
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("playful.forward.wowed")

    # --- Laser variant ---

    def _update_laser(self, dt):
        # Input and dot position always update so the player can move the laser
        # at any time. Skip during pouncing: slide compensation owns _laser_offset_x
        # for that phase.
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._laser_user_x -= LASER_USER_SPEED * dt
            if inp.is_pressed('right'):
                self._laser_user_x += LASER_USER_SPEED * dt
            self._laser_user_x = max(-LASER_USER_RANGE, min(LASER_USER_RANGE, self._laser_user_x))

        self._laser_wobble_phase += LASER_WOBBLE_SPEED * dt
        self._laser_offset_x = (self._laser_user_x
                                 + LASER_WOBBLE_AMPLITUDE * math.sin(self._laser_wobble_phase))
        self._eye_frame_override = _compute_eye_frame(
            self._laser_offset_x, self._character.mirror
        )

        if self._phase == "watching":
            self._update_laser_rolling(dt)
        elif self._phase == "pouncing":
            self._update_laser_pounce(dt)
        elif self._phase == "recovering":
            self._update_laser_recovering(dt)
        elif self._phase == "catching":
            if self._phase_timer >= LASER_CATCH_DURATION:
                self._progress = 1.0
                self._character.play_bursts()
                self.stop(completed=True)

    def _update_laser_rolling(self, dt):
        """Count down to the next pounce."""
        # Count down to pounce
        self._laser_pounce_timer -= dt
        if self._laser_pounce_timer <= 0:
            self._begin_laser_pounce()
            return

        self._progress = self._laser_pounces_done / self._laser_pounces_total

    def _begin_laser_pounce(self):
        """Start a pounce toward the current laser position."""
        self._laser_pounces_done += 1
        direction = 1 if self._laser_offset_x >= 0 else -1
        self._pounce_direction = direction
        self._character.mirror = direction > 0
        self._character.set_pose("leaning_forward.side.pounce")
        self._eye_frame_override = None
        self._phase = "pouncing"
        self._phase_timer = 0.0

    def _update_laser_pounce(self, dt):
        """Slide the cat toward the laser; keep the laser dot fixed on screen."""
        slide = self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        self._character.x += slide
        # Compensate offset so the dot stays at the same screen position
        self._laser_offset_x -= slide

        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    def _update_laser_recovering(self, dt):
        """Brief celebration pose after each pounce."""
        if self._phase_timer >= LASER_RECOVER_DURATION:
            if self._laser_pounces_done >= self._laser_pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._laser_pounce_timer = random.uniform(
                    LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX
                )
                self._eye_frame_override = _compute_eye_frame(
                    self._laser_offset_x, self._character.mirror
                )
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("playful.forward.wowed")

    # ------------------------------------------------------------------
    # Shared pounce helpers — reusable for other play variants
    # ------------------------------------------------------------------

    def _begin_pounce(self, offset_x=None):
        """Transition into the pouncing phase (reusable for any play variant).

        Turns the cat to face the target's side, sets the pounce pose, and
        releases the eye-tracking override so the side-facing pose looks correct.

        Args:
            offset_x: Horizontal offset of the target from the cat. Defaults to
                       the ball's current offset when not provided.
        """
        if offset_x is None:
            offset_x = self._ball_offset_x
        self._pounce_direction = 1 if offset_x >= 0 else -1
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
        elif self._variant == "laser":
            self._draw_laser(renderer, char_x, char_y)
        elif self._variant in ("string", "feather"):
            self._draw_string(renderer, char_x, char_y)
        elif self._bubble and self._phase == "excited":
            progress = min(1.0, self._phase_timer / self.excited_duration)
            draw_bubble(renderer, self._bubble, char_x, char_y, progress, mirror)

    def _draw_ball(self, renderer, char_x, char_y):
        """Draw the rolling yarn ball (visible in all active phases)."""
        if self._phase not in ("watching", "pouncing", "recovering"):
            return

        hw = YARN_BALL["width"] // 2
        hh = YARN_BALL["height"] // 2
        ball_x = char_x + int(self._ball_offset_x) - hw
        ball_y = char_y - BALL_Y_OFFSET - hh

        # Map rotation to the nearest pre-baked 90° frame (0°/90°/180°/270°)
        frame = int(self._ball_rotation // 90) % 4

        renderer.draw_sprite_obj(YARN_BALL, ball_x, ball_y, frame=frame)

    def _draw_laser(self, renderer, char_x, char_y):
        """Draw the laser dot and beam line (always together while visible)."""
        if self._phase not in ("watching", "pouncing", "recovering"):
            return

        dot_x = char_x + int(self._laser_offset_x)
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

        # Cache positions for the physics update
        self._str_last_char_x = char_x
        self._str_last_char_y = char_y

        if getattr(self, '_str_needs_init', True):
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
