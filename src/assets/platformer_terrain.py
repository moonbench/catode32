"""
Terrain tile sprites for the Prowl platformer minigame.
All tiles are 8x8 pixels.

Tile type constants are used in SOLID_CHUNKS block tuples:
  (block_x, block_y, tile_type, variant_idx)

TERRAIN_TILES[tile_type][variant_idx] gives the sprite to draw.
Adding variants never changes the TILE_* constant values.
"""

TILE_TOP          = 0
TILE_TOP_LEFT     = 1
TILE_TOP_RIGHT    = 2
TILE_SIDE_LEFT    = 3
TILE_SIDE_RIGHT   = 4
TILE_BOTTOM       = 5
TILE_BOTTOM_LEFT  = 6
TILE_BOTTOM_RIGHT = 7
# Single-cell-tall floating terrain: top and bottom edges both visible
TILE_TOP_BOTTOM       = 8
TILE_TOP_LEFT_BOTTOM  = 9
TILE_TOP_RIGHT_BOTTOM = 10
# Single-cell-wide column variants: left and right edges both visible
TILE_LEFT_RIGHT            = 11   # column middle (top+bottom buried)
TILE_LEFT_RIGHT_BOTTOM     = 12   # column bottom
TILE_TOP_LEFT_RIGHT        = 13   # column top
TILE_TOP_LEFT_BOTTOM_RIGHT = 14   # fully isolated single block

TERRAIN_TOP = {
    "width": 8, "height": 8,
    "frames": [b"\xff\x00\x77\x77\x00\xa1\x00\x00"],
}

TERRAIN_CORNER_TOP_LEFT = {
    "width": 8, "height": 8,
    "frames": [b"\x7f\x80\xb7\xb7\x80\x88\x40\x20"],
}

TERRAIN_CORNER_TOP_RIGHT = {
    "width": 8, "height": 8,
    "frames": [b"\xfe\x01\x75\x75\x01\xa1\x02\x04"],
}

TERRAIN_SIDE_LEFT = {
    "width": 8, "height": 8,
    "frames": [b"\x20\x28\x20\x20\x22\x20\x24\x20"],
}

TERRAIN_SIDE_RIGHT = {
    "width": 8, "height": 8,
    "frames": [b"\x04\x14\x84\x04\x04\x04\x24\x04"],
}

TERRAIN_CORNER_BOTTOM = {
    "width": 8, "height": 8,
    "frames": [b"\x00\x40\x00\x02\x00\x00\x67\x98"],
}

TERRAIN_CORNER_BOTTOM_LEFT = {
    "width": 8, "height": 8,
    "frames": [b"\x20\x20\x20\x12\x10\x08\x08\x07"],
}

TERRAIN_CORNER_BOTTOM_RIGHT = {
    "width": 8, "height": 8,
    "frames": [b"\x04\x14\x04\x08\x08\x10\x10\xe0"],
}

TERRAIN_TOP_BOTTOM = {
    "width": 8, "height": 8,
    "frames": [b"\xff\x00\x77\x77\x00\xa1\x0c\xf3"],
}

TERRAIN_CORNER_TOP_LEFT_BOTTOM = {
    "width": 8, "height": 8,
    "frames": [b"\x7f\x80\xb7\xb7\x80\x88\x60\x1f"],
}

TERRAIN_CORNER_TOP_RIGHT_BOTTOM = {
    "width": 8, "height": 8,
    "frames": [b"\xfe\x01\x75\x75\x01\xa1\x06\xf8"],
}

TERRAIN_LEFT_RIGHT = {
    "width": 8, "height": 8,
    "frames": [b"\x81\x81\x81\x41\x42\x81\x81\x81"],
}

TERRAIN_LEFT_RIGHT_BOTTOM = {
    "width": 8, "height": 8,
    "frames": [b"\x81\x81\x81\x41\x41\x42\x22\x1c"],
}

TERRAIN_TOP_LEFT_RIGHT = {
    "width": 8, "height": 8,
    "frames": [b"\xff\x81\xb5\xb5\x81\xa1\x85\x81"],
}

TERRAIN_TOP_LEFT_BOTTOM_RIGHT = {
    "width": 8, "height": 8,
    "frames": [b"\xff\x81\xb5\xa5\x81\x92\x42\x3c"],
}

PLATFORMER_CHECKPOINT_DOWN = {
    "width": 24, "height": 8,
    "frames": [b"\x1e\x00\x00\x21\x00\x00\x2d\x7f\xff\x2d\x7f\xff\x21\x00\x00\x21\x00\x7e\x61\x87\xfc\xff\xcf\xf8"],
}

PLATFORMER_CHECKPOINT_UP = {
    "width": 15, "height": 24,
    "frames": [b"\x0d\x80\x0d\xc0\x0d\xe0\x0d\xf0\x0d\xf8\x0d\xfc\x0c\x7e\x0c\x1e\x0c\x00\x0c\x00\x0c\x00\x0c\x00\x0c\x00\x0c\x00\x0c\x00\x00\x00\x1e\x00\x21\x00\x2d\x00\x2d\x00\x21\x00\x21\x00\x61\x80\xff\xc0"],
}

PLATFORMER_DOOR_LOCKED = {
    "width": 16, "height": 19,
    "frames": [b"\x33\xcc\x78\x1e\xff\xff\xe0\x07\xc0\x03\xd2\x4b\xdf\xfb\xd2\x4b\xd2\x4b\xd2\x4b\xd2\x4b\xd2\x4b\xd2\x4b\xd2\x4b\xd2\x4b\xd2\x4b\xdf\xfb\xd2\x4b\xd2\x4b"],
}

PLATFORMER_DOOR = {
    "width": 16, "height": 19,
    "frames": [b"\x33\xcc\x78\x1e\xff\xff\xe0\x07\xc0\x03\xd0\x03\xd8\x03\xdc\x03\xde\x03\xdc\x03\xde\x83\xdc\x43\xde\x83\xdc\x43\xde\x83\xdc\x43\xde\x83\xdc\x43\xde\x83"],
}

# Indexed by TILE_* constants — TERRAIN_TILES[tile_type][variant_idx] gives the sprite.
# Add more sprites to an inner tuple to introduce variants for that tile type.
TERRAIN_TILES = (
    (TERRAIN_TOP,),                      # 0  TILE_TOP
    (TERRAIN_CORNER_TOP_LEFT,),          # 1  TILE_TOP_LEFT
    (TERRAIN_CORNER_TOP_RIGHT,),         # 2  TILE_TOP_RIGHT
    (TERRAIN_SIDE_LEFT,),                # 3  TILE_SIDE_LEFT
    (TERRAIN_SIDE_RIGHT,),               # 4  TILE_SIDE_RIGHT
    (TERRAIN_CORNER_BOTTOM,),            # 5  TILE_BOTTOM
    (TERRAIN_CORNER_BOTTOM_LEFT,),       # 6  TILE_BOTTOM_LEFT
    (TERRAIN_CORNER_BOTTOM_RIGHT,),      # 7  TILE_BOTTOM_RIGHT
    (TERRAIN_TOP_BOTTOM,),               # 8  TILE_TOP_BOTTOM
    (TERRAIN_CORNER_TOP_LEFT_BOTTOM,),   # 9  TILE_TOP_LEFT_BOTTOM
    (TERRAIN_CORNER_TOP_RIGHT_BOTTOM,),  # 10 TILE_TOP_RIGHT_BOTTOM
    (TERRAIN_LEFT_RIGHT,),               # 11 TILE_LEFT_RIGHT
    (TERRAIN_LEFT_RIGHT_BOTTOM,),        # 12 TILE_LEFT_RIGHT_BOTTOM
    (TERRAIN_TOP_LEFT_RIGHT,),           # 13 TILE_TOP_LEFT_RIGHT
    (TERRAIN_TOP_LEFT_BOTTOM_RIGHT,),    # 14 TILE_TOP_LEFT_BOTTOM_RIGHT
)
