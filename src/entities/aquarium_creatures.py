"""FishEntity and OctopusEntity for the aquarium vacation scene.

These are NOT registered with environment.add_entity() — they are drawn
manually via a LAYER_MIDGROUND custom draw so they use midground (0.6x)
parallax, matching the tank windows they live inside.
"""

import random
import math

# ── Fish state constants ──────────────────────────────────────────────────────
# Frame layout (5 frames, same for all fish):
#   0         = swimming left
#   1, 2, 3   = turn transition
#   4 (last)  = swimming right
_ST_LEFT   = 0   # swimming left,  show frame 0
_ST_TURN_R = 1   # turning right,  animate frames 1 → 2 → 3
_ST_RIGHT  = 2   # swimming right, show last frame
_ST_TURN_L = 3   # turning left,   animate frames 3 → 2 → 1

_TURN_FRAME_TIME = 0.12   # seconds per intermediate turn frame (3 frames = 0.36 s turn)


class FishEntity:
    """A fish that swims left/right inside the aquarium tanks.

    Turning is animated through the intermediate frames. Y position drifts
    slowly to a new random depth every so often.
    """

    def __init__(self, sprite, x, y, speed=18.0,
                 bounds_left=6, bounds_right=249,
                 bounds_top=8, bounds_bottom=42):
        self.x = float(x)
        self.y = float(y)
        self._sprite  = sprite
        self._frames  = sprite["frames"]
        self._last_fr = len(self._frames) - 1  # 4

        # Start facing a random direction
        if random.random() < 0.5:
            self._state        = _ST_LEFT
            self._current_frame = 0
        else:
            self._state        = _ST_RIGHT
            self._current_frame = self._last_fr

        self._turn_idx   = 0
        self._turn_timer = 0.0

        self._speed      = speed + random.uniform(-3.0, 3.0)
        self._swim_timer = random.uniform(1.5, 6.0)   # seconds until next turn

        self._target_y      = float(y)
        self._y_drift_timer = random.uniform(4.0, 14.0)
        self._y_speed       = 6.0

        self.bounds_left   = bounds_left
        self.bounds_right  = bounds_right   # max x of sprite's right edge
        self.bounds_top    = bounds_top
        self.bounds_bottom = bounds_bottom  # max y of sprite's top edge

    # ------------------------------------------------------------------

    def _start_turn(self, new_state):
        self._state      = new_state
        self._turn_idx   = 0
        self._turn_timer = 0.0

    def _finish_swim_timer(self):
        self._swim_timer = random.uniform(1.5, 6.0)

    def update(self, dt):
        w = self._sprite["width"]

        # ── Turning animation ──────────────────────────────────────────
        if self._state in (_ST_TURN_R, _ST_TURN_L):
            self._turn_timer += dt
            while self._turn_timer >= _TURN_FRAME_TIME:
                self._turn_timer -= _TURN_FRAME_TIME
                self._turn_idx += 1
                if self._turn_idx >= 3:
                    # Turn complete — switch to swim state
                    if self._state == _ST_TURN_R:
                        self._state         = _ST_RIGHT
                        self._current_frame = self._last_fr
                    else:
                        self._state         = _ST_LEFT
                        self._current_frame = 0
                    self._finish_swim_timer()
                    break
            else:
                # Still mid-turn: set the intermediate frame
                if self._state == _ST_TURN_R:
                    self._current_frame = self._turn_idx + 1        # 1, 2, 3
                elif self._state == _ST_TURN_L:
                    self._current_frame = self._last_fr - 1 - self._turn_idx  # 3, 2, 1

        # ── Swimming ──────────────────────────────────────────────────
        if self._state == _ST_LEFT:
            self.x -= self._speed * dt
            self._current_frame = 0
            self._swim_timer -= dt
            if self.x <= self.bounds_left:
                self.x = float(self.bounds_left)
                self._start_turn(_ST_TURN_R)
            elif self._swim_timer <= 0:
                self._start_turn(_ST_TURN_R)

        elif self._state == _ST_RIGHT:
            self.x += self._speed * dt
            self._current_frame = self._last_fr
            self._swim_timer -= dt
            if self.x + w >= self.bounds_right:
                self.x = float(self.bounds_right - w)
                self._start_turn(_ST_TURN_L)
            elif self._swim_timer <= 0:
                self._start_turn(_ST_TURN_L)

        # ── Y drift ───────────────────────────────────────────────────
        self._y_drift_timer -= dt
        if self._y_drift_timer <= 0:
            self._target_y      = random.uniform(float(self.bounds_top),
                                                 float(self.bounds_bottom))
            self._y_drift_timer = random.uniform(4.0, 16.0)

        if self.y < self._target_y:
            self.y = min(self._target_y, self.y + self._y_speed * dt)
        elif self.y > self._target_y:
            self.y = max(self._target_y, self.y - self._y_speed * dt)

    def draw(self, renderer, camera_offset=0):
        renderer.draw_sprite(
            self._frames[self._current_frame],
            self._sprite["width"],
            self._sprite["height"],
            int(self.x) - camera_offset,
            int(self.y),
            transparent=True, transparent_color=0,
        )


# ─────────────────────────────────────────────────────────────────────────────

_OCT_ANIM_TIME     = 0.18   # seconds per animation frame
_OCT_FRAME_COUNT   = 8
_OCT_BOB_AMPLITUDE = 3.0    # pixels
_OCT_BOB_PERIOD    = 4.0    # seconds per full bob cycle
_OCT_DRIFT_SPEED   = 10.0   # px/s when drifting
_OCT_DRIFT_MIN     = 2.0    # seconds before a direction change is considered
_OCT_DRIFT_MAX     = 7.0


class OctopusEntity:
    """A slowly drifting, bobbing octopus that cycles through its animation."""

    def __init__(self, sprite, x, y, bounds_left=6, bounds_right=236):
        self.x = float(x)
        self.y = float(y)   # base Y (bob oscillates around this)
        self._sprite = sprite
        self._frames = sprite["frames"]

        self._anim_timer    = random.uniform(0, _OCT_ANIM_TIME)
        self._current_frame = random.randint(0, _OCT_FRAME_COUNT - 1)

        self._bob_phase = random.uniform(0, math.pi * 2)

        self._vx          = random.choice((-1, 1)) * random.uniform(0.2, 0.5)
        self._drift_timer = random.uniform(_OCT_DRIFT_MIN, _OCT_DRIFT_MAX)

        self.bounds_left  = bounds_left
        self.bounds_right = bounds_right   # max x of sprite's right edge

    def update(self, dt):
        w = self._sprite["width"]

        # ── Animation cycle ───────────────────────────────────────────
        self._anim_timer += dt
        if self._anim_timer >= _OCT_ANIM_TIME:
            self._anim_timer -= _OCT_ANIM_TIME
            self._current_frame = (self._current_frame + 1) % _OCT_FRAME_COUNT

        # ── Slow drift ────────────────────────────────────────────────
        self._drift_timer -= dt
        if self._drift_timer <= 0:
            self._vx          = random.choice((-1, 1)) * random.uniform(0.2, 0.5)
            self._drift_timer = random.uniform(_OCT_DRIFT_MIN, _OCT_DRIFT_MAX)

        self.x += self._vx * _OCT_DRIFT_SPEED * dt

        if self.x <= self.bounds_left:
            self.x  = float(self.bounds_left)
            self._vx = abs(self._vx)
        elif self.x + w >= self.bounds_right:
            self.x  = float(self.bounds_right - w)
            self._vx = -abs(self._vx)

        # ── Bob phase ─────────────────────────────────────────────────
        self._bob_phase = (self._bob_phase + dt * (2 * math.pi / _OCT_BOB_PERIOD)) % (2 * math.pi)

    def draw(self, renderer, camera_offset=0):
        bob_offset = int(math.sin(self._bob_phase) * _OCT_BOB_AMPLITUDE)
        renderer.draw_sprite_obj(
            self._sprite,
            int(self.x) - camera_offset,
            int(self.y) + bob_offset,
            frame=self._current_frame,
        )


# ─────────────────────────────────────────────────────────────────────────────

_BUBBLE_RISE_SPEED_MIN = 6.0    # px/s upward
_BUBBLE_RISE_SPEED_MAX = 10.0
_BUBBLE_WOBBLE_AMP     = 2.0    # max x deviation per bubble (pixels)
_BUBBLE_WOBBLE_FREQ    = 1.2    # wobble cycles per second
_BUBBLE_RESPAWN_MIN    = 3.0    # seconds between groups
_BUBBLE_RESPAWN_MAX    = 8.0
_BUBBLE_COUNT_MIN      = 3
_BUBBLE_COUNT_MAX      = 6
_BUBBLE_Y_SPREAD       = 10     # vertical pixels the cluster spans at spawn


class BubbleGroup:
    """One cluster of bubbles that rises through a tank window, reused after despawn.

    Each bubble has its own x offset, y position, and rise speed so they form
    a loose vertical cluster rather than a flat horizontal line.
    """

    def __init__(self, sprite, tank_ranges, tank_top, tank_floor):
        self._sprite      = sprite
        self._tank_ranges = tank_ranges
        self._tank_top    = tank_top
        self._tank_floor  = tank_floor

        self.active         = False
        self._respawn_timer = random.uniform(1.0, 3.0)

        # Each bubble: [world_x, y, wobble_phase, wobble_freq_mult, rise_speed]
        self._bubbles = []

    def _spawn(self):
        tank_left, tank_right = random.choice(self._tank_ranges)
        margin   = 8
        center_x = float(random.randint(tank_left + margin, tank_right - margin))
        base_y   = float(self._tank_floor - 2)
        n = random.randint(_BUBBLE_COUNT_MIN, _BUBBLE_COUNT_MAX)
        self._bubbles = [
            [
                center_x + random.uniform(-4.0, 4.0),
                base_y - random.uniform(0.0, float(_BUBBLE_Y_SPREAD)),
                random.uniform(0.0, math.pi * 2),
                random.uniform(0.8, 1.4),
                random.uniform(_BUBBLE_RISE_SPEED_MIN, _BUBBLE_RISE_SPEED_MAX),
            ]
            for _ in range(n)
        ]
        self.active = True

    def update(self, dt):
        if not self.active:
            self._respawn_timer -= dt
            if self._respawn_timer <= 0:
                self._spawn()
            return

        i = len(self._bubbles) - 1
        while i >= 0:
            b = self._bubbles[i]
            b[1] -= b[4] * dt
            b[2] = (b[2] + dt * _BUBBLE_WOBBLE_FREQ * b[3] * math.pi * 2) % (math.pi * 2)
            if b[1] < self._tank_top - 3:
                self._bubbles.pop(i)
            i -= 1

        if not self._bubbles:
            self.active         = False
            self._respawn_timer = random.uniform(_BUBBLE_RESPAWN_MIN, _BUBBLE_RESPAWN_MAX)

    def draw(self, renderer, camera_offset=0):
        if not self.active:
            return
        w  = self._sprite["width"]
        h  = self._sprite["height"]
        fd = self._sprite["frames"][0]
        for b in self._bubbles:
            sx = int(b[0] + math.sin(b[2]) * _BUBBLE_WOBBLE_AMP) - camera_offset
            renderer.draw_sprite(fd, w, h, sx, int(b[1]), transparent=True, transparent_color=0)


# ─────────────────────────────────────────────────────────────────────────────

_DEBRIS_SPEED_MIN      = 3.0   # px/s in any direction
_DEBRIS_SPEED_MAX      = 6.0
_DEBRIS_DIRECTION_MIN  = 3.0   # seconds before randomly nudging velocity
_DEBRIS_DIRECTION_MAX  = 8.0


class DebrisField:
    """Small pixels that float slowly in all directions through the tank windows,
    bouncing off the interior walls.
    """

    def __init__(self, count, tank_ranges, tank_top, tank_floor):
        self._tank_ranges = tank_ranges
        self._tank_top    = tank_top
        self._tank_floor  = tank_floor
        # Each particle: [world_x, y, vx, vy, dir_timer, tank_left, tank_right]
        self._particles = [self._make_particle() for _ in range(count)]

    def _rand_v(self):
        spd = random.uniform(_DEBRIS_SPEED_MIN, _DEBRIS_SPEED_MAX)
        angle = random.uniform(0, math.pi * 2)
        return math.cos(angle) * spd, math.sin(angle) * spd

    def _make_particle(self):
        tank_left, tank_right = random.choice(self._tank_ranges)
        wx = float(random.randint(tank_left + 2, tank_right - 2))
        y  = float(random.randint(self._tank_top + 2, self._tank_floor - 2))
        vx, vy = self._rand_v()
        return [wx, y, vx, vy,
                random.uniform(_DEBRIS_DIRECTION_MIN, _DEBRIS_DIRECTION_MAX),
                float(tank_left + 1), float(tank_right - 1)]

    def update(self, dt):
        for p in self._particles:
            p[0] += p[2] * dt
            p[1] += p[3] * dt

            # Bounce off left/right tank walls
            if p[0] < p[5]:
                p[0] = p[5]
                p[2] = abs(p[2])
            elif p[0] > p[6]:
                p[0] = p[6]
                p[2] = -abs(p[2])

            # Bounce off top/bottom
            if p[1] < self._tank_top + 1:
                p[1] = float(self._tank_top + 1)
                p[3] = abs(p[3])
            elif p[1] > self._tank_floor - 1:
                p[1] = float(self._tank_floor - 1)
                p[3] = -abs(p[3])

            # Occasional gentle nudge to velocity direction
            p[4] -= dt
            if p[4] <= 0:
                vx, vy = self._rand_v()
                p[2] = vx
                p[3] = vy
                p[4] = random.uniform(_DEBRIS_DIRECTION_MIN, _DEBRIS_DIRECTION_MAX)

    def draw(self, renderer, camera_offset=0):
        for p in self._particles:
            renderer.draw_pixel(int(p[0]) - camera_offset, int(p[1]))
