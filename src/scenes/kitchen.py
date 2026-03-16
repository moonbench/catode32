import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_FOREGROUND, LAYER_MIDGROUND
from entities.character import CharacterEntity
from assets.furniture import BOOKSHELF
from assets.nature import PLANTER1, PLANT1, PLANT3
from assets.items import BOX_SMALL_1, FOOD_BOWL
from clock import ClockWidget

class KitchenScene(MainScene):
    SCENE_NAME = 'kitchen'
    MODULES_TO_KEEP = ['assets.furniture', 'assets.nature']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.clock = None

    def setup_scene(self):
        self.environment = Environment(world_width=192)

        # Plant on the floor
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=10, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=9, y=63 - PLANTER1["height"] - PLANT1["height"]
        )

        # Small plant on the counter
        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER1,
            x=155, y=24 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT1,
            x=154, y=24 - PLANTER1["height"] - PLANT1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER1,
            x=62, y=24 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT3,
            x=63, y=24 - PLANTER1["height"] - PLANT3["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER1,
            x=45, y=24 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT1,
            x=44, y=24 - PLANTER1["height"] - PLANT1["height"]
        )

        self.context.scene_x_min = 10
        self.context.scene_x_max = 182

        self.clock = ClockWidget(world_x=110, world_y=0)

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

    def on_enter(self):
        # Counter is in midground so it sits visually behind the character and floor items
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_counter)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self.clock.draw)

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
