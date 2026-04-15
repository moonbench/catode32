"""
Memory minigame - find matching pairs. Board grows over rounds:
  round 1: 9x6, round 2: 10x7, round 3+: 11x8
"""
import random
from scene import Scene
from entities.character import CharacterEntity
from ui import Popup

MAX_COLS = 11
MAX_ROWS = 8
CELL = 8            # pixels per cell
MAX_TOTAL = MAX_COLS * MAX_ROWS  # 88 - used as reward cap
MAX_PAIRS = MAX_TOTAL // 2       # 44

# Board sizes per round (0-indexed): 9x6, 10x7, 11x8+
_BOARD_SIZES = ((9, 6), (10, 7), (11, 8))

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


# Predefined 4x4 glyphs (44 total = PAIRS). Each glyph is 4 bytes;
# the high nibble of each byte encodes one row of 4 pixels.
_GLYPHS = bytes([
    0x90, 0x90, 0x00, 0x90,  # memory_1
    0x90, 0xd0, 0xb0, 0x90,  # memory_2
    0xf0, 0x90, 0xf0, 0x90,  # memory_3
    0xc0, 0xe0, 0xb0, 0x90,  # memory_4
    0xc0, 0x40, 0xf0, 0x90,  # memory_5
    0xd0, 0xd0, 0xd0, 0xd0,  # memory_6
    0xb0, 0x80, 0x10, 0xd0,  # memory_7
    0xc0, 0xc0, 0x30, 0x30,  # memory_8
    0xd0, 0x40, 0x20, 0xb0,  # memory_9
    0x40, 0xf0, 0x10, 0x90,  # memory_10
    0xf0, 0x00, 0xf0, 0x60,  # memory_11
    0x50, 0xa0, 0x50, 0xa0,  # memory_12
    0x80, 0xf0, 0x80, 0xf0,  # memory_13
    0xe0, 0xb0, 0x10, 0xf0,  # memory_14
    0xf0, 0xf0, 0xf0, 0x60,  # memory_15
    0x60, 0xf0, 0xf0, 0x90,  # memory_16
    0xd0, 0x50, 0x70, 0xc0,  # memory_17
    0x90, 0x90, 0x90, 0xf0,  # memory_18
    0xf0, 0x90, 0x90, 0x90,  # memory_19
    0xf0, 0xc0, 0xc0, 0xf0,  # memory_20
    0xf0, 0x70, 0x70, 0xf0,  # memory_21
    0x70, 0x50, 0x50, 0xf0,  # memory_22
    0xe0, 0xb0, 0xa0, 0xf0,  # memory_23
    0xb0, 0x90, 0x90, 0xb0,  # memory_24
    0xd0, 0x90, 0x90, 0xf0,  # memory_25
    0xd0, 0xd0, 0x10, 0xf0,  # memory_26
    0xf0, 0xb0, 0x80, 0xf0,  # memory_27
    0xd0, 0x90, 0x90, 0xb0,  # memory_28
    0x40, 0xf0, 0x40, 0x40,  # memory_29
    0x10, 0x30, 0x70, 0xf0,  # memory_30
    0x90, 0xf0, 0x10, 0x10,  # memory_31
    0x60, 0xf0, 0x60, 0xf0,  # memory_32
    0xb0, 0xf0, 0x20, 0x30,  # memory_33
    0xf0, 0x60, 0x60, 0x60,  # memory_34
    0x90, 0x80, 0x80, 0x70,  # memory_35
    0x90, 0xf0, 0x10, 0xf0,  # memory_36
    0xa0, 0xa0, 0xf0, 0x20,  # memory_37
    0x90, 0x60, 0x60, 0x90,  # memory_38
    0x80, 0xf0, 0xe0, 0xe0,  # memory_39
    0x10, 0xf0, 0x70, 0x70,  # memory_40
    0x90, 0x50, 0x20, 0xd0,  # memory_41
    0xf0, 0x30, 0x50, 0x90,  # memory_42
    0x70, 0xc0, 0xb0, 0xb0,  # memory_43
    0x90, 0xf0, 0xf0, 0x60,  # memory_44
])


def _make_icon(glyph):
    """Expand a 4-byte glyph (high nibble per row) into an 8x8 bytearray using 2x2 blocks."""
    data = bytearray(8)
    for row in range(4):
        bits = glyph[row] >> 4
        b = 0
        for col in range(4):
            if bits & (1 << (3 - col)):
                b |= (0x3 << (6 - col * 2))
        data[row * 2] = b
        data[row * 2 + 1] = b
    return data


def _generate_icons(count):
    """Pick `count` icons from the predefined glyph set."""
    n = len(_GLYPHS) // 4
    indices = list(range(n))
    for i in range(n - 1, 0, -1):
        j = random.randint(0, i)
        indices[i], indices[j] = indices[j], indices[i]
    return [_make_icon(_GLYPHS[indices[i]*4 : indices[i]*4 + 4]) for i in range(count)]


class MemoryScene(Scene):

    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self.character = None
        self.icons = []
        self.cell_icons = []
        self.cell_state = bytearray(MAX_TOTAL)
        self.cursor = 0
        self.first_flipped = -1
        self.second_flipped = -1
        self.score = 0
        self.solved_count = 0
        self.total_solved = 0
        self.state = STATE_PLAYING
        self.mismatch_timer = 0.0
        self.match_show_timer = 0.0
        self.recently_solved = []
        self.win_timer = 0.0
        self.game_count = 0
        # Board dimensions — set properly in _init_game
        self.cols = MAX_COLS
        self.rows = MAX_ROWS
        self.total = MAX_TOTAL
        self.pairs = MAX_PAIRS
        self.panel_x = MAX_COLS * CELL

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

    def exit(self):
        total_solved = self.total_solved + self.solved_count
        if total_solved > 0:
            progress = min(1.0, total_solved / MAX_TOTAL) ** 0.5
            self.context.apply_stat_changes({
                'intelligence': 5 * progress,
                'focus':        4 * progress,
                'sociability':  3 * progress + 0.5,
                'loyalty':      1.0 * progress,
            })
            coins = int(5 * progress)
            if coins > 0:
                self.context.coins += coins
                print(f"[Memory] Awarded {coins} coins (total: {self.context.coins})")

    def _init_game(self):
        self.total_solved += self.solved_count

        # Board grows with each successive game in the session
        cols, rows = _BOARD_SIZES[min(self.game_count, len(_BOARD_SIZES) - 1)]
        self.cols = cols
        self.rows = rows
        self.total = cols * rows
        self.pairs = self.total // 2
        self.panel_x = cols * CELL
        self.game_count += 1

        self.icons = _generate_icons(self.pairs)

        # Two copies of each icon index, shuffled
        assign = list(range(self.pairs)) + list(range(self.pairs))
        for i in range(self.total - 1, 0, -1):
            j = random.randint(0, i)
            assign[i], assign[j] = assign[j], assign[i]

        self.cell_icons = assign
        self.cell_state = bytearray(self.total)
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

        col = self.cursor % self.cols
        row = self.cursor // self.cols
        inp = self.input

        if inp.was_just_pressed('left') and col > 0:
            self.cursor -= 1
        elif inp.was_just_pressed('right') and col < self.cols - 1:
            self.cursor += 1
        elif inp.was_just_pressed('up') and row > 0:
            self.cursor -= self.cols
        elif inp.was_just_pressed('down') and row < self.rows - 1:
            self.cursor += self.cols

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

                if self.solved_count == self.total:
                    best = self.context.memory_best_score
                    if best < 0 or self.score < best:
                        self.context.memory_best_score = self.score
                        best = self.score
                    best_line = ("Best: " + str(best)) if best >= 0 else ""
                    self.win_popup.set_text(
                        ("Incredible!" if self.score < 50 else "Amazing!" if self.score < 100 else "Impressive!" if self.score < 150 else "Well done!" if self.score < 250 else "Not bad!" if self.score < 500 else "Phwew!") + "\n\nScore: " + str(self.score) + "\n" + best_line,
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

        for i in range(self.total):
            col = i % self.cols
            row = i // self.cols
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
            cx = (i % self.cols) * CELL
            cy = (i // self.cols) * CELL
            r.draw_rect(cx - 2, cy - 2, 12, 12, filled=True, color=0)
            r.draw_sprite(self.icons[self.cell_icons[i]], 8, 8, cx, cy)
            r.draw_rect(cx - 2, cy - 2, 12, 12, filled=False)

        # Right panel: score at top
        r.draw_text(str(self.score), self.panel_x + 4, 2)

        # Cat avatar
        if self.character:
            self.character.draw(r)

        # Win popup overlay
        if self.state == STATE_WIN:
            self.win_popup.draw(show_scroll_indicators=False)
