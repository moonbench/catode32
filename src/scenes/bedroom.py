import config
from scenes.main_scene import MainScene
from environment import Environment, LAYER_FOREGROUND, LAYER_MIDGROUND, LAYER_BACKGROUND
from entities.character import CharacterEntity
from assets.furniture import BOOKSHELF, PILLOW, CAT_BED_SIDE
from assets.items import YARN_BALL


class BedroomScene(MainScene):
    SCENE_NAME = 'bedroom'

    PLANT_SURFACES = [
        {'y_snap': 63, 'layer': 'foreground', 'x_min': 34,  'x_max': 182},
        {'y_snap': 15, 'layer': 'foreground', 'x_min': 0,   'x_max': 33},
        {'y_snap': 60, 'layer': 'midground',  'x_min': 34,   'x_max': 90},
        {'y_snap': 16, 'layer': 'midground',  'x_min': 184, 'x_max': 188},
        {'y_snap': 56, 'layer': 'background', 'x_min': 34,   'x_max': 80},
    ]

    def setup_scene(self):
        self.environment = Environment(256)
        self.context.scene_x_min = 10
        self.context.scene_x_max = 246

        # Left wall: bookshelf with a box on top
        self.environment.add_object(
            LAYER_FOREGROUND, BOOKSHELF,
            x=0, y=63 - BOOKSHELF["height"]
        )

        # Pillow on bed
        self.environment.add_object(
            LAYER_MIDGROUND, PILLOW,
            x=158, y=23
        )

        # Yarn ball on the floor (toy)
        self.environment.add_object(
            LAYER_MIDGROUND, YARN_BALL,
            x=82, y=63 - YARN_BALL["height"]
        )

        self.context.scene_x_min = 10
        self.context.scene_x_max = 182

        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

    def on_enter(self):
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_bed)
        self.environment.add_custom_draw(LAYER_BACKGROUND, self._draw_lamp)
        self.context.cat_bed_x = 154  # approx foreground world-x of the bed interior (tunable)

    def on_exit(self):
        self.context.cat_bed_x = None
        if self.character:
            self.character.draw_y_offset = 0

    def _draw_bed(self, renderer, camera_x, parallax):
        """Draw a simple bed frame against the right side of the room."""
        offset = int(camera_x * parallax)

        bed_x = 108 - offset

        # Mask
        renderer.draw_rect(bed_x, 32, 82, 20, filled=True, color=0)

        # Pillow mask
        renderer.draw_rect(bed_x+50, 23, 23, 10, filled=True, color=0)
        renderer.draw_rect(bed_x+73, 25, 2, 8, filled=True, color=0)

        # Frame
        renderer.draw_rect(bed_x, 50, 80, 5, filled=True)
        renderer.draw_rect(bed_x, 55, 8, 9, filled=True)
        renderer.draw_rect(bed_x + 80, 16, 8, 48, filled=True)

        # Mattress
        renderer.draw_rect(bed_x, 32, 79, 16)

    def _draw_lamp(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)

        lamp_x = 146 - offset

        # Base
        renderer.draw_line(lamp_x - 5, 63, lamp_x + 5, 63)
        renderer.draw_line(lamp_x - 3, 62, lamp_x + 3, 62)

        # Stem
        renderer.draw_rect(lamp_x - 1, 20, 3, 48, filled=True)

        # Shade
        renderer.draw_line(lamp_x-12, 20, lamp_x+12, 20)
        renderer.draw_line(lamp_x-5, 8, lamp_x+5, 8)
        renderer.draw_line(lamp_x-12, 20, lamp_x-5, 8)
        renderer.draw_line(lamp_x+12, 20, lamp_x+5, 8)

        # Cord
        renderer.draw_line(lamp_x+4, 20, lamp_x+4, 28)

    def on_post_draw(self):
        camera_offset = int(self.environment.camera_x * 1.0)  # foreground parallax
        bed_x = 125 - camera_offset  # match your bed position

        # Left rim of cat bed
        self.renderer.draw_sprite_obj(CAT_BED_SIDE, bed_x, 52)
        # Middle of cat bed
        self.renderer.draw_rect(bed_x + CAT_BED_SIDE["width"], 54, 20, 10, filled=True, color=0)
        self.renderer.draw_line(bed_x + CAT_BED_SIDE["width"], 54, bed_x + CAT_BED_SIDE["width"] + 20, 54)
        self.renderer.draw_line(bed_x + CAT_BED_SIDE["width"], 63, bed_x + CAT_BED_SIDE["width"] + 20, 63)
        self.renderer.draw_line(bed_x + CAT_BED_SIDE["width"], 59, bed_x + CAT_BED_SIDE["width"] + 20, 59)

        # Right rim (mirrored)
        self.renderer.draw_sprite_obj(CAT_BED_SIDE, bed_x + CAT_BED_SIDE["width"] + 20, 52, mirror_h=True)
