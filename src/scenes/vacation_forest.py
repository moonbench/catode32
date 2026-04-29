import random
import config
from scenes.vacation_scene import VacationScene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from entities.flyer import FlyerEntity
from entities.jumper import JumperEntity
from assets.nature import BUSH, TREE_TRUNK, TREE_TRUNK_SMALL
from assets.plants import GRASS_YOUNG, GRASS_GROWING

_WORLD_WIDTH = 300
_GROUND_Y    = 63

# Large trunk (TREE_TRUNK) column geometry
_TREE_LEFT_LINE        = 6    # offset from trunk left edge for left outline
_TREE_RIGHT_LINE       = 16   # offset from trunk left edge for right outline

# Small trunk (TREE_TRUNK_SMALL) column geometry
_SMALL_TREE_LEFT_LINE  = 4
_SMALL_TREE_RIGHT_LINE = 8

# Trunk top y values — aligns trunk bottom with the layer's bush ground level
# Large trunk (h=13): bottom at y=44 → top at 32; bottom at y=54 → top at 42
# Small trunk (h=9):  bottom at y=44 → top at 36
_BG_TRUNK_TOP_Y       = 32   # large trunk in background
_BG_SMALL_TRUNK_TOP_Y = 36   # small trunk in background
_MG_TRUNK_TOP_Y       = 42   # large trunk in midground

# Background large trees: 3 trees, (world_x, details)
# Details: (x_offset, y_start, y_end); column max y = _BG_TRUNK_TOP_Y - 1 = 31
_BG_LARGE_TREE_X = [
    (18,  [(8, 20, 24), (10, 28, 29)]),
    (82,  [(8, 8, 16), (10, 25, 26)]),
    (150, [(8, 22, 25), (10, 10, 21)]),
]

# Background small trees: 4 trees
# Column max y = _BG_SMALL_TRUNK_TOP_Y - 1 = 35; interior x+5..x+7
_BG_SMALL_TREE_X = [
    (50,  [(6, 23, 27), (6, 33, 33), (6, 13, 10)]),
    (108, [(6, 26, 30), (6, 21, 21), (6, 35, 35)]),
    (132, [(6, 24, 28), (6, 35, 35), (6, 12, 8)]),
    (165, [(6, 28, 32), (6, 10, 8), (6, 34, 34)]),
]

# Midground trees: 2 large trees
# Column max y = _MG_TRUNK_TOP_Y - 1 = 41
_MG_TREE_X = [
    (45,  [(8, 30, 35), (10, 38, 41)]),
    (175, [(8, 27, 31), (10, 36, 38)]),
]

# Background bushes (world_x, y_bottom) — parallax 0.3, so spread across ~0-170
_BG_BUSH_POSITIONS = [
    (5,   45),
    (35,  43),
    (70,  46),
    (100, 44),
    (130, 45),
    (160, 43),
]

# Midground bushes (world_x, y_bottom) — parallax 0.6, spread across ~0-230
_MG_BUSH_POSITIONS = [
    (20,  55),
    (75,  54),
    (130, 55),
    (185, 54),
    (235, 55),
]

# (world_x, sprite, y_bottom)
_SCATTER_FOREGROUND = [
    (10,  GRASS_YOUNG,    63),
    (45,  GRASS_GROWING,  63),
    (85,  GRASS_YOUNG,    63),
    (120, GRASS_GROWING,  63),
    (160, GRASS_YOUNG,    63),
    (195, GRASS_GROWING,  63),
    (235, GRASS_YOUNG,    63),
    (270, GRASS_GROWING,  63),
    (295, GRASS_YOUNG,    63),
]

_SCATTER_MIDGROUND = [
    (30,  GRASS_YOUNG,   56),
    (80,  GRASS_GROWING, 55),
    (115, GRASS_YOUNG,   57),
    (165, GRASS_GROWING, 54),
    (210, GRASS_YOUNG,   56),
    (260, GRASS_GROWING, 55),
]


class VacationForestScene(VacationScene):
    SCENE_NAME     = 'vacation_forest'
    ENJOY_DURATION = 750.0   # ~12.5 minutes
    GRACE_DURATION = 120.0
    STAT_ACCRUAL   = {'serenity': 8.0, 'fulfillment': 8.0}

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._critter_entities = []
        self._jumper_respawn_timers = []   # list of [remaining_secs, variant]

    def setup_scene(self):
        self.environment = Environment(world_width=_WORLD_WIDTH)
        self.context.scene_x_min = 10
        self.context.scene_x_max = _WORLD_WIDTH - 10
        self.character = CharacterEntity(self.ENTRY_X, _GROUND_Y, context=self.context)

        for x, y_bot in _BG_BUSH_POSITIONS:
            self.environment.add_object(
                LAYER_BACKGROUND, BUSH,
                x, y_bot - BUSH["height"]
            )
        for x, y_bot in _MG_BUSH_POSITIONS:
            self.environment.add_object(
                LAYER_MIDGROUND, BUSH,
                x, y_bot - BUSH["height"]
            )

    def _is_daytime(self):
        h = self.context.environment.get('time_hours', 12)
        return 6 <= h < 20

    def _spawn_critters(self):
        if self._is_daytime():
            for _ in range(random.randint(1, 3)):
                self._add_flyer('butterfly', y_min=5, y_max=40)
        else:
            for _ in range(random.randint(1, 3)):
                self._add_flyer('moth', y_min=5, y_max=40)
            for _ in range(random.randint(1, 3)):
                self._add_flyer('firefly', y_min=30, y_max=55)

        for _ in range(random.randint(1, 2)):
            direction = random.choice((-1, 1))
            x = -12 if direction == 1 else _WORLD_WIDTH + 12
            j = JumperEntity('grasshopper', x, direction)
            self._critter_entities.append(j)
            self.environment.add_entity(j)

    def _add_flyer(self, variant, y_min, y_max):
        f = FlyerEntity(
            variant,
            random.randint(20, _WORLD_WIDTH - 20),
            random.randint(y_min, y_max),
        )
        if "speed" not in f._sprite:
            f.anim_speed = random.randint(7, 12)
        f.bounds_left  = 10
        f.bounds_right = _WORLD_WIDTH - 10
        f.bounds_top   = y_min
        f.bounds_bottom = y_max
        self._critter_entities.append(f)
        self.environment.add_entity(f)

    def on_enter(self):
        self.environment.add_custom_draw(LAYER_BACKGROUND, self._draw_trees_background)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_trees_midground)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_scatter_midground)
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_scatter_foreground)
        self._spawn_critters()

    def on_exit(self):
        for entity in self._critter_entities:
            self.environment.remove_entity(entity)
        self._critter_entities.clear()
        self._jumper_respawn_timers.clear()

    def on_update(self, dt):
        self.character.update(dt)

        # Tick grasshopper respawn timers
        i = len(self._jumper_respawn_timers) - 1
        while i >= 0:
            self._jumper_respawn_timers[i][0] -= dt
            if self._jumper_respawn_timers[i][0] <= 0:
                variant = self._jumper_respawn_timers.pop(i)[1]
                direction = random.choice((-1, 1))
                x = -12 if direction == 1 else _WORLD_WIDTH + 12
                j = JumperEntity(variant, x, direction)
                self._critter_entities.append(j)
                self.environment.add_entity(j)
            i -= 1

        # Remove despawned jumpers and maybe schedule a replacement
        i = len(self._critter_entities) - 1
        while i >= 0:
            entity = self._critter_entities[i]
            if getattr(entity, 'despawned', False):
                self._critter_entities.pop(i)
                self.environment.remove_entity(entity)
                if random.random() < 0.6:
                    delay = random.uniform(5, 90)
                    self._jumper_respawn_timers.append([delay, entity.variant])
            i -= 1

        self.environment.update(dt)
        self._tick_vacation(dt)

    def _draw_trees_background(self, renderer, camera_x, parallax):
        self._draw_trees(renderer, camera_x, parallax,
                         _BG_LARGE_TREE_X, _BG_TRUNK_TOP_Y,
                         TREE_TRUNK, _TREE_LEFT_LINE, _TREE_RIGHT_LINE)
        self._draw_trees(renderer, camera_x, parallax,
                         _BG_SMALL_TREE_X, _BG_SMALL_TRUNK_TOP_Y,
                         TREE_TRUNK_SMALL, _SMALL_TREE_LEFT_LINE, _SMALL_TREE_RIGHT_LINE)

    def _draw_trees_midground(self, renderer, camera_x, parallax):
        self._draw_trees(renderer, camera_x, parallax,
                         _MG_TREE_X, _MG_TRUNK_TOP_Y,
                         TREE_TRUNK, _TREE_LEFT_LINE, _TREE_RIGHT_LINE)

    def _draw_trees(self, renderer, camera_x, parallax,
                    positions, trunk_top_y, sprite, left_line, right_line):
        camera_offset = int(camera_x * parallax)
        fill_x = left_line + 1
        fill_w = right_line - left_line - 1
        w = sprite["width"]
        for world_x, details in positions:
            sx = world_x - camera_offset
            if sx + w < 0 or sx >= config.DISPLAY_WIDTH:
                continue
            # Black fill between the two trunk lines, from top of screen to trunk top
            renderer.draw_rect(sx + fill_x, 0, fill_w, trunk_top_y,
                               filled=True, color=0)
            # Left and right outline lines
            renderer.draw_line(sx + left_line,  0, sx + left_line,  trunk_top_y - 1, color=1)
            renderer.draw_line(sx + right_line, 0, sx + right_line, trunk_top_y - 1, color=1)
            # Interior bark detail lines/pixels, slightly varied per tree
            for x_off, y0, y1 in details:
                renderer.draw_line(sx + x_off, y0, sx + x_off, y1, color=1)
            # Trunk sprite at the base of the column (draw_sprite_obj handles fill)
            renderer.draw_sprite_obj(sprite, sx, trunk_top_y)

    def _draw_scatter_foreground(self, renderer, camera_x, parallax):
        self._draw_scatter(renderer, camera_x, parallax, _SCATTER_FOREGROUND)

    def _draw_scatter_midground(self, renderer, camera_x, parallax):
        self._draw_scatter(renderer, camera_x, parallax, _SCATTER_MIDGROUND)

    def _draw_scatter(self, renderer, camera_x, parallax, items):
        camera_offset = int(camera_x * parallax)
        screen_width = config.DISPLAY_WIDTH
        for world_x, sprite, y_bottom in items:
            sx = world_x - camera_offset
            if sx < -sprite["width"] or sx > screen_width + sprite["width"]:
                continue
            renderer.draw_sprite(
                sprite["frames"][0],
                sprite["width"],
                sprite["height"],
                sx, y_bottom - sprite["height"],
                transparent=True,
                transparent_color=0
            )
