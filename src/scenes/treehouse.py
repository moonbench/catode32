import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from assets.furniture import CAT_BED_SIDE
from sky import SkyRenderer


class TreehouseScene(MainScene):
    SCENE_NAME = 'treehouse'

    PLANT_SURFACES = [
        {'y_snap': 63, 'layer': 'foreground'},
        {'y_snap': 59, 'layer': 'midground'},
    ]

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.sky = SkyRenderer()
        self._last_weather = None

    def setup_scene(self):
        self.environment = Environment(world_width=256)

        self.context.scene_x_min = 20
        self.context.scene_x_max = 236

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

    def on_enter(self):
        if self.context.espnow and self.context.visit is None:
            self.context.espnow.start()

        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width, seed=self.context.pet_seed)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)
        self._last_weather = env_settings.get('weather', 'Clear')

        self.environment.add_custom_draw(LAYER_MIDGROUND, self.sky.make_precipitation_drawer(0.6, 1))
        self.environment.add_custom_draw(LAYER_FOREGROUND, self.sky.make_precipitation_drawer(1.0, 2))
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_platform)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_platform_mid)
        self.context.cat_bed_x = 150  # foreground world-x of the cat bed interior (tunable)

    def on_exit(self):
        self.context.cat_bed_x = None
        if self.character:
            self.character.draw_y_offset = 0

        if self.context.espnow and self.context.visit is None:
            self.context.espnow.stop()

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
        camera_offset = int(self.environment.camera_x * 1.0)  # foreground parallax
        bed_x = 130 - camera_offset

        # Left rim of cat bed
        self.renderer.draw_sprite_obj(CAT_BED_SIDE, bed_x, 52)
        # Middle gap (mask + lines)
        self.renderer.draw_rect(bed_x + CAT_BED_SIDE["width"], 54, 20, 10, filled=True, color=0)
        self.renderer.draw_line(bed_x + CAT_BED_SIDE["width"], 54, bed_x + CAT_BED_SIDE["width"] + 20, 54)
        self.renderer.draw_line(bed_x + CAT_BED_SIDE["width"], 63, bed_x + CAT_BED_SIDE["width"] + 20, 63)
        self.renderer.draw_line(bed_x + CAT_BED_SIDE["width"], 59, bed_x + CAT_BED_SIDE["width"] + 20, 59)
        # Right rim (mirrored)
        self.renderer.draw_sprite_obj(CAT_BED_SIDE, bed_x + CAT_BED_SIDE["width"] + 20, 52, mirror_h=True)

        self.renderer.invert(self.sky.get_lightning_invert_state())

    def _draw_platform(self, renderer, camera_x, parallax):
        """Draw wooden deck planks spanning the treehouse platform."""
        offset = int(camera_x * parallax)

        sx = max(0, 0 - offset)
        ex = min(config.DISPLAY_WIDTH, 256 - offset)
        if sx >= ex:
            return

        # Horizontal planks
        for py in [59, 61]:
            renderer.draw_line(sx, py, ex, py)

        # Vertical post supports
        for world_x in [10, 80, 160, 245]:
            px = world_x - offset
            if sx <= px <= ex:
                renderer.draw_line(px, 59, px, 63)

    def _draw_platform_mid(self, renderer, camera_x, parallax):
        """Draw wooden deck planks spanning the treehouse platform."""
        offset = int(camera_x * parallax)

        sx = max(0, 0 - offset)
        ex = min(config.DISPLAY_WIDTH, 300 - offset)
        if sx >= ex:
            return

        # Horizontal planks
        renderer.draw_line(sx, 57, ex, 57)

        # Vertical post supports
        for world_x in [20, 90, 160, 230]:
            px = world_x - offset
            if sx <= px <= ex:
                renderer.draw_line(px, 57, px, 59)
        
        renderer.draw_rect(0 - offset - 1, -1, 10, 61, filled=True, color=0)
        renderer.draw_rect(180 - offset, -1, 10, 61, filled=True, color=0)
        renderer.draw_rect(0 - offset - 1, -1, 10, 61)
        renderer.draw_rect(180 - offset, -1, 10, 61)
