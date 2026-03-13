# sky.py - Sky state and rendering logic for outdoor environments

import math
import random
import config
from assets.nature import SUN, MOON, CLOUD1, CLOUD2, CLOUD3, HOT_AIR_BALLOON, PLANE_TINY


def _xorshift32(x):
    """Simple xorshift PRNG for deterministic pseudo-random values"""
    x ^= (x << 13) & 0xFFFFFFFF
    x ^= (x >> 17)
    x ^= (x << 5) & 0xFFFFFFFF
    return x & 0xFFFFFFFF

# Time of day categories
DAYTIME_TIMES = ("Morning", "Noon", "Afternoon")
NIGHTTIME_TIMES = ("Night", "Late Night")
TRANSITION_TIMES = ("Dawn", "Dusk", "Evening")

# Hour-to-category mapping boundaries (hour >= threshold => category)
_TIME_CATEGORY_THRESHOLDS = (
    (21, "Night"),
    (19, "Evening"),
    (17, "Dusk"),
    (14, "Afternoon"),
    (12, "Noon"),
    (7,  "Morning"),
    (5,  "Dawn"),
    (0,  "Late Night"),
)

def hours_to_time_category(hours):
    """Convert 0-23 hour to a named time-of-day category"""
    for threshold, name in _TIME_CATEGORY_THRESHOLDS:
        if hours >= threshold:
            return name
    return "Late Night"

# Moon phase to frame index mapping
# MOON sprite has 7 frames (0-6), New moon uses None (just fill for star occlusion)
MOON_PHASE_FRAMES = {
    "New": None,
    "Wax Cres": 0,
    "1st Qtr": 1,
    "Wax Gib": 2,
    "Full": 3,
    "Wan Gib": 4,
    "3rd Qtr": 5,
    "Wan Cres": 6,
}

# Celestial arc constants (world coordinates, background layer, 0.3x parallax)
# x = -20 is safely off-screen left; x = 180 is safely off-screen right
# (background offset at max camera pan = 128 * 0.3 = 38; 180 - 38 = 142 > 128)
_SUN_RISE  = 5.0   # hour sun enters from left
_SUN_SET   = 19.0  # hour sun exits to right
_SUN_X_START, _SUN_X_END   = -20, 180
_SUN_Y_HORIZON, _SUN_Y_PEAK = 14, 2

_MOON_RISE = 17.0  # hour moon enters from left
_MOON_SET  = 29.0  # = 5:00 next day
_MOON_X_START, _MOON_X_END   = -20, 180
_MOON_Y_HORIZON, _MOON_Y_PEAK = 14, 3


def _calc_sun_position(hours_f):
    """Return (x, y) world position for the sun at fractional hour."""
    t = (hours_f - _SUN_RISE) / (_SUN_SET - _SUN_RISE)
    x = _SUN_X_START + t * (_SUN_X_END - _SUN_X_START)
    y = _SUN_Y_HORIZON - (_SUN_Y_HORIZON - _SUN_Y_PEAK) * 4 * t * (1 - t)
    return (int(x), int(y))


def _calc_moon_position(hours_f):
    """Return (x, y) world position for the moon at fractional hour."""
    mh = hours_f if hours_f >= _MOON_RISE else hours_f + 24
    t = (mh - _MOON_RISE) / (_MOON_SET - _MOON_RISE)
    x = _MOON_X_START + t * (_MOON_X_END - _MOON_X_START)
    y = _MOON_Y_HORIZON - (_MOON_Y_HORIZON - _MOON_Y_PEAK) * 4 * t * (1 - t)
    return (int(x), int(y))

# Weather to cloud configuration
# Returns (min_clouds, max_clouds, speed_multiplier)
WEATHER_CLOUD_CONFIG = {
    "Clear": (1, 2, 0.7),
    "Cloudy": (3, 5, 1.0),
    "Overcast": (5, 7, 0.8),
    "Rain": (5, 8, 1.0),
    "Storm": (8, 11, 1.3),
    "Snow": (5, 8, 0.6),
    "Windy": (3, 4, 1.5),
}

# Weather to precipitation type
WEATHER_PRECIPITATION = {
    "Clear": None,
    "Cloudy": None,
    "Overcast": None,
    "Rain": ("rain", 0.3),
    "Storm": ("rain", 1.0),
    "Snow": ("snow", 0.7),
    "Windy": None,
}

# Cloud templates with y positions and base speeds
CLOUD_TEMPLATES = [
    {"sprite": CLOUD1, "y": -7, "base_speed": 2.5},
    {"sprite": CLOUD1, "y": -17, "base_speed": 8.0},
    {"sprite": CLOUD2, "y": 0, "base_speed": 4.0},
    {"sprite": CLOUD2, "y": -10, "base_speed": 3.0},
    {"sprite": CLOUD1, "y": -5, "base_speed": 5.0},
    {"sprite": CLOUD2, "y": -15, "base_speed": 6.0},
    {"sprite": CLOUD1, "y": -12, "base_speed": 3.5},
    {"sprite": CLOUD3, "y": -3, "base_speed": 3.0},
    {"sprite": CLOUD3, "y": -14, "base_speed": 5.5},
]


# --- Star Constants ---

STAR_SEED = 42
STAR_COUNT = 50
TWINKLE_RATIO = 0.2

# Star field dimensions (larger than screen for scrolling)
STAR_FIELD_WIDTH = 256
STAR_FIELD_HEIGHT = 50  # ~4/5 of screen height


# Twinkle cycle: longer cycle with pauses between twinkles
# Phases 0-7 are "off", phases 8-9 are small twinkle, phase 10 is large twinkle
TWINKLE_CYCLE_LENGTH = 12
TWINKLE_SMALL_PHASES = (8, 9, 11)  # Growing and shrinking
TWINKLE_LARGE_PHASE = 10          # Peak twinkle

# Star tuple field indices
_STAR_X = 0
_STAR_Y = 1
_STAR_TWINKLE = 2
_STAR_PHASE = 3


def _generate_stars():
    """
    Generate deterministic star positions using xorshift PRNG.

    Returns:
        List of star tuples: (x, y, twinkle, phase_offset)
    """
    stars = []
    state = STAR_SEED
    for _ in range(STAR_COUNT):
        state = _xorshift32(state)
        x = state % STAR_FIELD_WIDTH
        state = _xorshift32(state)
        y = state % STAR_FIELD_HEIGHT
        state = _xorshift32(state)
        twinkle = (state % 100) < int(TWINKLE_RATIO * 100)
        state = _xorshift32(state)
        phase_offset = state % TWINKLE_CYCLE_LENGTH
        stars.append((x, y, twinkle, phase_offset))
    return stars


class ShootingStarEvent:
    """Manages a shooting star animation with grow/shrink and trailing particles"""

    def __init__(self, start_x, start_y):
        self.x = start_x
        self.y = start_y
        self.max_length = 22
        self.speed_x = 28
        self.speed_y = 7
        self.lifetime = 0.0
        self.max_lifetime = 3.1
        self.active = True

        # Timing for grow/shrink
        self.grow_duration = 0.5
        self.shrink_start = self.max_lifetime - 0.7

        # Trailing particles
        self.particles = []
        self.particle_timer = 0.0

    @property
    def length(self):
        """Dynamic length - grows at start, shrinks at end"""
        if self.lifetime < self.grow_duration:
            # Growing phase
            return self.max_length * (self.lifetime / self.grow_duration)
        elif self.lifetime > self.shrink_start:
            # Shrinking phase
            remaining = self.max_lifetime - self.lifetime
            shrink_duration = self.max_lifetime - self.shrink_start
            return self.max_length * (remaining / shrink_duration)
        else:
            return self.max_length

    def update(self, dt):
        # Move the head
        self.x += self.speed_x * dt
        self.y += self.speed_y * dt
        self.lifetime += dt

        # Spawn particles from the tail periodically
        self.particle_timer += dt
        if self.particle_timer > 0.12 and self.lifetime > self.grow_duration:
            self.particle_timer = 0
            tail_x, tail_y, _, _ = self.get_points()
            self.particles.append([float(tail_x), float(tail_y), 0.0])

        # Update particles - they slow down and fall
        for p in self.particles:
            p[_SS_X] += self.speed_x * 0.15 * dt  # Much slower horizontal
            p[_SS_Y] += self.speed_y * 0.1 * dt  # Falls downward
            p[_SS_LIFE] += dt

        # Remove old particles
        self.particles = [p for p in self.particles if p[_SS_LIFE] < 0.7]

        if self.lifetime >= self.max_lifetime:
            self.active = False

    def get_points(self):
        """Get line segment for the main streak"""
        current_length = self.length
        trail_x = self.x - (current_length * self.speed_x / self.max_length)
        trail_y = self.y - (current_length * self.speed_y / self.max_length)
        return (int(trail_x), int(trail_y), int(self.x), int(self.y))


# Daytime sky event types
SKY_EVENT_TYPES = [
    {"sprite": HOT_AIR_BALLOON, "speed": 4, "mirror_when_right": False},
    {"sprite": PLANE_TINY, "speed": 12, "mirror_when_right": True},
]


class SkyEvent:
    """A daytime sky object (balloon, plane, etc.) crossing the screen"""

    def __init__(self, event_type, start_x, y, going_right, world_width):
        self.sprite = event_type["sprite"]
        self.base_speed = event_type["speed"]
        self.mirror_when_right = event_type["mirror_when_right"]

        self.x = float(start_x)
        self.y = y
        self.going_right = going_right
        self.world_width = world_width
        self.active = True

        # Speed with slight variance
        self.speed = self.base_speed * (0.8 + random.random() * 0.4)

    @property
    def mirror(self):
        """Whether to mirror the sprite when drawing"""
        return self.mirror_when_right and self.going_right

    def update(self, dt):
        if self.going_right:
            self.x += self.speed * dt
            # Deactivate when fully off right side
            if self.x > self.world_width + self.sprite["width"] + 20:
                self.active = False
        else:
            self.x -= self.speed * dt
            # Deactivate when fully off left side
            if self.x < -self.sprite["width"] - 20:
                self.active = False


# Precipitation constants
RAIN_PARTICLE_COUNT = 40
SNOW_PARTICLE_COUNT = 30
RAIN_SPEED_Y = 100  # Pixels per second
RAIN_SPEED_X = 10   # Slight wind drift
RAIN_STREAK_LENGTH = 3
SNOW_SPEED_Y = 15
SNOW_DRIFT_SPEED = 8  # Horizontal wobble amplitude

# Particle list indices (replacing per-particle dicts)
_SS_X    = 0  # ShootingStarEvent trailing particle: x
_SS_Y    = 1  # y
_SS_LIFE = 2  # age in seconds

_P_X    = 0  # Precipitation particle: x (shared by rain & snow)
_P_Y    = 1  # y (shared)

# Rain particle layout: [x, y, speed_variance, x_variance, layer]
_RAIN_SPEED_VAR = 2
_RAIN_X_VAR     = 3
_RAIN_LAYER     = 4

# Snow particle layout: [x, y, drift_offset, drift_speed, speed_variance, layer]
_SNOW_DRIFT_OFFSET = 2
_SNOW_DRIFT_SPEED  = 3
_SNOW_SPEED_VAR    = 4
_SNOW_LAYER        = 5


class SkyRenderer:
    """
    Manages sky rendering including celestial bodies, stars, clouds, and animations.

    Usage:
        sky = SkyRenderer()
        sky.configure(context.environment, world_width=256)
        sky.add_to_environment(environment, layer)

        # In update loop:
        sky.update(dt)

        # Cleanup when leaving scene:
        sky.remove_from_environment(environment, layer)
    """

    def __init__(self):
        # Environment settings
        self.time_of_day = "Noon"
        self.moon_phase = "Full"
        self.weather = "Clear"
        self.season = "Summer"
        self.world_width = 256

        # Derived state
        self.show_stars = False
        self.star_brightness = 1.0
        self.cloud_count = 2
        self.cloud_speed_mult = 1.0
        self.precipitation_type = None
        self.precipitation_intensity = 0.0

        # Animation state
        self.elapsed_time = 0.0
        self.day_of_year = 0
        self.twinkle_timer = 0.0
        self.twinkle_phase = 0
        self._sun_anim_timer = 0.0
        self._sun_anim_frame = 0
        self.shooting_star = None
        self.sky_event = None  # Daytime events (balloon, plane, etc.)

        # Current fractional hours (set by configure/set_time)
        self._hours_f = 12.0

        # Managed objects (added to environment layer)
        self._sun_obj = None
        self._moon_obj = None
        self._cloud_objs = []  # List of {"obj": dict, "base_speed": float}

        # Cached sprites
        self._moon_sprite_cached = None

        # Stars (generated once)
        self.stars = _generate_stars()

        # Precipitation particles
        self._precip_particles = []

        # Lightning state (storm weather only)
        self._lightning_active = False
        self._lightning_flashes_remaining = 0
        self._lightning_timer = 0.0
        self._lightning_invert_state = False

        # Screen-space clip rect for window rendering (x, y, w, h). None = full screen.
        self._render_rect = None

    def set_time(self, hours, minutes=0):
        """Update celestial positions. Only updates star visibility on category change."""
        self._hours_f = hours + minutes / 60.0

        sx, sy = _calc_sun_position(self._hours_f)
        if self._sun_obj:
            self._sun_obj["x"] = sx
            self._sun_obj["y"] = sy

        mx, my = _calc_moon_position(self._hours_f)
        if self._moon_obj:
            self._moon_obj["x"] = mx
            self._moon_obj["y"] = my

        new_category = hours_to_time_category(hours)
        if new_category != self.time_of_day:
            self.time_of_day = new_category
            self._update_star_visibility()

    def configure(self, environment_settings, world_width=256, day_of_year=0):
        """
        Configure sky from environment settings dict.

        Args:
            environment_settings: dict with time_of_day, season, moon_phase, weather
            world_width: Width of the world for cloud wrapping
            day_of_year: day number 0-365 for seasonal star drift
        """
        time_hours = environment_settings.get("time_hours", 12)
        time_minutes = environment_settings.get("time_minutes", 0)
        self._hours_f = time_hours + time_minutes / 60.0
        self.time_of_day = hours_to_time_category(time_hours)
        self.moon_phase = environment_settings.get("moon_phase", "Full")
        self.weather = environment_settings.get("weather", "Clear")
        self.season = environment_settings.get("season", "Summer")
        self.world_width = world_width
        self.day_of_year = day_of_year

        self._update_star_visibility()
        self._update_cloud_config()
        self._update_precipitation()
        self._init_precipitation_particles()

    def _update_star_visibility(self):
        """Update star visibility based on time of day"""
        if self.time_of_day in NIGHTTIME_TIMES:
            self.show_stars = True
            self.star_brightness = 1.0
        elif self.time_of_day == "Dusk":
            self.show_stars = True
            self.star_brightness = 0.6
        elif self.time_of_day == "Evening":
            self.show_stars = True
            self.star_brightness = 0.85
        elif self.time_of_day == "Dawn":
            self.show_stars = True
            self.star_brightness = 0.4
        else:
            self.show_stars = False
            self.star_brightness = 0.0

    def _update_cloud_config(self):
        """Update cloud configuration based on weather"""
        cfg = WEATHER_CLOUD_CONFIG.get(self.weather, (2, 3, 1.0))
        self.cloud_count = (cfg[0] + cfg[1]) // 2
        self.cloud_speed_mult = cfg[2]

    def _update_precipitation(self):
        """Update precipitation based on weather"""
        precip = WEATHER_PRECIPITATION.get(self.weather)
        if precip:
            self.precipitation_type = precip[0]
            self.precipitation_intensity = precip[1]
        else:
            self.precipitation_type = None
            self.precipitation_intensity = 0.0

    def _init_precipitation_particles(self):
        """Initialize precipitation particles based on type and intensity"""
        self._precip_particles = []

        if not self.precipitation_type:
            return

        # Create particles for each layer (0=background, 1=midground, 2=foreground)
        if self.precipitation_type == "rain":
            count_per_layer = int(RAIN_PARTICLE_COUNT * self.precipitation_intensity)
            for layer in range(3):
                for _ in range(count_per_layer):
                    self._precip_particles.append(self._spawn_rain_particle(random_y=True, layer=layer))
        elif self.precipitation_type == "snow":
            count_per_layer = int(SNOW_PARTICLE_COUNT * self.precipitation_intensity)
            for layer in range(3):
                for _ in range(count_per_layer):
                    self._precip_particles.append(self._spawn_snow_particle(random_y=True, layer=layer))

    def _spawn_rain_particle(self, random_y=False, layer=None):
        """Spawn a rain particle at top of screen (or random y for init).
        Layout: [x, y, speed_variance, x_variance, layer]
        """
        return [
            random.random() * self.world_width,
            random.random() * config.DISPLAY_HEIGHT if random_y else -random.randint(0, 15),
            0.7 + random.random() * 0.6,
            0.8 + random.random() * 0.4,
            layer if layer is not None else random.randint(0, 2),
        ]

    def _spawn_snow_particle(self, random_y=False, layer=None):
        """Spawn a snow particle at top of screen (or random y for init).
        Layout: [x, y, drift_offset, drift_speed, speed_variance, layer]
        """
        return [
            random.random() * self.world_width,
            random.random() * config.DISPLAY_HEIGHT if random_y else -random.randint(0, 15),
            random.random() * 6.28,
            1.5 + random.random() * 2.0,
            0.6 + random.random() * 0.8,
            layer if layer is not None else random.randint(0, 2),
        ]

    def _get_moon_sprite(self):
        """Get moon sprite with fill_frames expanded for all phases"""
        if self._moon_sprite_cached is None:
            self._moon_sprite_cached = {
                "width": MOON["width"],
                "height": MOON["height"],
                "frames": MOON["frames"],
                "fill_frames": [MOON["fill_frames"][0]] * len(MOON["frames"]),
            }
        return self._moon_sprite_cached

    def add_to_environment(self, environment, layer):
        """
        Add sky objects (stars, celestial body, clouds) to an environment layer.

        Args:
            environment: Environment instance
            layer: Layer constant (e.g., LAYER_BACKGROUND)
        """
        # Add custom draw functions
        environment.add_custom_draw(layer, self._draw_stars)
        environment.add_custom_draw(layer, self._draw_sky_events)

        # Add sun and moon (both always present; one or both may be off-screen)
        sx, sy = _calc_sun_position(self._hours_f)
        self._sun_obj = {"sprite": SUN, "x": sx, "y": sy, "frame": self._sun_anim_frame}
        environment.layers[layer].append(self._sun_obj)

        moon_sprite = self._get_moon_sprite()
        moon_frame = MOON_PHASE_FRAMES.get(self.moon_phase)
        mx, my = _calc_moon_position(self._hours_f)
        self._moon_obj = {
            "sprite": moon_sprite,
            "x": mx, "y": my,
            "frame": moon_frame if moon_frame is not None else 0,
        }
        environment.layers[layer].append(self._moon_obj)

        # Add clouds
        self._cloud_objs.clear()
        count = min(self.cloud_count, len(CLOUD_TEMPLATES))
        spacing = self.world_width // max(count, 1)

        for i in range(count):
            template = CLOUD_TEMPLATES[i % len(CLOUD_TEMPLATES)]
            cloud_obj = {
                "sprite": template["sprite"],
                "x": float(i * spacing - 30),
                "y": template["y"],
            }
            self._cloud_objs.append({
                "obj": cloud_obj,
                "base_speed": template["base_speed"],
            })
            environment.layers[layer].append(cloud_obj)

    def remove_from_environment(self, environment, layer):
        """
        Remove sky objects from an environment layer.

        Args:
            environment: Environment instance
            layer: Layer constant
        """
        # Remove sun and moon
        if self._sun_obj and self._sun_obj in environment.layers[layer]:
            environment.layers[layer].remove(self._sun_obj)
        self._sun_obj = None
        if self._moon_obj and self._moon_obj in environment.layers[layer]:
            environment.layers[layer].remove(self._moon_obj)
        self._moon_obj = None

        # Remove clouds
        for cloud_data in self._cloud_objs:
            obj = cloud_data["obj"]
            if obj in environment.layers[layer]:
                environment.layers[layer].remove(obj)
        self._cloud_objs.clear()

    def update(self, dt):
        """
        Update sky animations and cloud positions. Call once per frame.

        Args:
            dt: Delta time in seconds
        """
        self.elapsed_time += dt

        # Twinkle animation cycle (4 phases)
        self.twinkle_timer += dt
        if self.twinkle_timer > 0.3:
            self.twinkle_timer = 0
            self.twinkle_phase = (self.twinkle_phase + 1) % TWINKLE_CYCLE_LENGTH

        # Sun ray animation
        self._sun_anim_timer += dt
        if self._sun_anim_timer > 0.5:
            self._sun_anim_timer = 0
            self._sun_anim_frame = (self._sun_anim_frame + 1) % len(SUN["frames"])
        if self._sun_obj:
            self._sun_obj["frame"] = self._sun_anim_frame

        # Shooting star
        if self.shooting_star:
            self.shooting_star.update(dt)
            if not self.shooting_star.active:
                self.shooting_star = None

        # Maybe spawn new shooting star
        if self.show_stars and not self.shooting_star:
            self._maybe_spawn_shooting_star()

        # Daytime sky events (balloon, plane, etc.)
        if self.sky_event:
            self.sky_event.update(dt)
            if not self.sky_event.active:
                self.sky_event = None

        # Maybe spawn new daytime sky event
        if self.time_of_day in DAYTIME_TIMES and not self.sky_event:
            self._maybe_spawn_sky_event()

        # Update cloud positions
        wrap_point = self.world_width + 65
        for cloud_data in self._cloud_objs:
            obj = cloud_data["obj"]
            base_speed = cloud_data["base_speed"]
            obj["x"] += dt * base_speed * self.cloud_speed_mult
            if obj["x"] > wrap_point:
                obj["x"] = -65

        # Update precipitation particles
        self._update_precipitation_particles(dt)

        # Update lightning
        self._update_lightning(dt)

    def _update_precipitation_particles(self, dt):
        """Update precipitation particle positions"""
        if not self._precip_particles:
            return

        screen_bottom = config.DISPLAY_HEIGHT + 5

        for p in self._precip_particles:
            if self.precipitation_type == "rain":
                # Rain falls fast with slight horizontal drift (varied per particle)
                p[_P_Y] += RAIN_SPEED_Y * p[_RAIN_SPEED_VAR] * dt
                p[_P_X] += RAIN_SPEED_X * p[_RAIN_X_VAR] * dt

                # Respawn at top when off bottom
                if p[_P_Y] > screen_bottom:
                    p[_P_Y] = -random.randint(0, 15)
                    p[_P_X] = random.random() * self.world_width
                    p[_RAIN_SPEED_VAR] = 0.7 + random.random() * 0.6
                    p[_RAIN_X_VAR] = 0.8 + random.random() * 0.4

                # Wrap horizontally
                if p[_P_X] > self.world_width:
                    p[_P_X] -= self.world_width

            elif self.precipitation_type == "snow":
                # Snow falls slowly with sinusoidal drift (varied frequency per particle)
                p[_P_Y] += SNOW_SPEED_Y * p[_SNOW_SPEED_VAR] * dt
                drift = math.sin(self.elapsed_time * p[_SNOW_DRIFT_SPEED] + p[_SNOW_DRIFT_OFFSET]) * SNOW_DRIFT_SPEED * dt
                p[_P_X] += drift

                # Respawn at top when off bottom
                if p[_P_Y] > screen_bottom:
                    p[_P_Y] = -random.randint(0, 15)
                    p[_P_X] = random.random() * self.world_width
                    p[_SNOW_DRIFT_OFFSET] = random.random() * 6.28
                    p[_SNOW_DRIFT_SPEED] = 1.5 + random.random() * 2.0
                    p[_SNOW_SPEED_VAR] = 0.6 + random.random() * 0.8

                # Wrap horizontally
                if p[_P_X] < 0:
                    p[_P_X] += self.world_width
                elif p[_P_X] > self.world_width:
                    p[_P_X] -= self.world_width

    def _update_lightning(self, dt):
        """Update lightning effect state"""
        if self._lightning_active:
            self._lightning_timer -= dt
            if self._lightning_timer <= 0:
                # Toggle invert state
                self._lightning_invert_state = not self._lightning_invert_state
                self._lightning_flashes_remaining -= 1

                if self._lightning_flashes_remaining <= 0:
                    # Lightning strike complete
                    self._lightning_active = False
                    self._lightning_invert_state = False
                else:
                    # Set timer for next flash toggle
                    self._lightning_timer = 0.05  # 50ms per flash state
        elif self.weather == "Storm":
            # Maybe spawn new lightning
            if random.random() < 0.003:  # ~0.3% chance per frame
                self._lightning_active = True
                # 2-5 inversions means 4-10 state changes (on/off pairs)
                num_inversions = random.randint(2, 5)
                self._lightning_flashes_remaining = num_inversions * 2
                self._lightning_timer = 0.05
                self._lightning_invert_state = True

    def get_lightning_invert_state(self):
        """Return True if display should be inverted due to lightning"""
        return self._lightning_invert_state

    def _maybe_spawn_shooting_star(self):
        """Check if a shooting star should spawn (very rare)"""
        if random.random() < 0.002:  # ~0.2% chance per frame
            start_x = random.randint(10, 70)
            start_y = random.randint(2, 22)
            self.shooting_star = ShootingStarEvent(start_x, start_y)

    def _maybe_spawn_sky_event(self):
        """Check if a daytime sky event should spawn (rare)"""
        if random.random() < 0.001:  # ~0.1% chance per frame
            self.spawn_sky_event()

    def spawn_sky_event(self, event_index=None, going_right=None):
        """
        Force spawn a sky event (useful for testing).

        Args:
            event_index: Index into SKY_EVENT_TYPES (0=balloon, 1=plane), or None for random
            going_right: Direction, or None for random
        """
        if event_index is None:
            event_type = random.choice(SKY_EVENT_TYPES)
        else:
            event_type = SKY_EVENT_TYPES[event_index % len(SKY_EVENT_TYPES)]

        if going_right is None:
            going_right = random.random() < 0.5

        # Start position - off screen on the appropriate side
        if going_right:
            start_x = -event_type["sprite"]["width"] - 10
        else:
            start_x = self.world_width + 10

        # Random height in upper portion of sky
        y = random.randint(2, 22)

        self.sky_event = SkyEvent(event_type, start_x, y, going_right, self.world_width)

    def get_star_offset(self):
        """Get combined star offset for time-of-night and seasonal drift"""
        time_offset = int((self.elapsed_time % 3600) / 3600 * 20)
        season_offset = int((self.day_of_year % 365) / 365 * 60)
        return time_offset + season_offset

    def _draw_sky_events(self, renderer, camera_x, parallax):
        """Draw daytime sky events (balloon, plane, etc.)"""
        if not self.sky_event or not self.sky_event.active:
            return

        camera_offset = int(camera_x * parallax)
        event = self.sky_event
        screen_x = int(event.x - camera_offset)

        # Only draw if on screen
        if -event.sprite["width"] < screen_x < config.DISPLAY_WIDTH + event.sprite["width"]:
            renderer.draw_sprite_obj(
                event.sprite,
                screen_x,
                event.y,
                frame=0,
                mirror_h=event.mirror
            )

    def _draw_stars(self, renderer, camera_x, parallax):
        """Draw stars (used as custom draw function)"""
        if not self.show_stars:
            return

        camera_offset = int(camera_x * parallax)
        offset_x = self.get_star_offset()

        for i, star in enumerate(self.stars):
            # Skip some stars based on brightness
            if self.star_brightness < 1.0:
                skip_threshold = int((1.0 - self.star_brightness) * 100)
                if (i * 17) % 100 < skip_threshold:
                    continue

            # Calculate position with offset and wrapping
            world_x = (star[_STAR_X] + offset_x) % STAR_FIELD_WIDTH
            screen_x = int(world_x - camera_offset)
            screen_y = star[_STAR_Y]

            # Skip if off-screen
            if screen_x < 0 or screen_x >= config.DISPLAY_WIDTH:
                continue
            if screen_y < 0 or screen_y >= STAR_FIELD_HEIGHT:
                continue

            # Clip to window rect if set
            if self._render_rect:
                rx, ry, rw, rh = self._render_rect
                if screen_x < rx or screen_x >= rx + rw or screen_y < ry or screen_y >= ry + rh:
                    continue

            # Draw star with twinkle effect (each star has its own phase offset)
            if star[_STAR_TWINKLE]:
                star_phase = (self.twinkle_phase + star[_STAR_PHASE]) % TWINKLE_CYCLE_LENGTH
                if star_phase == TWINKLE_LARGE_PHASE:
                    # Large twinkle - cross shape
                    renderer.draw_pixel(screen_x, screen_y)
                    renderer.draw_pixel(screen_x - 1, screen_y)
                    renderer.draw_pixel(screen_x + 1, screen_y)
                    renderer.draw_pixel(screen_x, screen_y - 1)
                    renderer.draw_pixel(screen_x, screen_y + 1)
                elif star_phase in TWINKLE_SMALL_PHASES:
                    # Small twinkle - horizontal only
                    renderer.draw_pixel(screen_x, screen_y)
                    renderer.draw_pixel(screen_x - 1, screen_y)
                    renderer.draw_pixel(screen_x + 1, screen_y)
                else:
                    # Normal single pixel
                    renderer.draw_pixel(screen_x, screen_y)
            else:
                renderer.draw_pixel(screen_x, screen_y)

        # Draw shooting star if active
        if self.shooting_star and self.shooting_star.active:
            # Draw trailing particles first (behind the main streak)
            for p in self.shooting_star.particles:
                px, py = int(p[_SS_X]), int(p[_SS_Y])
                if 0 <= px < config.DISPLAY_WIDTH and 0 <= py < 50:
                    renderer.draw_pixel(px, py)

            # Draw main streak
            x1, y1, x2, y2 = self.shooting_star.get_points()
            if 0 <= x2 < config.DISPLAY_WIDTH and 0 <= y2 < STAR_FIELD_HEIGHT + 10:
                renderer.draw_line(x1, y1, x2, y2)

    def make_precipitation_drawer(self, parallax, layer_index):
        """
        Create a precipitation draw function for a specific parallax layer.

        Args:
            parallax: Parallax factor for this layer (e.g., 0.3 for background, 1.0 for foreground)
            layer_index: Which particle layer to draw (0=background, 1=midground, 2=foreground)

        Returns:
            A draw function compatible with environment.add_custom_draw()
        """
        def draw_func(renderer, camera_x, layer_parallax):
            self._draw_precipitation(renderer, camera_x, parallax, layer_index)
        return draw_func

    def _draw_precipitation(self, renderer, camera_x, parallax, layer_index):
        """Draw precipitation particles with parallax for a specific layer"""
        if not self._precip_particles or not self.precipitation_type:
            return

        camera_offset = int(camera_x * parallax)

        p_layer = _RAIN_LAYER if self.precipitation_type == "rain" else _SNOW_LAYER
        for p in self._precip_particles:
            # Only draw particles assigned to this layer
            if p[p_layer] != layer_index:
                continue

            # Apply parallax to x position
            screen_x = int(p[_P_X] - camera_offset)
            screen_y = int(p[_P_Y])

            # Wrap for screen visibility
            while screen_x < 0:
                screen_x += config.DISPLAY_WIDTH
            while screen_x >= config.DISPLAY_WIDTH:
                screen_x -= config.DISPLAY_WIDTH

            # Skip if off screen vertically
            if screen_y < -RAIN_STREAK_LENGTH or screen_y > config.DISPLAY_HEIGHT:
                continue

            # Clip to window rect if set
            if self._render_rect:
                rx, ry, rw, rh = self._render_rect
                if screen_x < rx or screen_x >= rx + rw or screen_y < ry or screen_y >= ry + rh:
                    continue

            if self.precipitation_type == "rain":
                # Draw rain as a short vertical line (streak)
                streak_end_y = screen_y - RAIN_STREAK_LENGTH
                # Slight angle matching the drift
                streak_end_x = screen_x - 1
                renderer.draw_line(streak_end_x, streak_end_y, screen_x, screen_y)
            elif self.precipitation_type == "snow":
                # Draw snow as a single pixel
                if 0 <= screen_y < config.DISPLAY_HEIGHT:
                    renderer.draw_pixel(screen_x, screen_y)
