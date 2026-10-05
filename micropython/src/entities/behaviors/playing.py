"""Playing behavior for energetic fun."""

import math
import random
from entities.behaviors.base import BaseBehavior, stat_rejection_chance, log_stat_rejection
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

# Shared pounce constants
POUNCE_SLIDE_SPEED = 28
POUNCE_SLIDE_DURATION = 0.9
POUNCE_RECOVER_DURATION = 0.8
POUNCE_CATCH_DURATION = 1.5
POUNCE_COUNT_MIN = 4
POUNCE_COUNT_MAX = 8

# Session constants
PLAY_MIN_DURATION = 15.0

# Shared toy movement bounds
TOY_SCREEN_MARGIN = 8

# Ball / mouse / hand constants
BALL_PUSH_FORCE = 140
BALL_MAX_SPEED = 65
BALL_FRICTION = 0.20
BALL_BOUNCE_DAMPING = 0.55
BALL_ROLL_RANGE = 60
BALL_Y_OFFSET = 8
MOUSE_Y_OFFSET = 4
HAND_Y_OFFSET = 6
HAND_ANIM_STEP = 10
HAND_PUSH_FORCE = 300
HAND_MAX_SPEED = 90
HAND_FRICTION = 0.08
HAND_BOUNCE_DAMPING = 0.20
BALL_POUNCE_DELAY_MIN = 2.5
BALL_POUNCE_DELAY_MAX = 6.0

# Laser constants
LASER_WOBBLE_AMPLITUDE = 8
LASER_WOBBLE_SPEED = 2.5
LASER_USER_SPEED = 50
LASER_Y_OFFSET = 1
LASER_POUNCE_DELAY_MIN = 2.0
LASER_POUNCE_DELAY_MAX = 5.0
LASER_DOT_RADIUS = 2
LASER_LINE_TOP_Y = -64

# String / feather constants
STRING_SEGMENTS = 8
STRING_SEG_LEN_TOP = 20
STRING_SEG_LEN_BOT = 4
STRING_GRAVITY = 120
STRING_DAMPING = 0.45
STRING_ITERATIONS = 3
STRING_ANCHOR_SPEED = 60
STRING_ANCHOR_Y = -70
STRING_POUNCE_DELAY_MIN = 2.0
STRING_POUNCE_DELAY_MAX = 8.0
FEATHER_SEGMENTS = 8
FEATHER_WIDTH = 2

# Bubble wand constants
WAND_PUSH_FORCE = 200
WAND_MAX_SPEED = 80
WAND_FRICTION = 0.15
WAND_BOUNCE_DAMPING = 0.4
WAND_RANGE = 60
WAND_SCREEN_TOP = 8
WAND_POUNCE_DELAY_MIN = 2.5
WAND_POUNCE_DELAY_MAX = 6.5
BUBBLE_MAX = 10
BUBBLE_SPAWN_DIST = 20
BUBBLE_SPAWN_SPEED_MIN = 12
BUBBLE_FALL_SPEED = 9
BUBBLE_DRIFT_SPEED = 2.5
BUBBLE_POP_FPS = 7.0
BUBBLE_POP_DURATION = 4 / BUBBLE_POP_FPS

BUBBLE_SURPRISED_POSES = (
    "leaning_forward.side.crazy",
    "playful.forward.wowed",
    "sitting.forward.shocked",
    "standing.side.crazy",
)


def _compute_eye_frame(ball_offset_x, mirror):
    t = max(-1.0, min(1.0, ball_offset_x / BALL_ROLL_RANGE))
    if mirror:
        t = -t
    return max(0, min(4, round(2 - t * 2)))


class PlayingBehavior(BaseBehavior):
    """Pet plays energetically."""

    NAME = "playing"

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
        return stat_rejection_chance(context, cls._REJECTION_THRESHOLDS)

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

        self.excited_duration = random.uniform(1.0, 3.0)
        self.play_duration = random.uniform(5.0, 20.0)
        self.tired_duration = random.uniform(1.0, 10.0)

        self._variant = "string"
        self._bubble = None

        # Shared screen-space cache updated by draw() so update() can read it
        self._play_char_x = 64
        self._play_char_y = 40

        # Shared sliding-toy state (ball / mouse / hand / laser / wand)
        self._toy_x = 64.0
        self._toy_vel_x = 0.0
        self._toy_facing_right = False
        self._toy_anim_dist = 0.0

        # Shared pounce state
        self._pounce_direction = 1
        self._pounce_timer = 0.0
        self._pounces_total = 3
        self._pounces_done = 0

        self._session_timer = 0.0
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
            if self._rejecting:
                log_stat_rejection("Playing", context, self._REJECTION_THRESHOLDS, chance)
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

    def _start_sliding_toy(self, delay_min, delay_max, pose="playful.forward.wowed"):
        """Shared init for all left/right sliding toy variants."""
        self._toy_x = float(self._play_char_x)
        self._toy_vel_x = 0.0
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(delay_min, delay_max)
        self._eye_frame_override = _compute_eye_frame(0.0, self._character.mirror)
        self._phase = "watching"
        self._character.set_pose(pose)

    def _start_ball(self):
        self._ball_rotation = 0.0
        self._start_sliding_toy(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

    def _start_mouse(self):
        self._toy_facing_right = False
        self._start_sliding_toy(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

    def _start_hand(self):
        self._toy_facing_right = False
        self._toy_anim_dist = 0.0
        self._start_sliding_toy(BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

    def _start_laser(self):
        self._laser_base_x = float(self._play_char_x)
        self._laser_wobble_phase = 0.0
        self._laser_line_x_top = random.randint(20, 108)
        self._start_sliding_toy(LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX)

    def _start_string(self):
        self._str_node_count = FEATHER_SEGMENTS if self._variant == "feather" else STRING_SEGMENTS
        n_segs = self._str_node_count - 1
        self._str_seg_lens = [
            STRING_SEG_LEN_TOP + (STRING_SEG_LEN_BOT - STRING_SEG_LEN_TOP) * (i / max(n_segs - 1, 1))
            for i in range(n_segs)
        ]
        n = self._str_node_count
        self._str_px = [0.0] * n
        self._str_py = [0.0] * n
        self._str_ox = [0.0] * n
        self._str_oy = [0.0] * n
        self._str_anchor_x = 0.0
        self._str_needs_init = True
        self._pounces_total = random.randint(POUNCE_COUNT_MIN, POUNCE_COUNT_MAX)
        self._pounces_done = 0
        self._pounce_timer = random.uniform(STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX)
        self._phase = "watching"
        self._character.set_pose("playful.forward.wowed")

    def _start_bubbles(self):
        self._toy_facing_right = True
        self._wand_spawn_dist = 0.0
        self._bubbles = []
        self._had_bubbles = False
        self._bubble_pose_timer = 0.0
        self._start_sliding_toy(WAND_POUNCE_DELAY_MIN, WAND_POUNCE_DELAY_MAX, pose="sitting.forward.neutral")

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

        inp = getattr(self._character.context, 'input', None)
        if inp and inp.was_just_pressed('b'):
            if self._session_timer < PLAY_MIN_DURATION:
                self._progress = 0.0
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

    # ------------------------------------------------------------------
    # Shared sliding-toy helpers
    # ------------------------------------------------------------------

    def _update_toy_physics(self, dt, push, max_spd, friction, bounce):
        """Apply input, friction, and boundary bounce to _toy_x / _toy_vel_x."""
        inp = getattr(self._character.context, 'input', None)
        if inp:
            if inp.is_pressed('left'):
                self._toy_vel_x -= push * dt
            if inp.is_pressed('right'):
                self._toy_vel_x += push * dt
            if self._toy_vel_x > max_spd:
                self._toy_vel_x = max_spd
            elif self._toy_vel_x < -max_spd:
                self._toy_vel_x = -max_spd
        self._toy_vel_x *= friction ** dt
        self._toy_x += self._toy_vel_x * dt
        lo = TOY_SCREEN_MARGIN
        hi = 128 - TOY_SCREEN_MARGIN
        if self._toy_x >= hi:
            self._toy_x = hi
            self._toy_vel_x = -abs(self._toy_vel_x) * bounce
        elif self._toy_x <= lo:
            self._toy_x = lo
            self._toy_vel_x = abs(self._toy_vel_x) * bounce

    def _update_watching(self, dt, delay_min, delay_max, offset_x):
        """Countdown to the next pounce; shared by all sliding variants."""
        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                self._begin_pounce(offset_x)
                return
            self._pounce_timer = random.uniform(delay_min, delay_max)
        self._progress = self._pounces_done / self._pounces_total

    def _dispatch_sliding_phase(self, dt, delay_min, delay_max):
        """Drive watching→pouncing→recovering→catching for all sliding toys."""
        offset = self._toy_x - self._play_char_x
        if self._phase == "watching":
            self._update_watching(dt, delay_min, delay_max, offset)
        elif self._phase == "pouncing":
            self._update_pounce(dt)
        elif self._phase == "recovering":
            self._update_recovering(dt, delay_min, delay_max, offset)
        elif self._phase == "catching":
            self._update_catching(dt)

    # --- Ball variant ---

    def _update_ball(self, dt):
        self._update_toy_physics(dt, BALL_PUSH_FORCE, BALL_MAX_SPEED, BALL_FRICTION, BALL_BOUNCE_DAMPING)
        angle = self._toy_vel_x * dt / (YARN_BALL["width"] / 2.0) * (180.0 / math.pi)
        self._ball_rotation = (self._ball_rotation + angle) % 360.0
        self._eye_frame_override = _compute_eye_frame(self._toy_x - self._play_char_x, self._character.mirror)
        self._dispatch_sliding_phase(dt, BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

    # --- Mouse variant ---

    def _update_mouse(self, dt):
        self._update_toy_physics(dt, BALL_PUSH_FORCE, BALL_MAX_SPEED, BALL_FRICTION, BALL_BOUNCE_DAMPING)
        if abs(self._toy_vel_x) > 2.0:
            self._toy_facing_right = self._toy_vel_x > 0
        self._eye_frame_override = _compute_eye_frame(self._toy_x - self._play_char_x, self._character.mirror)
        self._dispatch_sliding_phase(dt, BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

    # --- Hand variant ---

    def _update_hand(self, dt):
        self._update_toy_physics(dt, HAND_PUSH_FORCE, HAND_MAX_SPEED, HAND_FRICTION, HAND_BOUNCE_DAMPING)
        self._toy_anim_dist += abs(self._toy_vel_x) * dt
        if abs(self._toy_vel_x) > 2.0:
            self._toy_facing_right = self._toy_vel_x > 0
        self._eye_frame_override = _compute_eye_frame(self._toy_x - self._play_char_x, self._character.mirror)
        self._dispatch_sliding_phase(dt, BALL_POUNCE_DELAY_MIN, BALL_POUNCE_DELAY_MAX)

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
        self._toy_x = self._laser_base_x + LASER_WOBBLE_AMPLITUDE * math.sin(self._laser_wobble_phase)
        self._eye_frame_override = _compute_eye_frame(self._toy_x - self._play_char_x, self._character.mirror)
        self._dispatch_sliding_phase(dt, LASER_POUNCE_DELAY_MIN, LASER_POUNCE_DELAY_MAX)

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
            for i in range(1, n):
                if self._str_py[i] > floor_y:
                    self._str_py[i] = floor_y
                    self._str_oy[i] = floor_y

        self._str_needs_init = False

    def _update_string_physics(self, dt):
        char_x = getattr(self, '_str_last_char_x', None)
        char_y = getattr(self, '_str_last_char_y', None)
        if char_x is None:
            return

        if self._str_needs_init:
            self._str_init_positions(char_x, char_y + STRING_ANCHOR_Y, char_y)

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
        damp = STRING_DAMPING ** dt
        for i in range(1, n):
            px, py = self._str_px[i], self._str_py[i]
            ox, oy = self._str_ox[i], self._str_oy[i]
            vx = (px - ox) * damp
            vy = (py - oy) * damp + STRING_GRAVITY * dt * dt
            self._str_ox[i] = px
            self._str_oy[i] = py
            self._str_px[i] = px + vx
            self._str_py[i] = py + vy

        self._str_ox[0] = self._str_px[0]
        self._str_oy[0] = self._str_py[0]
        self._str_px[0] = self._str_anchor_x
        self._str_py[0] = anchor_sy

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
                    self._str_px[i + 1] -= cx_ * 2
                    self._str_py[i + 1] -= cy_ * 2
                else:
                    self._str_px[i] += cx_
                    self._str_py[i] += cy_
                    self._str_px[i + 1] -= cx_
                    self._str_py[i + 1] -= cy_

        for i in range(1, n):
            if self._str_py[i] > char_y:
                self._str_py[i] = char_y
                self._str_oy[i] = char_y

        tip_sx = self._str_px[n - 1]
        self._eye_frame_override = _compute_eye_frame(tip_sx - char_x, self._character.mirror)

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
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        self._update_string_physics(dt)
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    def _update_string_recovering(self, dt):
        self._update_string_physics(dt)
        if self._phase_timer >= POUNCE_RECOVER_DURATION:
            if self._pounces_done >= self._pounces_total:
                self._phase = "catching"
                self._phase_timer = 0.0
            else:
                self._pounce_timer = random.uniform(STRING_POUNCE_DELAY_MIN, STRING_POUNCE_DELAY_MAX)
                self._phase = "watching"
                self._phase_timer = 0.0
                self._character.set_pose("playful.forward.wowed")

    # --- Bubbles variant ---

    def _update_bubbles(self, dt):
        self._update_bubble_particles(dt, self._play_char_y)

        if self._phase == "catching":
            self._update_bubbles_catching(dt)
            return

        self._update_toy_physics(dt, WAND_PUSH_FORCE, WAND_MAX_SPEED, WAND_FRICTION, WAND_BOUNCE_DAMPING)
        if abs(self._toy_vel_x) > 2.0:
            self._toy_facing_right = self._toy_vel_x > 0

        speed = abs(self._toy_vel_x)
        if speed >= BUBBLE_SPAWN_SPEED_MIN and len(self._bubbles) < BUBBLE_MAX:
            self._wand_spawn_dist += speed * dt
            while self._wand_spawn_dist >= BUBBLE_SPAWN_DIST and len(self._bubbles) < BUBBLE_MAX:
                self._wand_spawn_dist -= BUBBLE_SPAWN_DIST
                drift = random.uniform(-BUBBLE_DRIFT_SPEED, BUBBLE_DRIFT_SPEED)
                size = random.randint(0, 1)
                wy = float(WAND_SCREEN_TOP + BUBBLE_WAND["height"] // 2)
                self._bubbles.append([size, self._toy_x, wy, drift, -1.0])

        if self._phase == "watching":
            self._update_bubbles_watching(dt)
        elif self._phase == "pouncing":
            self._update_pounce(dt, zero_vel=False)
        elif self._phase == "recovering":
            self._update_bubbles_recovering(dt)

    def _update_bubble_particles(self, dt, ground_y):
        i = 0
        while i < len(self._bubbles):
            b = self._bubbles[i]
            if b[4] < 0:
                b[2] += BUBBLE_FALL_SPEED * dt
                b[1] += b[3] * dt
                if b[2] >= ground_y:
                    b[2] = float(ground_y)
                    b[4] = 0.0
            else:
                b[4] += dt
                if b[4] >= BUBBLE_POP_DURATION:
                    self._bubbles.pop(i)
                    continue
            i += 1

    def _update_bubbles_watching(self, dt):
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

        self._pounce_timer -= dt
        if self._pounce_timer <= 0:
            if not self._rejecting:
                self._pounces_done += 1
                target = random.uniform(-WAND_RANGE * 0.7, WAND_RANGE * 0.7)
                self._begin_pounce(target)
                return
            self._pounce_timer = random.uniform(WAND_POUNCE_DELAY_MIN, WAND_POUNCE_DELAY_MAX)
        self._progress = self._pounces_done / self._pounces_total

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
        if self._phase_timer >= POUNCE_CATCH_DURATION and not self._bubbles:
            self._progress = 1.0
            self._character.play_bursts()
            self.stop(completed=True)

    # ------------------------------------------------------------------
    # Shared pounce helpers
    # ------------------------------------------------------------------

    def _begin_pounce(self, offset_x):
        self._pounce_direction = 1 if offset_x >= 0 else -1
        self._character.mirror = self._pounce_direction > 0
        self._character.set_pose("leaning_forward.side.pounce")
        self._eye_frame_override = None
        self._phase = "pouncing"
        self._phase_timer = 0.0

    def _update_pounce(self, dt, zero_vel=True):
        """Slide the cat toward the toy; zero_vel=False keeps wand moving (bubbles)."""
        self._character.x += self._pounce_direction * POUNCE_SLIDE_SPEED * dt
        if self._phase_timer >= POUNCE_SLIDE_DURATION:
            x_min, x_max = self._get_scene_bounds()
            self._character.x = max(x_min, min(x_max, self._character.x))
            if zero_vel:
                self._toy_vel_x = 0.0
            self._phase = "recovering"
            self._phase_timer = 0.0
            self._character.set_pose("sitting_silly.side.happy")

    def _update_recovering(self, dt, delay_min, delay_max, offset_x):
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

    def _draw_simple_toy(self, renderer, char_x, char_y, sprite, y_off, frame=0, mirror=False):
        """Draw a toy sprite centered on _toy_x; caches char pos for update()."""
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering"):
            return
        tx = int(self._toy_x) - sprite["width"] // 2
        ty = char_y - y_off - sprite["height"] // 2
        renderer.draw_sprite_obj(sprite, tx, ty, frame=frame, mirror_h=mirror)

    def _draw_ball(self, renderer, char_x, char_y):
        frame = int(self._ball_rotation // 90) % 4
        self._draw_simple_toy(renderer, char_x, char_y, YARN_BALL, BALL_Y_OFFSET, frame=frame)

    def _draw_mouse(self, renderer, char_x, char_y):
        self._draw_simple_toy(renderer, char_x, char_y, MOUSE_TOY, MOUSE_Y_OFFSET, mirror=self._toy_facing_right)

    def _draw_hand(self, renderer, char_x, char_y):
        frame = int(self._toy_anim_dist / HAND_ANIM_STEP) % 2
        self._draw_simple_toy(renderer, char_x, char_y, HAND_SCRATCH, HAND_Y_OFFSET, frame=frame, mirror=self._toy_facing_right)

    def _draw_laser(self, renderer, char_x, char_y):
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering"):
            return
        dot_x = int(self._toy_x)
        dot_y = char_y - LASER_Y_OFFSET
        renderer.draw_line(self._laser_line_x_top, LASER_LINE_TOP_Y, dot_x, dot_y)
        renderer.draw_circle(dot_x, dot_y, LASER_DOT_RADIUS, filled=True)

    def _draw_string(self, renderer, char_x, char_y):
        if self._phase not in ("watching", "pouncing", "recovering"):
            return
        self._play_char_x = char_x
        self._play_char_y = char_y
        self._str_last_char_x = char_x
        self._str_last_char_y = char_y
        if self._str_needs_init:
            return

        n = self._str_node_count
        if self._variant == "feather":
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
            for i in range(n - 1):
                renderer.draw_line(
                    int(self._str_px[i]), int(self._str_py[i]),
                    int(self._str_px[i + 1]), int(self._str_py[i + 1]),
                )
            tx = int(self._str_px[n - 1])
            ty = int(self._str_py[n - 1])
            renderer.draw_circle(tx, ty, 1, filled=True)

    def _draw_feather_tip(self, renderer, base_x, base_y, tip_x, tip_y):
        ddx = tip_x - base_x
        ddy = tip_y - base_y
        mag = math.sqrt(ddx * ddx + ddy * ddy)
        if mag < 0.001:
            return
        fx = ddx / mag
        fy = ddy / mag
        nx = fy
        ny = -fx

        L = mag
        W = float(FEATHER_WIDTH)

        def p(fl, wl):
            return int(base_x + fx*fl + nx*wl), int(base_y + fy*fl + ny*wl)

        A = p(0,        0)
        B = p(L,        0)
        C = p(L*1.036,  W*0.33)
        D = p(L*0.964,  W)
        E = p(L*0.321,  W)
        F = p(L*0.214,  0)
        G = p(L*0.321, W*0.33);  H = p(L*0.964, W*0.33)
        I = p(L*0.357, W*0.67);  J = p(L*0.929, W*0.67)

        renderer.draw_line(F[0], F[1], G[0], G[1], 0)
        renderer.draw_line(G[0], G[1], H[0], H[1], 0)
        renderer.draw_line(I[0], I[1], J[0], J[1], 0)
        renderer.draw_line(A[0], A[1], B[0], B[1])
        renderer.draw_line(C[0], C[1], D[0], D[1])
        renderer.draw_line(D[0], D[1], E[0], E[1])
        renderer.draw_line(E[0], E[1], F[0], F[1])

    def _draw_bubbles(self, renderer, char_x, char_y):
        self._play_char_x = char_x
        self._play_char_y = char_y
        if self._phase not in ("watching", "pouncing", "recovering", "catching"):
            return

        if self._phase != "catching":
            wx = int(self._toy_x) - BUBBLE_WAND["width"] // 2
            renderer.draw_sprite_obj(BUBBLE_WAND, wx, WAND_SCREEN_TOP, mirror_h=not self._toy_facing_right)

        pw = BUBBLE_POP["width"] // 2
        ph = BUBBLE_POP["height"] // 2
        for b in self._bubbles:
            bx = int(b[1])
            by = int(b[2])
            if b[4] < 0:
                if b[0] == 0:
                    renderer.draw_sprite_obj(BUBBLE1, bx - 3, by - 3)
                else:
                    renderer.draw_sprite_obj(BUBBLE2, bx - 4, by - 4)
            else:
                frame = min(3, int(b[4] * BUBBLE_POP_FPS))
                renderer.draw_sprite_obj(BUBBLE_POP, bx - pw, by - ph, frame=frame)
