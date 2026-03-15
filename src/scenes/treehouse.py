import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from assets.nature import PLANTER1, PLANT1, PLANT3, SMALLTREE1
from sky import SkyRenderer


class TreehouseScene(MainScene):
    SCENE_NAME = 'treehouse'
    MODULES_TO_KEEP = ['assets.nature', 'sky']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.sky = SkyRenderer()
        self._last_weather = None

    def setup_scene(self):
        self.environment = Environment(world_width=256)


        # Some foliage and planters on the platform
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=15, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT3,
            x=16, y=63 - PLANTER1["height"] - PLANT3["height"]
        )
        
        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER1,
            x=30, y=57 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT1,
            x=29, y=57 - PLANTER1["height"] - PLANT1["height"]
        )

        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=200, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=199, y=63 - PLANTER1["height"] - PLANT1["height"]
        )

        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER1,
            x=120, y=57 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT3,
            x=121, y=57 - PLANTER1["height"] - PLANT3["height"]
        )

        self.context.scene_x_min = 20
        self.context.scene_x_max = 236

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

    def on_enter(self):
        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)
        self._last_weather = env_settings.get('weather', 'Clear')

        self.environment.add_custom_draw(LAYER_MIDGROUND, self.sky.make_precipitation_drawer(0.6, 1))
        self.environment.add_custom_draw(LAYER_FOREGROUND, self.sky.make_precipitation_drawer(1.0, 2))
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_platform)

    def on_exit(self):
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

    def on_post_draw(self):
        self.renderer.invert(self.sky.get_lightning_invert_state())

    def _draw_platform(self, renderer, camera_x, parallax):
        """Draw wooden deck planks spanning the treehouse platform."""
        offset = int(camera_x * parallax)

        sx = max(0, 0 - offset)
        ex = min(config.DISPLAY_WIDTH, 256 - offset)
        if sx >= ex:
            return

        # Horizontal planks
        for py in [57, 59, 61]:
            renderer.draw_line(sx, py, ex, py)

        # Vertical post supports
        for world_x in [10, 80, 160, 245]:
            px = world_x - offset
            if sx <= px <= ex:
                renderer.draw_line(px, 57, px, 63)
