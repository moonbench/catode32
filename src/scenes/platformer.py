"""
Prowl - platformer minigame
Character controller: walk, jump, solid blocks, one-way platforms, camera scrolling.
Combat: cat swipe attack, slime enemies.
"""
import random
from scene import Scene
from sprite_transform import mirror_sprite_h
from assets.minigame_character import (
    PLATFORMER_CAT_RUN,
    PLATFORMER_CAT_SIT,
    PLATFORMER_CAT_JUMP,
    PLATFORMER_CAT_SIT_SWIPE,
    PLATFORMER_CAT_RUN_SWIPE,
    PLATFORMER_SLIME_IDLE,
    PLATFORMER_STRIKE,
)

# Physics
GRAVITY   = 400
JUMP_VEL  = -185
RUN_SPEED = 84

# Cat logical hitbox (centered on self.x / self.feet_y)
CAT_HALF_W = 6
CAT_H      = 12

# Terrain tile sizes
BLOCK_W = 8
BLOCK_H = 8
PLAT_H  = 4

# World dimensions
WORLD_W  = 800
GROUND_Y = 64   # y-coordinate of the ground tile tops
KILL_Y   = GROUND_Y + 24  # falling below this respawns the cat

# Gaps in the ground floor — (x_start, x_end) pairs, both at 8-px boundaries.
# Chosen to avoid ground-level slime spawns and create interesting traversal.
GROUND_GAPS = (
    (128, 168),   # 5 blocks (40px) — near start of chunk 1
    (296, 328),   # 4 blocks (32px) — chunk 2
    (496, 544),   # 6 blocks (48px) — chunk 4
    (648, 680),   # 4 blocks (32px) — chunk 5, crossable via elevated shelf
)

# Chunk dimensions — match screen size so at most 4 chunks are ever on screen
CHUNK_W = 128
CHUNK_H = 64

# Camera scroll thresholds (screen pixels)
LEFT_SCROLL_PX  = 57   # ~45% of 128
RIGHT_SCROLL_PX = 83   # ~65% of 128
TOP_SCROLL_PX   = 22   # ~35% of 64
BOT_SCROLL_PX   = 42   # ~65% of 64

# Camera speed and bounds
CAM_LERP  = 5.0
CAM_X_MIN = 0
CAM_X_MAX = WORLD_W - 128   # 672
CAM_Y_MIN = -128             # max upward scroll
CAM_Y_MAX = 0

# Animation
IDLE_FPS        = 6
RUN_FPS         = 10
JUMP_PEAK_RANGE = 70

# Cat combat
CAT_START_HP     = 2
CAT_BLINK_DUR    = 1.5    # invincibility + blink duration after taking damage
CAT_BLINK_INT    = 0.1    # visibility toggle interval while blinking
CAT_KNOCKBACK_VX = 160.0
CAT_KNOCKBACK_VY = -110.0

# Swipe attack
SWIPE_FPS         = 12   # frames per second for swipe animation
ATTACK_FRAME      = 2    # frame index (0-based) that deals damage
SIT_SWIPE_FRAMES  = 5    # total frames in PLATFORMER_CAT_SIT_SWIPE
RUN_SWIPE_FRAMES  = 3    # total frames in PLATFORMER_CAT_RUN_SWIPE
ATK_REACH    = 16          # px of reach ahead of the cat body edge
STRIKE_VX    = 55          # px/s the strike effect drifts forward

# Slime
SLIME_SPEED          = 18
SLIME_HALF_W         = 8
SLIME_H              = 8
SLIME_START_HP       = 2
SLIME_HIT_FLASH      = 0.15   # seconds to show fill frame after being hit
SLIME_ANIM_SPF       = 0.5    # seconds per frame (2 FPS)
SLIME_PATROL_RADIUS  = 48

# Slime spawn positions: (world_x, feet_y)
SLIME_SPAWNS = (
    (90,  56),   # chunk 0, ground floor
    (195, 56),   # chunk 1, ground floor
    (262, 28),   # chunk 2, on elevated group B
    (440, 56),   # chunk 3, ground floor
    (546, 44),   # chunk 4, on elevated block
    (700, 56),   # chunk 5, ground floor
)


def _make_level():
    """Build and return (solid_chunks, platforms).

    solid_chunks: dict {(chunk_col, chunk_row): tuple of (bx, by)}
    platforms:    indexed tuple of (px, py, pw) for one-way platforms

    All block positions are at 8-px offsets, so no block ever straddles
    two chunks — each block belongs to exactly one chunk bucket.
    """
    solid = {}

    def add_solid(bx, by):
        key = (bx // CHUNK_W, by // CHUNK_H)
        if key not in solid:
            solid[key] = []
        solid[key].append((bx, by))

    # --- Ground floor (y=56, chunk row 0, cols 0-6) — gaps skipped ---
    x = 0
    while x < WORLD_W:
        in_gap = False
        for g0, g1 in GROUND_GAPS:
            if g0 <= x < g1:
                in_gap = True
                break
        if not in_gap:
            add_solid(x, 56)
        x += BLOCK_W

    # --- Chunk col 0 (x: 0..127) ---
    # Elevated solid group A
    for i in range(8):
        add_solid(56 + i * BLOCK_W, 20)

    # --- Chunks col 1-2 (x: 128..383) ---
    # Elevated solid group B — straddles cols 1 and 2
    for i in range(6):
        add_solid(240 + i * BLOCK_W, 28)

    # --- Chunk col 2 (x: 256..383) ---
    for i in range(5):
        add_solid(304 + i * BLOCK_W, 12)

    # --- Chunk col 3 (x: 384..511) ---
    for i in range(6):
        add_solid(392 + i * BLOCK_W, 36)
    for i in range(4):
        add_solid(456 + i * BLOCK_W, 16)

    # --- Chunk col 4 (x: 512..639) ---
    for i in range(5):
        add_solid(528 + i * BLOCK_W, 44)
    for i in range(4):
        add_solid(576 + i * BLOCK_W, 24)

    # --- Chunk col 5 (x: 640..767) ---
    for i in range(5):
        add_solid(648 + i * BLOCK_W, 36)
    # High shelf — row -1 (y: -64..−1)
    for i in range(5):
        add_solid(680 + i * BLOCK_W, -8)

    # --- Chunk col 6 (x: 768..800) ---
    for i in range(4):
        add_solid(768 + i * BLOCK_W, 40)

    # Freeze into tuples for efficient per-frame iteration
    solid_chunks = {k: tuple(v) for k, v in solid.items()}

    platforms = (
        # --- Left section ---
        (8,   28, 32),   # near start
        (148, 36, 32),   # mid-level
        (192, 20, 32),   # mid-high
        (244,  4, 24),   # near screen top
        (272, -16, 32),  # above screen — requires scrolling up
        (320, 36, 32),   # far right of first section
        # --- Extended sections ---
        (384,  8, 32),   # col 3 entry
        (448, -8, 32),   # col 3 high
        (520, 36, 32),   # col 4 low
        (576,  4, 32),   # col 4 high
        (640, -24, 40),  # col 5 very high
        (720, 36, 32),   # col 5 low
        (760, 16, 24),   # col 6 near end
    )

    return solid_chunks, platforms


# Build once at import time; freed when scene module is unloaded
SOLID_CHUNKS, PLATFORMS = _make_level()


def _supported(x, feet_y, half_w):
    """Return True if (x, feet_y) is resting on any solid block or platform."""
    fy  = int(feet_y)
    cl  = int(x) - half_w
    cr  = int(x) + half_w
    row = fy // CHUNK_H
    col0 = (cl - BLOCK_W + 1) // CHUNK_W
    col1 = (cr - 1) // CHUNK_W
    for col in range(col0, col1 + 1):
        bucket = SOLID_CHUNKS.get((col, row))
        if bucket:
            for bx, by in bucket:
                if by == fy and cl < bx + BLOCK_W and cr > bx:
                    return True
    for px, py, pw in PLATFORMS:
        if py == fy and cl < px + pw and cr > px:
            return True
    return fy >= GROUND_Y


def _precompute_frames(sprite):
    """Return (right_frames, left_frames) as lists of bytearrays."""
    w, h = sprite["width"], sprite["height"]
    right = [bytearray(f) for f in sprite["frames"]]
    left  = [mirror_sprite_h(f, w, h) for f in sprite["frames"]]
    return right, left


class Slime:
    def __init__(self, x, feet_y):
        self.x        = float(x)
        self.feet_y   = float(feet_y)
        self.chunk_col = int(x) // CHUNK_W
        # Patrol range: ±SLIME_PATROL_RADIUS from spawn, clamped to spawn chunk
        cx_min = float(self.chunk_col * CHUNK_W + SLIME_HALF_W + 1)
        cx_max = float((self.chunk_col + 1) * CHUNK_W - SLIME_HALF_W - 1)
        self.patrol_min = max(float(int(x) - SLIME_PATROL_RADIUS), cx_min)
        self.patrol_max = min(float(int(x) + SLIME_PATROL_RADIUS), cx_max)
        self.vx        = float(SLIME_SPEED)
        self.hp        = SLIME_START_HP
        self.alive     = True
        self.anim_frame = 0
        self.anim_timer = 0.0
        self.hit_timer  = 0.0   # > 0: show fill frame (white flash)
        self.dir_timer  = 1.0 + random.random() * 2.0


class PlatformerScene(Scene):

    def enter(self):
        # Precompute mirrored frames — no per-frame allocation
        self._run_r,   self._run_l   = _precompute_frames(PLATFORMER_CAT_RUN)
        self._sit_r,   self._sit_l   = _precompute_frames(PLATFORMER_CAT_SIT)
        self._jump_r,  self._jump_l  = _precompute_frames(PLATFORMER_CAT_JUMP)
        self._swipe_r,     self._swipe_l     = _precompute_frames(PLATFORMER_CAT_SIT_SWIPE)
        self._run_swipe_r, self._run_swipe_l = _precompute_frames(PLATFORMER_CAT_RUN_SWIPE)

        # Strike effect frames: default faces right, mirrored faces left
        stw, sth = PLATFORMER_STRIKE["width"], PLATFORMER_STRIKE["height"]
        self._strike_r = [bytearray(f) for f in PLATFORMER_STRIKE["frames"]]
        self._strike_l = [mirror_sprite_h(f, stw, sth) for f in PLATFORMER_STRIKE["frames"]]

        # Slime frames: default faces right, mirrored faces left
        sw, sh = PLATFORMER_SLIME_IDLE["width"], PLATFORMER_SLIME_IDLE["height"]
        self._slime_r      = [bytearray(f) for f in PLATFORMER_SLIME_IDLE["frames"]]
        self._slime_l      = [mirror_sprite_h(f, sw, sh) for f in PLATFORMER_SLIME_IDLE["frames"]]
        # fill_inv: pre-inverted fill — drawn with transparent_color=1 to get a
        # black silhouette (same technique as draw_sprite_obj)
        self._slime_fill_inv_r = [bytearray(b ^ 0xFF for b in f)
                                  for f in PLATFORMER_SLIME_IDLE["fill_frames"]]
        self._slime_fill_inv_l = [mirror_sprite_h(f, sw, sh)
                                  for f in self._slime_fill_inv_r]
        # fill (un-inverted): white blob used for the hit flash
        self._slime_fill_r = [bytearray(f) for f in PLATFORMER_SLIME_IDLE["fill_frames"]]
        self._slime_fill_l = [mirror_sprite_h(f, sw, sh) for f in self._slime_fill_r]

        # self.x = hitbox centre x;  self.feet_y = hitbox bottom
        self.x        = 20.0
        self.feet_y   = 56.0
        self.vx       = 0.0
        self.vy       = 0.0
        self.on_ground    = True
        self.just_landed  = False
        self.facing_right = True

        self._on_platform   = -1  # platform index cat stands on (-1 = solid/none)
        self._drop_platform = -1  # platform index being dropped through

        self.anim_timer = 0.0
        self.anim_frame = 0

        # Combat state
        self._cat_hp         = CAT_START_HP
        self._cat_blink_timer = 0.0   # > 0: cat blinks and is invincible
        self._swipe_frame    = -1     # -1 = idle; otherwise current swipe frame index
        self._swipe_timer    = 0.0
        self._swipe_is_run   = False  # True when using run/jump swipe sprite
        self._strike_active  = False  # impact effect spawned on ATTACK_FRAME
        self._strike_x       = 0.0
        self._strike_y       = 0.0
        self._strike_vx      = 0.0
        self._strike_right   = True   # which mirror set to use
        self._strike_frame   = 0
        self._strike_timer   = 0.0

        # Enemies
        self._slimes = [Slime(x, fy) for x, fy in SLIME_SPAWNS]

        # Camera: world coord of top-left of screen
        self.camera_x      = 0.0
        self.camera_y      = 0.0
        self.target_cam_x  = 0.0
        self.target_cam_y  = 0.0

    def exit(self):
        self._run_r   = self._run_l   = None
        self._sit_r   = self._sit_l   = None
        self._jump_r  = self._jump_l  = None
        self._swipe_r      = self._swipe_l      = None
        self._run_swipe_r  = self._run_swipe_l  = None
        self._strike_r = self._strike_l = None
        self._slime_r = self._slime_l = None
        self._slime_fill_r = self._slime_fill_l = None
        self._slime_fill_inv_r = self._slime_fill_inv_l = None
        self._slimes  = None

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        self.just_landed = False

        # Walk-off-edge detection: if nothing beneath feet, start falling
        if self.on_ground and not self._is_supported():
            self.on_ground    = False
            self._on_platform = -1

        # Horizontal movement + collision
        self.x += self.vx * dt
        if self.x < CAT_HALF_W:
            self.x = float(CAT_HALF_W)
        elif self.x > WORLD_W - CAT_HALF_W:
            self.x = float(WORLD_W - CAT_HALF_W)
        self._resolve_x()

        # Vertical physics + collision
        if not self.on_ground:
            self.vy += GRAVITY * dt
            prev_feet = self.feet_y
            self.feet_y += self.vy * dt
            self._resolve_y(prev_feet)

        # Fell through a gap — counts as a death
        if self.feet_y > KILL_Y:
            self._respawn_cat()
            return

        # Clear drop-through once fully below the platform
        if self._drop_platform >= 0:
            _, py, _ = PLATFORMS[self._drop_platform]
            if self.feet_y > py + PLAT_H:
                self._drop_platform = -1

        # Ground animation (suppressed while swiping)
        if self.on_ground and self._swipe_frame < 0:
            fps = RUN_FPS if abs(self.vx) > 1 else IDLE_FPS
            self.anim_timer += dt
            if self.anim_timer >= 1.0 / fps:
                self.anim_timer -= 1.0 / fps
                n = (len(PLATFORMER_CAT_RUN["frames"]) if abs(self.vx) > 1
                     else len(PLATFORMER_CAT_SIT["frames"]))
                self.anim_frame = (self.anim_frame + 1) % n

        # Swipe animation — advance one frame per tick, fire hit on frame 2
        if self._swipe_frame >= 0:
            total_frames = RUN_SWIPE_FRAMES if self._swipe_is_run else SIT_SWIPE_FRAMES
            old_frame = self._swipe_frame
            self._swipe_timer += dt
            if self._swipe_timer >= 1.0 / SWIPE_FPS:
                self._swipe_timer -= 1.0 / SWIPE_FPS
                self._swipe_frame += 1
            if old_frame < ATTACK_FRAME <= self._swipe_frame:
                self._apply_cat_attack()
            if self._swipe_frame >= total_frames:
                self._swipe_frame = -1
                self.anim_frame   = 0

        # Drift the strike effect forward and advance its own frame counter
        if self._strike_active:
            self._strike_x     += self._strike_vx * dt
            self._strike_timer += dt
            if self._strike_timer >= 1.0 / SWIPE_FPS:
                self._strike_timer -= 1.0 / SWIPE_FPS
                self._strike_frame += 1
            if self._strike_frame >= len(PLATFORMER_STRIKE["frames"]):
                self._strike_active = False

        # Blink / invincibility countdown
        if self._cat_blink_timer > 0:
            self._cat_blink_timer -= dt

        # Update slimes — only those in visible chunks
        cam_col0 = int(self.camera_x) // CHUNK_W
        cam_col1 = (int(self.camera_x) + 127) // CHUNK_W
        for slime in self._slimes:
            if slime.alive and cam_col0 <= slime.chunk_col <= cam_col1:
                self._update_slime(slime, dt)

        # Check slime-cat contact damage
        self._check_slime_cat_contact()

        self._update_camera(dt)

    # ------------------------------------------------------------------
    # Enemy logic
    # ------------------------------------------------------------------

    def _update_slime(self, slime, dt):
        # Random direction change
        slime.dir_timer -= dt
        if slime.dir_timer <= 0:
            slime.vx      = SLIME_SPEED if random.random() > 0.5 else -SLIME_SPEED
            slime.dir_timer = 1.0 + random.random() * 2.0

        # Edge detection: probe just past the leading foot edge
        look_x = slime.x + ((SLIME_HALF_W + 2) if slime.vx > 0 else -(SLIME_HALF_W + 2))
        if not _supported(look_x, slime.feet_y, 1):
            slime.vx = -slime.vx

        # Move and clamp to patrol bounds
        next_x = slime.x + slime.vx * dt
        if next_x <= slime.patrol_min:
            next_x   = slime.patrol_min
            slime.vx = SLIME_SPEED
        elif next_x >= slime.patrol_max:
            next_x   = slime.patrol_max
            slime.vx = -SLIME_SPEED
        slime.x = next_x

        # Animation at 2 FPS
        slime.anim_timer += dt
        if slime.anim_timer >= SLIME_ANIM_SPF:
            slime.anim_timer -= SLIME_ANIM_SPF
            slime.anim_frame ^= 1

        # Hit flash countdown
        if slime.hit_timer > 0:
            slime.hit_timer -= dt

    # ------------------------------------------------------------------
    # Combat
    # ------------------------------------------------------------------

    def _apply_cat_attack(self):
        """Deal 1 damage to every slime inside the attack zone on swipe frame 2."""
        # Cat body is left-aligned (facing right) or right-aligned (facing left)
        # during a swipe; attack zone is ATK_REACH px beyond that edge.
        if self.facing_right:
            atk_cl = int(self.x) + CAT_H          # body right edge
            atk_cr = atk_cl + ATK_REACH
        else:
            atk_cr = int(self.x) - CAT_H          # body left edge
            atk_cl = atk_cr - ATK_REACH

        # Spawn strike effect at the start of the attack zone, drifting forward
        self._strike_active = True
        self._strike_x      = float(atk_cl if self.facing_right else atk_cr)
        self._strike_y      = float(int(self.feet_y) - CAT_H // 2)
        self._strike_vx     = STRIKE_VX if self.facing_right else -STRIKE_VX
        self._strike_right  = self.facing_right
        self._strike_frame  = 0
        self._strike_timer  = 0.0
        atk_ct = int(self.feet_y) - CAT_H
        atk_cb = int(self.feet_y)

        for slime in self._slimes:
            if not slime.alive:
                continue
            scl = int(slime.x) - SLIME_HALF_W
            scr = int(slime.x) + SLIME_HALF_W
            sct = int(slime.feet_y) - SLIME_H
            scb = int(slime.feet_y)
            if scl >= atk_cr or scr <= atk_cl:
                continue
            if sct >= atk_cb or scb <= atk_ct:
                continue
            slime.hp -= 1
            slime.hit_timer = SLIME_HIT_FLASH
            if slime.hp <= 0:
                slime.alive = False

    def _check_slime_cat_contact(self):
        """If any live slime touches the cat, deal 1 damage and knock the cat back."""
        if self._cat_blink_timer > 0:
            return  # invincible during blink

        # During a swipe the cat's damage hitbox shifts:
        #   facing right → left-aligned  (cl = self.x,   cr = self.x + CAT_H)
        #   facing left  → right-aligned (cl = self.x - CAT_H, cr = self.x)
        # This gives a margin so the cat isn't trivially hit while attacking.
        if self._swipe_frame >= 0:
            if self.facing_right:
                ccl = int(self.x)
                ccr = int(self.x) + CAT_H
            else:
                ccl = int(self.x) - CAT_H
                ccr = int(self.x)
        else:
            ccl = int(self.x) - CAT_HALF_W
            ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)

        for slime in self._slimes:
            if not slime.alive:
                continue
            scl = int(slime.x) - SLIME_HALF_W
            scr = int(slime.x) + SLIME_HALF_W
            sct = int(slime.feet_y) - SLIME_H
            scb = int(slime.feet_y)
            if ccl >= scr or ccr <= scl:
                continue
            if cct >= scb or ccb <= sct:
                continue
            # Contact — take damage and knock back away from the slime
            self._cat_hp -= 1
            self._cat_blink_timer = CAT_BLINK_DUR
            self.vx = CAT_KNOCKBACK_VX if self.x >= slime.x else -CAT_KNOCKBACK_VX
            self.vy = CAT_KNOCKBACK_VY
            if self.on_ground:
                self.on_ground    = False
                self._on_platform = -1
            if self._cat_hp <= 0:
                self._respawn_cat()
            break  # one slime contact per frame is enough

    def _respawn_cat(self):
        self.x        = 20.0
        self.feet_y   = 56.0
        self.vx       = 0.0
        self.vy       = 0.0
        self.on_ground    = True
        self._on_platform = -1
        self._drop_platform = -1
        self.facing_right = True
        self.camera_x      = 0.0
        self.camera_y      = 0.0
        self.target_cam_x  = 0.0
        self.target_cam_y  = 0.0
        self._cat_hp         = CAT_START_HP
        self._cat_blink_timer = CAT_BLINK_DUR
        self._swipe_frame    = -1
        self.anim_frame      = 0

    # ------------------------------------------------------------------
    # Terrain queries
    # ------------------------------------------------------------------

    def _is_supported(self):
        fy = int(self.feet_y)
        cl = int(self.x) - CAT_HALF_W
        cr = int(self.x) + CAT_HALF_W

        row  = fy // CHUNK_H
        col0 = (cl - BLOCK_W + 1) // CHUNK_W
        col1 = (cr - 1) // CHUNK_W
        for col in range(col0, col1 + 1):
            bucket = SOLID_CHUNKS.get((col, row))
            if bucket:
                for bx, by in bucket:
                    if by == fy and cl < bx + BLOCK_W and cr > bx:
                        return True

        if self._on_platform >= 0:
            px, py, pw = PLATFORMS[self._on_platform]
            if py == fy and cl < px + pw and cr > px:
                return True

        return False

    def _resolve_x(self):
        cl = int(self.x) - CAT_HALF_W
        cr = int(self.x) + CAT_HALF_W
        ct = int(self.feet_y) - CAT_H
        cb = int(self.feet_y)

        col0 = (cl - BLOCK_W + 1) // CHUNK_W
        col1 = (cr - 1) // CHUNK_W
        row0 = (ct - BLOCK_H + 1) // CHUNK_H
        row1 = (cb - 1) // CHUNK_H

        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                bucket = SOLID_CHUNKS.get((col, row))
                if not bucket:
                    continue
                for bx, by in bucket:
                    br = bx + BLOCK_W
                    bb = by + BLOCK_H
                    if ct >= bb or cb <= by:
                        continue
                    if cl >= br or cr <= bx:
                        continue
                    if self.vx > 0:
                        self.x = float(bx - CAT_HALF_W)
                    elif self.vx < 0:
                        self.x = float(br + CAT_HALF_W)
                    else:
                        if cr - bx < br - cl:
                            self.x = float(bx - CAT_HALF_W)
                        else:
                            self.x = float(br + CAT_HALF_W)
                    self.vx = 0.0
                    cl = int(self.x) - CAT_HALF_W
                    cr = int(self.x) + CAT_HALF_W

    def _resolve_y(self, prev_feet):
        cl = int(self.x) - CAT_HALF_W
        cr = int(self.x) + CAT_HALF_W

        col0 = (cl - BLOCK_W + 1) // CHUNK_W
        col1 = (cr - 1) // CHUNK_W

        if self.vy >= 0:  # descending
            row0 = int(prev_feet) // CHUNK_H
            row1 = int(self.feet_y) // CHUNK_H
            for col in range(col0, col1 + 1):
                for row in range(row0, row1 + 1):
                    bucket = SOLID_CHUNKS.get((col, row))
                    if not bucket:
                        continue
                    for bx, by in bucket:
                        if cl >= bx + BLOCK_W or cr <= bx:
                            continue
                        if prev_feet <= by <= self.feet_y:
                            self.feet_y   = float(by)
                            self.vy       = 0.0
                            self.on_ground    = True
                            self._on_platform = -1
                            self.just_landed  = True
                            return

            for i, (px, py, pw) in enumerate(PLATFORMS):
                if i == self._drop_platform:
                    continue
                if cl >= px + pw or cr <= px:
                    continue
                if prev_feet <= py <= self.feet_y:
                    self.feet_y   = float(py)
                    self.vy       = 0.0
                    self.on_ground    = True
                    self._on_platform = i
                    self.just_landed  = True
                    return

            # No hard-floor fallback — gaps are death zones (see KILL_Y check)

        else:  # ascending — solid block ceilings only
            prev_head = prev_feet - CAT_H
            curr_head = self.feet_y - CAT_H
            row0 = int(curr_head - BLOCK_H + 1) // CHUNK_H
            row1 = int(prev_head - BLOCK_H) // CHUNK_H
            for col in range(col0, col1 + 1):
                for row in range(row0, row1 + 1):
                    bucket = SOLID_CHUNKS.get((col, row))
                    if not bucket:
                        continue
                    for bx, by in bucket:
                        bb = by + BLOCK_H
                        if cl >= bx + BLOCK_W or cr <= bx:
                            continue
                        if prev_head >= bb > curr_head:
                            self.feet_y = float(bb + CAT_H)
                            self.vy     = 0.0
                            return

    def _update_camera(self, dt):
        cat_sx = self.x      - self.camera_x
        cat_sy = self.feet_y - self.camera_y

        if self.facing_right and cat_sx > RIGHT_SCROLL_PX:
            self.target_cam_x = self.x - RIGHT_SCROLL_PX
        elif not self.facing_right and cat_sx < LEFT_SCROLL_PX:
            self.target_cam_x = self.x - LEFT_SCROLL_PX

        if cat_sy < TOP_SCROLL_PX:
            self.target_cam_y = self.feet_y - TOP_SCROLL_PX
        elif cat_sy > BOT_SCROLL_PX:
            self.target_cam_y = self.feet_y - BOT_SCROLL_PX

        if self.target_cam_x < CAM_X_MIN:
            self.target_cam_x = float(CAM_X_MIN)
        elif self.target_cam_x > CAM_X_MAX:
            self.target_cam_x = float(CAM_X_MAX)
        if self.target_cam_y < CAM_Y_MIN:
            self.target_cam_y = float(CAM_Y_MIN)
        elif self.target_cam_y > CAM_Y_MAX:
            self.target_cam_y = float(CAM_Y_MAX)

        self.camera_x += (self.target_cam_x - self.camera_x) * CAM_LERP * dt
        self.camera_y += (self.target_cam_y - self.camera_y) * CAM_LERP * dt

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        # During a run/jump swipe movement is NOT locked — cat keeps momentum.
        # During a sit swipe movement is locked as before.
        # B can chain a new swipe once the hit frame has fired (frame 2+).
        if self._swipe_frame >= 0:
            if not self._swipe_is_run:
                self.vx = 0.0
            if self._swipe_frame >= ATTACK_FRAME and self.input.was_just_pressed('b'):
                moving = abs(self.vx) > 1 or not self.on_ground
                self._swipe_is_run = moving
                self._swipe_frame  = 0
                self._swipe_timer  = 0.0
                self.anim_frame    = 0
            return

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
            if self.anim_frame >= len(PLATFORMER_CAT_SIT["frames"]):
                self.anim_frame = 0

        if self.input.was_just_pressed('a') and self.on_ground and not self.just_landed:
            self.vy = JUMP_VEL
            self.on_ground    = False
            self._on_platform = -1
            self.anim_frame   = 0
            self.anim_timer   = 0.0

        if (self.input.was_just_pressed('down')
                and self.on_ground
                and self._on_platform >= 0):
            self._drop_platform = self._on_platform
            self._on_platform   = -1
            self.on_ground      = False
            self.vy             = 20.0
            self.anim_frame     = 0
            self.anim_timer     = 0.0

        # Swipe: standing still uses sit swipe; running or airborne uses run swipe
        if self.input.was_just_pressed('b'):
            self._swipe_is_run  = moving or not self.on_ground
            self._swipe_frame   = 0
            self._swipe_timer   = 0.0
            self.anim_frame     = 0

    # ------------------------------------------------------------------
    # Draw
    # ------------------------------------------------------------------

    def _jump_frame(self):
        if self.vy < -JUMP_PEAK_RANGE:
            return 0   # rising
        if self.vy <= JUMP_PEAK_RANGE:
            return 1   # near peak
        return 2       # falling

    def draw(self):
        cam_x = int(self.camera_x)
        cam_y = int(self.camera_y)

        # Solid terrain — only iterate chunks that overlap the viewport
        col0 = cam_x // CHUNK_W
        col1 = (cam_x + 127) // CHUNK_W
        row0 = cam_y // CHUNK_H
        row1 = (cam_y + 63) // CHUNK_H
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                bucket = SOLID_CHUNKS.get((col, row))
                if not bucket:
                    continue
                for bx, by in bucket:
                    sx = bx - cam_x
                    sy = by - cam_y
                    if -BLOCK_W < sx < 128 and -BLOCK_H < sy < 64:
                        self.renderer.draw_rect(sx, sy, BLOCK_W, BLOCK_H, color=1)

        # Platforms — 13 total, cheap to iterate flat
        for px, py, pw in PLATFORMS:
            sx = px - cam_x
            sy = py - cam_y
            if -pw < sx < 128 and -PLAT_H < sy < 64:
                self.renderer.draw_rect(sx, sy, pw, PLAT_H, color=1)

        # Slimes — only those in visible chunks
        sw = PLATFORMER_SLIME_IDLE["width"]
        sh = PLATFORMER_SLIME_IDLE["height"]
        for slime in self._slimes:
            if not slime.alive:
                continue
            if not (col0 <= slime.chunk_col <= col1):
                continue
            sx = int(slime.x) - sw // 2 - cam_x
            sy = int(slime.feet_y) - sh - cam_y
            facing_right = slime.vx >= 0
            fi = slime.anim_frame
            if slime.hit_timer > 0:
                # Hit flash: solid white blob
                data = self._slime_fill_r[fi] if facing_right else self._slime_fill_l[fi]
                self.renderer.draw_sprite(data, sw, sh, sx, sy)
            else:
                # Normal: black silhouette first, then white outline on top
                fill = self._slime_fill_inv_r[fi] if facing_right else self._slime_fill_inv_l[fi]
                self.renderer.draw_sprite(fill, sw, sh, sx, sy, transparent_color=1)
                outline = self._slime_r[fi] if facing_right else self._slime_l[fi]
                self.renderer.draw_sprite(outline, sw, sh, sx, sy)

        # Strike effect — independent frame counter, always plays all 3 frames
        if self._strike_active:
            stw = PLATFORMER_STRIKE["width"]
            sth = PLATFORMER_STRIKE["height"]
            data = (self._strike_r[self._strike_frame] if self._strike_right
                    else self._strike_l[self._strike_frame])
            sx = int(self._strike_x) - stw // 2 - cam_x
            sy = int(self._strike_y) - sth // 2 - cam_y
            self.renderer.draw_sprite(data, stw, sth, sx, sy)

        # Cat sprite — invisible on even blink intervals while damaged
        if self._cat_blink_timer > 0:
            cat_visible = int(self._cat_blink_timer / CAT_BLINK_INT) % 2 == 1
        else:
            cat_visible = True

        if cat_visible:
            if self._swipe_frame >= 0:
                if self._swipe_is_run:
                    frames_r, frames_l = self._run_swipe_r, self._run_swipe_l
                    sprite = PLATFORMER_CAT_RUN_SWIPE
                    frame  = min(self._swipe_frame, RUN_SWIPE_FRAMES - 1)
                else:
                    frames_r, frames_l = self._swipe_r, self._swipe_l
                    sprite = PLATFORMER_CAT_SIT_SWIPE
                    frame  = min(self._swipe_frame, SIT_SWIPE_FRAMES - 1)
            elif not self.on_ground or self.just_landed:
                frames_r, frames_l = self._jump_r, self._jump_l
                sprite = PLATFORMER_CAT_JUMP
                frame  = self._jump_frame() if not self.on_ground else 2
            elif abs(self.vx) > 1:
                frames_r, frames_l = self._run_r, self._run_l
                sprite = PLATFORMER_CAT_RUN
                frame  = self.anim_frame % len(frames_r)
            else:
                frames_r, frames_l = self._sit_r, self._sit_l
                sprite = PLATFORMER_CAT_SIT
                frame  = self.anim_frame % len(frames_r)

            data   = frames_r[frame] if self.facing_right else frames_l[frame]
            draw_x = int(self.x) - sprite["width"] // 2 - cam_x
            draw_y = int(self.feet_y) - sprite["height"] - cam_y
            self.renderer.draw_sprite(data, sprite["width"], sprite["height"],
                                      draw_x, draw_y)
