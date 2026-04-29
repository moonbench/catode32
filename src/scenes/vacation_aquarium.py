import random
import config
from scenes.vacation_scene import VacationScene
from environment import Environment, LAYER_MIDGROUND
from entities.character import CharacterEntity
from entities.aquarium_creatures import FishEntity, OctopusEntity, BubbleGroup, DebrisField
from assets.nature import ROCK_PILE, SEAWEED, SEA_PLANT, FISH1, FISH2, FISH3, OCTOPUS
from assets.plants import TINY_FLOWER

_WORLD_WIDTH = 342
_GROUND_Y    = 63

# Tank window geometry (world coordinates, midground 0.6x parallax)
_TANK1_LEFT  = 4
_TANK1_RIGHT = 123   # 120px wide
_TANK2_LEFT  = 132
_TANK2_RIGHT = 251   # 120px wide
_TANK_TOP    = 0
_TANK_FLOOR  = 50    # y of tank floor; rocks/plants sit here

_ROCK_W = ROCK_PILE["width"]    # 47
_ROCK_H = ROCK_PILE["height"]   # 23
_ROCK_Y = _TANK_FLOOR - _ROCK_H # 27 — rock sprite top edge

# Rock 1: partially behind left wall (10px hidden), 37px visible in tank 1
_ROCK1_X = _TANK1_LEFT - 14     # -10
# Rock 2: straddles divider — 20px visible in each tank
_ROCK2_X = _TANK1_RIGHT - 19    # 104
# Rock 3: partially behind right wall (14px hidden), 33px visible in tank 2
_ROCK3_X = _TANK2_RIGHT - 33    # 218

_SW_W = SEAWEED["width"]    # 4
_SW_H = SEAWEED["height"]   # 15
_SW_FRAMES = 8
_SW_FRAME_INTERVAL = 0.3    # seconds per frame

# (world_x, y_bottom, frame_offset)
# Offsets spread across 8 frames so strands move independently.
_SEAWEED_POS = [
    (8,   _ROCK_Y,     0),   # on rock 1, left
    (22,  _ROCK_Y+4,   3),   # on rock 1, right
    (50,  _TANK_FLOOR, 6),   # floor, tank 1
    (82,  _TANK_FLOOR, 1),   # floor, tank 1
    (88,  _TANK_FLOOR, 5),   # floor, tank 1
    (110, _ROCK_Y+12,  5),   # on rock 2, tank 1 side
    (140, _ROCK_Y+4,   2),   # on rock 2, tank 2 side
    (162, _TANK_FLOOR, 1),   # floor, tank 2
    (168, _TANK_FLOOR, 7),   # floor, tank 2
    (205, _TANK_FLOOR, 4),   # floor, tank 2
    (230, _ROCK_Y+4,   2),   # on rock 3
]

_SP_W = SEA_PLANT["width"]    # 17
_SP_H = SEA_PLANT["height"]   # 11

# (world_x, y_top) — one in each tank, on the floor between rocks
_SEA_PLANT_POS = [
    (55,  _TANK_FLOOR - _SP_H),   # tank 1, between rock 1 and rock 2
    (180, _TANK_FLOOR - _SP_H),   # tank 2, between rock 2 and rock 3
]

# Creature swim bounds (world x — sprite right edge max = bounds_right)
_C_X_LEFT  = _TANK1_LEFT + 2        # 6
_C_X_RIGHT = _TANK2_RIGHT - 2       # 249
_C_Y_TOP   = _TANK_TOP + 3          # 8

# (sprite, start_x, start_y, speed, y_bottom_for_this_sprite)
# y_bottom is the max y for the sprite's top-left corner (sprite top edge).
_FISH_SPAWNS = [
    (FISH1, 30,  14, 22.0, _TANK_FLOOR - FISH1["height"] - 2),
    (FISH1, 190, 28, 24.0, _TANK_FLOOR - FISH1["height"] - 2),
    (FISH1, 100, 28, 24.0, _TANK_FLOOR - FISH1["height"] - 2),
    (FISH1, 220, 22, 24.0, _TANK_FLOOR - FISH1["height"] - 2),
    (FISH2, 60,  20, 16.0, _TANK_FLOOR - FISH2["height"] - 2),
    (FISH2, 270, 35, 15.0, _TANK_FLOOR - FISH2["height"] - 2),
    (FISH2, 200, 35, 15.0, _TANK_FLOOR - FISH2["height"] - 2),
    (FISH3, 100, 30, 12.0, _TANK_FLOOR - FISH3["height"] - 2),
    (FISH3, 200, 20, 12.0, _TANK_FLOOR - FISH3["height"] - 2),
]

_OCT_START_X = 45
_OCT_START_Y = 28   # octopus h=14; floor is 50, so 50-14-8=28 keeps it mid-water

# Interior x ranges for bubbles and debris (1px inside the tank borders)
_TANK_RANGES = [
    (_TANK1_LEFT + 1, _TANK1_RIGHT - 1),
    (_TANK2_LEFT + 1, _TANK2_RIGHT - 1),
]

_DEBRIS_COUNT = 10


class VacationAquariumScene(VacationScene):
    SCENE_NAME     = 'vacation_aquarium'
    ENJOY_DURATION = 750.0   # ~12.5 minutes
    GRACE_DURATION = 120.0
    STAT_ACCRUAL   = {'serenity': 8.0, 'fulfillment': 8.0}

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._sw_timer = 0.0
        self._sw_frame = 0
        self._fish     = []
        self._octopus  = None
        self._bubbles  = None
        self._debris   = None

    def setup_scene(self):
        self.environment = Environment(world_width=_WORLD_WIDTH)
        self.context.scene_x_min = 10
        self.context.scene_x_max = _WORLD_WIDTH - 10
        self.character = CharacterEntity(self.ENTRY_X, _GROUND_Y, context=self.context)

    def on_enter(self):
        for spr, sx, sy, spd, y_bot in _FISH_SPAWNS:
            self._fish.append(FishEntity(
                spr, sx, sy, speed=spd,
                bounds_left=_C_X_LEFT, bounds_right=_C_X_RIGHT,
                bounds_top=_C_Y_TOP,   bounds_bottom=y_bot,
            ))

        self._octopus = OctopusEntity(
            OCTOPUS, _OCT_START_X, _OCT_START_Y,
            bounds_left=_C_X_LEFT,
            bounds_right=_C_X_RIGHT - OCTOPUS["width"],
        )

        self._bubbles = BubbleGroup(
            TINY_FLOWER, _TANK_RANGES, _TANK_TOP + 1, _TANK_FLOOR - 1,
        )
        self._debris = DebrisField(
            _DEBRIS_COUNT, _TANK_RANGES, _TANK_TOP + 1, _TANK_FLOOR - 1,
        )

        # Draw order: rocks/plants → seaweed → creatures → bubbles/debris → occluders → outlines
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_rocks_and_plants)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_seaweed)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_creatures)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_particles)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_occluders)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_tank_outlines)

    def on_exit(self):
        self._fish.clear()
        self._octopus = None
        self._bubbles = None
        self._debris  = None

    def on_update(self, dt):
        self._sw_timer += dt
        if self._sw_timer >= _SW_FRAME_INTERVAL:
            self._sw_timer -= _SW_FRAME_INTERVAL
            self._sw_frame = (self._sw_frame + 1) % _SW_FRAMES
        for fish in self._fish:
            fish.update(dt)
        if self._octopus:
            self._octopus.update(dt)
        if self._bubbles:
            self._bubbles.update(dt)
        if self._debris:
            self._debris.update(dt)
        self.character.update(dt)
        self.environment.update(dt)
        self._tick_vacation(dt)

    # ------------------------------------------------------------------
    # Custom draw passes
    # ------------------------------------------------------------------

    def _draw_rocks_and_plants(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)
        sw = config.DISPLAY_WIDTH

        for wx in (_ROCK1_X, _ROCK2_X, _ROCK3_X):
            sx = wx - offset
            if sx + _ROCK_W >= 0 and sx < sw:
                renderer.draw_sprite_obj(ROCK_PILE, sx, _ROCK_Y)

        for wx, sy in _SEA_PLANT_POS:
            sx = wx - offset
            if sx + _SP_W >= 0 and sx < sw:
                renderer.draw_sprite(SEA_PLANT["frames"][0], _SP_W, _SP_H, sx, sy,
                                     transparent=True, transparent_color=0)

    def _draw_seaweed(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)
        sw = config.DISPLAY_WIDTH
        frames = SEAWEED["frames"]
        for wx, y_bot, frame_off in _SEAWEED_POS:
            sx = wx - offset
            if sx + _SW_W >= 0 and sx < sw:
                frame_data = frames[(self._sw_frame + frame_off) % _SW_FRAMES]
                renderer.draw_sprite(frame_data, _SW_W, _SW_H, sx, y_bot - _SW_H,
                                     transparent=True, transparent_color=0)

    def _draw_creatures(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)
        if self._octopus:
            self._octopus.draw(renderer, camera_offset=offset)
        for fish in self._fish:
            fish.draw(renderer, camera_offset=offset)

    def _draw_particles(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)
        if self._bubbles:
            self._bubbles.draw(renderer, camera_offset=offset)
        if self._debris:
            self._debris.draw(renderer, camera_offset=offset)

    def _draw_occluders(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)
        sw     = config.DISPLAY_WIDTH
        tank_h = _TANK_FLOOR - _TANK_TOP + 1   # 46px

        # Left wall: screen left edge up to (but not including) tank 1's left border
        left_sx = _TANK1_LEFT - offset
        if left_sx > 0:
            renderer.draw_rect(0, _TANK_TOP, left_sx, tank_h, filled=True, color=0)

        # Divider between tank 1 and tank 2
        div_sx = _TANK1_RIGHT + 1 - offset
        div_w  = _TANK2_LEFT - _TANK1_RIGHT - 1   # 8px
        if div_sx < sw and div_sx + div_w > 0:
            renderer.draw_rect(div_sx, _TANK_TOP, div_w, tank_h, filled=True, color=0)

        # Right wall: just past tank 2's right border to screen right edge
        right_sx = _TANK2_RIGHT + 1 - offset
        if right_sx < sw:
            renderer.draw_rect(right_sx, _TANK_TOP, sw - right_sx, tank_h, filled=True, color=0)

    def _draw_tank_outlines(self, renderer, camera_x, parallax):
        offset = int(camera_x * parallax)
        x1 = _TANK1_LEFT - offset
        renderer.draw_rect(x1, _TANK_TOP,
                           _TANK1_RIGHT - _TANK1_LEFT + 1,
                           _TANK_FLOOR - _TANK_TOP + 1,
                           filled=False, color=1)
        x2 = _TANK2_LEFT - offset
        renderer.draw_rect(x2, _TANK_TOP,
                           _TANK2_RIGHT - _TANK2_LEFT + 1,
                           _TANK_FLOOR - _TANK_TOP + 1,
                           filled=False, color=1)
