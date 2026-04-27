import random

import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from entities.flyer import FlyerEntity
from entities.jumper import JumperEntity
from sky import SkyRenderer

_WORLD_WIDTH = 256

# ---------------------------------------------------------------------------
# Critter specs
#
# spawn_weight  - probability that any critters of this type appear at all
#                 when entering the scene (0.0–1.0)
# max_spawn     - maximum count spawned in one go when the weight roll passes
# seasons       - tuple of season strings in which this critter can appear
# ---------------------------------------------------------------------------
_CRITTER_SPECS = [
    {
        'type': 'flyer',
        'variant': 'butterfly',
        'seasons': ('Spring', 'Summer'),
        'spawn_weight': 0.65,
        'max_spawn': 3,
    },
    {
        'type': 'flyer',
        'variant': 'moth',
        'seasons': ('Spring', 'Summer', 'Fall'),
        'spawn_weight': 0.55,
        'max_spawn': 2,
        'night_only': True,
    },
    {
        'type': 'flyer',
        'variant': 'firefly',
        'seasons': ('Spring', 'Summer', 'Fall'),
        'spawn_weight': 0.50,
        'max_spawn': 3,
        'night_only': True,
    },
    {
        'type': 'jumper',
        'variant': 'frog',
        'seasons': ('Spring', 'Summer'),
        'spawn_weight': 0.30,
        'max_spawn': 1,
    },
    {
        'type': 'jumper',
        'variant': 'grasshopper',
        'seasons': ('Summer', 'Fall'),
        'spawn_weight': 0.25,
        'max_spawn': 1,
    },
]


class OutsideScene(MainScene):
    SCENE_NAME = 'outside'

    PLANT_SURFACES = [
        {'y_snap': 63, 'layer': 'foreground'},
        {'y_snap': 61, 'layer': 'midground'},
        {'y_snap': 56, 'layer': 'background'},
    ]

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.sky = SkyRenderer()
        self._last_weather = None
        self._critter_entities = []
        self._respawn_timers = []   # list of [remaining_secs, spec]

    def setup_scene(self):
        self.environment = Environment(world_width=_WORLD_WIDTH)

        # Set movement bounds for behaviors like zoomies (world coordinates)
        self.context.scene_x_min = 10
        self.context.scene_x_max = 246

        self.character = CharacterEntity(64, 64, context=self.context)

    def _is_daytime(self):
        h = self.context.environment.get('time_hours', 12)
        return 6 <= h < 20

    def _make_critter(self, spec):
        """Instantiate and return a single critter entity for the given spec."""
        if spec['type'] == 'flyer':
            f = FlyerEntity(
                spec['variant'],
                random.randint(20, _WORLD_WIDTH - 20),
                random.randint(10, 35)
            )
            if "speed" not in f._sprite:
                f.anim_speed = random.randint(7, 12)
            f.bounds_left = 10
            f.bounds_right = _WORLD_WIDTH - 10
            return f

        if spec['type'] == 'jumper':
            direction = random.choice((-1, 1))
            x = -12 if direction == 1 else _WORLD_WIDTH + 12
            return JumperEntity(spec['variant'], x, direction)

        return None

    def _spawn_critters(self):
        """Roll and spawn initial critters based on current season/time."""
        season = self.context.environment.get('season', 'Summer')
        is_day = self._is_daytime()

        for spec in _CRITTER_SPECS:
            if season not in spec['seasons']:
                continue
            if spec.get('night_only', False) == is_day:
                continue
            if random.random() > spec['spawn_weight']:
                continue
            count = random.randint(1, spec['max_spawn'])
            for _ in range(count):
                entity = self._make_critter(spec)
                if entity:
                    self._critter_entities.append(entity)
                    self.environment.add_entity(entity)

    def _find_jumper_spec(self, variant):
        for spec in _CRITTER_SPECS:
            if spec.get('type') == 'jumper' and spec.get('variant') == variant:
                return spec
        return None

    def on_enter(self):
        if self.context.espnow and self.context.visit is None:
            self.context.espnow.start()

        # Re-add all custom draws fresh (cleared on exit to prevent accumulation)
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_grass)

        # Configure and add sky objects when entering scene
        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width, seed=self.context.pet_seed)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)
        self._last_weather = env_settings.get('weather', 'Clear')

        self.environment.add_custom_draw(LAYER_BACKGROUND, self.sky.make_precipitation_drawer(0.3, 0))

        self._spawn_critters()

    def on_exit(self):
        if self.context.espnow and self.context.visit is None:
            self.context.espnow.stop()

        # Remove sky objects (celestial body, clouds) from environment layers
        self.sky.remove_from_environment(self.environment, LAYER_BACKGROUND)

        # Remove critters and clear timers so re-entry starts fresh
        for entity in self._critter_entities:
            self.environment.remove_entity(entity)
        self._critter_entities.clear()
        self._respawn_timers.clear()

    def on_update(self, dt):
        env = self.context.environment
        self.sky.set_time(env.get('time_hours', 12), env.get('time_minutes', 0))

        # Re-enter if weather changed so clouds, precipitation, etc. all rebuild
        current_weather = env.get('weather', 'Clear')
        if current_weather != self._last_weather:
            self.exit()
            self.enter()

        self.sky.update(dt)
        self.character.update(dt)

        # Tick respawn timers
        i = len(self._respawn_timers) - 1
        while i >= 0:
            self._respawn_timers[i][0] -= dt
            if self._respawn_timers[i][0] <= 0:
                spec = self._respawn_timers.pop(i)[1]
                if self._is_daytime():
                    season = self.context.environment.get('season', 'Summer')
                    if season in spec['seasons']:
                        entity = self._make_critter(spec)
                        if entity:
                            self._critter_entities.append(entity)
                            self.environment.add_entity(entity)
            i -= 1

        # Remove despawned jumpers and roll for replacement
        i = len(self._critter_entities) - 1
        while i >= 0:
            entity = self._critter_entities[i]
            if getattr(entity, 'despawned', False):
                self._critter_entities.pop(i)
                self.environment.remove_entity(entity)
                spec = self._find_jumper_spec(entity.variant)
                if spec and random.random() < 0.6:
                    delay = random.uniform(5, 90)
                    self._respawn_timers.append([delay, spec])
            i -= 1

        self.environment.update(dt)

    def on_post_draw(self):
        # Apply lightning inversion (hardware-level, affects display after show())
        self.renderer.invert(self.sky.get_lightning_invert_state())

    def _draw_grass(self, renderer, camera_x, parallax):
        """Draw procedural grass tufts"""
        camera_offset = int(camera_x * parallax)
        for world_x in [10, 35, 80, 110, 150, 190, 230]:
            screen_x = world_x - camera_offset
            if screen_x < -5 or screen_x > config.DISPLAY_WIDTH + 5:
                continue
            renderer.draw_line(screen_x, 64, screen_x - 2, 60)
            renderer.draw_line(screen_x, 64, screen_x, 60)
            renderer.draw_line(screen_x, 64, screen_x + 2, 60)
