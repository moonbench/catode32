import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_FOREGROUND, LAYER_MIDGROUND
from entities.character import CharacterEntity
from assets.furniture import BOOKSHELF
from assets.nature import PLANTER1, PLANT3
from assets.items import BOX_SMALL_1, YARN_BALL


class BedroomScene(MainScene):
    SCENE_NAME = 'bedroom'
    MODULES_TO_KEEP = ['assets.furniture', 'assets.nature', 'assets.items']

    def setup_scene(self):
        self.environment = Environment(world_width=192)

        # Left wall: bookshelf with a box on top
        self.environment.add_object(
            LAYER_FOREGROUND, BOOKSHELF,
            x=0, y=63 - BOOKSHELF["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, BOX_SMALL_1,
            x=2, y=63 - BOOKSHELF["height"] - BOX_SMALL_1["height"]
        )

        # Yarn ball on the floor (toy)
        self.environment.add_object(
            LAYER_FOREGROUND, YARN_BALL,
            x=80, y=63 - YARN_BALL["height"]
        )

        self.context.scene_x_min = 10
        self.context.scene_x_max = 182

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

    def on_enter(self):
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_bed)

    def _draw_bed(self, renderer, camera_x, parallax):
        """Draw a simple bed frame against the right side of the room."""
        offset = int(camera_x * parallax)

        bed_x = 70 - offset

        # Frame
        renderer.draw_rect(bed_x, 50, 80, 5, filled=True)
        renderer.draw_rect(bed_x, 55, 8, 9, filled=True)
        renderer.draw_rect(bed_x + 80, 16, 8, 48, filled=True)

        # Mattress
        renderer.draw_rect(bed_x, 32, 79, 16)

        # Pillow
        px = 141 - offset
        renderer.draw_rect(bed_x + 50, 22, 29, 10)
