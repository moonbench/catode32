# sky.py - Sky state and rendering logic for outdoor environments

import random
import config
from assets.nature import SUN, MOON, CLOUD1, CLOUD2


def _xorshift32(x):
    """Simple xorshift PRNG for deterministic pseudo-random values"""
    x ^= (x << 13) & 0xFFFFFFFF
    x ^= (x >> 17)
    x ^= (x << 5) & 0xFFFFFFFF
    return x & 0xFFFFFFFF

# Time of day categories
DAYTIME_TIMES = ("Morning", "Noon", "Afternoon")
NIGHTTIME_TIMES = ("Night",)
TRANSITION_TIMES = ("Dawn", "Dusk", "Evening")

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

# Sky position for celestial bodies based on time of day
# Returns (x, y) in world coordinates for background layer
# x ranges across the sky, y is height (lower = higher in sky)
CELESTIAL_POSITIONS = {
    "Dawn": (20, 12),      # Low on left horizon
    "Morning": (50, 6),    # Rising
    "Noon": (80, 2),       # High in sky
    "Afternoon": (110, 6), # Descending
    "Evening": (140, 10),  # Low on right
    "Dusk": (160, 14),     # Setting
    "Night": (90, 4),      # Moon high in sky
}

# Weather to cloud configuration
# Returns (min_clouds, max_clouds, speed_multiplier)
WEATHER_CLOUD_CONFIG = {
    "Clear": (1, 2, 1.0),
    "Cloudy": (3, 5, 1.0),
    "Overcast": (5, 7, 0.8),
    "Rain": (4, 6, 1.2),
    "Storm": (5, 7, 1.5),
    "Snow": (4, 6, 0.6),
    "Windy": (3, 4, 2.5),
}

# Weather to precipitation type
WEATHER_PRECIPITATION = {
    "Clear": None,
    "Cloudy": None,
    "Overcast": None,
    "Rain": ("rain", 0.6),
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
]


# --- Star Constants ---

STAR_SEED = 42
STAR_COUNT = 50
TWINKLE_RATIO = 0.2

# Star field dimensions (larger than screen for scrolling)
STAR_FIELD_WIDTH = 256
STAR_FIELD_HEIGHT = 40


def _generate_stars():
    """
    Generate deterministic star positions using xorshift PRNG.

    Returns:
        List of star dicts: {"x": int, "y": int, "twinkle": bool}
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
        stars.append({"x": x, "y": y, "twinkle": twinkle})
    return stars


class ShootingStarEvent:
    """Manages a shooting star animation"""

    def __init__(self, start_x, start_y):
        self.x = start_x
        self.y = start_y
        self.length = 8
        self.speed_x = 60
        self.speed_y = 20
        self.lifetime = 0.0
        self.max_lifetime = 0.5
        self.active = True

    def update(self, dt):
        self.x += self.speed_x * dt
        self.y += self.speed_y * dt
        self.lifetime += dt
        if self.lifetime >= self.max_lifetime:
            self.active = False

    def get_points(self):
        trail_x = self.x - (self.length * self.speed_x / 60)
        trail_y = self.y - (self.length * self.speed_y / 60)
        return (int(trail_x), int(trail_y), int(self.x), int(self.y))


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
        self.celestial_sprite = None
        self.celestial_frame = 0
        self.celestial_x = 80
        self.celestial_y = 5
        self.cloud_count = 2
        self.cloud_speed_mult = 1.0
        self.precipitation_type = None
        self.precipitation_intensity = 0.0

        # Animation state
        self.elapsed_time = 0.0
        self.day_of_year = 0
        self.twinkle_timer = 0.0
        self.twinkle_phase = 0
        self.celestial_anim_timer = 0.0
        self.celestial_anim_frame = 0
        self.shooting_star = None

        # Managed objects (added to environment layer)
        self._celestial_obj = None
        self._cloud_objs = []  # List of {"obj": dict, "base_speed": float}

        # Cached sprites
        self._moon_sprite_cached = None

        # Stars (generated once)
        self.stars = _generate_stars()

    def configure(self, environment_settings, world_width=256, day_of_year=0):
        """
        Configure sky from environment settings dict.

        Args:
            environment_settings: dict with time_of_day, season, moon_phase, weather
            world_width: Width of the world for cloud wrapping
            day_of_year: day number 0-365 for seasonal star drift
        """
        self.time_of_day = environment_settings.get("time_of_day", "Noon")
        self.moon_phase = environment_settings.get("moon_phase", "Full")
        self.weather = environment_settings.get("weather", "Clear")
        self.season = environment_settings.get("season", "Summer")
        self.world_width = world_width
        self.day_of_year = day_of_year

        self._update_celestial_body()
        self._update_star_visibility()
        self._update_cloud_config()
        self._update_precipitation()

    def _update_celestial_body(self):
        """Update celestial body sprite, frame, and position"""
        pos = CELESTIAL_POSITIONS.get(self.time_of_day, (80, 5))
        self.celestial_x = pos[0]
        self.celestial_y = pos[1]

        if self.time_of_day in DAYTIME_TIMES:
            self.celestial_sprite = SUN
            self.celestial_frame = 0
        elif self.time_of_day in NIGHTTIME_TIMES or self.time_of_day in ("Dusk", "Evening"):
            self.celestial_sprite = MOON
            self.celestial_frame = MOON_PHASE_FRAMES.get(self.moon_phase)
        else:  # Dawn
            self.celestial_sprite = SUN
            self.celestial_frame = 0

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
        # Add star drawing as custom draw
        environment.add_custom_draw(layer, self._draw_stars)

        # Add celestial body
        if self.celestial_sprite:
            sprite = self.celestial_sprite
            frame = self.celestial_frame
            if sprite == MOON and frame is not None:
                sprite = self._get_moon_sprite()

            self._celestial_obj = {
                "sprite": sprite,
                "x": self.celestial_x,
                "y": self.celestial_y,
                "frame": frame if frame is not None else 0,
            }
            environment.layers[layer].append(self._celestial_obj)

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
        # Remove celestial body
        if self._celestial_obj and self._celestial_obj in environment.layers[layer]:
            environment.layers[layer].remove(self._celestial_obj)
        self._celestial_obj = None

        # Remove clouds
        for cloud_data in self._cloud_objs:
            obj = cloud_data["obj"]
            if obj in environment.layers[layer]:
                environment.layers[layer].remove(obj)
        self._cloud_objs.clear()

        # Note: custom draws are not easily removed, but they check show_stars

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
            self.twinkle_phase = (self.twinkle_phase + 1) % 4

        # Celestial body animation (sun rays)
        if self.celestial_sprite == SUN:
            self.celestial_anim_timer += dt
            if self.celestial_anim_timer > 0.5:
                self.celestial_anim_timer = 0
                num_frames = len(SUN["frames"])
                self.celestial_anim_frame = (self.celestial_anim_frame + 1) % num_frames

            # Update the object in the layer
            if self._celestial_obj:
                self._celestial_obj["frame"] = self.celestial_anim_frame

        # Shooting star
        if self.shooting_star:
            self.shooting_star.update(dt)
            if not self.shooting_star.active:
                self.shooting_star = None

        # Maybe spawn new shooting star
        if self.show_stars and not self.shooting_star:
            self._maybe_spawn_shooting_star()

        # Update cloud positions
        wrap_point = self.world_width + 65
        for cloud_data in self._cloud_objs:
            obj = cloud_data["obj"]
            base_speed = cloud_data["base_speed"]
            obj["x"] += dt * base_speed * self.cloud_speed_mult
            if obj["x"] > wrap_point:
                obj["x"] = -65

    def _maybe_spawn_shooting_star(self):
        """Check if a shooting star should spawn (very rare)"""
        if random.random() < 0.002:  # ~0.2% chance per frame
            start_x = random.randint(10, 70)
            start_y = random.randint(2, 22)
            self.shooting_star = ShootingStarEvent(start_x, start_y)

    def get_star_offset(self):
        """Get combined star offset for time-of-night and seasonal drift"""
        time_offset = int((self.elapsed_time % 3600) / 3600 * 20)
        season_offset = int((self.day_of_year % 365) / 365 * 60)
        return time_offset + season_offset

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
            world_x = (star["x"] + offset_x) % STAR_FIELD_WIDTH
            screen_x = int(world_x - camera_offset)
            screen_y = star["y"]

            # Skip if off-screen
            if screen_x < 0 or screen_x >= config.DISPLAY_WIDTH:
                continue
            if screen_y < 0 or screen_y >= STAR_FIELD_HEIGHT:
                continue

            # Draw star with twinkle effect
            if star["twinkle"] and self.twinkle_phase in (1, 2):
                renderer.draw_pixel(screen_x, screen_y)
                if self.twinkle_phase == 2:
                    # Large twinkle - cross shape
                    renderer.draw_pixel(screen_x - 1, screen_y)
                    renderer.draw_pixel(screen_x + 1, screen_y)
                    renderer.draw_pixel(screen_x, screen_y - 1)
                    renderer.draw_pixel(screen_x, screen_y + 1)
                else:
                    # Small twinkle - horizontal only
                    renderer.draw_pixel(screen_x - 1, screen_y)
                    renderer.draw_pixel(screen_x + 1, screen_y)
            else:
                renderer.draw_pixel(screen_x, screen_y)

        # Draw shooting star if active
        if self.shooting_star and self.shooting_star.active:
            x1, y1, x2, y2 = self.shooting_star.get_points()
            if 0 <= x2 < config.DISPLAY_WIDTH and 0 <= y2 < STAR_FIELD_HEIGHT + 10:
                renderer.draw_line(x1, y1, x2, y2)
