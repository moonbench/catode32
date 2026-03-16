"""
Memory minigame - find matching pairs on an 11x8 grid
"""
import random
from scene import Scene
from entities.character import CharacterEntity
from ui import Popup

COLS = 11
ROWS = 8
CELL = 8            # pixels per cell
TOTAL = COLS * ROWS  # 88
PAIRS = TOTAL // 2  # 44

PANEL_X = COLS * CELL  # 88 - right panel starts here

# Cell states
_HIDDEN         = 0
_FLIPPED        = 1
_SOLVED_SHOWING = 2  # just matched, briefly showing icon
_SOLVED_BLANK   = 3  # done showing, cell is empty

# Game states
STATE_PLAYING  = 0
STATE_MISMATCH = 1
STATE_WIN      = 2

MISMATCH_DELAY    = 1.0   # seconds to show mismatched pair
MATCH_SHOW_DELAY  = 0.75  # seconds to show a newly matched pair
WIN_RESET_DELAY   = 3.0   # seconds before auto-reset on win


def _make_icon(p):
    """Expand a 16-bit pattern into an 8x8 bytearray using 2x2 pixel blocks."""
    data = bytearray(8)
    for row in range(4):
        bits = (p >> (row * 4)) & 0x0F
        b = 0
        for col in range(4):
            if bits & (1 << (3 - col)):
                b |= (0x3 << (6 - col * 2))
        data[row * 2] = b
        data[row * 2 + 1] = b
    return data


def _generate_icons(count):
    """Generate `count` visually distinct 8x8 icons from random 4x4 patterns."""
    icons = []
    used = []
    while len(icons) < count:
        p = random.randint(1, 65534)
        unique = True
        for q in used:
            if q == p:
                unique = False
                break
        if unique:
            used.append(p)
            icons.append(_make_icon(p))
    return icons


class MemoryScene(Scene):
    MODULES_TO_KEEP = []

    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self.character = None
        self.icons = []
        self.cell_icons = []
        self.cell_state = bytearray(TOTAL)
        self.cursor = 0
        self.first_flipped = -1
        self.second_flipped = -1
        self.score = 0
        self.solved_count = 0
        self.state = STATE_PLAYING
        self.mismatch_timer = 0.0
        self.match_show_timer = 0.0
        self.recently_solved = []
        self.win_timer = 0.0

    def load(self):
        super().load()
        self.character = CharacterEntity(102, 63)
        self.character.set_pose("sitting.side.neutral")
        # Centered popup: 100x40 at (14, 12)
        self.win_popup = Popup(self.renderer, x=14, y=12, width=100, height=40)

    def unload(self):
        super().unload()

    def enter(self):
        self._init_game()

    def _init_game(self):
        self.icons = _generate_icons(PAIRS)

        # Two copies of each icon index, shuffled
        assign = list(range(PAIRS)) + list(range(PAIRS))
        for i in range(TOTAL - 1, 0, -1):
            j = random.randint(0, i)
            assign[i], assign[j] = assign[j], assign[i]

        self.cell_icons = assign
        self.cell_state = bytearray(TOTAL)
        self.cursor = 0
        self.first_flipped = -1
        self.second_flipped = -1
        self.score = 0
        self.solved_count = 0
        self.state = STATE_PLAYING
        self.mismatch_timer = 0.0
        self.match_show_timer = 0.0
        self.recently_solved = []
        self.win_timer = 0.0

        if self.character:
            self.character.set_pose("sitting.side.neutral")

    def handle_input(self):
        if self.state == STATE_MISMATCH:
            return

        if self.state == STATE_WIN:
            if self.input.was_just_pressed('a'):
                self._init_game()
            return

        col = self.cursor % COLS
        row = self.cursor // COLS
        inp = self.input

        if inp.was_just_pressed('left') and col > 0:
            self.cursor -= 1
        elif inp.was_just_pressed('right') and col < COLS - 1:
            self.cursor += 1
        elif inp.was_just_pressed('up') and row > 0:
            self.cursor -= COLS
        elif inp.was_just_pressed('down') and row < ROWS - 1:
            self.cursor += COLS

        if inp.was_just_pressed('a'):
            self._try_flip(self.cursor)

    def _try_flip(self, idx):
        st = self.cell_state[idx]
        if st in (_SOLVED_SHOWING, _SOLVED_BLANK):
            return
        if st == _FLIPPED:  # already the first-flipped card (or during mismatch, guarded above)
            return

        self.cell_state[idx] = _FLIPPED

        if self.first_flipped == -1:
            # First card of a pair
            self.first_flipped = idx
        else:
            # Second card
            if self.cell_icons[idx] == self.cell_icons[self.first_flipped]:
                # Match! Transition any previous showing pair to blank first.
                for i in self.recently_solved:
                    self.cell_state[i] = _SOLVED_BLANK
                self.cell_state[idx] = _SOLVED_SHOWING
                self.cell_state[self.first_flipped] = _SOLVED_SHOWING
                self.recently_solved = [self.first_flipped, idx]
                self.match_show_timer = MATCH_SHOW_DELAY
                self.solved_count += 2
                self.first_flipped = -1
                self.character.set_pose("sitting.side.happy")

                if self.solved_count == TOTAL:
                    best = self.context.memory_best_score
                    if best < 0 or self.score < best:
                        self.context.memory_best_score = self.score
                        best = self.score
                    best_line = ("Best: " + str(best)) if best >= 0 else ""
                    self.win_popup.set_text(
                        "You win!\nScore: " + str(self.score) + "\n" + best_line + "\nA: play again",
                        wrap=False, center=True)
                    self.state = STATE_WIN
                    self.win_timer = 0.0
            else:
                # Mismatch
                self.second_flipped = idx
                self.score += 1
                self.state = STATE_MISMATCH
                self.mismatch_timer = 0.0
                self.character.set_pose("sitting.side.annoyed")

    def update(self, dt):
        if self.character:
            self.character.update(dt)

        if self.match_show_timer > 0 and self.state != STATE_WIN:
            self.match_show_timer -= dt
            if self.match_show_timer <= 0:
                for i in self.recently_solved:
                    self.cell_state[i] = _SOLVED_BLANK
                self.recently_solved = []
                if self.state != STATE_MISMATCH:
                    self.character.set_pose("sitting.side.neutral")

        if self.state == STATE_MISMATCH:
            self.mismatch_timer += dt
            if self.mismatch_timer >= MISMATCH_DELAY:
                self.cell_state[self.first_flipped] = _HIDDEN
                self.cell_state[self.second_flipped] = _HIDDEN
                self.first_flipped = -1
                self.second_flipped = -1
                self.state = STATE_PLAYING
                self.character.set_pose("sitting.side.neutral")

        elif self.state == STATE_WIN:
            self.win_timer += dt
            if self.win_timer >= WIN_RESET_DELAY:
                self._init_game()

    def draw(self):
        r = self.renderer

        for i in range(TOTAL):
            col = i % COLS
            row = i // COLS
            cx = col * CELL
            cy = row * CELL
            st = self.cell_state[i]
            is_cursor = (i == self.cursor)

            if st == _SOLVED_BLANK:
                # Empty cell - just a subtle cursor marker if needed
                if is_cursor:
                    r.draw_rect(cx + 2, cy + 2, 4, 4, filled=False)
            elif st == _HIDDEN:
                if is_cursor:
                    r.draw_rect(cx + 1, cy + 1, 6, 6, filled=True)
                else:
                    r.draw_rect(cx + 1, cy + 1, 6, 6, filled=False)
            else:
                # Flipped or solved-showing: draw the icon
                r.draw_sprite(self.icons[self.cell_icons[i]], 8, 8, cx, cy)
                # Cursor outline on non-hidden cells (drawn now; attempt cells redrawn below)
                if is_cursor:
                    r.draw_rect(cx - 2, cy - 2, 12, 12, filled=False)

        # Draw current-attempt cells last so their black background and outline
        # are always on top of the rest of the grid.
        for i in (self.first_flipped,
                  self.second_flipped if self.state == STATE_MISMATCH else -1):
            if i < 0:
                continue
            cx = (i % COLS) * CELL
            cy = (i // COLS) * CELL
            r.draw_rect(cx - 2, cy - 2, 12, 12, filled=True, color=0)
            r.draw_sprite(self.icons[self.cell_icons[i]], 8, 8, cx, cy)
            r.draw_rect(cx - 2, cy - 2, 12, 12, filled=False)

        # Right panel: score at top
        r.draw_text(str(self.score), PANEL_X + 4, 2)

        # Cat avatar
        if self.character:
            self.character.draw(r)

        # Win popup overlay
        if self.state == STATE_WIN:
            self.win_popup.draw(show_scroll_indicators=False)
