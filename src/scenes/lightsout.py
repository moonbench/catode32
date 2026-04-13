"""
Lights Out minigame - toggle all lights off
Press a cell to toggle it and its orthogonal neighbours. Goal: all cells off.
"""
import random
from scene import Scene
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from ui import Popup

# Supported grid sizes and display labels
_SIZES = [4, 5, 6]

# Difficulty ramp: presses start low and increase by 1 every 2 session wins.
_PRESS_MIN = {4: 3, 5: 3, 6: 4}
_PRESS_MAX = {4: 10, 5: 15, 6: 20}

# Total pixel size per cell (inner fill + 1px gap on each side)
_CELL_PX = {4: 12, 5: 10, 6: 8}

# Pixel region reserved for the grid (right side is the character panel)
_GRID_AREA_W = 90
_GRID_AREA_H = 64

STATE_PLAYING = 0
STATE_WIN = 1

WIN_RESET_DELAY = 3.0  # seconds before auto-reset after win


class LightsOutScene(Scene):
    """Lights Out puzzle minigame"""



    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self.character = None
        self.options_menu = Menu(renderer, input_handler)
        self.menu_active = False
        self.win_popup = None

        self.grid_size = 5
        self.grid = None        # bytearray, 1=lit, 0=off
        self.cursor = 0
        self.state = STATE_PLAYING
        self.win_timer = 0.0
        self.move_count = 0
        self.par = 0
        self._seed = []
        self._session_wins = 0

    def load(self):
        super().load()
        self.character = CharacterEntity(100, 63)
        self.character.set_pose("sitting.side.neutral")
        self.win_popup = Popup(self.renderer, x=10, y=14, width=108, height=36)

    def unload(self):
        super().unload()

    def enter(self):
        self._session_wins = 0
        self._init_game()

    def exit(self):
        if self._session_wins > 0:
            scale = (self._session_wins / 5.0) ** 0.5
            self.context.apply_stat_changes({
                'intelligence': 5 * scale,
                'focus':        5 * scale,
            })
            coins = int(3 * self._session_wins)
            if coins > 0:
                self.context.coins += coins
                print(f"[LightsOut] Awarded {coins} coins (total: {self.context.coins})")

    # -------------------------------------------------------------------------
    # Game logic
    # -------------------------------------------------------------------------

    def _init_game(self, size=None):
        if size is not None:
            self.grid_size = size
        n = self.grid_size
        total = n * n

        # Start solved (all off) then press N distinct random cells backwards.
        # The resulting board is guaranteed solvable in exactly those same N presses.
        self.grid = bytearray(total)
        pressed = set()
        attempts = 0
        target = min(_PRESS_MAX[n], _PRESS_MIN[n] + self._session_wins // 2)
        while len(pressed) < target and attempts < target * 4:
            attempts += 1
            idx = random.randint(0, total - 1)
            if idx not in pressed:
                pressed.add(idx)
                self._apply_toggle(idx)

        self._seed = list(pressed)  # store so retry can rebuild the same board
        self.par = len(pressed)
        self._reset_board()

    def _reset_board(self):
        """Rebuild the grid from the stored seed, resetting move count."""
        n = self.grid_size
        total = n * n
        self.grid = bytearray(total)
        for idx in self._seed:
            self._apply_toggle(idx)
        self.cursor = total // 2
        self.state = STATE_PLAYING
        self.win_timer = 0.0
        self.move_count = 0
        if self.character:
            self.character.set_pose("sitting.side.neutral")

    def _apply_toggle(self, idx):
        """Toggle cell at idx and its orthogonal neighbours."""
        n = self.grid_size
        row = idx // n
        col = idx % n
        for dr, dc in ((0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)):
            r, c = row + dr, col + dc
            if 0 <= r < n and 0 <= c < n:
                self.grid[r * n + c] ^= 1

    def _check_win(self):
        for v in self.grid:
            if v:
                return False
        return True

    # -------------------------------------------------------------------------
    # Input
    # -------------------------------------------------------------------------

    def _build_options_items(self):
        size_items = [
            MenuItem("4x4 Easy",   action='size_4'),
            MenuItem("5x5 Normal", action='size_5'),
            MenuItem("6x6 Hard",   action='size_6'),
        ]
        return [
            MenuItem("Retry",      action='retry'),
            MenuItem("New Board",  action='new_board'),
            MenuItem("Grid Size",  submenu=size_items),
        ]

    def _handle_menu_action(self, action):
        if action == 'retry':
            self._reset_board()
        elif action == 'new_board':
            self._init_game()
        elif action == 'size_4':
            self._init_game(size=4)
        elif action == 'size_5':
            self._init_game(size=5)
        elif action == 'size_6':
            self._init_game(size=6)

    def handle_input(self):
        if self.menu_active:
            result = self.options_menu.handle_input()
            if result == 'closed':
                self.menu_active = False
            elif result is not None:
                self.menu_active = False
                self._handle_menu_action(result)
            return None

        if self.state == STATE_WIN:
            if self.input.was_just_pressed('a'):
                self._init_game()
            return None

        if self.input.was_just_pressed('menu2'):
            self.menu_active = True
            self.options_menu.open(self._build_options_items())
            return None

        n = self.grid_size
        row = self.cursor // n
        col = self.cursor % n

        if self.input.was_just_pressed('up') and row > 0:
            self.cursor -= n
        elif self.input.was_just_pressed('down') and row < n - 1:
            self.cursor += n
        elif self.input.was_just_pressed('left') and col > 0:
            self.cursor -= 1
        elif self.input.was_just_pressed('right') and col < n - 1:
            self.cursor += 1

        if self.input.was_just_pressed('a'):
            self._apply_toggle(self.cursor)
            self.move_count += 1
            if self._check_win():
                self._session_wins += 1
                self.state = STATE_WIN
                self.win_timer = 0.0
                self.win_popup.set_text(
                    "All off!\n\nMoves: " + str(self.move_count),
                    wrap=False, center=True
                )
                self.character.set_pose("sitting.side.happy")

        return None

    # -------------------------------------------------------------------------
    # Update
    # -------------------------------------------------------------------------

    def update(self, dt):
        if self.character:
            self.character.update(dt)

        if self.state == STATE_WIN:
            self.win_timer += dt
            if self.win_timer >= WIN_RESET_DELAY:
                self._init_game()

    # -------------------------------------------------------------------------
    # Draw
    # -------------------------------------------------------------------------

    def draw(self):
        r = self.renderer

        if self.menu_active:
            self.options_menu.draw()
            return

        self._draw_grid(r)
        self._draw_sidebar(r)
        self.character.draw(r)

        if self.state == STATE_WIN:
            self.win_popup.draw(show_scroll_indicators=False)

    def _draw_grid(self, r):
        n = self.grid_size
        px = _CELL_PX[n]
        span = n * px
        ox = (_GRID_AREA_W - span) // 2
        oy = (_GRID_AREA_H - span) // 2
        inner = px - 2  # filled/outlined area inside the 1px gap

        for i in range(n * n):
            row = i // n
            col = i % n
            cx = ox + col * px
            cy = oy + row * px

            if self.grid[i]:
                r.draw_rect(cx + 1, cy + 1, inner, inner, filled=True)
            else:
                r.draw_rect(cx + 1, cy + 1, inner, inner, filled=False)

            if i == self.cursor and self.state == STATE_PLAYING:
                # Cursor: outer box around entire cell
                r.draw_rect(cx, cy, px, px, filled=False)

    def _draw_sidebar(self, r):
        # Move counter and par above the character
        r.draw_text(str(self.move_count), 92, 2)
        r.draw_text("Par:" + str(self.par), 82, 12)
