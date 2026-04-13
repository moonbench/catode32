import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_FOREGROUND, LAYER_MIDGROUND
from entities.character import CharacterEntity
from assets.furniture import BOOKSHELF, FAUCET
from assets.items import BOX_SMALL_1, FOOD_BOWL
from clock import ClockWidget

class KitchenScene(MainScene):
    SCENE_NAME = 'kitchen'

    PLANT_SURFACES = [
        {'y_snap': 63, 'layer': 'foreground', 'x_min': 0,  'x_max': 180},
        {'y_snap': 60, 'layer': 'midground',  'x_min': 0,  'x_max': 16},
        {'y_snap': 24, 'layer': 'midground',  'x_min': 25, 'x_max': 160},
    ]

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.clock = None

    def setup_scene(self):
        self.environment = Environment(world_width=192)

        self.context.scene_x_min = 10
        self.context.scene_x_max = 182

        self.clock = ClockWidget(world_x=100, world_y=0)

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

    def on_enter(self):
        # Counter is in midground so it sits visually behind the character and floor items
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_counter)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self.clock.draw)

    def on_update(self, dt):
        super().on_update(dt)
        env = self.context.environment
        hours = env.get('time_hours', 12)
        minutes = env.get('time_minutes', 0)
        self.clock.set_time(hours, minutes)

    def _draw_counter(self, renderer, camera_x, parallax):
        """Draw a kitchen counter running most of the world width."""
        offset = int(camera_x * parallax)

        sx = max(0, 25 - offset)
        ex = min(config.DISPLAY_WIDTH, 175 - offset)
        if sx >= ex:
            return

        # Counter top (solid bar)
        renderer.draw_rect(sx - 3, 24, ex - sx + 6, 4, filled=True)

        # Cabinet front outline below counter
        renderer.draw_rect(sx, 28, ex - sx, 30)

        # Cabinet door dividers every 30 world units
        for door_wx in range(40, 175, 30):
            dx = door_wx - offset
            if sx < dx < ex:
                renderer.draw_line(dx, 28, dx, 57)

        renderer.draw_sprite_obj(FAUCET, 82 - offset, 24-FAUCET["height"])