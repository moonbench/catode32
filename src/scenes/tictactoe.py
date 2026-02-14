"""
TicTacToe scene - Play tic-tac-toe against the pet
"""
import random
import config
from scene import Scene
from assets.minigame_assets import PAW_LARGE1, RING_17


class TicTacToeScene(Scene):
    """Tic-tac-toe minigame against the pet"""

    # Board layout
    CELL_SIZE = 19
    BOARD_OFFSET_X = 2
    BOARD_OFFSET_Y = 3

    # Cell values
    EMPTY = 0
    PLAYER = 1  # Ring (O)
    PET = 2     # Paw (X)

    # Game states
    STATE_PLAYER_TURN = 0
    STATE_PET_TURN = 1
    STATE_PLAYER_WIN = 2
    STATE_PET_WIN = 3
    STATE_DRAW = 4

    # Win lines (indices of winning combinations)
    WIN_LINES = [
        [0, 1, 2],  # Top row
        [3, 4, 5],  # Middle row
        [6, 7, 8],  # Bottom row
        [0, 3, 6],  # Left column
        [1, 4, 7],  # Middle column
        [2, 5, 8],  # Right column
        [0, 4, 8],  # Diagonal top-left to bottom-right
        [2, 4, 6],  # Diagonal top-right to bottom-left
    ]

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.round_number = 0
        self.player_score = 0
        self.pet_score = 0
        self.reset_game()

    def reset_game(self):
        """Reset game state for a new round"""
        self.board = [self.EMPTY] * 9
        self.cursor_pos = 4  # Start in center
        self.winning_line = None
        self.pet_think_timer = 0.0
        self.end_delay_timer = 0.0

        # Alternate who goes first each round
        if self.round_number % 2 == 0:
            self.state = self.STATE_PLAYER_TURN
        else:
            self.state = self.STATE_PET_TURN

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.round_number = 0
        self.player_score = 0
        self.pet_score = 0
        self.reset_game()

    def exit(self):
        pass

    def update(self, dt):
        # Handle pet's turn with a small delay
        if self.state == self.STATE_PET_TURN:
            self.pet_think_timer += dt
            if self.pet_think_timer >= 0.5:
                self._make_pet_move()
                self.pet_think_timer = 0.0

        # Handle end game delay before auto-restart
        if self.state in (self.STATE_PLAYER_WIN, self.STATE_PET_WIN, self.STATE_DRAW):
            self.end_delay_timer += dt

    def _cell_to_pixel(self, cell_idx):
        """Convert cell index (0-8) to top-left pixel coordinates"""
        row = cell_idx // 3
        col = cell_idx % 3
        # Stride is CELL_SIZE + 1 to account for grid lines
        x = self.BOARD_OFFSET_X + col * (self.CELL_SIZE + 1)
        y = self.BOARD_OFFSET_Y + row * (self.CELL_SIZE + 1)
        return x, y

    def _check_winner(self, mark):
        """Check if the given mark has won, return winning line indices or None"""
        for line in self.WIN_LINES:
            if self.board[line[0]] == mark and self.board[line[1]] == mark and self.board[line[2]] == mark:
                return line
        return None

    def _is_board_full(self):
        """Check if the board is full (draw)"""
        for cell in self.board:
            if cell == self.EMPTY:
                return False
        return True

    def _make_pet_move(self):
        """Pet makes a move using minimax AI"""
        best_move = self._find_best_move()

        # Fallback: pick first empty cell if minimax fails
        if best_move is None:
            for i in range(9):
                if self.board[i] == self.EMPTY:
                    best_move = i
                    break

        if best_move is not None:
            self.board[best_move] = self.PET

            # Check for win
            win_line = self._check_winner(self.PET)
            if win_line:
                self.winning_line = win_line
                self.state = self.STATE_PET_WIN
                self.pet_score += 1
                self.end_delay_timer = 0.0
            elif self._is_board_full():
                self.state = self.STATE_DRAW
                self.end_delay_timer = 0.0
            else:
                self.state = self.STATE_PLAYER_TURN
        else:
            # No valid move (shouldn't happen), go back to player
            self.state = self.STATE_PLAYER_TURN

    def _find_best_move(self):
        """Find the best move for the pet using minimax"""
        # Find empty cells
        empty_cells = []
        for i in range(9):
            if self.board[i] == self.EMPTY:
                empty_cells.append(i)

        num_empty = len(empty_cells)

        # 5% chance to make a random move (makes game winnable)
        if random.randint(1, 100) <= 5 and num_empty > 1:
            return empty_cells[random.randint(0, len(empty_cells) - 1)]

        # Early game: use strategic moves instead of slow minimax
        if num_empty >= 8:
            # First move: pick randomly from center and corners
            good_moves = []
            if self.board[4] == self.EMPTY:
                good_moves.append(4)
            corners = [0, 2, 6, 8]
            for c in corners:
                if self.board[c] == self.EMPTY:
                    good_moves.append(c)
            if len(good_moves) > 0:
                return good_moves[random.randint(0, len(good_moves) - 1)]

        # Mid/late game: use minimax (fast enough with fewer cells)
        best_score = -1000
        best_moves = []

        for cell in empty_cells:
            self.board[cell] = self.PET
            score = self._minimax(0, False)
            self.board[cell] = self.EMPTY

            if score > best_score:
                best_score = score
                best_moves = [cell]
            elif score == best_score:
                best_moves.append(cell)

        # Randomly pick from equally good moves
        if len(best_moves) > 0:
            return best_moves[random.randint(0, len(best_moves) - 1)]
        return None

    def _minimax(self, depth, is_maximizing):
        """Minimax algorithm for AI decision making"""
        # Check terminal states
        if self._check_winner(self.PET):
            return 10 - depth
        if self._check_winner(self.PLAYER):
            return depth - 10
        if self._is_board_full():
            return 0

        if is_maximizing:
            best_score = -1000
            for i in range(9):
                if self.board[i] == self.EMPTY:
                    self.board[i] = self.PET
                    score = self._minimax(depth + 1, False)
                    self.board[i] = self.EMPTY
                    if score > best_score:
                        best_score = score
            return best_score
        else:
            best_score = 1000
            for i in range(9):
                if self.board[i] == self.EMPTY:
                    self.board[i] = self.PLAYER
                    score = self._minimax(depth + 1, True)
                    self.board[i] = self.EMPTY
                    if score < best_score:
                        best_score = score
            return best_score

    def _player_place_mark(self):
        """Player places their mark at the cursor position"""
        if self.board[self.cursor_pos] == self.EMPTY:
            self.board[self.cursor_pos] = self.PLAYER

            # Check for win
            win_line = self._check_winner(self.PLAYER)
            if win_line:
                self.winning_line = win_line
                self.state = self.STATE_PLAYER_WIN
                self.player_score += 1
                self.end_delay_timer = 0.0
            elif self._is_board_full():
                self.state = self.STATE_DRAW
                self.end_delay_timer = 0.0
            else:
                self.state = self.STATE_PET_TURN
                self.pet_think_timer = 0.0

    def draw(self):
        self.renderer.clear()

        # Draw board grid
        self._draw_board()

        # Draw marks
        self._draw_marks()

        # Draw cursor (only during player turn)
        if self.state == self.STATE_PLAYER_TURN:
            self._draw_cursor()

        # Draw score
        self._draw_score()

        # Draw game state messages
        self._draw_state_message()

    def _draw_board(self):
        """Draw the 3x3 grid lines"""
        # Board span: 3 cells of 19px + 2 lines of 1px = 59px
        board_span = 3 * self.CELL_SIZE + 2

        # Vertical lines (between columns)
        for i in range(1, 3):
            x = self.BOARD_OFFSET_X + i * self.CELL_SIZE + (i - 1)
            y1 = self.BOARD_OFFSET_Y
            y2 = self.BOARD_OFFSET_Y + board_span - 1
            self.renderer.draw_line(x, y1, x, y2)

        # Horizontal lines (between rows)
        for i in range(1, 3):
            x1 = self.BOARD_OFFSET_X
            x2 = self.BOARD_OFFSET_X + board_span - 1
            y = self.BOARD_OFFSET_Y + i * self.CELL_SIZE + (i - 1)
            self.renderer.draw_line(x1, y, x2, y)

    def _draw_marks(self):
        """Draw all placed marks on the board"""
        for i in range(9):
            if self.board[i] == self.EMPTY:
                continue

            cell_x, cell_y = self._cell_to_pixel(i)

            if self.board[i] == self.PLAYER:
                # Draw ring (17x17) centered in 19x19 cell
                sprite_x = cell_x + (self.CELL_SIZE - RING_17["width"]) // 2
                sprite_y = cell_y + (self.CELL_SIZE - RING_17["height"]) // 2
                self.renderer.draw_sprite_obj(RING_17, sprite_x, sprite_y, transparent=True)
            else:
                # Draw paw (15x17) centered in 19x19 cell
                sprite_x = cell_x + (self.CELL_SIZE - PAW_LARGE1["width"]) // 2
                sprite_y = cell_y + (self.CELL_SIZE - PAW_LARGE1["height"]) // 2
                self.renderer.draw_sprite_obj(PAW_LARGE1, sprite_x, sprite_y, transparent=True)

    def _draw_cursor(self):
        """Draw cursor highlight around selected cell"""
        cell_x, cell_y = self._cell_to_pixel(self.cursor_pos)
        # Draw a 17x17 rect centered in the 19x19 cell (1px margin on each side)
        self.renderer.draw_rect(cell_x + 1, cell_y + 1, 17, 17, filled=False)

    def _draw_score(self):
        """Draw score in the top-right area, side by side"""
        # Score area starts after the board (board ends at x=59)
        score_x = 62

        self.renderer.draw_text("You", score_x, 4)
        self.renderer.draw_text(str(self.player_score), score_x, 12)
        self.renderer.draw_text("Pet", score_x + 28, 4)
        self.renderer.draw_text(str(self.pet_score), score_x + 28, 12)

    def _draw_state_message(self):
        """Draw end game messages"""
        if self.state == self.STATE_PLAYER_WIN:
            self.renderer.draw_text("WIN!", 70, 22)
        elif self.state == self.STATE_PET_WIN:
            self.renderer.draw_text("LOSE", 70, 22)
        elif self.state == self.STATE_DRAW:
            self.renderer.draw_text("DRAW", 70, 22)
        elif self.state == self.STATE_PET_TURN:
            self.renderer.draw_text("...", 76, 22)

    def handle_input(self):
        # Handle game over - press A to restart
        if self.state in (self.STATE_PLAYER_WIN, self.STATE_PET_WIN, self.STATE_DRAW):
            if self.input.was_just_pressed('a') and self.end_delay_timer >= 0.5:
                self.round_number += 1
                self.reset_game()
            return None

        # Handle player turn
        if self.state == self.STATE_PLAYER_TURN:
            # Cursor movement
            row = self.cursor_pos // 3
            col = self.cursor_pos % 3

            if self.input.was_just_pressed('up') and row > 0:
                self.cursor_pos -= 3
            elif self.input.was_just_pressed('down') and row < 2:
                self.cursor_pos += 3
            elif self.input.was_just_pressed('left') and col > 0:
                self.cursor_pos -= 1
            elif self.input.was_just_pressed('right') and col < 2:
                self.cursor_pos += 1

            # Place mark
            if self.input.was_just_pressed('a'):
                self._player_place_mark()

        return None
