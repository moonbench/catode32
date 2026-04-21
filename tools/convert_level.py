#!/usr/bin/env python3
"""
Convert a platformer level text file to a Python module for PetPython.

Usage:
    python tools/convert_level.py <input.txt> <level_name>

    input.txt   – path to the level text file (e.g. levels/level_01.txt)
    level_name  – output module name without extension (e.g. level_01)
                  written to src/platformer_levels/<level_name>.py

Character key
─────────────
  1   solid terrain block, variant 0
  2   solid terrain block, variant 1
  _   one-way platform (consecutive underscores on the same row → one entry)
  g   grass decoration (sprite picked randomly at parse time, baked into output)
  V   vines decoration (hangs downward from the marker row)
  S   slime enemy spawn point
  $   player spawn point
  @   checkpoint (respawn point; multiple allowed, activated in order of contact)
  #   level exit door (paired with destination lines at the bottom of the file)
  X   locked door — requires the key to open (also paired with a destination line)
  K   key pickup (bobbing collectible; obtaining it unlocks all locked doors)
  .   empty (any other character is also treated as empty)

Door destinations
─────────────────
  After the grid, add lines starting with '-' to name destination levels for
  each '#' or 'X' door, matched in reading order (top-to-bottom, left-to-right)
  across both door types combined.
  An optional blank line may separate the grid from the destination block.

  Example:
      - level_02
      - level_03_bonus

World coordinates
─────────────────
  Each cell is 8×8 px.  Column c → world_x = c*8.  Row r → world_y = r*8.
  The bottom row of the grid is the bottom of the world.
  WORLD_W = num_cols * 8,  WORLD_H = num_rows * 8.

Spawn positions
───────────────
  g / S / $ placed at row r, col c → surface / feet at y = (r+1)*8,
  i.e. one row below the marker cell (the marker sits visually above the
  terrain it refers to).

Tile auto-detection
───────────────────
  For each terrain cell the four cardinal neighbours are checked to decide
  which TILE_* constant to assign.  Fully interior cells (all four neighbours
  are also terrain) are skipped — no tile is emitted for them.
"""

import os
import random
import sys

# ── Match platformer.py constants ────────────────────────────────────────────
BLOCK_W = 8
BLOCK_H = 8
CHUNK_W = 128
CHUNK_H = 64

# Tile type constants — must stay in sync with platformer_terrain.py
TILE_TOP              = 0
TILE_TOP_LEFT         = 1
TILE_TOP_RIGHT        = 2
TILE_SIDE_LEFT        = 3
TILE_SIDE_RIGHT       = 4
TILE_BOTTOM           = 5
TILE_BOTTOM_LEFT      = 6
TILE_BOTTOM_RIGHT     = 7
TILE_TOP_BOTTOM       = 8
TILE_TOP_LEFT_BOTTOM  = 9
TILE_TOP_RIGHT_BOTTOM = 10
TILE_LEFT_RIGHT            = 11
TILE_LEFT_RIGHT_BOTTOM     = 12
TILE_TOP_LEFT_RIGHT        = 13
TILE_TOP_LEFT_BOTTOM_RIGHT = 14

GRASS_VARIANTS = 5   # indices 0..4 (SEEDLING → THRIVING)


# ── Helpers ───────────────────────────────────────────────────────────────────

def _is_terrain(grid, r, c, num_rows, num_cols):
    if r < 0 or r >= num_rows or c < 0 or c >= num_cols:
        return False
    return grid[r][c] in ('1', '2')


def _tile_type(grid, r, c, num_rows, num_cols):
    """Return the TILE_* constant for the terrain cell at (r, c), or None if interior."""
    above = _is_terrain(grid, r - 1, c, num_rows, num_cols)
    below = _is_terrain(grid, r + 1, c, num_rows, num_cols)
    left  = _is_terrain(grid, r, c - 1, num_rows, num_cols)
    right = _is_terrain(grid, r, c + 1, num_rows, num_cols)

    top_exp = not above   # top edge is exposed (no terrain above)
    bot_exp = not below   # bottom edge is exposed (no terrain below)

    if not top_exp and not bot_exp:
        # Middle row — top and bottom both buried
        if not left and not right:
            return TILE_LEFT_RIGHT        # single-wide column middle
        if not left:
            return TILE_SIDE_LEFT
        if not right:
            return TILE_SIDE_RIGHT
        return None   # fully interior — skip

    if top_exp and not bot_exp:
        # Top row
        if not left and not right:
            return TILE_TOP_LEFT_RIGHT    # top of a single-wide column
        if not left:
            return TILE_TOP_LEFT
        if not right:
            return TILE_TOP_RIGHT
        return TILE_TOP

    if not top_exp and bot_exp:
        # Bottom row
        if not left and not right:
            return TILE_LEFT_RIGHT_BOTTOM  # bottom of a single-wide column
        if not left:
            return TILE_BOTTOM_LEFT
        if not right:
            return TILE_BOTTOM_RIGHT
        return TILE_BOTTOM

    # Single-height (top and bottom both exposed)
    if not left and not right:
        return TILE_TOP_LEFT_BOTTOM_RIGHT  # fully isolated single block
    if not left:
        return TILE_TOP_LEFT_BOTTOM
    if not right:
        return TILE_TOP_RIGHT_BOTTOM
    return TILE_TOP_BOTTOM


# ── Main converter ────────────────────────────────────────────────────────────

def convert(txt_path, out_name):
    with open(txt_path) as fh:
        all_lines = fh.read().splitlines()

    # Peel door-destination lines from the bottom (lines starting with '-').
    # An optional blank separator line between the grid and destinations is skipped.
    dest_lines = []
    i = len(all_lines) - 1
    while i >= 0 and all_lines[i].strip().startswith('-'):
        dest_lines.insert(0, all_lines[i].strip()[1:].strip())
        i -= 1
    while i >= 0 and not all_lines[i].strip():   # skip blank separator
        i -= 1
    lines = all_lines[:i + 1]

    # Normalise to a rectangular grid
    num_cols = max((len(l) for l in lines), default=0)
    grid     = [l.ljust(num_cols, '.') for l in lines]
    num_rows = len(grid)

    if num_rows == 0 or num_cols == 0:
        print("ERROR: empty level file", file=sys.stderr)
        sys.exit(1)

    world_w = num_cols * BLOCK_W
    world_h = num_rows * BLOCK_H

    solid        = {}   # (chunk_col, chunk_row) → list of (bx, by, tile_type, variant)
    platforms    = []   # (px, py, pw)
    grass        = []   # (wx, surface_y, sprite_idx)
    vines        = []   # (wx, top_y)
    slime_spawns = []   # (world_x, feet_y)
    checkpoints  = []   # (wx_left, wy_bottom) — bottom-left of sprite
    all_door_positions = []  # (wx_left, wy_bottom, is_locked) — reading order, paired with dest_lines
    key_spawns   = []   # (wx_center, wy_feet)
    player_spawn = None

    for r in range(num_rows):
        c = 0
        while c < num_cols:
            ch = grid[r][c]

            if ch in ('1', '2'):
                variant = 0 if ch == '1' else 1
                tt = _tile_type(grid, r, c, num_rows, num_cols)
                if tt is not None:
                    bx  = c * BLOCK_W
                    by  = r * BLOCK_H
                    key = (bx // CHUNK_W, by // CHUNK_H)
                    solid.setdefault(key, []).append((bx, by, tt, variant))

            elif ch == '_':
                # Consume the full run of underscores as a single platform
                start_c = c
                while c < num_cols and grid[r][c] == '_':
                    c += 1
                px = start_c * BLOCK_W
                py = r * BLOCK_H
                pw = (c - start_c) * BLOCK_W
                platforms.append((px, py, pw))
                continue   # c already advanced past the run

            elif ch == 'g':
                wx = c * BLOCK_W + BLOCK_W // 2
                sy = (r + 1) * BLOCK_H
                grass.append((wx, sy, random.randint(0, GRASS_VARIANTS - 1)))

            elif ch == 'V':
                wx = c * BLOCK_W + BLOCK_W // 2
                ty = r * BLOCK_H
                vines.append((wx, ty))

            elif ch == 'S':
                wx = c * BLOCK_W + BLOCK_W // 2
                fy = (r + 1) * BLOCK_H
                slime_spawns.append((wx, fy))

            elif ch == '$':
                wx = c * BLOCK_W + BLOCK_W // 2
                fy = (r + 1) * BLOCK_H
                player_spawn = (wx, fy)

            elif ch == '@':
                wx = c * BLOCK_W
                wy = (r + 1) * BLOCK_H
                checkpoints.append((wx, wy))

            elif ch == '#':
                wx = c * BLOCK_W
                wy = (r + 1) * BLOCK_H
                all_door_positions.append((wx, wy, False))

            elif ch == 'X':
                wx = c * BLOCK_W
                wy = (r + 1) * BLOCK_H
                all_door_positions.append((wx, wy, True))

            elif ch == 'K':
                wx = c * BLOCK_W + BLOCK_W // 2
                wy = (r + 1) * BLOCK_H
                key_spawns.append((wx, wy))

            c += 1

    if player_spawn is None:
        print("WARNING: no player spawn ($) found — defaulting to (8, 8)", file=sys.stderr)
        player_spawn = (8, 8)

    if len(all_door_positions) != len(dest_lines):
        print(f"WARNING: {len(all_door_positions)} '#'/'X' doors but {len(dest_lines)} '-' destinations",
              file=sys.stderr)
    doors = []
    locked_doors = []
    for (wx, wy, is_locked), dest in zip(all_door_positions, dest_lines):
        if is_locked:
            locked_doors.append((wx, wy, dest))
        else:
            doors.append((wx, wy, dest))

    # Pre-bucket grass into chunks (same scheme as SOLID_CHUNKS)
    grass_chunks = {}
    for wx, sy, si in grass:
        key = (wx // CHUNK_W, sy // CHUNK_H)
        grass_chunks.setdefault(key, []).append((wx, sy, si))

    # Pre-bucket vines into chunks
    vine_chunks = {}
    for wx, ty in vines:
        key = (wx // CHUNK_W, ty // CHUNK_H)
        vine_chunks.setdefault(key, []).append((wx, ty))

    # Freeze lists → tuples
    solid_chunks = {k: tuple(v) for k, v in solid.items()}
    grass_chunks = {k: tuple(v) for k, v in grass_chunks.items()}
    vine_chunks  = {k: tuple(v) for k, v in vine_chunks.items()}

    # ── Write output ──────────────────────────────────────────────────────────
    script_dir = os.path.dirname(os.path.abspath(__file__))
    out_dir    = os.path.join(script_dir, '..', 'src', 'platformer_levels')
    out_path   = os.path.normpath(os.path.join(out_dir, out_name + '.py'))

    with open(out_path, 'w') as fh:
        src_rel = os.path.relpath(txt_path, os.path.dirname(out_path))
        fh.write(f'# Generated by tools/convert_level.py from {os.path.basename(txt_path)}\n')
        fh.write('# Do not edit manually — regenerate from the source .txt file.\n\n')

        fh.write(f'WORLD_W = {world_w}\n')
        fh.write(f'WORLD_H = {world_h}\n\n')

        fh.write(f'PLAYER_SPAWN = {player_spawn!r}\n\n')

        fh.write('SLIME_SPAWNS = (\n')
        for s in slime_spawns:
            fh.write(f'    {s!r},\n')
        fh.write(')\n\n')

        fh.write('SOLID_CHUNKS = {\n')
        for key in sorted(solid_chunks):
            fh.write(f'    {key!r}: (\n')
            for block in solid_chunks[key]:
                fh.write(f'        {block!r},\n')
            fh.write('    ),\n')
        fh.write('}\n\n')

        fh.write('GRASS_CHUNKS = {\n')
        for key in sorted(grass_chunks):
            fh.write(f'    {key!r}: (\n')
            for g in grass_chunks[key]:
                fh.write(f'        {g!r},\n')
            fh.write('    ),\n')
        fh.write('}\n\n')

        fh.write('VINE_CHUNKS = {\n')
        for key in sorted(vine_chunks):
            fh.write(f'    {key!r}: (\n')
            for v in vine_chunks[key]:
                fh.write(f'        {v!r},\n')
            fh.write('    ),\n')
        fh.write('}\n\n')

        fh.write('CHECKPOINTS = (\n')
        for cp in checkpoints:
            fh.write(f'    {cp!r},\n')
        fh.write(')\n\n')

        fh.write('DOORS = (\n')
        for d in doors:
            fh.write(f'    {d!r},\n')
        fh.write(')\n\n')

        fh.write('LOCKED_DOORS = (\n')
        for d in locked_doors:
            fh.write(f'    {d!r},\n')
        fh.write(')\n\n')

        fh.write('KEY_SPAWNS = (\n')
        for k in key_spawns:
            fh.write(f'    {k!r},\n')
        fh.write(')\n\n')

        fh.write('PLATFORMS = (\n')
        for p in platforms:
            fh.write(f'    {p!r},\n')
        fh.write(')\n')

    # ── Summary ───────────────────────────────────────────────────────────────
    total_blocks = sum(len(v) for v in solid_chunks.values())
    print(f'Written : {out_path}')
    print(f'World   : {world_w}×{world_h} px  ({num_cols}×{num_rows} cells)')
    print(f'Blocks  : {total_blocks}')
    print(f'Platforms: {len(platforms)}')
    print(f'Grass   : {len(grass)}')
    print(f'Vines   : {len(vines)}')
    print(f'Slimes  : {len(slime_spawns)}')
    print(f'Checkpoints: {len(checkpoints)}')
    print(f'Doors   : {len(doors)}')
    print(f'Locked doors: {len(locked_doors)}')
    print(f'Keys    : {len(key_spawns)}')
    print(f'Player  : {player_spawn}')


if __name__ == '__main__':
    if len(sys.argv) != 3:
        print('Usage: python tools/convert_level.py <input.txt> <level_name>')
        sys.exit(1)
    convert(sys.argv[1], sys.argv[2])
