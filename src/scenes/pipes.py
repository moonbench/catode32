"""
Pipes minigame — rotate pipe pieces to route water from inlet to outlet.
Water flows automatically; arrange pieces ahead of the flow to win.
"""
import random
from scene import Scene
from assets.minigame_assets import (
    PIPE_STRAIGHT, PIPE_CORNER, PIPE_FAT,
    PIPE_OUTLET_LEFT, PIPE_OUTLET_RIGHT,
)
from menu import Menu, MenuItem
from ui import Popup

# Grid dimensions
CELL      = 9        # pixel size per cell
PLAY_COLS = 12        # rotatable columns (grid cols 1-8)
PLAY_ROWS = 7        # rows
OUTLET_ROW = 3       # row index of inlet and outlet pipes

# Derived grid layout constants
INLET_COL  = 0
OUTLET_COL = PLAY_COLS + 1          # = 9
TOTAL_COLS = PLAY_COLS + 2          # = 10 (inlet + play + outlet)

# Screen layout
GRID_X = 0
GRID_Y = 0
CHAR_X = 107
CHAR_Y = 63

# Flow timing
FLOW_SPEED_NORMAL = 6.0    # px/s  (1 cell/s)
FLOW_SPEED_FAT    = 2.0    # px/s  (2 s/cell)
START_DELAY       = 16.0    # seconds of setup time before flow starts
# Rise animation fills the full vertical channel (PLAY_ROWS * CELL px) over START_DELAY seconds
INLET_RISE_SPEED  = float(PLAY_ROWS * CELL) / START_DELAY / 2.0  # px/s
WIN_DELAY         = 2.5    # auto-reset after win
BROKEN_DELAY      = 2.0    # auto-reset after break

# Directions: L=0, R=1, U=2, D=3
_L, _R, _U, _D = 0, 1, 2, 3
# _OPPOSITE[d] = opposite direction (indexed by direction int)
_OPPOSITE  = (_R, _L, _D, _U)
# row / col deltas per direction
_DELTA_ROW = (0,  0, -1, 1)
_DELTA_COL = (-1, 1,  0, 0)

# Pipe type constants
P_STRAIGHT = 0
P_CORNER   = 1
P_FAT      = 2

# Connection table: _CONNS[ptype][rot] = (dir_a, dir_b)
_CONNS = (
    ((_L, _R), (_U, _D)),                              # P_STRAIGHT
    ((_R, _D), (_L, _D), (_L, _U), (_R, _U)),          # P_CORNER
    ((_L, _R), (_U, _D)),                              # P_FAT
)

# Sprite objects per pipe type
_SPRITES = (PIPE_STRAIGHT, PIPE_CORNER, PIPE_FAT)

# Game states
STATE_FLOWING = 0
STATE_WIN     = 1
STATE_BROKEN  = 2


def _get_exit(ptype, rot, entry_dir):
    """Return exit direction for a pipe given entry direction, or -1 if no connection."""
    max_rot = 4 if ptype == P_CORNER else 2
    conn = _CONNS[ptype][rot % max_rot]
    if conn[0] == entry_dir:
        return conn[1]
    if conn[1] == entry_dir:
        return conn[0]
    return -1


def _pipe_for(entry, exit_):
    """Return (ptype, solution_rot) for a pipe connecting entry and exit openings."""
    # Straight / fat
    if (entry == _L and exit_ == _R) or (entry == _R and exit_ == _L):
        ptype = P_FAT if random.randint(0, 4) == 0 else P_STRAIGHT
        return ptype, 0
    if (entry == _U and exit_ == _D) or (entry == _D and exit_ == _U):
        ptype = P_FAT if random.randint(0, 4) == 0 else P_STRAIGHT
        return ptype, 1
    # Corner — find which rotation connects these two openings
    for rot in range(4):
        c = _CONNS[P_CORNER][rot]
        if (c[0] == entry and c[1] == exit_) or (c[1] == entry and c[0] == exit_):
            return P_CORNER, rot
    return P_STRAIGHT, 0  # unreachable


def _gen_solution(ptypes_out, rots_out, on_path_out):
    """Fill in a guaranteed-solvable path from inlet to outlet using DFS.

    Writes pipe types and solution rotations into the output arrays for cells
    on the path.  Non-path cells are left at 0 (caller fills them randomly).
    Falls back to a boring straight line if DFS fails (shouldn't happen).
    """
    visited = bytearray(PLAY_ROWS * PLAY_COLS)

    def solve(row, col, entry):
        # Reached the outlet column?
        if col == OUTLET_COL:
            return row == OUTLET_ROW
        # Out of play bounds?
        if col < 1 or col > PLAY_COLS or row < 0 or row >= PLAY_ROWS:
            return False
        pi = row * PLAY_COLS + (col - 1)
        if visited[pi]:
            return False
        visited[pi] = 1

        # Build exit candidates [R, U, D] with a right-first bias
        cands = [_R, _U, _D]
        for i in range(2, 0, -1):  # Fisher-Yates on last two
            j = random.randint(0, i)
            cands[i], cands[j] = cands[j], cands[i]
        if random.randint(0, 9) < 7 and cands[0] != _R:
            idx = 1 if cands[1] == _R else 2
            cands[0], cands[idx] = cands[idx], cands[0]

        for exit_dir in cands:
            if exit_dir == entry:       # can't exit back the way we came
                continue
            nr = row + _DELTA_ROW[exit_dir]
            nc = col + _DELTA_COL[exit_dir]
            if solve(nr, nc, _OPPOSITE[exit_dir]):
                ptype, rot = _pipe_for(entry, exit_dir)
                ptypes_out[pi] = ptype
                rots_out[pi]   = rot
                on_path_out[pi] = 1
                return True

        visited[pi] = 0  # backtrack
        return False

    if not solve(OUTLET_ROW, 1, _L):
        # Fallback: straight line through outlet row
        for c in range(1, PLAY_COLS + 1):
            pi = OUTLET_ROW * PLAY_COLS + (c - 1)
            ptypes_out[pi]  = P_STRAIGHT
            rots_out[pi]    = 0
            on_path_out[pi] = 1


class PipeScene(Scene):
    """Pipe routing minigame"""

    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self.options_menu = Menu(renderer, input_handler)
        self.menu_active = False
        self.result_popup = None
        self._session_wins = 0

        # Board state
        self.ptypes = bytearray(PLAY_ROWS * PLAY_COLS)
        self.rots   = bytearray(PLAY_ROWS * PLAY_COLS)

        # Fill state: 1 = fully filled by water
        # Index: row * TOTAL_COLS + col  (col 0=inlet, 1-8=play, 9=outlet)
        self.cell_filled = bytearray(PLAY_ROWS * TOTAL_COLS)

        # Cursor in play-area coordinates
        self.cur_row = OUTLET_ROW
        self.cur_col = 0   # 0-indexed within play cols

        # Flow state
        self.state            = STATE_FLOWING
        self.flow_row         = OUTLET_ROW
        self.flow_col         = INLET_COL
        self.flow_entry       = _L
        self.flow_exit        = _R
        self.flow_progress    = 0.0
        self.flow_speed       = FLOW_SPEED_NORMAL
        self.base_flow_speed  = FLOW_SPEED_NORMAL   # scaled per difficulty
        self.inlet_rise_speed = INLET_RISE_SPEED    # scaled per difficulty
        self.start_timer      = START_DELAY
        self.end_timer        = 0.0
        self.inlet_rise_px    = 0.0
        self.speed_mult       = 1.0

    # -------------------------------------------------------------------------
    # Lifecycle
    # -------------------------------------------------------------------------

    def load(self):
        super().load()
        self.result_popup = Popup(self.renderer, x=10, y=4, width=108, height=46)

    def unload(self):
        super().unload()

    def enter(self):
        self._session_wins = 0
        self._init_game()

    def exit(self):
        if self._session_wins > 0:
            scale = (self._session_wins / 3.0) ** 0.5
            self.context.apply_stat_changes({
                'intelligence': 3 * scale,
                'focus':        4 * scale,
                'loyalty':      0.5 * scale,
            })
            coins = int(2 * self._session_wins)
            if coins > 0:
                self.context.coins += coins
                print(f"[Pipes] Awarded {coins} coins (total: {self.context.coins})")

    # -------------------------------------------------------------------------
    # Game logic
    # -------------------------------------------------------------------------

    def _init_game(self):
        """Generate a solvable board and scramble rotations."""
        total = PLAY_ROWS * PLAY_COLS

        # Difficulty ramp: more required turns and faster flow each win
        min_corners = min(2 + self._session_wins, 10)
        speed_scale = 1.0 + 0.1 * self._session_wins
        self.base_flow_speed  = min(FLOW_SPEED_NORMAL * speed_scale, FLOW_SPEED_NORMAL * 3.0)
        self.inlet_rise_speed = min(INLET_RISE_SPEED  * speed_scale, INLET_RISE_SPEED  * 3.0)

        # Step 1: generate solution path — retry until it has enough turns
        sol_ptypes  = bytearray(total)
        sol_rots    = bytearray(total)
        on_path     = bytearray(total)
        while True:
            _gen_solution(sol_ptypes, sol_rots, on_path)
            corner_count = 0
            for i in range(total):
                if on_path[i] and sol_ptypes[i] == P_CORNER:
                    corner_count += 1
            if corner_count >= min_corners:
                break
            for i in range(total):
                sol_ptypes[i] = 0
                sol_rots[i]   = 0
                on_path[i]    = 0

        # Step 2: fill non-path cells with random pipe types
        for i in range(total):
            if not on_path[i]:
                r = random.randint(0, 9)
                if r < 5:
                    sol_ptypes[i] = P_STRAIGHT
                elif r < 9:
                    sol_ptypes[i] = P_CORNER
                else:
                    sol_ptypes[i] = P_FAT

        # Step 3: copy types to board and scramble rotations
        # Path cells get a random rotation that differs from the solution
        # so the player always has something to fix.
        for i in range(total):
            self.ptypes[i] = sol_ptypes[i]
            max_r = 4 if sol_ptypes[i] == P_CORNER else 2
            rot = random.randint(0, max_r - 1)
            if on_path[i] and rot == sol_rots[i]:
                rot = (rot + 1) % max_r
            self.rots[i] = rot

        for i in range(len(self.cell_filled)):
            self.cell_filled[i] = 0

        self.flow_row      = OUTLET_ROW
        self.flow_col      = INLET_COL
        self.flow_entry    = _L
        self.flow_exit     = _R
        self.flow_progress = 0.0
        self.flow_speed    = self.base_flow_speed
        self.start_timer   = START_DELAY
        self.end_timer     = 0.0
        self.inlet_rise_px = 0.0
        self.state         = STATE_FLOWING
        self.cur_row       = OUTLET_ROW
        self.cur_col       = 0

    def _advance_flow(self):
        """Mark current cell filled and move flow to the next cell."""
        self.cell_filled[self.flow_row * TOTAL_COLS + self.flow_col] = 1
        self.flow_progress -= CELL

        dr = _DELTA_ROW[self.flow_exit]
        dc = _DELTA_COL[self.flow_exit]
        next_row = self.flow_row + dr
        next_col = self.flow_col + dc

        # Reached outlet column?
        if next_col == OUTLET_COL:
            if next_row == OUTLET_ROW:
                self.flow_row   = next_row
                self.flow_col   = next_col
                self.flow_entry = _OPPOSITE[self.flow_exit]
                self.flow_exit  = _R
                self.flow_speed = self.base_flow_speed
            else:
                self._set_broken()
            return

        # Out of play bounds?
        if next_row < 0 or next_row >= PLAY_ROWS or next_col < INLET_COL or next_col > OUTLET_COL:
            self._set_broken()
            return

        # Play cell (cols 1-8)
        if INLET_COL < next_col < OUTLET_COL:
            pi         = next_row * PLAY_COLS + (next_col - 1)
            ptype      = self.ptypes[pi]
            rot        = self.rots[pi]
            entry_dir  = _OPPOSITE[self.flow_exit]
            exit_dir   = _get_exit(ptype, rot, entry_dir)
            if exit_dir < 0:
                self._set_broken()
                return
            self.flow_row   = next_row
            self.flow_col   = next_col
            self.flow_entry = entry_dir
            self.flow_exit  = exit_dir
            fat_speed = self.base_flow_speed * (FLOW_SPEED_FAT / FLOW_SPEED_NORMAL)
            self.flow_speed = fat_speed if ptype == P_FAT else self.base_flow_speed
        else:
            self._set_broken()

    def _set_broken(self):
        self.state     = STATE_BROKEN
        self.end_timer = 0.0
        self.result_popup.set_text("Burst!\n\nA: New Game", wrap=False, center=True)

    def _set_win(self):
        self.cell_filled[self.flow_row * TOTAL_COLS + self.flow_col] = 1
        self.state = STATE_WIN
        self.end_timer = 0.0
        self._session_wins += 1
        self.result_popup.set_text(f"Connected!\nWins: {self._session_wins}\nA: New Game", wrap=False, center=True)

    # -------------------------------------------------------------------------
    # Input
    # -------------------------------------------------------------------------

    def handle_input(self):
        if self.menu_active:
            result = self.options_menu.handle_input()
            if result == 'closed':
                self.menu_active = False
            elif result == 'new_board':
                self.menu_active = False
                self._init_game()
            return None

        if self.state in (STATE_WIN, STATE_BROKEN):
            if self.input.was_just_pressed('a'):
                self._init_game()
            return None

        if self.input.was_just_pressed('menu2'):
            self.menu_active = True
            self.options_menu.open([MenuItem("New Board", action='new_board')])
            return None

        # Cursor movement (always available)
        if self.input.was_just_pressed('up') and self.cur_row > 0:
            self.cur_row -= 1
        elif self.input.was_just_pressed('down') and self.cur_row < PLAY_ROWS - 1:
            self.cur_row += 1
        elif self.input.was_just_pressed('left') and self.cur_col > 0:
            self.cur_col -= 1
        elif self.input.was_just_pressed('right') and self.cur_col < PLAY_COLS - 1:
            self.cur_col += 1

        # Rotate with A — only if cell not yet filled or currently flowing
        if self.input.was_just_pressed('a'):
            grid_col = self.cur_col + 1
            if not self.cell_filled[self.cur_row * TOTAL_COLS + grid_col]:
                if not (self.flow_row == self.cur_row and self.flow_col == grid_col):
                    pi    = self.cur_row * PLAY_COLS + self.cur_col
                    ptype = self.ptypes[pi]
                    max_r = 4 if ptype == P_CORNER else 2
                    self.rots[pi] = (self.rots[pi] + 1) % max_r

        self.speed_mult = 7.0 if self.input.is_pressed('b') else 1.0

        return None

    # -------------------------------------------------------------------------
    # Update
    # -------------------------------------------------------------------------

    def update(self, dt):
        if self.state == STATE_FLOWING:
            # Advance inlet rise animation (runs until fully risen, independent of flow)
            grid_h = PLAY_ROWS * CELL
            if self.inlet_rise_px < grid_h:
                self.inlet_rise_px = min(self.inlet_rise_px + self.inlet_rise_speed * dt * self.speed_mult,
                                         float(grid_h))

            if self.start_timer > 0:
                # Flow starts once water rises to the horizontal center of the outlet row
                outlet_threshold = grid_h - (OUTLET_ROW * CELL + CELL // 2)
                if self.inlet_rise_px >= outlet_threshold:
                    self.start_timer = 0
                return

            self.flow_progress += self.flow_speed * dt * self.speed_mult

            # Advance through fully-filled cells
            while self.flow_progress >= CELL:
                # If we just finished filling the outlet, that's a win
                if self.flow_col == OUTLET_COL:
                    self._set_win()
                    break
                self._advance_flow()
                if self.state != STATE_FLOWING:
                    break

        elif self.state in (STATE_WIN, STATE_BROKEN):
            self.end_timer += dt
            delay = WIN_DELAY if self.state == STATE_WIN else BROKEN_DELAY
            if self.end_timer >= delay:
                self._init_game()

    # -------------------------------------------------------------------------
    # Draw
    # -------------------------------------------------------------------------

    def draw(self):
        r = self.renderer

        if self.menu_active:
            self.options_menu.draw()
            return

        self._draw_pipes(r)
        self._draw_water(r)
        self._draw_cursor(r)

        if self.state in (STATE_WIN, STATE_BROKEN):
            self.result_popup.draw(show_scroll_indicators=False)

        self._draw_inlet_rise(r)

    def _draw_pipes(self, r):
        """Draw all pipe sprites, filled or empty based on flow progress."""
        # Left/right border lines
        grid_h = PLAY_ROWS * CELL
        r.draw_line(GRID_X+1, GRID_Y, GRID_X+1, GRID_Y + grid_h - 1)
        r.draw_line(GRID_X+6, GRID_Y, GRID_X+6, GRID_Y + 28)
        r.draw_line(GRID_X+6, GRID_Y + 36, GRID_X+6, GRID_Y + grid_h - 1)
        r.draw_line(GRID_X + TOTAL_COLS * CELL - 2, GRID_Y, GRID_X + TOTAL_COLS * CELL - 2, GRID_Y + grid_h - 1)
        r.draw_line(GRID_X + TOTAL_COLS * CELL - 7, GRID_Y, GRID_X + TOTAL_COLS * CELL - 7, GRID_Y + 28)
        r.draw_line(GRID_X + TOTAL_COLS * CELL - 7, GRID_Y + 36, GRID_X + TOTAL_COLS * CELL - 7, GRID_Y + grid_h - 1)

        # Inlet (right-facing outlet: wall on left, opens right)
        filled = self.cell_filled[OUTLET_ROW * TOTAL_COLS + INLET_COL]
        cx = GRID_X + INLET_COL * CELL
        cy = GRID_Y + OUTLET_ROW * CELL
        r.draw_sprite(PIPE_OUTLET_RIGHT["frames"][filled], 9, 9, cx, cy, transparent=True)

        # Outlet (left-facing outlet: wall on right, opens left)
        filled = self.cell_filled[OUTLET_ROW * TOTAL_COLS + OUTLET_COL]
        cx = GRID_X + OUTLET_COL * CELL
        cy = GRID_Y + OUTLET_ROW * CELL
        r.draw_sprite(PIPE_OUTLET_LEFT["frames"][filled], 9, 9, cx, cy, transparent=True)

        # Play cells
        for row in range(PLAY_ROWS):
            for pc in range(PLAY_COLS):
                grid_col = pc + 1
                pi    = row * PLAY_COLS + pc
                ptype = self.ptypes[pi]
                rot   = self.rots[pi]
                max_r = 4 if ptype == P_CORNER else 2
                rot_c = rot % max_r
                filled = self.cell_filled[row * TOTAL_COLS + grid_col]
                frame = rot_c + (4 if ptype == P_CORNER else 2) if filled else rot_c
                cx = GRID_X + grid_col * CELL
                cy = GRID_Y + row * CELL
                r.draw_sprite(_SPRITES[ptype]["frames"][frame], 9, 9, cx, cy, transparent=True)

    def _draw_water(self, r):
        """Draw water animation overlay for the currently-flowing cell."""
        if self.state != STATE_FLOWING or self.start_timer > 0:
            return

        row = self.flow_row
        col = self.flow_col
        entry = self.flow_entry
        exit_ = self.flow_exit
        p = self.flow_progress

        cx = GRID_X + col * CELL
        cy = GRID_Y + row * CELL
        ip = int(p)
        if ip <= 0:
            return

        # Straight / straight-through (LR or UD)
        if (entry == _L and exit_ == _R) or (entry == _R and exit_ == _L):
            if entry == _L:
                r.draw_rect(cx, cy + 4, ip, 1)
            else:
                r.draw_rect(cx + CELL - ip, cy + 4, ip, 1)
            return

        if (entry == _U and exit_ == _D) or (entry == _D and exit_ == _U):
            if entry == _U:
                r.draw_rect(cx + 4, cy, 1, ip)
            else:
                r.draw_rect(cx + 4, cy + CELL - ip, 1, ip)
            return

        # Corner — split into entry leg and exit leg
        p1 = ip if ip < 5 else 5          # entry leg: 0-5 pixels toward center
        p2 = ip - 4 if ip > 4 else 0      # exit leg: 0-5 pixels from center

        # Entry leg
        if entry == _L:
            r.draw_rect(cx, cy + 4, p1, 1)
        elif entry == _R:
            r.draw_rect(cx + CELL - p1, cy + 4, p1, 1)
        elif entry == _U:
            r.draw_rect(cx + 4, cy, 1, p1)
        else:  # _D
            r.draw_rect(cx + 4, cy + CELL - p1, 1, p1)

        # Exit leg
        if p2 > 0:
            if exit_ == _R:
                r.draw_rect(cx + 4, cy + 4, p2, 1)
            elif exit_ == _L:
                r.draw_rect(cx + 5 - p2, cy + 4, p2, 1)
            elif exit_ == _D:
                r.draw_rect(cx + 4, cy + 4, 1, p2)
            else:  # _U
                r.draw_rect(cx + 4, cy + 5 - p2, 1, p2)

    def _draw_cursor(self, r):
        if self.state != STATE_FLOWING:
            return
        cx = GRID_X + (self.cur_col + 1) * CELL
        cy = GRID_Y + self.cur_row * CELL
        r.draw_rect(cx, cy, CELL, CELL, filled=False)

    def _draw_inlet_rise(self, r):
        """Water rising and staying filled in the left vertical pipe channel."""
        rise_px = int(self.inlet_rise_px)
        if rise_px <= 0:
            return
        grid_h = PLAY_ROWS * CELL
        rise_px = min(rise_px, grid_h)
        top_y = GRID_Y + grid_h - rise_px
        r.draw_rect(GRID_X + 2, top_y, 4, rise_px, filled=True)
