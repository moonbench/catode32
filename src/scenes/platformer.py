"""
Prowl - platformer minigame
Character controller: walk, jump, solid blocks, one-way platforms, camera scrolling.
Combat: cat swipe attack, slime enemies.
"""
import random
from scene import Scene
from sprite_transform import mirror_sprite_h
from assets.platformer_terrain import (
    TERRAIN_TILES,
    TILE_TOP,
    TILE_TOP_LEFT,
    TILE_TOP_RIGHT,
    TILE_SIDE_LEFT,
    TILE_SIDE_RIGHT,
    TILE_BOTTOM,
    TILE_BOTTOM_LEFT,
    TILE_BOTTOM_RIGHT,
    TILE_TOP_BOTTOM,
    TILE_TOP_LEFT_BOTTOM,
    TILE_TOP_RIGHT_BOTTOM,
    PLATFORMER_CHECKPOINT_DOWN,
    PLATFORMER_CHECKPOINT_UP,
    PLATFORMER_DOOR,
    PLATFORMER_DOOR_LOCKED,
)
from assets.items import KEY
from assets.plants import (
    GRASS_SEEDLING,
    GRASS_YOUNG,
    GRASS_GROWING,
    GRASS_MATURE,
    GRASS_THRIVING,
)
from assets.effects import POOF
from assets.minigame_character import (
    PLATFORMER_CAT_RUN,
    PLATFORMER_CAT_SIT,
    PLATFORMER_CAT_JUMP,
    PLATFORMER_CAT_SIT_SWIPE,
    PLATFORMER_CAT_RUN_SWIPE,
    PLATFORMER_SLIME_IDLE,
    PLATFORMER_SLIME_BURST,
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

# Chunk dimensions — match screen size so at most 4 chunks are ever on screen
CHUNK_W = 128
CHUNK_H = 64

# Camera scroll thresholds (screen pixels)
LEFT_SCROLL_PX  = 60
RIGHT_SCROLL_PX = 68
TOP_SCROLL_PX   = 30
BOT_SCROLL_PX   = 42

# Camera speed and fixed left bound; X_MAX / Y_MIN / Y_MAX derived from level
CAM_LERP  = 5.0
CAM_X_MIN = 0

# Player upgrades
DOUBLE_JUMP_ENABLED = True

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
ATK_REACH     = 16         # px of reach ahead of the cat body edge (standing swipe)
RUN_ATK_REACH = 28         # px of reach for run/jump swipe
STRIKE_VX    = 55          # px/s the strike effect drifts forward

# Slime
SLIME_SPEED          = 18
SLIME_HALF_W         = 8
SLIME_H              = 8
SLIME_START_HP       = 2
SLIME_HIT_FLASH      = 0.15   # seconds to show fill frame after being hit
SLIME_ANIM_SPF       = 0.5    # seconds per frame (2 FPS)
SLIME_BURST_SPF      = 1.0 / 12   # seconds per frame for death burst
SLIME_PATROL_RADIUS  = 48

# Poof death animation
POOF_SPF = 1.0 / 8   # POOF["speed"] = 8

# Key collectible
KEY_BOB_PERIOD = 1.2   # seconds per full oscillation cycle
KEY_BOB_AMP    = 6     # pixels of vertical travel

# Door transition timing
DOOR_WALK_DELAY    = 0.35   # seconds the cat walks into the door before fade starts
DOOR_FADE_DURATION = 0.25   # seconds per fade phase (matches config.TRANSITION_DURATION)

# Checkpoint and door hitboxes (match sprite dimensions)
CHECKPOINT_W = 24   # down-state sprite width — trigger zone
CHECKPOINT_H = 8    # down-state sprite height
DOOR_W = 16
DOOR_H = 19

# Grass sprite variants: index 0..4 (SEEDLING=0, YOUNG=1, GROWING=2, MATURE=3, THRIVING=4)
GRASS_SPRITES = (GRASS_SEEDLING, GRASS_YOUNG, GRASS_GROWING, GRASS_MATURE, GRASS_THRIVING)

# ── Level data globals ────────────────────────────────────────────────────────
# Populated by load_level(); kept at module scope so _supported() and the draw
# loop can reference them without threading level data through every call.
SOLID_CHUNKS = {}
PLATFORMS    = ()
GRASS_CHUNKS = {}
SLIME_SPAWNS = ()
CHECKPOINTS  = ()
DOORS        = ()
LOCKED_DOORS = ()
KEY_SPAWNS   = ()
PLAYER_SPAWN = (8, 8)
WORLD_W      = 128
WORLD_H      = 64


def load_level(name):
    """Import platformer_levels/<name>.py, copy its data into module globals,
    then unload the module to recover the RAM its code object occupied."""
    global SOLID_CHUNKS, PLATFORMS, GRASS_CHUNKS, SLIME_SPAWNS
    global CHECKPOINTS, DOORS, LOCKED_DOORS, KEY_SPAWNS, PLAYER_SPAWN, WORLD_W, WORLD_H
    import sys
    full = 'platformer_levels.' + name
    __import__(full, None, None, (name,))
    mod = sys.modules[full]
    SOLID_CHUNKS = mod.SOLID_CHUNKS
    PLATFORMS    = mod.PLATFORMS
    GRASS_CHUNKS = mod.GRASS_CHUNKS
    SLIME_SPAWNS = mod.SLIME_SPAWNS
    CHECKPOINTS  = getattr(mod, 'CHECKPOINTS', ())
    DOORS        = getattr(mod, 'DOORS', ())
    LOCKED_DOORS = getattr(mod, 'LOCKED_DOORS', ())
    KEY_SPAWNS   = getattr(mod, 'KEY_SPAWNS', ())
    PLAYER_SPAWN = mod.PLAYER_SPAWN
    WORLD_W      = mod.WORLD_W
    WORLD_H      = mod.WORLD_H
    sys.modules.pop(full, None)
    sys.modules.pop('platformer_levels', None)


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
            for bx, by, _, __ in bucket:
                if by == fy and cl < bx + BLOCK_W and cr > bx:
                    return True
    for px, py, pw in PLATFORMS:
        if py == fy and cl < px + pw and cr > px:
            return True
    return False


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
        self.dying     = False
        self.anim_frame = 0
        self.anim_timer = 0.0
        self.burst_frame = 0
        self.burst_timer = 0.0
        self.hit_timer  = 0.0   # > 0: show fill frame (white flash)
        self.dir_timer  = 1.0 + random.random() * 2.0


class PlatformerScene(Scene):

    def enter(self):
        load_level(getattr(self, '_current_level', 'level_01'))

        # Camera and kill-zone limits derived from loaded world size
        self._cam_x_max = max(0, WORLD_W - 128)
        self._cam_y_min = 0
        self._cam_y_max = max(0, WORLD_H - 64)
        self._kill_y    = WORLD_H + 24

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
        px, py = PLAYER_SPAWN
        self.x        = float(px)
        self.feet_y   = float(py)
        self.vx       = 0.0
        self.vy       = 0.0
        self.on_ground    = True
        self.just_landed  = False
        self.facing_right = True

        self._on_platform    = -1   # platform index cat stands on (-1 = solid/none)
        self._drop_platform  = -1   # platform index being dropped through
        self._can_double_jump = False  # recharged on each ground touch

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

        # Poof death animation
        self._poof_active = False
        self._poof_x      = 0
        self._poof_y      = 0
        self._poof_frame  = 0
        self._poof_timer  = 0.0

        # Checkpoint state
        self._checkpoint = PLAYER_SPAWN
        self._checkpoint_activated = [False] * len(CHECKPOINTS)

        # Door transition state
        self._door_dest        = None   # destination level name while sequencing
        self._door_walk_timer  = 0.0   # walk-in delay countdown
        self._door_fade_phase  = None  # None | 'out' | 'in'
        self._door_fade_prog   = 0.0

        # Key collectibles
        self._key_active = [True] * len(KEY_SPAWNS)
        self._has_key    = False
        self._key_timer  = 0.0

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
        # Door fade-out phase: freeze the game and darken the screen
        if self._door_fade_phase == 'out':
            self._door_fade_prog += dt / DOOR_FADE_DURATION
            if self._door_fade_prog >= 1.0:
                dest = self._door_dest
                self._transition_to_level(dest)   # calls exit() + enter(), resets state
                self._door_fade_phase = 'in'
                self._door_fade_prog  = 0.0
                self._door_dest       = dest      # restore after enter() cleared it
            return

        # Door fade-in phase: new level is loaded, reveal it
        if self._door_fade_phase == 'in':
            self._door_fade_prog += dt / DOOR_FADE_DURATION
            if self._door_fade_prog >= 1.0:
                self._door_fade_phase = None
                self._door_dest       = None
            return

        # Walk-in delay: cat continues moving, countdown until fade starts
        if self._door_dest is not None:
            self._door_walk_timer -= dt
            if self._door_walk_timer <= 0.0:
                self._door_fade_phase = 'out'
                self._door_fade_prog  = 0.0
            # Fall through — let physics and movement keep running

        if self._poof_active:
            self._poof_timer += dt
            if self._poof_timer >= POOF_SPF:
                self._poof_timer -= POOF_SPF
                self._poof_frame += 1
            if self._poof_frame >= len(POOF["frames"]):
                self._poof_active = False
                self._respawn_cat()
            return

        self.just_landed = False

        # Walk-off-edge detection: if nothing beneath feet, start falling
        if self.on_ground and not self._is_supported():
            self.on_ground    = False
            self._on_platform = -1

        # Vertical physics + collision first — so feet_y is correct before _resolve_x
        # evaluates the cat's vertical bounds (prevents ceiling blocks being mistaken
        # for walls when the cat's head briefly overlaps them during ascent)
        if not self.on_ground:
            self.vy += GRAVITY * dt
            prev_feet = self.feet_y
            self.feet_y += self.vy * dt
            self._resolve_y(prev_feet)

        # Horizontal movement + collision
        self.x += self.vx * dt
        if self.x < CAT_HALF_W:
            self.x = float(CAT_HALF_W)
        elif self.x > WORLD_W - CAT_HALF_W:
            self.x = float(WORLD_W - CAT_HALF_W)
        self._resolve_x()

        # Recharge double jump on landing
        if self.just_landed:
            self._can_double_jump = DOUBLE_JUMP_ENABLED

        # Fell through a gap — counts as a death
        if self.feet_y > self._kill_y:
            self._respawn_cat()
            return

        self._key_timer += dt
        self._check_checkpoints()
        self._check_key_pickup()
        self._check_doors()

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
        if slime.dying:
            slime.burst_timer += dt
            if slime.burst_timer >= SLIME_BURST_SPF:
                slime.burst_timer -= SLIME_BURST_SPF
                slime.burst_frame += 1
            if slime.burst_frame >= len(PLATFORMER_SLIME_BURST["frames"]):
                slime.alive = False
            return

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
        reach = RUN_ATK_REACH if self._swipe_is_run else ATK_REACH
        if self.facing_right:
            atk_cl = int(self.x) + CAT_H          # body right edge
            atk_cr = atk_cl + reach
        else:
            atk_cr = int(self.x) - CAT_H          # body left edge
            atk_cl = atk_cr - reach

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
                slime.dying = True

    def _check_slime_cat_contact(self):
        """If any live slime touches the cat, deal 1 damage and knock the cat back."""
        if self._cat_blink_timer > 0:
            return  # invincible during blink

        ccl = int(self.x) - CAT_HALF_W
        ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)

        for slime in self._slimes:
            if not slime.alive or slime.dying:
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
                self._start_poof()
            break  # one slime contact per frame is enough

    def _start_poof(self):
        self._poof_active = True
        self._poof_x      = int(self.x)
        self._poof_y      = int(self.feet_y)
        self._poof_frame  = 0
        self._poof_timer  = 0.0
        self.vx = 0.0
        self.vy = 0.0

    def _respawn_cat(self):
        px, py = self._checkpoint
        self.x        = float(px)
        self.feet_y   = float(py)
        self.vx       = 0.0
        self.vy       = 0.0
        self.on_ground    = True
        self._on_platform = -1
        self._drop_platform = -1
        self.facing_right = True
        # Point target_cam_x at the checkpoint so the lerp goes there in X.
        # Without this, _update_camera only updates target_cam_x when the cat
        # exceeds the scroll thresholds in the direction they're facing, so a
        # checkpoint to the left of the camera would never pull the camera back.
        self.target_cam_x = max(float(CAM_X_MIN),
                                min(float(px) - RIGHT_SCROLL_PX, self._cam_x_max))
        self._cat_hp          = CAT_START_HP
        self._cat_blink_timer = CAT_BLINK_DUR
        self._swipe_frame     = -1
        self.anim_frame       = 0
        self._can_double_jump = DOUBLE_JUMP_ENABLED

    def _transition_to_level(self, name):
        self.exit()
        self._current_level = name
        self.enter()

    def _check_checkpoints(self):
        ccl = int(self.x) - CAT_HALF_W
        ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)
        for i, (cx, cy) in enumerate(CHECKPOINTS):
            if self._checkpoint_activated[i]:
                continue
            if ccl >= cx + CHECKPOINT_W or ccr <= cx:
                continue
            if cct >= cy or ccb <= cy - CHECKPOINT_H:
                continue
            self._checkpoint = (cx + BLOCK_W // 2, cy)
            self._checkpoint_activated[i] = True

    def _check_key_pickup(self):
        if not any(self._key_active):
            return
        ccl = int(self.x) - CAT_HALF_W
        ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)
        kw = KEY["width"]
        kh = KEY["height"]
        for i, (kx, ky) in enumerate(KEY_SPAWNS):
            if not self._key_active[i]:
                continue
            if ccl >= kx + kw // 2 or ccr <= kx - kw // 2:
                continue
            if cct >= ky or ccb <= ky - kh:
                continue
            self._key_active[i] = False
            self._has_key = True
            return

    def _check_doors(self):
        if self._door_dest is not None:
            return  # already sequencing a transition
        ccl = int(self.x) - CAT_HALF_W
        ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)
        for cx, cy, dest in DOORS:
            if ccl >= cx + DOOR_W or ccr <= cx:
                continue
            if cct >= cy or ccb <= cy - DOOR_H:
                continue
            self._door_dest       = dest
            self._door_walk_timer = DOOR_WALK_DELAY
            return
        if self._has_key:
            for cx, cy, dest in LOCKED_DOORS:
                if ccl >= cx + DOOR_W or ccr <= cx:
                    continue
                if cct >= cy or ccb <= cy - DOOR_H:
                    continue
                self._door_dest       = dest
                self._door_walk_timer = DOOR_WALK_DELAY
                return

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
                for bx, by, _, __ in bucket:
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
                for bx, by, _, __ in bucket:
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
                    for bx, by, _, __ in bucket:
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
                    for bx, by, _, __ in bucket:
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
        elif self.target_cam_x > self._cam_x_max:
            self.target_cam_x = float(self._cam_x_max)
        if self.target_cam_y < self._cam_y_min:
            self.target_cam_y = float(self._cam_y_min)
        elif self.target_cam_y > self._cam_y_max:
            self.target_cam_y = float(self._cam_y_max)

        self.camera_x += (self.target_cam_x - self.camera_x) * CAM_LERP * dt
        self.camera_y += (self.target_cam_y - self.camera_y) * CAM_LERP * dt

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self._poof_active or self._door_fade_phase is not None:
            return

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

        if self.input.was_just_pressed('a'):
            if self.on_ground and not self.just_landed:
                self.vy = JUMP_VEL
                self.on_ground    = False
                self._on_platform = -1
                self.anim_frame   = 0
                self.anim_timer   = 0.0
            elif self._can_double_jump:
                self.vy = JUMP_VEL
                self._can_double_jump = False
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
                for bx, by, tt, vi in bucket:
                    sx = bx - cam_x
                    sy = by - cam_y
                    if -BLOCK_W < sx < 128 and -BLOCK_H < sy < 64:
                        tile = TERRAIN_TILES[tt][vi]
                        self.renderer.draw_sprite(tile["frames"][0], BLOCK_W, BLOCK_H, sx, sy)

        # Platforms — 13 total, cheap to iterate flat
        for px, py, pw in PLATFORMS:
            sx = px - cam_x
            sy = py - cam_y
            if -pw < sx < 128 and -PLAT_H < sy < 64:
                self.renderer.draw_rect(sx, sy, pw, PLAT_H, color=1)

        # Grass decorations — chunk-culled, drawn above terrain
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                bucket = GRASS_CHUNKS.get((col, row))
                if not bucket:
                    continue
                for wx, surface_y, si in bucket:
                    sprite = GRASS_SPRITES[si]
                    sw = sprite["width"]
                    sh = sprite["height"]
                    gx = wx - sw // 2 - cam_x
                    gy = surface_y - sh - cam_y
                    self.renderer.draw_sprite(sprite["frames"][0], sw, sh, gx, gy)

        # Checkpoints
        for i, (cx, cy) in enumerate(CHECKPOINTS):
            if self._checkpoint_activated[i]:
                sp = PLATFORMER_CHECKPOINT_UP
            else:
                sp = PLATFORMER_CHECKPOINT_DOWN
            sw = sp["width"]
            sh = sp["height"]
            draw_x = cx - cam_x
            draw_y = cy - sh - cam_y
            if -sw < draw_x < 128 and -sh < draw_y < 64:
                self.renderer.draw_sprite(sp["frames"][0], sw, sh, draw_x, draw_y)

        # Doors
        dw = PLATFORMER_DOOR["width"]
        dh = PLATFORMER_DOOR["height"]
        for cx, cy, _ in DOORS:
            draw_x = cx - cam_x
            draw_y = cy - dh - cam_y
            if -dw < draw_x < 128 and -dh < draw_y < 64:
                self.renderer.draw_sprite(PLATFORMER_DOOR["frames"][0], dw, dh, draw_x, draw_y)

        # Locked doors — show locked sprite until key obtained, then unlocked sprite
        locked_door_sprite = PLATFORMER_DOOR if self._has_key else PLATFORMER_DOOR_LOCKED
        for cx, cy, _ in LOCKED_DOORS:
            draw_x = cx - cam_x
            draw_y = cy - dh - cam_y
            if -dw < draw_x < 128 and -dh < draw_y < 64:
                self.renderer.draw_sprite(locked_door_sprite["frames"][0], dw, dh, draw_x, draw_y)

        # Key collectibles — triangle-wave bob
        kw = KEY["width"]
        kh = KEY["height"]
        t = self._key_timer % KEY_BOB_PERIOD
        half = KEY_BOB_PERIOD * 0.5
        bob = int(t / half * KEY_BOB_AMP) if t < half else int((KEY_BOB_PERIOD - t) / half * KEY_BOB_AMP)
        for i, (kx, ky) in enumerate(KEY_SPAWNS):
            if not self._key_active[i]:
                continue
            draw_x = kx - kw // 2 - cam_x
            draw_y = ky - kh - bob - cam_y
            if -kw < draw_x < 128 and -kh < draw_y < 64:
                self.renderer.draw_sprite(KEY["frames"][0], kw, kh, draw_x, draw_y)

        # Slimes — only those in visible chunks
        sw = PLATFORMER_SLIME_IDLE["width"]
        sh = PLATFORMER_SLIME_IDLE["height"]
        bw = PLATFORMER_SLIME_BURST["width"]
        bh = PLATFORMER_SLIME_BURST["height"]
        for slime in self._slimes:
            if not slime.alive:
                continue
            if not (col0 <= slime.chunk_col <= col1):
                continue
            if slime.dying:
                fi = slime.burst_frame
                sx = int(slime.x) - bw // 2 - cam_x
                sy = int(slime.feet_y) - bh - cam_y
                self.renderer.draw_sprite(PLATFORMER_SLIME_BURST["frames"][fi], bw, bh, sx, sy)
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

        # Poof death animation
        if self._poof_active:
            pw = POOF["width"]
            ph = POOF["height"]
            fi = min(self._poof_frame, len(POOF["frames"]) - 1)
            self.renderer.draw_sprite(POOF["frames"][fi], pw, ph,
                                      self._poof_x - pw // 2 - cam_x,
                                      self._poof_y - ph - cam_y)

        # Cat sprite — invisible on even blink intervals while damaged
        if self._poof_active:
            cat_visible = False
        elif self._cat_blink_timer > 0:
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

        # Door transition scanline overlay — drawn last so it covers everything
        if self._door_fade_phase is not None:
            if self._door_fade_phase == 'out':
                progress = self._door_fade_prog
            else:
                progress = 1.0 - self._door_fade_prog
            passes = int(progress * 8) + 1
            disp = self.renderer.display
            if passes >= 8:
                disp.fill(0)
            else:
                for offset in range(passes):
                    for y in range(offset, 64, 8):
                        disp.hline(0, y, 128, 0)
