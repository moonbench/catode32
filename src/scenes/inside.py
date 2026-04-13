import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from assets.furniture import BOOKSHELF
from assets.items import BOX_SMALL_1
from sky import SkyRenderer
from clock import ClockWidget


class InsideScene(MainScene):
    SCENE_NAME = 'inside'

    # Valid surfaces for plant placement.
    # x_exclude: (x_min, x_max) world range where placement is blocked (behind bookshelf).
    # x_min / x_max: restrict surface to a world x range.
    PLANT_SURFACES = [
        {'y_snap': 63, 'layer': 'foreground', 'x_min': 26, 'x_max': 182},
        {'y_snap': 60, 'layer': 'midground',  'x_min': 26, 'x_max': 150},
        {'y_snap': 29, 'layer': 'midground',  'x_min': 95, 'x_max': 150},
    ]

    # Window position and size (world x, screen y, width, height)
    WINDOW_WORLD_X = 100
    WINDOW_Y = -10
    WINDOW_W = 56
    WINDOW_H = 36

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.sky = SkyRenderer()
        self._last_weather = None
        self.clock = None

    def setup_scene(self):
        self.environment = Environment(world_width=192)

        # Furniture (foreground)
        self.environment.add_object(
            LAYER_FOREGROUND, BOOKSHELF,
            x=0, y=63 - BOOKSHELF["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, BOX_SMALL_1,
            x=2, y=63 - BOOKSHELF["height"] - BOX_SMALL_1["height"]
        )

        # Set movement bounds for behaviors like zoomies (world coordinates)
        self.context.scene_x_min = 10
        self.context.scene_x_max = 182

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

        self.clock = ClockWidget(world_x=36, world_y=0)

    def on_enter(self):
        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width, seed=self.context.pet_seed)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)
        self._last_weather = env_settings.get('weather', 'Clear')
        self.environment.add_custom_draw(LAYER_MIDGROUND, self.sky.make_precipitation_drawer(0.3, 0))
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_window)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self.clock.draw)

    def on_exit(self):
        self.sky.remove_from_environment(self.environment, LAYER_BACKGROUND)

    def on_update(self, dt):
        env = self.context.environment
        hours = env.get('time_hours', 12)
        minutes = env.get('time_minutes', 0)
        self.sky.set_time(hours, minutes)
        self.clock.set_time(hours, minutes)

        # Re-enter if weather changed so clouds, precipitation, etc. all rebuild
        current_weather = env.get('weather', 'Clear')
        if current_weather != self._last_weather:
            self.exit()
            self.enter()

        self.sky.update(dt)
        self.character.update(dt)

    def on_pre_draw(self):
        # Sky render rect must match the window's midground position (0.6x parallax)
        # so the clip region aligns with the frame when environment.draw() runs.
        win_sx = self.WINDOW_WORLD_X - int(self.environment.camera_x * 0.6)
        self.sky._render_rect = (win_sx, self.WINDOW_Y, self.WINDOW_W, self.WINDOW_H)

    def on_post_draw(self):
        # Lightning outside flashes the whole room
        self.renderer.invert(self.sky.get_lightning_invert_state())

    def _draw_window(self, renderer, camera_x, parallax):
        """Draw window mask and frame on the midground layer (0.6x parallax)."""
        win_sx = self.WINDOW_WORLD_X - int(camera_x * parallax)
        wall_bottom = self.WINDOW_Y + self.WINDOW_H
        screen_left = max(0, win_sx)
        screen_right = min(config.DISPLAY_WIDTH, win_sx + self.WINDOW_W)

        # Mask everything outside the window opening. Full DISPLAY_HEIGHT on the sides
        # so tall sprites (balloon, plane) that extend below the window bottom are covered.
        # Foreground furniture draws on top of these rects afterward.
        if screen_left > 0:
            renderer.draw_rect(0, 0, screen_left, config.DISPLAY_HEIGHT, filled=True, color=0)
        if screen_right < config.DISPLAY_WIDTH:
            renderer.draw_rect(screen_right, 0, config.DISPLAY_WIDTH - screen_right, config.DISPLAY_HEIGHT, filled=True, color=0)
        if self.WINDOW_Y > 0:
            renderer.draw_rect(screen_left, 0, screen_right - screen_left, self.WINDOW_Y, filled=True, color=0)
        if wall_bottom < config.DISPLAY_HEIGHT and screen_right > screen_left:
            renderer.draw_rect(screen_left, wall_bottom, screen_right - screen_left, config.DISPLAY_HEIGHT - wall_bottom, filled=True, color=0)

        # Window frame and sill
        renderer.draw_rect(win_sx, self.WINDOW_Y, self.WINDOW_W, self.WINDOW_H)
        renderer.draw_rect(win_sx - 4, self.WINDOW_Y - 4, self.WINDOW_W + 8, self.WINDOW_H + 8)
        renderer.draw_rect(win_sx - 6, self.WINDOW_Y + self.WINDOW_H + 4, self.WINDOW_W + 12, 3, filled=True)
