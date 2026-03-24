import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from entities.butterfly import ButterflyEntity
from assets.nature import PLANT1, PLANTER1, PLANT2
from sky import SkyRenderer


class OutsideScene(MainScene):
    SCENE_NAME = 'outside'
    MODULES_TO_KEEP = ['assets.nature', 'sky', 'entities.butterfly']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.sky = SkyRenderer()
        self._last_weather = None

    def setup_scene(self):
        self.environment = Environment(world_width=256)

        # Add plants to foreground
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=10, y=64 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=9, y=64 - PLANTER1["height"] - PLANT1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=94, y=64 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT2,
            x=90, y=64 - PLANTER1["height"] - PLANT2["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=180, y=64 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=179, y=64 - PLANTER1["height"] - PLANT1["height"]
        )

        # Plants for midground

        self.environment.add_object(
            LAYER_MIDGROUND, PLANT2,
            x=30, y=61-PLANT2["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT1,
            x=144, y=61-PLANT1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT2,
            x=120, y=61-PLANT2["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT1,
            x=174, y=61-PLANT1["height"],
            mirror_h=True
        )

        # Background plants
        self.environment.add_object(
            LAYER_BACKGROUND, PLANT2,
            x=130, y=58-PLANT2["height"]
        )

        # Set movement bounds for behaviors like zoomies (world coordinates)
        self.context.scene_x_min = 10
        self.context.scene_x_max = 246

        self.character = CharacterEntity(64, 64, context=self.context)

        butterfly1 = ButterflyEntity(110, 20)
        butterfly2 = ButterflyEntity(50, 30)
        butterfly2.anim_speed = 10
        butterfly1.bounds_right = 200
        butterfly2.bounds_right = 200
        self.environment.add_entity(butterfly1)
        self.environment.add_entity(butterfly2)

    def on_enter(self):
        # Re-add all custom draws fresh (cleared on exit to prevent accumulation)
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_grass)

        # Configure and add sky objects when entering scene
        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width, seed=self.context.pet_seed)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)
        self._last_weather = env_settings.get('weather', 'Clear')

        self.environment.add_custom_draw(LAYER_MIDGROUND, self.sky.make_precipitation_drawer(0.6, 1))
        self.environment.add_custom_draw(LAYER_FOREGROUND, self.sky.make_precipitation_drawer(1.0, 2))

    def on_exit(self):
        # Remove sky objects (celestial body, clouds) from environment layers
        self.sky.remove_from_environment(self.environment, LAYER_BACKGROUND)

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

        # Update environment entities (butterflies)
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
