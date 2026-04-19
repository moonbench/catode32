"""
Prowl - platformer minigame
Character controller prototype: walk, jump, idle animations.
"""
from scene import Scene
from sprite_transform import mirror_sprite_h
from assets.minigame_character import (
    PLATFORMER_CAT_RUN,
    PLATFORMER_CAT_SIT,
    PLATFORMER_CAT_JUMP,
)

# Physics
GRAVITY      = 500   # px/s²
JUMP_VEL     = -185  # px/s (negative = upward); peak height ~34px
RUN_SPEED    = 85    # px/s

# Ground is the bottom of the 128x64 screen
GROUND_Y = 64

# Jump frame thresholds (based on vertical velocity)
JUMP_PEAK_RANGE = 70  # |vy| below this = "near peak" frame

# Animation frame rates
IDLE_FPS = 6   # sit cycles slowly
RUN_FPS  = 10  # run cycles quickly


def _precompute_frames(sprite):
    """Return (right_frames, left_frames) as lists of bytearrays."""
    w, h = sprite["width"], sprite["height"]
    right = [bytearray(f) for f in sprite["frames"]]
    left  = [mirror_sprite_h(f, w, h) for f in sprite["frames"]]
    return right, left


class PlatformerScene(Scene):

    def enter(self):
        # Precompute mirrored sprite frames once to avoid per-frame allocation
        self._run_r,  self._run_l  = _precompute_frames(PLATFORMER_CAT_RUN)
        self._sit_r,  self._sit_l  = _precompute_frames(PLATFORMER_CAT_SIT)
        self._jump_r, self._jump_l = _precompute_frames(PLATFORMER_CAT_JUMP)

        # Horizontal position (left edge of sprite)
        self.x = 52.0
        # feet_y: y coordinate of the bottom of the cat sprite
        self.feet_y = float(GROUND_Y)
        self.vx = 0.0
        self.vy = 0.0
        self.on_ground = True
        self.just_landed = False
        self.facing_right = True

        # Animation state
        self.anim_timer = 0.0
        self.anim_frame = 0

    def exit(self):
        # Release precomputed frame buffers
        self._run_r = self._run_l = None
        self._sit_r = self._sit_l = None
        self._jump_r = self._jump_l = None

    def update(self, dt):
        self.just_landed = False

        # Horizontal movement
        self.x += self.vx * dt

        # Clamp to screen edges using current sprite width
        w = PLATFORMER_CAT_JUMP["width"] if not self.on_ground else (
            PLATFORMER_CAT_RUN["width"] if abs(self.vx) > 1 else PLATFORMER_CAT_SIT["width"]
        )
        if self.x < 0:
            self.x = 0.0
        elif self.x > 128 - w:
            self.x = float(128 - w)

        # Vertical physics (gravity)
        if not self.on_ground:
            self.vy += GRAVITY * dt
            self.feet_y += self.vy * dt

            if self.feet_y >= GROUND_Y:
                self.feet_y = float(GROUND_Y)
                self.vy = 0.0
                self.on_ground = True
                self.just_landed = True

        # Advance animation timer (ground states only)
        if self.on_ground:
            fps = RUN_FPS if abs(self.vx) > 1 else IDLE_FPS
            self.anim_timer += dt
            frame_duration = 1.0 / fps
            if self.anim_timer >= frame_duration:
                self.anim_timer -= frame_duration
                n = len(PLATFORMER_CAT_RUN["frames"]) if abs(self.vx) > 1 else len(PLATFORMER_CAT_SIT["frames"])
                self.anim_frame = (self.anim_frame + 1) % n

    def handle_input(self):
        moving = False

        if self.input.is_pressed('left'):
            self.vx = -RUN_SPEED
            self.facing_right = False
            moving = True
        elif self.input.is_pressed('right'):
            self.vx = RUN_SPEED
            self.facing_right = True
            moving = True
        else:
            self.vx = 0.0

        if not moving and self.on_ground:
            # Reset animation index when stopping so idle starts clean
            if self.anim_frame >= len(PLATFORMER_CAT_SIT["frames"]):
                self.anim_frame = 0

        if self.input.was_just_pressed('a') and self.on_ground and not self.just_landed:
            self.vy = JUMP_VEL
            self.on_ground = False
            self.anim_frame = 0
            self.anim_timer = 0.0

    def _jump_frame(self):
        if self.vy < -JUMP_PEAK_RANGE:
            return 0  # rising
        if self.vy <= JUMP_PEAK_RANGE:
            return 1  # near/at peak
        return 2       # falling

    def draw(self):
        if not self.on_ground or self.just_landed:
            frames_r, frames_l = self._jump_r, self._jump_l
            sprite = PLATFORMER_CAT_JUMP
            frame = self._jump_frame() if not self.on_ground else 2
        elif abs(self.vx) > 1:
            frames_r, frames_l = self._run_r, self._run_l
            sprite = PLATFORMER_CAT_RUN
            frame = self.anim_frame % len(frames_r)
        else:
            frames_r, frames_l = self._sit_r, self._sit_l
            sprite = PLATFORMER_CAT_SIT
            frame = self.anim_frame % len(frames_r)

        data = frames_r[frame] if self.facing_right else frames_l[frame]
        draw_y = int(self.feet_y) - sprite["height"]

        self.renderer.draw_sprite(data, sprite["width"], sprite["height"], int(self.x), draw_y)

        # Ground line
        self.renderer.draw_line(0, GROUND_Y - 1, 127, GROUND_Y - 1)
