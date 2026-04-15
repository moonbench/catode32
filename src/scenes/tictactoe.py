"""
TicTacToe scene - Play tic-tac-toe against the pet
"""
import random
import config
from scene import Scene
from assets.minigame_assets import PAW_LARGE1, PAW_MED, PAW_SMALL1
from entities.character import CharacterEntity
from ui import Popup


class TicTacToeScene(Scene):
    """Tic-tac-toe minigame against the pet"""

    # Board layout
    BOARD_OFFSET_X = 2
    BOARD_OFFSET_Y = 3

    # Per-board-size config
    CELL_SIZES    = {3: 19, 4: 14, 5: 11}
    CIRCLE_RADII  = {3: 7,  4: 5,  5: 4}
    PAW_SPRITES   = {3: PAW_LARGE1, 4: PAW_MED, 5: PAW_SMALL1}

    # Cell values
    EMPTY = 0
    PLAYER = 1  # Circle (O)
    PET = 2     # Paw (X)

    # Game states
    STATE_PLAYER_TURN = 0
    STATE_PET_TURN = 1
    STATE_PLAYER_WIN = 2
    STATE_PET_WIN = 3
    STATE_DRAW = 4

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.round_number = 0
        self.player_score = 0
        self.pet_score = 0
        self.character = None
        # Win/lose/draw popup - centered horizontally, at top of screen
        self.result_popup = Popup(renderer, x=14, y=0, width=100, height=40)
        # Initialised properly in reset_game()
        self.board_size = 3
        self.cell_size = 19
        self.win_lines = []
        self.board = []
        self.cursor_pos = 4
        self.winning_line = None
        self.draw_winner = None
        self.pet_think_timer = 0.0
        self.end_delay_timer = 0.0
        self.state = self.STATE_PLAYER_TURN
        self.reset_game()

    def _generate_win_lines(self):
        """Generate all winning lines for the current board size (NxN, win=N)"""
        n = self.board_size
        lines = []
        for r in range(n):
            lines.append([r * n + c for c in range(n)])
        for c in range(n):
            lines.append([r * n + c for r in range(n)])
        lines.append([i * n + i for i in range(n)])
        lines.append([i * n + (n - 1 - i) for i in range(n)])
        return lines

    def reset_game(self):
        """Reset game state for a new round"""
        if self.round_number < 3:
            self.board_size = 3
        elif self.round_number < 7:
            self.board_size = 4
        else:
            self.board_size = 5

        self.cell_size = self.CELL_SIZES[self.board_size]
        self.win_lines = self._generate_win_lines()

        n = self.board_size
        self.board = [self.EMPTY] * (n * n)
        self.cursor_pos = (n // 2) * n + (n // 2)
        self.winning_line = None
        self.draw_winner = None
        self.pet_think_timer = 0.0
        self.end_delay_timer = 0.0

        if self.character:
            self.character.set_pose("laying.side.neutral")

        if self.round_number % 2 == 0:
            self.state = self.STATE_PLAYER_TURN
        else:
            self.state = self.STATE_PET_TURN

    def load(self):
        super().load()
        self.character = CharacterEntity(100, 63)
        self.character.set_pose("laying.side.neutral")

    def unload(self):
        super().unload()

    def enter(self):
        self.round_number = 0
        self.player_score = 0
        self.pet_score = 0
        self.reset_game()

    def exit(self):
        current_ended = self.state in (self.STATE_PLAYER_WIN, self.STATE_PET_WIN, self.STATE_DRAW)
        total_rounds = self.round_number + (1 if current_ended else 0)
        if total_rounds > 0:
            scale = (total_rounds / 8.0) ** 0.5
            print(f"Reward scaling {scale}")
            self.context.apply_stat_changes({
                'sociability':   3 * scale,
                'intelligence':  4 * scale,
                'focus':         3 * scale,
                'fulfillment':   3 * scale,
                'loyalty':       1.0 * scale,
            })
            coins = 2 * total_rounds
            if coins > 0:
                self.context.coins += coins
                print(f"[TicTacToe] Awarded {coins} coins (total: {self.context.coins})")

    def update(self, dt):
        self.character.update(dt)

        if self.state == self.STATE_PET_TURN:
            self.pet_think_timer += dt
            if self.pet_think_timer >= 0.5:
                self._make_pet_move()
                self.pet_think_timer = 0.0

        if self.state in (self.STATE_PLAYER_WIN, self.STATE_PET_WIN, self.STATE_DRAW):
            self.end_delay_timer += dt

    def _cell_to_pixel(self, cell_idx):
        """Convert cell index to top-left pixel coordinates"""
        row = cell_idx // self.board_size
        col = cell_idx % self.board_size
        x = self.BOARD_OFFSET_X + col * (self.cell_size + 1)
        y = self.BOARD_OFFSET_Y + row * (self.cell_size + 1)
        return x, y

    def _check_winner(self, mark):
        """Check if the given mark has won, return winning line indices or None"""
        for line in self.win_lines:
            won = True
            for c in line:
                if self.board[c] != mark:
                    won = False
                    break
            if won:
                return line
        return None

    def _is_board_full(self):
        for cell in self.board:
            if cell == self.EMPTY:
                return False
        return True

    def _find_longest_run(self, mark):
        """Find the longest contiguous run of mark within any win line"""
        longest = 0
        for line in self.win_lines:
            run = 0
            for c in line:
                if self.board[c] == mark:
                    run += 1
                    if run > longest:
                        longest = run
                else:
                    run = 0
        return longest

    def _resolve_draw(self):
        """Handle a draw - on larger boards, award 0.5 to whoever had the longest run"""
        self.state = self.STATE_DRAW
        self.end_delay_timer = 0.0
        self.draw_winner = None
        if self.board_size > 3:
            player_run = self._find_longest_run(self.PLAYER)
            pet_run = self._find_longest_run(self.PET)
            if player_run > pet_run:
                self.draw_winner = self.PLAYER
                self.player_score += 0.5
                self.character.set_pose("laying.side.annoyed")
            elif pet_run > player_run:
                self.draw_winner = self.PET
                self.pet_score += 0.5
                self.character.set_pose("laying.side.happy")

    def _make_pet_move(self):
        best_move = self._find_best_move()

        if best_move is None:
            for i in range(self.board_size * self.board_size):
                if self.board[i] == self.EMPTY:
                    best_move = i
                    break

        if best_move is not None:
            self.board[best_move] = self.PET
            win_line = self._check_winner(self.PET)
            if win_line:
                self.winning_line = win_line
                self.state = self.STATE_PET_WIN
                self.pet_score += 1
                self.end_delay_timer = 0.0
                self.character.set_pose("laying.side.happy")
            elif self._is_board_full():
                self._resolve_draw()
            else:
                self.state = self.STATE_PLAYER_TURN
        else:
            self.state = self.STATE_PLAYER_TURN

    def _score_cell(self, cell):
        """Score an empty cell for the pet based on line potential"""
        n = self.board_size
        row = cell // n
        col = cell % n
        center = n // 2
        # Center preference
        score = n - abs(row - center) - abs(col - center)

        for line in self.win_lines:
            if cell not in line:
                continue
            pet_count = 0
            player_count = 0
            for c in line:
                if self.board[c] == self.PET:
                    pet_count += 1
                elif self.board[c] == self.PLAYER:
                    player_count += 1
            if player_count == 0:
                score += pet_count * 3
            elif pet_count == 0:
                score += player_count  # mild defensive value

        return score

    def _find_best_move(self):
        """Find the best move: win > block > scored heuristic"""
        total = self.board_size * self.board_size
        empty_cells = [i for i in range(total) if self.board[i] == self.EMPTY]

        if not empty_cells:
            return None

        # 5% random move (keeps game winnable)
        if random.randint(1, 100) <= 5 and len(empty_cells) > 1:
            return empty_cells[random.randint(0, len(empty_cells) - 1)]

        # Win immediately
        for i in empty_cells:
            self.board[i] = self.PET
            won = self._check_winner(self.PET)
            self.board[i] = self.EMPTY
            if won:
                return i

        # Block player win
        for i in empty_cells:
            self.board[i] = self.PLAYER
            won = self._check_winner(self.PLAYER)
            self.board[i] = self.EMPTY
            if won:
                return i

        # Score each empty cell
        best_score = -1
        best_moves = []
        for i in empty_cells:
            score = self._score_cell(i)
            if score > best_score:
                best_score = score
                best_moves = [i]
            elif score == best_score:
                best_moves.append(i)

        return best_moves[random.randint(0, len(best_moves) - 1)]

    def _player_place_mark(self):
        """Player places their mark at the cursor position"""
        if self.board[self.cursor_pos] == self.EMPTY:
            self.board[self.cursor_pos] = self.PLAYER

            win_line = self._check_winner(self.PLAYER)
            if win_line:
                self.winning_line = win_line
                self.state = self.STATE_PLAYER_WIN
                self.player_score += 1
                self.end_delay_timer = 0.0
                self.character.set_pose("laying.side.annoyed")
            elif self._is_board_full():
                self._resolve_draw()
            else:
                self.state = self.STATE_PET_TURN
                self.pet_think_timer = 0.0

    def draw(self):
        self._draw_board()
        self._draw_marks()

        if self.state == self.STATE_PLAYER_TURN:
            self._draw_cursor()

        self._draw_score()
        self._draw_state_message()
        self.character.draw(self.renderer)

    def _draw_board(self):
        """Draw the NxN grid lines"""
        n = self.board_size
        board_span = n * self.cell_size + (n - 1)

        for i in range(1, n):
            x = self.BOARD_OFFSET_X + i * (self.cell_size + 1) - 1
            self.renderer.draw_line(x, self.BOARD_OFFSET_Y, x, self.BOARD_OFFSET_Y + board_span - 1)

        for i in range(1, n):
            y = self.BOARD_OFFSET_Y + i * (self.cell_size + 1) - 1
            self.renderer.draw_line(self.BOARD_OFFSET_X, y, self.BOARD_OFFSET_X + board_span - 1, y)

    def _draw_marks(self):
        """Draw all placed marks on the board"""
        n = self.board_size
        paw = self.PAW_SPRITES[n]
        radius = self.CIRCLE_RADII[n]
        half = self.cell_size // 2

        for i in range(n * n):
            if self.board[i] == self.EMPTY:
                continue

            cell_x, cell_y = self._cell_to_pixel(i)

            if self.board[i] == self.PLAYER:
                self.renderer.draw_circle(cell_x + half, cell_y + half, radius)
            else:
                sprite_x = cell_x + (self.cell_size - paw["width"]) // 2
                sprite_y = cell_y + (self.cell_size - paw["height"]) // 2
                self.renderer.draw_sprite_obj(paw, sprite_x, sprite_y, transparent=True)

    def _draw_cursor(self):
        """Draw cursor highlight around selected cell"""
        cell_x, cell_y = self._cell_to_pixel(self.cursor_pos)
        cursor_size = self.cell_size - 2
        self.renderer.draw_rect(cell_x + 1, cell_y + 1, cursor_size, cursor_size, filled=False)

    def _fmt_score(self, v):
        return str(int(v)) if v % 1 == 0 else str(v)

    def _draw_score(self):
        """Draw score in the top-right area"""
        score_x = 62
        self.renderer.draw_text("You", score_x, 4)
        self.renderer.draw_text(self._fmt_score(self.player_score), score_x, 12)
        self.renderer.draw_text("Pet", score_x + 28, 4)
        self.renderer.draw_text(self._fmt_score(self.pet_score), score_x + 28, 12)

    def _draw_state_message(self):
        """Draw end game messages"""
        if self.state == self.STATE_PLAYER_WIN:
            self.result_popup.set_text("You Win!\nA: New Game", wrap=False, center=True)
            self.result_popup.draw(show_scroll_indicators=False)
        elif self.state == self.STATE_PET_WIN:
            self.result_popup.set_text("You Lose!\nA: New Game", wrap=False, center=True)
            self.result_popup.draw(show_scroll_indicators=False)
        elif self.state == self.STATE_DRAW:
            if self.draw_winner == self.PLAYER:
                msg = "Draw!\nYou had the\nlongest row\nA: New Game"
            elif self.draw_winner == self.PET:
                msg = "Draw!\nPet had the\nlongest row\nA: New Game"
            else:
                msg = "Draw!\nA: New Game"
            self.result_popup.set_text(msg, wrap=False, center=True)
            self.result_popup.draw(show_scroll_indicators=False)
        elif self.state == self.STATE_PET_TURN:
            self.renderer.draw_text("...", 76, 20)

    def handle_input(self):
        # Handle game over - press A to restart
        if self.state in (self.STATE_PLAYER_WIN, self.STATE_PET_WIN, self.STATE_DRAW):
            if self.input.was_just_pressed('a') and self.end_delay_timer >= 0.5:
                self.round_number += 1
                self.reset_game()
            return None

        # Handle player turn
        if self.state == self.STATE_PLAYER_TURN:
            n = self.board_size
            row = self.cursor_pos // n
            col = self.cursor_pos % n

            if self.input.was_just_pressed('up') and row > 0:
                self.cursor_pos -= n
            elif self.input.was_just_pressed('down') and row < n - 1:
                self.cursor_pos += n
            elif self.input.was_just_pressed('left') and col > 0:
                self.cursor_pos -= 1
            elif self.input.was_just_pressed('right') and col < n - 1:
                self.cursor_pos += 1

            if self.input.was_just_pressed('a'):
                self._player_place_mark()

        return None
