"""ui_keyboard.py - Reusable on-screen keyboard for text/hex entry.

Full keyboard layout (11 cols × 4 rows):
  Row 0: 1 2 3 4 5 6 7 8 9 0   (unshifted: digits / shifted: symbols)
  Row 1: a b c d e f g h i j k
  Row 2: l m n o p q r s t u v
  Row 3: w x y z _ ^ < OK       (_ = space, ^ = shift, < = backspace)

Usage:
    kb = OnScreenKeyboard(renderer, input, charset='full', max_len=12)
    kb.open(initial_text)
    result = kb.handle_input()   # None while editing; string when confirmed
"""

CHARSET_HEX = '0123456789ABCDEF'

# 7x9 sprites for special keys
_ICON_BACK  = b'\x1e\x22\x42\x82\x82\x82\x42\x22\x1e'
_ICON_SHIFT = b'\x10\x28\x44\x82\xee\x28\x28\x28\x38'

_KEY_BACK  = '\x08'
_KEY_DONE  = '\r'
_KEY_SHIFT = '\x0e'
_KEY_EMPTY = '\x00'

_SEP_Y  = 9
_GRID_Y = 11
_CELL_H = 13

_FULL_COLS  = 11
_FULL_CELL_W = 128 // _FULL_COLS  # 11px

# 11 cols × 4 rows = 44 positions.
# Row 0: ten digit/symbol keys + 1 empty pad
# Rows 1-2: 11 letters each
# Row 3: w-z (4) + space + shift + back + done + 3 empty pads
_FULL_LOWER = list('1234567890') + [_KEY_EMPTY] + \
              list('abcdefghijk') + \
              list('lmnopqrstuv') + \
              list('wxyz .,') + [_KEY_SHIFT, _KEY_BACK, _KEY_DONE] + [_KEY_EMPTY]

_FULL_UPPER = list('!@#$%^&*()') + [_KEY_EMPTY] + \
              list('ABCDEFGHIJK') + \
              list('LMNOPQRSTUV') + \
              list('WXYZ .,') + [_KEY_SHIFT, _KEY_BACK, _KEY_DONE] + [_KEY_EMPTY]

# Hex: 9 cols × 2 rows = 18 positions, no shift
_HEX_COLS  = 9
_HEX_CELL_W = 128 // _HEX_COLS  # 14px
_HEX_CHARS = list(CHARSET_HEX) + [_KEY_BACK, _KEY_DONE]


class OnScreenKeyboard:
    """On-screen keyboard with d-pad navigation and shift support.

    A: select key / confirm on OK / delete on <  / toggle shift on ^
    B: backspace shortcut
    menu1 / menu2: confirm whatever is typed
    """

    def __init__(self, renderer, input_handler, charset='full', max_len=12):
        self.renderer = renderer
        self.input    = input_handler
        self.max_len  = max_len

        if charset == 'hex':
            self._lower    = _HEX_CHARS
            self._upper    = _HEX_CHARS
            self._cols     = _HEX_COLS
            self._cell_w   = _HEX_CELL_W
            self._has_shift = False
        else:
            self._lower    = _FULL_LOWER
            self._upper    = _FULL_UPPER
            self._cols     = _FULL_COLS
            self._cell_w   = _FULL_CELL_W
            self._has_shift = True

        self._rows   = (len(self._lower) + self._cols - 1) // self._cols
        self._shift  = False
        self._text   = ''
        self._cur_row = 0
        self._cur_col = 0

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    @property
    def text(self):
        return self._text

    def open(self, label='', initial_text=''):
        self._text    = initial_text[:self.max_len]
        self._shift   = False
        self._cur_row = 0
        self._cur_col = 0

    def handle_input(self):
        """Return confirmed string when done; None while still editing."""
        inp   = self.input
        chars = self._upper if self._shift else self._lower

        if inp.was_just_pressed('up'):
            if self._cur_row > 0:
                self._cur_row -= 1
                self._clamp_col(chars)
        elif inp.was_just_pressed('down'):
            max_row = (len(chars) - 1) // self._cols
            if self._cur_row < max_row:
                self._cur_row += 1
                self._clamp_col(chars)
        elif inp.was_just_pressed('left'):
            if self._cur_col > 0:
                self._cur_col -= 1
                self._skip_empty_left(chars)
            elif self._cur_row > 0:
                self._cur_row -= 1
                self._cur_col = self._cols - 1
                self._clamp_col(chars)
        elif inp.was_just_pressed('right'):
            cur_idx = self._cur_row * self._cols + self._cur_col
            if cur_idx < len(chars) - 1:
                if self._cur_col < self._cols - 1:
                    self._cur_col += 1
                else:
                    self._cur_row += 1
                    self._cur_col = 0
                self._skip_empty_right(chars)

        if inp.was_just_pressed('b'):
            self._text = self._text[:-1]
            return None

        if inp.was_just_pressed('menu1') or inp.was_just_pressed('menu2'):
            return self._text

        if inp.was_just_pressed('a'):
            idx = self._cur_row * self._cols + self._cur_col
            if idx < len(chars):
                key = chars[idx]
                if key == _KEY_DONE:
                    return self._text
                elif key == _KEY_BACK:
                    self._text = self._text[:-1]
                elif key == _KEY_SHIFT:
                    self._shift = not self._shift
                elif key != _KEY_EMPTY and len(self._text) < self.max_len:
                    self._text += key

        return None

    def draw(self):
        r     = self.renderer
        chars = self._upper if self._shift else self._lower

        r.draw_text((self._text + '_')[:21], 0, 0)
        r.draw_line(0, _SEP_Y, 127, _SEP_Y)

        for i, char in enumerate(chars):
            if char == _KEY_EMPTY:
                continue
            row = i // self._cols
            col = i % self._cols
            x   = col * self._cell_w
            y   = _GRID_Y + row * _CELL_H

            selected       = (row == self._cur_row and col == self._cur_col)
            shift_active   = (char == _KEY_SHIFT and self._shift)

            if selected or shift_active:
                w = 16 if char == _KEY_DONE else self._cell_w
                r.draw_rect(x, y, w, _CELL_H, filled=True, color=1)

            if char == _KEY_BACK:
                ix = x + (self._cell_w - 7) // 2
                iy = y + (_CELL_H - 9) // 2
                r.draw_sprite(_ICON_BACK, 7, 9, ix, iy,
                              invert=selected, transparent=not selected)
            elif char == _KEY_SHIFT:
                ix = x + (self._cell_w - 7) // 2
                iy = y + (_CELL_H - 9) // 2
                lit = selected or shift_active
                r.draw_sprite(_ICON_SHIFT, 7, 9, ix, iy,
                              invert=lit, transparent=not lit)
            else:
                if char == _KEY_DONE:
                    label = 'OK'
                elif char == ' ':
                    label = '_'
                else:
                    label = char
                tc = 0 if (selected or shift_active) else 1
                tx = x + (self._cell_w - 8) // 2
                ty = y + (_CELL_H - 8) // 2
                r.draw_text(label, tx, ty, tc)

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _clamp_col(self, chars):
        """Move cursor to the last non-empty cell in the current row."""
        row_start = self._cur_row * self._cols
        max_col   = 0
        for c in range(self._cols):
            idx = row_start + c
            if idx < len(chars) and chars[idx] != _KEY_EMPTY:
                max_col = c
        if self._cur_col > max_col:
            self._cur_col = max_col

    def _skip_empty_right(self, chars):
        """If cursor landed on an empty cell, advance (wrapping to next row) until non-empty."""
        max_row = (len(chars) - 1) // self._cols
        while True:
            idx = self._cur_row * self._cols + self._cur_col
            if idx >= len(chars) or chars[idx] != _KEY_EMPTY:
                break
            if self._cur_col < self._cols - 1:
                self._cur_col += 1
            elif self._cur_row < max_row:
                self._cur_row += 1
                self._cur_col = 0
            else:
                break

    def _skip_empty_left(self, chars):
        """If cursor landed on an empty cell, step left until non-empty."""
        while self._cur_col > 0:
            idx = self._cur_row * self._cols + self._cur_col
            if idx >= len(chars) or chars[idx] == _KEY_EMPTY:
                self._cur_col -= 1
            else:
                break
