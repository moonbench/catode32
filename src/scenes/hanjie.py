from lang import t
"""
Hanjie (Nonogram) minigame - fill the grid using row/column clues.

Layout (128x64 screen):
  x=0..23   row clue area (right-aligned, up to "111" = 3 chars, no spaces)
  x=24..71  6-column grid  (6 cols * 8px, grows to 7/8/9 by round 3/6/9)
  x=80..99  timer / spare
  x=100..127 cat

  y=0..15   column clue area (2 rows of 8px for up to 2 groups)
  y=16..63  6-row grid  (6 rows * 8px)

Row clues omit spaces between digits (all single-digit since max = COLS).
"""
import random
from scene import Scene
from entities.character import CharacterEntity
from ui import Popup

COLS = 6
ROWS = 6
CELL = 8

GRID_X = 24   # left pixel of column 0
ROW_CLUE_PITCH = 7  # pixels between digit starts in row clues (digits drawn individually)
GRID_Y = 16   # top pixel of row 0

# Cell states
UNKNOWN = 0
FILLED  = 1
CROSSED = 2   # known-empty mark (backslash)

# Game states
STATE_PLAYING = 0
STATE_WIN     = 1

WIN_RESET_DELAY = 2.5


# ---------------------------------------------------------------------------
# Puzzle generation
# ---------------------------------------------------------------------------

def _run_lengths(cells, length):
    """Return run-length groups for a slice of `cells` (list/bytearray of 0/1)."""
    groups = []
    count = 0
    for i in range(length):
        if cells[i]:
            count += 1
        elif count:
            groups.append(count)
            count = 0
    if count:
        groups.append(count)
    return groups if groups else [0]


def _compute_clues(sol, cols):
    """Compute row and column clues from a solution bytearray."""
    row_clues = []
    for r in range(ROWS):
        row_clues.append(_run_lengths(
            [sol[r * cols + c] for c in range(cols)], cols))

    col_clues = []
    for c in range(cols):
        col_clues.append(_run_lengths(
            [sol[r * cols + c] for r in range(ROWS)], ROWS))

    return row_clues, col_clues


def _generate_puzzle(cols):
    """Generate a random solution + clues satisfying display constraints:
       - each column has at most 2 clue groups
       - each row has at most 3 clue groups
       - at least 5 cells filled (non-trivial)
    """
    for _ in range(200):
        sol = bytearray(cols * ROWS)
        for i in range(cols * ROWS):
            sol[i] = 1 if random.randint(0, 1) else 0

        filled = sum(sol)
        if filled < 5 or filled > cols * ROWS - 3:
            continue

        row_clues, col_clues = _compute_clues(sol, cols)

        if (all(len(r) <= 3 for r in row_clues) and
                all(len(c) <= 2 for c in col_clues)):
            return sol, row_clues, col_clues

    # Fallback: guaranteed-valid checkerboard-ish puzzle
    sol = bytearray(cols * ROWS)
    for r in range(ROWS):
        for c in range(cols):
            sol[r * cols + c] = 1 if (r + c) % 2 == 0 else 0
    row_clues, col_clues = _compute_clues(sol, cols)
    return sol, row_clues, col_clues


# ---------------------------------------------------------------------------
# Win checking
# ---------------------------------------------------------------------------

def _check_win(board, row_clues, col_clues, cols):
    """Return True if the board's filled cells match all clues."""
    for r in range(ROWS):
        groups = _run_lengths(
            [1 if board[r * cols + c] == FILLED else 0 for c in range(cols)],
            cols)
        if groups != row_clues[r]:
            return False

    for c in range(cols):
        groups = _run_lengths(
            [1 if board[r * cols + c] == FILLED else 0 for r in range(ROWS)],
            ROWS)
        if groups != col_clues[c]:
            return False

    return True


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _format_time(secs):
    s = int(secs)
    m = s // 60
    s = s % 60
    if m:
        return str(m) + ":" + ("0" if s < 10 else "") + str(s)
    return str(s) + "s"


# ---------------------------------------------------------------------------
# Scene
# ---------------------------------------------------------------------------

class HanjieScene(Scene):
    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self.character = None
        self.solution = None
        self.board = None
        self.row_clues = None
        self.col_clues = None
        self.cols = COLS
        self.cursor = 0
        self.state = STATE_PLAYING
        self.elapsed = 0.0
        self.win_timer = 0.0
        self.win_popup = None

    def load(self):
        super().load()
        self.character = CharacterEntity(100, 63)
        self.character.set_pose("sitting.side.neutral")
        self.win_popup = Popup(self.renderer, x=10, y=14, width=108, height=36)

    def unload(self):
        super().unload()

    def enter(self):
        self._init_game()

    def exit(self):
        completions = getattr(self, '_session_completions', 0)
        if completions > 0:
            reward = (completions / 3.0) ** 0.5
            best_time = getattr(self, '_session_best_time', 0.0)
            time_bonus = max(0.0, 1.0 - best_time / 180.0) if best_time > 0.0 else 0.0
            print(f"Reward: {reward}")
            print(f"Time bonus: {time_bonus}")
            self.context.apply_stat_changes({
                'intelligence': 4 * reward + 2 * time_bonus,
                'focus':        3 * reward,
                'serenity':     3 * reward,
                'sociability':  2,
                'loyalty':      0.5 * reward,
            })
            coins = int(5 * reward)
            if coins > 0:
                self.context.coins += coins
                print(f"[Zoomies] Awarded {coins} coins (total: {self.context.coins})")

    def _init_game(self):
        # Accumulate completion stats before resetting state
        if getattr(self, 'state', STATE_PLAYING) == STATE_WIN:
            self._session_completions = getattr(self, '_session_completions', 0) + 1
            t = getattr(self, 'elapsed', 0.0)
            prev_best = getattr(self, '_session_best_time', 0.0)
            self._session_best_time = t if prev_best == 0.0 else min(prev_best, t)

        completions = getattr(self, '_session_completions', 0)
        if completions >= 8:
            self.cols = 9
        elif completions >= 5:
            self.cols = 8
        elif completions >= 2:
            self.cols = 7
        else:
            self.cols = COLS  # 6

        self.solution, self.row_clues, self.col_clues = _generate_puzzle(self.cols)
        self.board = bytearray(self.cols * ROWS)  # all UNKNOWN
        self.cursor = 0
        self.state = STATE_PLAYING
        self.elapsed = 0.0
        self.win_timer = 0.0
        if self.character:
            self.character.x = 108 if self.cols >= 9 else 100
            self.character.set_pose("sitting.side.neutral")

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        inp = self.input

        if self.state == STATE_WIN:
            if inp.was_just_pressed('a'):
                self._init_game()
            return

        col = self.cursor % self.cols
        row = self.cursor // self.cols

        if inp.was_just_pressed('up') and row > 0:
            self.cursor -= self.cols
        elif inp.was_just_pressed('down') and row < ROWS - 1:
            self.cursor += self.cols
        elif inp.was_just_pressed('left') and col > 0:
            self.cursor -= 1
        elif inp.was_just_pressed('right') and col < self.cols - 1:
            self.cursor += 1

        if inp.was_just_pressed('a'):
            cur = self.board[self.cursor]
            if cur == FILLED:
                self.board[self.cursor] = UNKNOWN
            else:
                self.board[self.cursor] = FILLED
            self._check_win_state()

        elif inp.was_just_pressed('b'):
            cur = self.board[self.cursor]
            if cur == CROSSED:
                self.board[self.cursor] = UNKNOWN
            else:
                self.board[self.cursor] = CROSSED
            self._check_win_state()

    def _check_win_state(self):
        if not _check_win(self.board, self.row_clues, self.col_clues, self.cols):
            return

        self.state = STATE_WIN
        self.win_timer = 0.0

        time_str = _format_time(self.elapsed)
        best = self.context.hanjie_best_time
        if best < 0 or self.elapsed < best:
            self.context.hanjie_best_time = self.elapsed
            best = self.elapsed
        best_str = _format_time(best)

        self.win_popup.set_text(
            t("Well done!") + "\n" + t("Time: {v}", v=time_str) + "\n" + t("Best: {v}", v=best_str),
            wrap=False, center=True)

        if self.character:
            self.character.set_pose("sitting.side.happy")

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        if self.character:
            self.character.update(dt)

        if self.state == STATE_PLAYING:
            self.elapsed += dt
        elif self.state == STATE_WIN:
            self.win_timer += dt
            if self.win_timer >= WIN_RESET_DELAY:
                self._init_game()

    # ------------------------------------------------------------------
    # Draw
    # ------------------------------------------------------------------

    def draw(self):
        r = self.renderer
        self._draw_col_clues(r)
        self._draw_row_clues(r)
        self._draw_grid(r)
        if self.character:
            self.character.draw(r)
        if self.state == STATE_WIN:
            self.win_popup.draw(show_scroll_indicators=False)

    def _draw_col_clues(self, r):
        """Draw column clues above the grid (y=0..15)."""
        for c in range(self.cols):
            cx = GRID_X + c * CELL
            clue = self.col_clues[c]
            if len(clue) == 2:
                r.draw_text(str(clue[0]), cx, 0)
                r.draw_text(str(clue[1]), cx, 8)
            else:
                # Single group: bottom-align so it sits just above the grid
                r.draw_text(str(clue[0]), cx, 8)

    def _draw_row_clues(self, r):
        """Draw row clues to the left of the grid, right-aligned.

        Each digit is placed individually at ROW_CLUE_PITCH intervals (6px),
        which is narrower than the 8px font width, giving compact but readable
        spacing without ambiguity (all values are single-digit).
        """
        for row in range(ROWS):
            ry = GRID_Y + row * CELL
            clue = self.row_clues[row]
            n = len(clue)
            # Right-align: last digit ends flush with GRID_X
            x0 = GRID_X - n * ROW_CLUE_PITCH
            for j, g in enumerate(clue):
                r.draw_text(str(g), x0 + j * ROW_CLUE_PITCH, ry)

    def _draw_grid(self, r):
        """Draw all cells with cursor highlight."""
        for i in range(self.cols * ROWS):
            col = i % self.cols
            row = i // self.cols
            cx = GRID_X + col * CELL
            cy = GRID_Y + row * CELL
            state = self.board[i]
            is_cursor = (i == self.cursor)

            if state == FILLED:
                # Filled square (1px inset so grid lines show)
                r.draw_rect(cx + 1, cy + 1, CELL - 2, CELL - 2, filled=True)
                if is_cursor:
                    # Small black dot in centre to mark cursor position
                    r.draw_rect(cx + 3, cy + 3, 2, 2, filled=True, color=0)

            elif state == CROSSED:
                r.draw_rect(cx + 1, cy + 1, CELL - 2, CELL - 2, filled=False)
                # Backslash mark
                r.draw_line(cx + 2, cy + 2, cx + CELL - 3, cy + CELL - 3)
                if is_cursor:
                    r.draw_rect(cx, cy, CELL, CELL, filled=False)

            else:  # UNKNOWN
                if is_cursor:
                    # Filled inner square = cursor indicator on empty cell
                    r.draw_rect(cx + 1, cy + 1, CELL - 2, CELL - 2, filled=True)
                else:
                    r.draw_rect(cx + 1, cy + 1, CELL - 2, CELL - 2, filled=False)

