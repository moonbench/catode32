from lang import t
"""
Prowl - platformer minigame
Character controller: walk, jump, solid blocks, one-way platforms, camera scrolling.
Combat: cat swipe attack, slime enemies.
"""
import random
import struct
from scene import Scene
from sprite_transform import mirror_sprite_h
from assets.platformer_terrain import (
    TERRAIN_TILES,
    PLATFORMER_CHECKPOINT_DOWN,
    PLATFORMER_CHECKPOINT_UP,
    PLATFORMER_DOOR,
    PLATFORMER_DOOR_LOCKED,
    PLATFORMER_BG_TILES,
)
from assets.items import BANDAGE, KEY, SPIN_COIN
import assets.platformer_levels as _platformer_levels
from assets.plants import (
    GRASS_SEEDLING,
    GRASS_YOUNG,
    GRASS_GROWING,
    GRASS_MATURE,
    GRASS_THRIVING,
)
from assets.effects import POOF
from ui import BurstEffect
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
IDLE_FPS        = 4
RUN_FPS         = 12
JUMP_PEAK_RANGE = 70

# Cat combat
CAT_START_HP     = 2
CAT_BLINK_DUR    = 1.5    # invincibility + blink duration after taking damage
CAT_BLINK_INT    = 0.1    # visibility toggle interval while blinking
CAT_KNOCKBACK_VX = 100.0
CAT_KNOCKBACK_VY = -100.0

# Swipe attack
SWIPE_FPS         = 12   # frames per second for swipe animation
ATTACK_FRAME      = 2    # frame index (0-based) that deals damage
SIT_SWIPE_FRAMES  = 5    # total frames in PLATFORMER_CAT_SIT_SWIPE
RUN_SWIPE_FRAMES  = 3    # total frames in PLATFORMER_CAT_RUN_SWIPE
ATK_REACH     = 16         # px of reach ahead of the cat body edge (standing swipe)
RUN_ATK_REACH = 28         # px of reach for run/jump swipe
STRIKE_VX          = 55   # px/s the strike effect drifts forward
RUN_STRIKE_OFFSET  = 10   # extra px ahead for the run/jump strike spawn

# Slime
SLIME_SPEED          = 18
SLIME_HALF_W         = 8
SLIME_H              = 8
SLIME_START_HP       = 2
SLIME_HIT_FLASH      = 0.15   # seconds to show fill frame after being hit
SLIME_ANIM_SPF       = 0.5    # seconds per frame (2 FPS)
SLIME_BURST_SPF      = 1.0 / 12   # seconds per frame for death burst
SLIME_PATROL_RADIUS  = 84
# Minimum inset from chunk edge for slime patrol bounds. Guarantees that all
# edge-detection and wall-collision probes stay within the slime's spawn chunk,
# so physics never needs to look up blocks outside the cached viewport.
# Must be ≥ SLIME_HALF_W + BLOCK_W + 2 + SLIME_SPEED/FPS ≈ 19.5  →  use 20.
_SLIME_INSET = SLIME_HALF_W + BLOCK_W + 4

# Poof death animation
POOF_SPF = 1.0 / 8   # POOF["speed"] = 8

# Level start banner
LEVEL_BANNER_DUR  = 2.5   # seconds the banner is visible
LEVEL_BANNER_RISE = 16    # pixels it rises over its lifetime

# Collectible items (keys and coins)
KEY_BOB_PERIOD  = 1.2   # seconds per full oscillation cycle
KEY_BOB_AMP     = 6     # pixels of vertical travel
COIN_BOB_PERIOD = 1.2
COIN_BOB_AMP    = 4
COIN_ANIM_SPF   = 0.2   # 5 FPS spin

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

# Precomputed draw lookups — avoids dict key access inside hot draw loops.
# These are just references to existing frame bytes objects; no copies are made.
_TERRAIN_FRAMES = [[t["frames"][0] for t in variants] for variants in TERRAIN_TILES]
_BG_FRAMES      = [[t["frames"][0] for t in variants] for variants in PLATFORMER_BG_TILES]
_GRASS_DATA     = tuple((s["frames"][0], s["width"], s["height"], s["width"] // 2) for s in GRASS_SPRITES)

# Precomputed checkpoint and door draw data — avoids dict lookups inside the draw loop.
_CP_DOWN_FRAME = PLATFORMER_CHECKPOINT_DOWN["frames"][0]
_CP_DOWN_W     = PLATFORMER_CHECKPOINT_DOWN["width"]
_CP_DOWN_H     = PLATFORMER_CHECKPOINT_DOWN["height"]
_CP_UP_FRAME   = PLATFORMER_CHECKPOINT_UP["frames"][0]
_CP_UP_W       = PLATFORMER_CHECKPOINT_UP["width"]
_CP_UP_H       = PLATFORMER_CHECKPOINT_UP["height"]
_DOOR_FRAME        = PLATFORMER_DOOR["frames"][0]
_DOOR_LOCKED_FRAME = PLATFORMER_DOOR_LOCKED["frames"][0]
_DOOR_W            = PLATFORMER_DOOR["width"]
_DOOR_H            = PLATFORMER_DOOR["height"]

# ── Level data globals ────────────────────────────────────────────────────────
# _LEVEL_DATA holds a reference to the frozen bytes object (lives in flash).
# The four INDEX dicts map (chunk_col, chunk_row) → (byte_offset, block_count)
# into _LEVEL_DATA, so block data is unpacked on demand rather than pre-parsed.
# PLATFORMS/CHECKPOINTS/DOORS/LOCKED_DOORS are small and iterated every frame,
# so they remain fully parsed tuples.
# _SLIME_SEC/_KEY_SEC/_COIN_SEC are (offset, count) pairs consumed once during
# _init_level_state() to build the per-session instance structures.
_LEVEL_DATA  = None

SOLID_INDEX  = {}
BG_INDEX     = {}
GRASS_INDEX  = {}

PLATFORMS    = ()
CHECKPOINTS  = ()
DOORS        = ()
LOCKED_DOORS = ()

_SLIME_SEC   = (0, 0)
_KEY_SEC     = (0, 0)
_COIN_SEC    = (0, 0)

PLAYER_SPAWN = (8, 8)
WORLD_W      = 128
WORLD_H      = 64


def load_level(name):
    """Build offset indices into a frozen level bytes object.
    Chunk block data (solid terrain, BG, grass, vines) is NOT pre-parsed —
    only (byte_offset, count) pairs are stored per chunk so that collision and
    draw loops can call struct.unpack_from on demand directly from flash.
    PLATFORMS, CHECKPOINTS, DOORS and LOCKED_DOORS are small and iterated every
    frame so they remain fully parsed.  Spawn sections (slimes, keys, coins) are
    recorded as (offset, count) pairs and consumed once by _init_level_state().
    See tools/convert_level.py for the binary format and
    tools/build_levels.py to regenerate assets/platformer_levels.py."""
    global _LEVEL_DATA, SOLID_INDEX, BG_INDEX, GRASS_INDEX
    global PLATFORMS, CHECKPOINTS, DOORS, LOCKED_DOORS
    global _SLIME_SEC, _KEY_SEC, _COIN_SEC, PLAYER_SPAWN, WORLD_W, WORLD_H
    # Drop all references to old level data before touching the new level.
    # Sprite caches (_TERRAIN_FRAMES etc.) are left alone — identical across levels.
    _LEVEL_DATA  = None
    SOLID_INDEX  = None; BG_INDEX     = None
    GRASS_INDEX  = None
    PLATFORMS    = None; CHECKPOINTS  = None
    DOORS        = None; LOCKED_DOORS = None
    _SLIME_SEC   = (0, 0); _KEY_SEC   = (0, 0); _COIN_SEC = (0, 0)
    import gc; gc.collect()

    data   = getattr(_platformer_levels, name)
    offset = 0

    # Header: version(B) WORLD_W(H) WORLD_H(H) SPAWN_X(H) SPAWN_Y(H)
    _ver, world_w, world_h, sx, sy = struct.unpack_from('<BHHHH', data, offset)
    offset += 9
    WORLD_W      = world_w
    WORLD_H      = world_h
    PLAYER_SPAWN = (sx, sy)

    solid_idx = {}
    bg_idx    = {}
    grass_idx = {}
    cps       = []
    doors     = []
    ldoors    = []
    plats     = []

    while offset < len(data):
        sec_id, count = struct.unpack_from('<BH', data, offset)
        offset += 3

        if sec_id == 0x01:    # SLIME_SPAWNS — record location, parse later
            _SLIME_SEC = (offset, count)
            offset += 4 * count

        elif sec_id == 0x02:  # SOLID_CHUNKS — store offset of first block per chunk
            for _ in range(count):
                col, row, n = struct.unpack_from('<BBB', data, offset)
                offset += 3
                solid_idx[(col, row)] = (offset, n)
                offset += 6 * n

        elif sec_id == 0x03:  # BG_CHUNKS
            for _ in range(count):
                col, row, n = struct.unpack_from('<BBB', data, offset)
                offset += 3
                bg_idx[(col, row)] = (offset, n)
                offset += 6 * n

        elif sec_id == 0x04:  # GRASS_CHUNKS
            for _ in range(count):
                col, row, n = struct.unpack_from('<BBB', data, offset)
                offset += 3
                grass_idx[(col, row)] = (offset, n)
                offset += 5 * n

        elif sec_id == 0x05:  # VINE_CHUNKS — removed, skip bytes
            for _ in range(count):
                n = struct.unpack_from('<BBB', data, offset)[2]
                offset += 3 + 4 * n

        elif sec_id == 0x06:  # CHECKPOINTS — small, parse fully
            for i in range(count):
                cps.append(struct.unpack_from('<HH', data, offset + i * 4))
            offset += 4 * count

        elif sec_id == 0x07:  # PLATFORMS — small, parse fully
            for i in range(count):
                plats.append(struct.unpack_from('<HHH', data, offset + i * 6))
            offset += 6 * count

        elif sec_id == 0x08:  # DOORS — small, parse fully
            for _ in range(count):
                x, y, dlen = struct.unpack_from('<HHB', data, offset)
                offset += 5
                doors.append((x, y, data[offset:offset + dlen].decode()))
                offset += dlen

        elif sec_id == 0x09:  # LOCKED_DOORS — small, parse fully
            for _ in range(count):
                x, y, dlen = struct.unpack_from('<HHB', data, offset)
                offset += 5
                ldoors.append((x, y, data[offset:offset + dlen].decode()))
                offset += dlen

        elif sec_id == 0x0A:  # KEY_SPAWNS — record location, parse later
            _KEY_SEC = (offset, count)
            offset += 4 * count

        elif sec_id == 0x0B:  # COIN_SPAWNS — record location, parse later
            _COIN_SEC = (offset, count)
            offset += 4 * count

    _LEVEL_DATA  = data
    SOLID_INDEX  = solid_idx;  solid_idx = None
    BG_INDEX     = bg_idx;     bg_idx    = None
    GRASS_INDEX  = grass_idx;  grass_idx = None
    PLATFORMS    = tuple(plats);   plats  = None
    CHECKPOINTS  = tuple(cps);     cps    = None
    DOORS        = tuple(doors);   doors  = None
    LOCKED_DOORS = tuple(ldoors);  ldoors = None


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
        # feet_y never changes (slimes don't jump/fall), so cache all row lookups once.
        _ify = int(feet_y)
        self.ify        = _ify                              # int(feet_y)
        self.ground_row = _ify // CHUNK_H                  # row for edge-detection probe
        self.wall_row0  = (_ify - SLIME_H - BLOCK_H + 1) // CHUNK_H   # top of slime body
        self.wall_row1  = (_ify - 1) // CHUNK_H            # bottom of slime body
        # Patrol range: ±SLIME_PATROL_RADIUS from spawn, clamped so all edge/wall
        # probes stay within the spawn chunk (see _SLIME_INSET for the math).
        cx_min = float(self.chunk_col * CHUNK_W + _SLIME_INSET)
        cx_max = float((self.chunk_col + 1) * CHUNK_W - _SLIME_INSET)
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
        level = getattr(self, '_current_level', 'level_01')
        self._current_level = level
        load_level(level)
        self._load_sprites()
        self._init_level_state()
        self._coins_collected = 0
        self._session_slimes_killed = 0
        self._session_levels_completed = 0

    def _load_sprites(self):
        """Allocate precomputed mirrored sprite frames. Called once per platformer
        session — kept alive across level transitions to avoid heap fragmentation."""
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

    def _init_level_state(self):
        """Reset all per-level state from the currently loaded level globals.
        Called on initial enter and on every level transition."""
        # Camera and kill-zone limits derived from loaded world size
        self._cam_x_max = max(0, WORLD_W - 128)
        self._cam_y_min = 0
        self._cam_y_max = max(0, WORLD_H - 64)
        self._kill_y    = WORLD_H + 24

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

        # Key collectibles — count driven by _KEY_SEC, data read from _LEVEL_DATA
        _koff, _kn = _KEY_SEC
        self._key_active     = [True] * _kn
        self._has_key        = False
        self._key_timer      = 0.0
        self._keys_remaining = _kn

        # Coin collectibles — count driven by _COIN_SEC
        _coff, _cn = _COIN_SEC
        self._coin_active       = [True] * _cn
        self._coin_anim_frame   = 0
        self._coin_anim_timer   = 0.0
        self._coins_remaining   = _cn

        # Enemies — read spawn positions from _LEVEL_DATA, index by chunk column
        slimes_by_chunk = {}
        off, n = _SLIME_SEC
        for i in range(n):
            x, fy = struct.unpack_from('<HH', _LEVEL_DATA, off + i * 4)
            col = int(x) // CHUNK_W
            slimes_by_chunk.setdefault(col, []).append(Slime(x, fy))
        self._slimes_by_chunk = slimes_by_chunk

        # Collectibles — read from _LEVEL_DATA, index by chunk column
        key_chunk = {}
        for i in range(_kn):
            kx, ky = struct.unpack_from('<HH', _LEVEL_DATA, _koff + i * 4)
            key_chunk.setdefault(kx // CHUNK_W, []).append((i, kx, ky))
        self._key_chunk = key_chunk

        coin_chunk = {}
        for i in range(_cn):
            cx, cy = struct.unpack_from('<HH', _LEVEL_DATA, _coff + i * 4)
            coin_chunk.setdefault(cx // CHUNK_W, []).append((i, cx, cy))
        self._coin_chunk = coin_chunk

        # Per-level stats
        self._level_num             = int(self._current_level.split('_')[-1])
        self._injuries              = 0
        self._slimes_killed         = 0
        self._total_slimes          = _SLIME_SEC[1]
        self._total_coins           = _COIN_SEC[1]
        self._level_coins_collected = 0
        self._level_time            = 0.0
        self._banner_timer          = 0.0
        self._summary_slime_frame   = 0
        self._summary_slime_timer   = 0.0
        self._level_flawless        = False
        self._fireworks_count       = 0
        self._fireworks_timer       = 0.0
        self._burst_effect          = BurstEffect()   # summary screen (screen coords)

        # Camera: world coord of top-left of screen — snapped to player spawn
        # so there's no lerp from the top-left corner on entry.
        init_cam_x = max(float(CAM_X_MIN),
                         min(float(px) - RIGHT_SCROLL_PX, float(self._cam_x_max)))
        init_cam_y = max(float(self._cam_y_min),
                         min(float(py) - BOT_SCROLL_PX, float(self._cam_y_max)))
        self.camera_x      = init_cam_x
        self.camera_y      = init_cam_y
        self.target_cam_x  = init_cam_x
        self.target_cam_y  = init_cam_y

        # Solid-terrain cache: (bx, by, frame) tuples per chunk, used by both draw
        # and physics. Pre-built here so physics can use it from the very first frame.
        # Rebuilt in draw() when the visible chunk set changes.
        _cx0 = int(init_cam_x); _cy0 = int(init_cam_y)
        _vc = (_cx0 // CHUNK_W, (_cx0 + 127) // CHUNK_W,
               _cy0 // CHUNK_H, (_cy0 + 63) // CHUNK_H)
        self._solid_cache_vis = _vc
        self._solid_cache     = {}
        self._bg_cache        = {}
        self._grass_cache     = {}
        self._rebuild_solid_cache(_vc[0], _vc[1], _vc[2], _vc[3])

    def exit(self):
        coins  = getattr(self, '_coins_collected', 0)
        levels = getattr(self, '_session_levels_completed', 0)
        slimes = getattr(self, '_session_slimes_killed', 0)
        if coins > 0:
            self.context.coins += coins
            print(f"[Platformer] Awarded {coins} coins (total: {self.context.coins})")
        if levels > 0 or slimes > 0:
            lp = (levels / 5.0) ** 0.5
            sp = (slimes / 62.0) ** 0.5
            print(f"[Platformer] levels={levels} lp={lp:.2f}  slimes={slimes} sp={sp:.2f}")
            self.context.apply_stat_changes({
                'fitness':     3 * lp,
                'fulfillment': 2 * lp,
                'playfulness': 3 * lp,
                'courage':     2 * sp,
            })
        self._run_r   = self._run_l   = None
        self._sit_r   = self._sit_l   = None
        self._jump_r  = self._jump_l  = None
        self._swipe_r      = self._swipe_l      = None
        self._run_swipe_r  = self._run_swipe_l  = None
        self._strike_r = self._strike_l = None
        self._slime_r = self._slime_l = None
        self._slime_fill_r = self._slime_fill_l = None
        self._slime_fill_inv_r = self._slime_fill_inv_l = None
        self._slimes_by_chunk = None
        self._key_chunk = None
        self._coin_chunk = None

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        # Door fade-out phase: freeze the game and darken the screen
        if self._door_fade_phase == 'out':
            self._door_fade_prog += dt / DOOR_FADE_DURATION
            if self._door_fade_prog >= 1.0:
                # Flush a fully-black frame so the display doesn't hold the last
                # partially-faded image while we show the summary screen.
                self.renderer.clear()
                self.renderer.show()
                # Flawless bonus: 1.5× coins towards the session total
                _flawless = (self._injuries == 0
                             and self._slimes_killed == self._total_slimes
                             and self._level_coins_collected == self._total_coins)
                if _flawless:
                    self._coins_collected += self._level_coins_collected // 2
                self._level_flawless  = _flawless
                self._door_fade_phase = 'summary'
                self._door_fade_prog  = 0.0
            return

        # Summary screen: game is frozen; keep coin and slime icons animated
        if self._door_fade_phase == 'summary':
            self._coin_anim_timer += dt
            if self._coin_anim_timer >= COIN_ANIM_SPF:
                self._coin_anim_timer -= COIN_ANIM_SPF
                self._coin_anim_frame = (self._coin_anim_frame + 1) % len(SPIN_COIN["frames"])
            self._summary_slime_timer += dt
            if self._summary_slime_timer >= SLIME_ANIM_SPF:
                self._summary_slime_timer -= SLIME_ANIM_SPF
                self._summary_slime_frame ^= 1
            if self._level_flawless and self._fireworks_count < 5:
                self._fireworks_timer += dt
                if self._fireworks_count == 0 or self._fireworks_timer >= 0.5:
                    self._fireworks_timer = 0.0
                    self._fireworks_count += 1
                    for _ in range(random.randint(2, 3)):
                        self._burst_effect.trigger(
                            anchor_x=random.randint(15, 113),
                            anchor_y=random.randint(8, 50),
                            count=4, spread_x=8,
                            spread_y_min=-10, spread_y_max=10,
                        )
            self._burst_effect.update(dt)
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

        self._level_time   += dt
        self._banner_timer += dt

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
        self._coin_anim_timer += dt
        if self._coin_anim_timer >= COIN_ANIM_SPF:
            self._coin_anim_timer -= COIN_ANIM_SPF
            self._coin_anim_frame = (self._coin_anim_frame + 1) % len(SPIN_COIN["frames"])
        self._check_checkpoints()
        self._check_item_pickups()
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
        for col in range(cam_col0, cam_col1 + 1):
            chunk = self._slimes_by_chunk.get(col)
            if not chunk:
                continue
            dead = False
            for slime in chunk:
                if slime.alive:
                    self._update_slime(slime, dt)
                    if not slime.alive:
                        dead = True
            if dead:
                self._slimes_by_chunk[col] = [s for s in chunk if s.alive]

        # Check slime-cat contact damage
        self._check_slime_cat_contact()

        self._burst_effect.update(dt)
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

        # Edge detection: probe just past the leading foot edge.
        # feet_y is constant so ground_row and chunk_col are pre-computed on the slime.
        look_x = slime.x + ((SLIME_HALF_W + 2) if slime.vx > 0 else -(SLIME_HALF_W + 2))
        ilook = int(look_x)
        fy = slime.ify
        _sc = self._solid_cache
        has_ground = False
        blocks = _sc.get((slime.chunk_col, slime.ground_row))
        if blocks:
            for bx, by, _ in blocks:
                if by == fy and ilook - 1 < bx + BLOCK_W and ilook + 1 > bx:
                    has_ground = True
                    break
        if not has_ground:
            for px, py, pw in PLATFORMS:
                if py == fy and ilook - 1 < px + pw and ilook + 1 > px:
                    has_ground = True
                    break
        if not has_ground:
            slime.vx = -slime.vx

        # Move with wall collision — always the same chunk column; rows pre-computed.
        next_x = slime.x + slime.vx * dt
        nl = int(next_x) - SLIME_HALF_W
        nr = int(next_x) + SLIME_HALF_W
        st = fy - SLIME_H
        sb = fy
        wall_hit = False
        for row in range(slime.wall_row0, slime.wall_row1 + 1):
            blocks = _sc.get((slime.chunk_col, row))
            if not blocks:
                continue
            for bx, by, _ in blocks:
                if st >= by + BLOCK_H or sb <= by:
                    continue
                if nl >= bx + BLOCK_W or nr <= bx:
                    continue
                next_x = float(bx - SLIME_HALF_W) if slime.vx > 0 else float(bx + BLOCK_W + SLIME_HALF_W)
                slime.vx = -slime.vx
                wall_hit = True
                break
            if wall_hit:
                break

        # Clamp to patrol bounds
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
        # Run/jump swipes spawn the effect further ahead.
        strike_offset = RUN_STRIKE_OFFSET if self._swipe_is_run else 0
        self._strike_active = True
        self._strike_x      = float((atk_cl + strike_offset) if self.facing_right
                                    else (atk_cr - strike_offset))
        self._strike_y      = float(int(self.feet_y) - CAT_H // 2)
        self._strike_vx     = STRIKE_VX if self.facing_right else -STRIKE_VX
        self._strike_right  = self.facing_right
        self._strike_frame  = 0
        self._strike_timer  = 0.0
        atk_ct = int(self.feet_y) - CAT_H
        atk_cb = int(self.feet_y)

        cat_col = int(self.x) // CHUNK_W
        for col in range(cat_col - 1, cat_col + 2):
            for slime in self._slimes_by_chunk.get(col, ()):
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
                slime.hp -= 2 if self._swipe_is_run else 1
                slime.hit_timer = SLIME_HIT_FLASH
                if slime.hp <= 0 and not slime.dying:
                    slime.dying = True
                    self._slimes_killed += 1

    def _check_slime_cat_contact(self):
        """If any live slime touches the cat, deal 1 damage and knock the cat back."""
        if self._cat_blink_timer > 0:
            return  # invincible during blink

        ccl = int(self.x) - CAT_HALF_W
        ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)

        cat_col = int(self.x) // CHUNK_W
        for col in range(cat_col - 1, cat_col + 2):
            for slime in self._slimes_by_chunk.get(col, ()):
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
                return  # one slime contact per frame is enough

    def _start_poof(self):
        self._poof_active = True
        self._poof_x      = int(self.x)
        self._poof_y      = int(self.feet_y)
        self._poof_frame  = 0
        self._poof_timer  = 0.0
        self.vx = 0.0
        self.vy = 0.0

    def _respawn_cat(self):
        self._injuries += 1
        px, py = self._checkpoint
        self.x        = float(px)
        self.feet_y   = float(py)
        self.vx       = 0.0
        self.vy       = 0.0
        self.on_ground    = True
        self._on_platform = -1
        self._drop_platform = -1
        self.facing_right = True
        # Snap camera to checkpoint and immediately rebuild the solid cache.
        # Without the snap, update() on the next frame runs with the cache from
        # the old camera position, so _is_supported() misses and the cat falls
        # through solid ground at the respawn point.
        self.target_cam_x = max(float(CAM_X_MIN),
                                min(float(px) - RIGHT_SCROLL_PX, self._cam_x_max))
        self.target_cam_y = max(float(self._cam_y_min),
                                min(float(py) - BOT_SCROLL_PX, float(self._cam_y_max)))
        self.camera_x = self.target_cam_x
        self.camera_y = self.target_cam_y
        _cx0 = int(self.camera_x); _cy0 = int(self.camera_y)
        _vc = (_cx0 // CHUNK_W, (_cx0 + 127) // CHUNK_W,
               _cy0 // CHUNK_H, (_cy0 + 63) // CHUNK_H)
        if _vc != self._solid_cache_vis:
            self._rebuild_solid_cache(_vc[0], _vc[1], _vc[2], _vc[3])
            self._solid_cache_vis = _vc
        self._cat_hp          = CAT_START_HP
        self._cat_blink_timer = CAT_BLINK_DUR
        self._swipe_frame     = -1
        self.anim_frame       = 0
        self._can_double_jump = DOUBLE_JUMP_ENABLED

    def _transition_to_level(self, name):
        # Sprite frames are intentionally kept alive across transitions — they
        # are the same for every level and freeing/reallocating them each time
        # fragments the heap.  Only level data and game state are reset.
        self._session_slimes_killed += self._slimes_killed
        self._session_levels_completed += 1
        self._current_level = name
        load_level(name)
        self._init_level_state()

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

    def _check_item_pickups(self):
        ccl = int(self.x) - CAT_HALF_W
        ccr = int(self.x) + CAT_HALF_W
        cct = int(self.feet_y) - CAT_H
        ccb = int(self.feet_y)
        cat_col = int(self.x) // CHUNK_W

        if self._keys_remaining:
            kw = KEY["width"]
            kh = KEY["height"]
            for col in range(cat_col - 1, cat_col + 2):
                for i, kx, ky in self._key_chunk.get(col, ()):
                    if not self._key_active[i]:
                        continue
                    if ccl >= kx + kw // 2 or ccr <= kx - kw // 2:
                        continue
                    if cct >= ky or ccb <= ky - kh:
                        continue
                    self._key_active[i] = False
                    self._has_key = True
                    self._keys_remaining -= 1
                    return

        if self._coins_remaining:
            coin_w = SPIN_COIN["width"]
            coin_h = SPIN_COIN["height"]
            for col in range(cat_col - 1, cat_col + 2):
                for i, cx, cy in self._coin_chunk.get(col, ()):
                    if not self._coin_active[i]:
                        continue
                    if ccl >= cx + coin_w // 2 or ccr <= cx - coin_w // 2:
                        continue
                    if cct >= cy or ccb <= cy - coin_h:
                        continue
                    self._coin_active[i] = False
                    self._coins_collected += 1
                    self._level_coins_collected += 1
                    self._coins_remaining -= 1

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
        _sc = self._solid_cache
        for col in range(col0, col1 + 1):
            blocks = _sc.get((col, row))
            if blocks:
                for bx, by, _ in blocks:
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

        _sc = self._solid_cache
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                blocks = _sc.get((col, row))
                if not blocks:
                    continue
                for bx, by, _ in blocks:
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

        _sc = self._solid_cache
        if self.vy >= 0:  # descending
            row0 = int(prev_feet) // CHUNK_H
            row1 = int(self.feet_y) // CHUNK_H
            for col in range(col0, col1 + 1):
                for row in range(row0, row1 + 1):
                    blocks = _sc.get((col, row))
                    if not blocks:
                        continue
                    for bx, by, _ in blocks:
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
                    blocks = _sc.get((col, row))
                    if not blocks:
                        continue
                    for bx, by, _ in blocks:
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
    # Terrain draw cache
    # ------------------------------------------------------------------

    def _rebuild_solid_cache(self, col0, col1, row0, row1):
        """Unpack solid, BG, and grass block data for all visible chunks into
        cached tuples. Reuses existing dicts to avoid heap fragmentation.
        Called only when the visible chunk set changes, not every frame."""
        cache = self._solid_cache
        cache.clear()
        for col in range(col0, col1 + 1):
            for row in range(row0 - 1, row1 + 2):
                entry = SOLID_INDEX.get((col, row))
                if not entry:
                    continue
                off, n = entry
                blocks = [None] * n
                for i in range(n):
                    bx, by, tt, vi = struct.unpack_from('<HHBB', _LEVEL_DATA, off + i * 6)
                    blocks[i] = (bx, by, _TERRAIN_FRAMES[tt][vi])
                cache[(col, row)] = blocks

        bg_cache = self._bg_cache
        bg_cache.clear()
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                entry = BG_INDEX.get((col, row))
                if not entry:
                    continue
                off, n = entry
                blocks = [None] * n
                for i in range(n):
                    wx, wy, gi, vi = struct.unpack_from('<HHBB', _LEVEL_DATA, off + i * 6)
                    blocks[i] = (wx, wy, _BG_FRAMES[gi][vi])
                bg_cache[(col, row)] = blocks

        grass_cache = self._grass_cache
        grass_cache.clear()
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                entry = GRASS_INDEX.get((col, row))
                if not entry:
                    continue
                off, n = entry
                blocks = [None] * n
                for i in range(n):
                    wx, surface_y, si = struct.unpack_from('<HHB', _LEVEL_DATA, off + i * 5)
                    gframe, sw, sh, sw2 = _GRASS_DATA[si]
                    blocks[i] = (wx - sw2, surface_y - sh, gframe, sw, sh)
                grass_cache[(col, row)] = blocks

    # ------------------------------------------------------------------
    # Level summary and banner
    # ------------------------------------------------------------------

    def _draw_level_summary(self):
        r = self.renderer
        mins = int(self._level_time) // 60
        secs = int(self._level_time) % 60
        r.draw_text(f"Level {self._level_num} - {mins}:{secs:02d}", 0, 1)

        # Fixed icon column width (widest icon is 18px); text always starts at TEXT_X
        ICON_W  = 18
        TEXT_X  = 1 + ICON_W + 4   # 23

        # Bandage icon — injuries row
        bh = BANDAGE["height"]   # 18
        r.draw_sprite(BANDAGE["frames"][0], BANDAGE["width"], bh, 1, 12)
        text_y = 12 + (bh - 8) // 2
        r.draw_text(str(self._injuries), TEXT_X, text_y)
        flawless = (self._injuries == 0
                    and self._slimes_killed == self._total_slimes
                    and self._level_coins_collected == self._total_coins)
        if flawless:
            r.draw_text(t("Flawless!"), 128 - 9 * 8, text_y)

        # Slime icon — slimes killed row (draw_sprite_obj handles fill + outline)
        sh = PLATFORMER_SLIME_IDLE["height"]
        r.draw_sprite_obj(PLATFORMER_SLIME_IDLE, 1, 32, frame=self._summary_slime_frame)
        r.draw_text(f"{self._slimes_killed}/{self._total_slimes}", TEXT_X, 32 + (sh - 8) // 2)

        # Coin icon — centered in the icon column so it aligns with the wider icons
        cw = SPIN_COIN["width"]
        coin_x = 1 + (ICON_W - cw) // 2
        r.draw_sprite(SPIN_COIN["frames"][self._coin_anim_frame], cw, SPIN_COIN["height"], coin_x, 44)
        r.draw_text(f"{self._level_coins_collected}/{self._total_coins}", TEXT_X, 44)

        self._burst_effect.draw(r)

    def _draw_level_banner(self):
        prog = min(1.0, self._banner_timer / LEVEL_BANNER_DUR)
        text = f"Level {self._level_num}"
        tw = len(text) * 8
        bx = (128 - tw) // 2
        by = 20 - int(prog * LEVEL_BANNER_RISE)
        self.renderer.draw_text(text, bx, by)

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self._poof_active:
            return

        if self._door_fade_phase == 'summary':
            for btn in ('a', 'b', 'up', 'down', 'left', 'right'):
                if self.input.was_just_pressed(btn):
                    dest = self._door_dest
                    self._transition_to_level(dest)
                    self._door_fade_phase = 'in'
                    self._door_fade_prog  = 0.0
                    self._door_dest       = dest
                    break
            return

        if self._door_fade_phase is not None:
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
        # Summary screen overrides normal drawing — screen is already black
        if self._door_fade_phase == 'summary':
            self._draw_level_summary()
            return

        _ds = self.renderer.draw_sprite
        cam_x = int(self.camera_x)
        cam_y = int(self.camera_y)

        # Background tiles — drawn first, behind everything
        col0 = cam_x // CHUNK_W
        col1 = (cam_x + 127) // CHUNK_W
        row0 = cam_y // CHUNK_H
        row1 = (cam_y + 63) // CHUNK_H
        _bg_cache = self._bg_cache
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                blocks = _bg_cache.get((col, row))
                if not blocks:
                    continue
                for wx, wy, frame in blocks:
                    sx = wx - cam_x
                    sy = wy - cam_y
                    if -BLOCK_W < sx < 128 and -BLOCK_H < sy < 64:
                        _ds(frame, BLOCK_W, BLOCK_H, sx, sy)

        # Solid terrain — rebuild cache only when visible chunks change, then draw from cache
        _vis = (col0, col1, row0, row1)
        if _vis != self._solid_cache_vis:
            self._rebuild_solid_cache(col0, col1, row0, row1)
            self._solid_cache_vis = _vis
        _sc = self._solid_cache
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                blocks = _sc.get((col, row))
                if not blocks:
                    continue
                for bx, by, frame in blocks:
                    sx = bx - cam_x
                    sy = by - cam_y
                    if -BLOCK_W < sx < 128 and -BLOCK_H < sy < 64:
                        _ds(frame, BLOCK_W, BLOCK_H, sx, sy)

        # Platforms and grass — decorations layer
        for px, py, pw in PLATFORMS:
            sx = px - cam_x
            sy = py - cam_y
            if -pw < sx < 128 and -PLAT_H < sy < 64:
                self.renderer.draw_rect(sx, sy, pw, PLAT_H, color=1)

        # Grass decorations — chunk-culled, drawn above terrain
        _grass_cache = self._grass_cache
        for col in range(col0, col1 + 1):
            for row in range(row0, row1 + 1):
                blocks = _grass_cache.get((col, row))
                if not blocks:
                    continue
                for gx_base, gy_base, gframe, sw, sh in blocks:
                    _ds(gframe, sw, sh, gx_base - cam_x, gy_base - cam_y)

        # Checkpoints, doors, collectibles, slimes, cat, effects
        for i, (cx, cy) in enumerate(CHECKPOINTS):
            if self._checkpoint_activated[i]:
                frame, cw, ch = _CP_UP_FRAME, _CP_UP_W, _CP_UP_H
            else:
                frame, cw, ch = _CP_DOWN_FRAME, _CP_DOWN_W, _CP_DOWN_H
            draw_x = cx - cam_x
            draw_y = cy - ch - cam_y
            if -cw < draw_x < 128 and -ch < draw_y < 64:
                _ds(frame, cw, ch, draw_x, draw_y)

        # Doors
        for cx, cy, _ in DOORS:
            draw_x = cx - cam_x
            draw_y = cy - _DOOR_H - cam_y
            if -_DOOR_W < draw_x < 128 and -_DOOR_H < draw_y < 64:
                _ds(_DOOR_FRAME, _DOOR_W, _DOOR_H, draw_x, draw_y)

        # Locked doors — show locked sprite until key obtained, then unlocked sprite
        locked_frame = _DOOR_FRAME if self._has_key else _DOOR_LOCKED_FRAME
        for cx, cy, _ in LOCKED_DOORS:
            draw_x = cx - cam_x
            draw_y = cy - _DOOR_H - cam_y
            if -_DOOR_W < draw_x < 128 and -_DOOR_H < draw_y < 64:
                _ds(locked_frame, _DOOR_W, _DOOR_H, draw_x, draw_y)

        # Collectible items — triangle-wave bob, chunk-culled
        t = self._key_timer % KEY_BOB_PERIOD
        half = KEY_BOB_PERIOD * 0.5
        key_bob = int(t / half * KEY_BOB_AMP) if t < half else int((KEY_BOB_PERIOD - t) / half * KEY_BOB_AMP)
        kw = KEY["width"]
        kh = KEY["height"]
        for col in range(col0, col1 + 1):
            for i, kx, ky in self._key_chunk.get(col, ()):
                if not self._key_active[i]:
                    continue
                draw_x = kx - kw // 2 - cam_x
                draw_y = ky - kh - key_bob - cam_y
                if -kw < draw_x < 128 and -kh < draw_y < 64:
                    _ds(KEY["frames"][0], kw, kh, draw_x, draw_y)

        t = self._key_timer % COIN_BOB_PERIOD
        half = COIN_BOB_PERIOD * 0.5
        coin_bob = int(t / half * COIN_BOB_AMP) if t < half else int((COIN_BOB_PERIOD - t) / half * COIN_BOB_AMP)
        cw = SPIN_COIN["width"]
        ch = SPIN_COIN["height"]
        cf = self._coin_anim_frame
        for col in range(col0, col1 + 1):
            for i, cx, cy in self._coin_chunk.get(col, ()):
                if not self._coin_active[i]:
                    continue
                draw_x = cx - cw // 2 - cam_x
                draw_y = cy - ch - coin_bob - cam_y
                if -cw < draw_x < 128 and -ch < draw_y < 64:
                    _ds(SPIN_COIN["frames"][cf], cw, ch, draw_x, draw_y)

        # Slimes — only those in visible chunks
        sw = PLATFORMER_SLIME_IDLE["width"]
        sh = PLATFORMER_SLIME_IDLE["height"]
        bw = PLATFORMER_SLIME_BURST["width"]
        bh = PLATFORMER_SLIME_BURST["height"]
        for col in range(col0, col1 + 1):
            for slime in self._slimes_by_chunk.get(col, ()):
                if not slime.alive:
                    continue
                if slime.dying:
                    fi = slime.burst_frame
                    sx = int(slime.x) - bw // 2 - cam_x
                    sy = int(slime.feet_y) - bh - cam_y
                    _ds(PLATFORMER_SLIME_BURST["frames"][fi], bw, bh, sx, sy)
                    continue
                sx = int(slime.x) - sw // 2 - cam_x
                sy = int(slime.feet_y) - sh - cam_y
                facing_right = slime.vx >= 0
                fi = slime.anim_frame
                if slime.hit_timer > 0:
                    # Hit flash: solid white blob
                    data = self._slime_fill_r[fi] if facing_right else self._slime_fill_l[fi]
                    _ds(data, sw, sh, sx, sy)
                else:
                    # Normal: black silhouette first, then white outline on top
                    fill = self._slime_fill_inv_r[fi] if facing_right else self._slime_fill_inv_l[fi]
                    _ds(fill, sw, sh, sx, sy, transparent_color=1)
                    outline = self._slime_r[fi] if facing_right else self._slime_l[fi]
                    _ds(outline, sw, sh, sx, sy)

        # Strike effect — independent frame counter, always plays all 3 frames
        if self._strike_active:
            stw = PLATFORMER_STRIKE["width"]
            sth = PLATFORMER_STRIKE["height"]
            data = (self._strike_r[self._strike_frame] if self._strike_right
                    else self._strike_l[self._strike_frame])
            sx = int(self._strike_x) - stw // 2 - cam_x
            sy = int(self._strike_y) - sth // 2 - cam_y
            _ds(data, stw, sth, sx, sy)

        # Poof death animation
        if self._poof_active:
            pw = POOF["width"]
            ph = POOF["height"]
            fi = min(self._poof_frame, len(POOF["frames"]) - 1)
            _ds(POOF["frames"][fi], pw, ph,
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
            _ds(data, sprite["width"], sprite["height"], draw_x, draw_y)

        # Level start banner — centered on screen, rises and disappears
        if self._banner_timer < LEVEL_BANNER_DUR:
            self._draw_level_banner()

        self._burst_effect.draw(self.renderer)

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
