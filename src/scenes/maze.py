"""
Maze scene - Find the fish minigame
"""
import random
import math
import framebuf
import config
from scene import Scene
from assets.minigame_character import SITCAT1
from assets.items import FISH1
from ui import Popup

# Wall bitmask constants (int for fast bitwise checks)
WALL_N = 1
WALL_S = 2
WALL_E = 4
WALL_W = 8


class MazeScene(Scene):
    """Maze minigame - guide cat to fish through a maze"""

    # Grid constants
    GRID_WIDTH = 25
    GRID_HEIGHT = 12
    CELL_WIDTH = 5
    CELL_HEIGHT = 5
    GRID_OFFSET_X = 1
    GRID_OFFSET_Y = 2

    # Game constants
    WIN_DISPLAY_DURATION = 2.5

    # State constants
    STATE_PLAYING = 0
    STATE_WIN = 1

    # Hard mode: starting on round 3, the maze extends upward and the fish
    # is hidden above the top of the screen until the player scrolls up.
    EXTRA_ROWS_PER_STEP = 3   # 3 rows × 5px = 15px added each step
    HARD_MODE_ROUND = 3
    SCROLL_TOP = 16           # top 25% of 64px screen — scroll up when player crosses this
    SCROLL_BOT = 48           # bottom 75% of 64px screen — scroll down when player crosses this

    # Wide mode: every 5th round the maze grows one fish-width to the right.
    EXTRA_COLS_PER_STEP = 3   # 3 cols × 5px = 15px = 1 fish width
    WIDE_MODE_ROUND = 5
    MAX_SCALING_ROUND = 15    # both 3 and 5 meet here; maze size is capped beyond this
    SCROLL_LEFT = 32          # left 25% of 128px screen
    SCROLL_RIGHT = 96         # right 75% of 128px screen

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        # Win message popup - centered on screen
        self.win_popup = Popup(renderer, x=14, y=16, width=100, height=32)
        self.reset_game()

    START_CLEAR_WIDTH = 4
    START_CLEAR_HEIGHT = 4
    GOAL_CLEAR_WIDTH = 4
    GOAL_CLEAR_HEIGHT = 3

    def reset_game(self):
        """Reset all game state for a new maze"""
        # Accumulate completion stats before resetting state
        if getattr(self, 'state', self.STATE_PLAYING) == self.STATE_WIN:
            self._session_completions = getattr(self, '_session_completions', 0) + 1

        # Advance round counter and compute hard/wide-mode parameters
        self._session_round = getattr(self, '_session_round', 0) + 1
        scaled_round = min(self._session_round, self.MAX_SCALING_ROUND)
        self._extra_rows = (scaled_round // self.HARD_MODE_ROUND) * self.EXTRA_ROWS_PER_STEP
        self._grid_height = self.GRID_HEIGHT + self._extra_rows
        self._max_camera_y = self._extra_rows * self.CELL_HEIGHT
        self._camera_y = self._max_camera_y  # start showing the bottom (cat area)
        self._extra_cols = (scaled_round // self.WIDE_MODE_ROUND) * self.EXTRA_COLS_PER_STEP
        self._grid_width = self.GRID_WIDTH + self._extra_cols
        self._max_camera_x = self._extra_cols * self.CELL_WIDTH
        self._camera_x = 0  # start showing the left (cat area)

        # Generate maze with reserved open areas for sprites
        self.maze = self.generate_maze()

        # On even rounds, carve reward rooms into the generated maze
        self._reward_cells = []
        self._reward_collected = set()
        if self._session_round % 2 == 0:
            count = 3 if self._session_round >= 12 else (2 if self._session_round >= 8 else 1)
            self._setup_reward_areas(count)

        # Pre-render static maze walls into a FrameBuffer so draw_maze is a
        # single blit instead of ~1200 draw_line calls every frame.
        self._build_maze_cache()

        # Precompute cell center pixel coords (maze-space, row-major) for path/indicator
        cx_base = self.GRID_OFFSET_X + self.CELL_WIDTH // 2
        cy_base = self.GRID_OFFSET_Y + self.CELL_HEIGHT // 2
        gw = self._grid_width
        self._cell_centers = [
            (cx_base + (i % gw) * self.CELL_WIDTH,
             cy_base + (i // gw) * self.CELL_HEIGHT)
            for i in range(gw * self._grid_height)
        ]

        # Player state (bottom-left of extended maze)
        self.player_x = 4
        self.player_y = self._grid_height - 1

        # Goal state (top-right)
        self.goal_x = self.GRID_WIDTH - 1
        self.goal_y = 0

        # Path tracking (list for ordered traversal, set for O(1) backtrack lookup)
        self.path = [(self.player_x, self.player_y)]
        self.path_set = {(self.player_x, self.player_y)}

        self._anim_t = 0.0
        self.win_display_timer = 0.0
        self._b_held_time = 0.0
        self._b_rewind_timer = 0.0

        # Game state
        self.state = self.STATE_PLAYING

    def _build_maze_cache(self):
        """Pre-render maze walls into a FrameBuffer (runs once per generation)."""
        fb_height = 64 + self._extra_rows * self.CELL_HEIGHT
        maze_px_wide = self.GRID_OFFSET_X + self._grid_width * self.CELL_WIDTH
        fb_width = (maze_px_wide + 7) & ~7  # round up to multiple of 8 for MONO_HLSB
        buf = bytearray((fb_width // 8) * fb_height)
        fb = framebuf.FrameBuffer(buf, fb_width, fb_height, framebuf.MONO_HLSB)
        cw1 = self.CELL_WIDTH - 1
        ch1 = self.CELL_HEIGHT - 1
        ox = self.GRID_OFFSET_X
        oy = self.GRID_OFFSET_Y
        cw = self.CELL_WIDTH
        ch = self.CELL_HEIGHT
        for cy in range(self._grid_height):
            row = self.maze[cy]
            for cx in range(self._grid_width):
                walls = row[cx]
                px = ox + cx * cw
                py = oy + cy * ch
                if walls & WALL_N:
                    fb.line(px, py, px + cw1, py, 1)
                if walls & WALL_S:
                    fb.line(px, py + ch1, px + cw1, py + ch1, 1)
                if walls & WALL_E:
                    fb.line(px + cw1, py, px + cw1, py + ch1, 1)
                if walls & WALL_W:
                    fb.line(px, py, px, py + ch1, 1)
        self._maze_fb = fb

    def _setup_reward_areas(self, count):
        """Carve `count` 3×3 reward rooms at non-overlapping random spots."""
        placed = []  # top-left corners already placed

        for _ in range(count):
            for _attempt in range(100):
                rx = random.randint(1, self._grid_width - 4)
                ry = random.randint(1, self._grid_height - 4)
                in_start = (rx < self.START_CLEAR_WIDTH and
                            ry + 2 >= self._grid_height - self.START_CLEAR_HEIGHT)
                in_goal = (rx + 2 >= self._grid_width - self.GOAL_CLEAR_WIDTH and
                           ry < self.GOAL_CLEAR_HEIGHT)
                # Require at least 4-cell separation from other reward areas
                too_close = any(abs(rx - px) < 4 and abs(ry - py) < 4 for px, py in placed)
                if not in_start and not in_goal and not too_close:
                    placed.append((rx, ry))
                    break

        for rx, ry in placed:
            # Clear internal walls of the 3×3 block
            for dy in range(3):
                for dx in range(3):
                    cx, cy = rx + dx, ry + dy
                    if dy > 0:
                        self.maze[cy][cx] &= ~WALL_N
                        self.maze[cy - 1][cx] &= ~WALL_S
                    if dx > 0:
                        self.maze[cy][cx] &= ~WALL_W
                        self.maze[cy][cx - 1] &= ~WALL_E
            self._reward_cells.append((rx + 1, ry + 1))

    def generate_maze(self):
        """Generate a perfect maze using Prim's algorithm (more branching, harder mazes)"""
        # Initialize grid: each cell is a bitmask of present walls (all walls = 15)
        all_walls = WALL_N | WALL_S | WALL_E | WALL_W
        maze = [[all_walls] * self._grid_width for _ in range(self._grid_height)]

        visited = [[False] * self._grid_width for _ in range(self._grid_height)]

        # Define start area (bottom-left) and goal area (top-right)
        start_area = {
            'x': 0, 'y': self._grid_height - self.START_CLEAR_HEIGHT,
            'w': self.START_CLEAR_WIDTH, 'h': self.START_CLEAR_HEIGHT
        }
        goal_area = {
            'x': self._grid_width - self.GOAL_CLEAR_WIDTH, 'y': 0,
            'w': self.GOAL_CLEAR_WIDTH, 'h': self.GOAL_CLEAR_HEIGHT
        }

        # Clear internal walls in start and goal areas, mark as visited
        for area in [start_area, goal_area]:
            for ay in range(area['y'], area['y'] + area['h']):
                for ax in range(area['x'], area['x'] + area['w']):
                    visited[ay][ax] = True
                    if ay > area['y']:
                        maze[ay][ax] &= ~WALL_N
                        maze[ay - 1][ax] &= ~WALL_S
                    if ax > area['x']:
                        maze[ay][ax] &= ~WALL_W
                        maze[ay][ax - 1] &= ~WALL_E

        directions = [
            (0, -1, WALL_N, WALL_S),
            (0, 1,  WALL_S, WALL_N),
            (1, 0,  WALL_E, WALL_W),
            (-1, 0, WALL_W, WALL_E),
        ]

        # Prim's algorithm: maintain a frontier of walls to potentially remove
        start_gen_x = self.START_CLEAR_WIDTH
        start_gen_y = self._grid_height - 1
        visited[start_gen_y][start_gen_x] = True

        # Connect the start area to the maze
        maze[start_gen_y][start_gen_x] &= ~WALL_W
        maze[start_gen_y][start_gen_x - 1] &= ~WALL_E

        # Frontier: list of (x, y, nx, ny, wall, opposite)
        frontier = []

        def add_frontier(x, y):
            for dx, dy, wall, opposite in directions:
                nx, ny = x + dx, y + dy
                if 0 <= nx < self._grid_width and 0 <= ny < self._grid_height:
                    if not visited[ny][nx]:
                        frontier.append((x, y, nx, ny, wall, opposite))

        add_frontier(start_gen_x, start_gen_y)

        while frontier:
            idx = random.randint(0, len(frontier) - 1)
            x, y, nx, ny, wall, opposite = frontier.pop(idx)

            if not visited[ny][nx]:
                maze[y][x] &= ~wall
                maze[ny][nx] &= ~opposite
                visited[ny][nx] = True
                add_frontier(nx, ny)

        # Connect goal area to the maze (open wall on left side of goal area)
        goal_entry_x = goal_area['x'] - 1
        goal_entry_y = goal_area['y'] + goal_area['h'] - 1
        maze[goal_entry_y][goal_entry_x] &= ~WALL_E
        maze[goal_entry_y][goal_entry_x + 1] &= ~WALL_W

        return maze

    def cell_to_pixel(self, cell_x, cell_y):
        """Convert grid coordinates to pixel coordinates (top-left of cell)"""
        px = self.GRID_OFFSET_X + cell_x * self.CELL_WIDTH
        py = self.GRID_OFFSET_Y + cell_y * self.CELL_HEIGHT
        return px, py

    def can_move(self, dx, dy):
        """Check if player can move in given direction"""
        cell = self.maze[self.player_y][self.player_x]
        if dx == 1:
            return not (cell & WALL_E)
        elif dx == -1:
            return not (cell & WALL_W)
        elif dy == -1:
            return not (cell & WALL_N)
        elif dy == 1:
            return not (cell & WALL_S)
        return False

    def _update_camera(self):
        """Adjust camera to keep the player within the scroll dead zones.

        Tracks the player in lockstep when they cross the 25%/75% boundary
        on either axis. Clamped so the maze never scrolls past its edges.
        """
        if self._max_camera_y:
            player_py = self.GRID_OFFSET_Y + self.player_y * self.CELL_HEIGHT + self.CELL_HEIGHT // 2
            screen_py = player_py - self._camera_y
            if screen_py < self.SCROLL_TOP:
                self._camera_y = max(0, player_py - self.SCROLL_TOP)
            elif screen_py > self.SCROLL_BOT:
                self._camera_y = min(self._max_camera_y, player_py - self.SCROLL_BOT)

        if self._max_camera_x:
            player_px = self.GRID_OFFSET_X + self.player_x * self.CELL_WIDTH + self.CELL_WIDTH // 2
            screen_px = player_px - self._camera_x
            if screen_px < self.SCROLL_LEFT:
                self._camera_x = max(0, player_px - self.SCROLL_LEFT)
            elif screen_px > self.SCROLL_RIGHT:
                self._camera_x = min(self._max_camera_x, player_px - self.SCROLL_RIGHT)

    def _rewind_step(self):
        """Remove the last step from the path, moving the player back one cell."""
        if len(self.path) <= 1:
            return
        self.path_set.discard(self.path.pop())
        self.player_x, self.player_y = self.path[-1]
        self._update_camera()

    def move_player(self, dx, dy):
        """Move player and update path"""
        if not self.can_move(dx, dy):
            return

        new_x = self.player_x + dx
        new_y = self.player_y + dy
        new_pos = (new_x, new_y)

        # Check for backtracking (O(1) set lookup instead of O(n) list scan)
        if new_pos in self.path_set:
            while self.path[-1] != new_pos:
                self.path_set.discard(self.path.pop())
        else:
            self.path.append(new_pos)
            self.path_set.add(new_pos)

        self.player_x = new_x
        self.player_y = new_y

        self._update_camera()

        # Check reward collection
        pos = (self.player_x, self.player_y)
        if pos in self._reward_cells and pos not in self._reward_collected:
            self._reward_collected.add(pos)
            self._reward_coins_earned = getattr(self, '_reward_coins_earned', 0) + 1

        # Check win - player enters the goal area (top-right clear zone)
        in_goal_x = self.player_x >= self._grid_width - self.GOAL_CLEAR_WIDTH
        in_goal_y = self.player_y < self.GOAL_CLEAR_HEIGHT
        if in_goal_x and in_goal_y:
            self.state = self.STATE_WIN
            self.win_display_timer = 0.0

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self._session_round = 0  # restart round count each visit
        self.reset_game()

    def exit(self):
        completions = getattr(self, '_session_completions', 0)
        if completions > 0:
            scale = (completions / 2.0) ** 0.5
            print(f"Reward scale: {scale}")
            self.context.apply_stat_changes({
                'intelligence': 3 * scale,
                'curiosity':    3 * scale,
                'focus':        3 * scale,
                'sociability':   2,
                'loyalty':      1.0 * scale,
            })
            coins = int(5 * scale) + getattr(self, '_reward_coins_earned', 0)
            if coins > 0:
                self.context.coins += coins
                print(f"[Maze] Awarded {coins} coins (total: {self.context.coins})")

    def update(self, dt):
        self._anim_t += dt
        if self.state == self.STATE_PLAYING and self.input.is_pressed('b'):
            if len(self.path) > 1:
                self._b_held_time += dt
                self._b_rewind_timer += dt
                interval = max(0.05, 0.3 - self._b_held_time * 0.12)
                if self._b_rewind_timer >= interval:
                    self._b_rewind_timer -= interval
                    self._rewind_step()
        else:
            self._b_held_time = 0.0
            self._b_rewind_timer = 0.0
        if self.state == self.STATE_WIN:
            self.win_display_timer += dt
            if self.win_display_timer >= self.WIN_DISPLAY_DURATION:
                self.reset_game()

    def draw(self):

        # Draw maze walls
        self.draw_maze()

        # Draw path trail
        self.draw_path()

        # Draw reward diamond (if present and not yet collected)
        self.draw_reward()

        # Draw goal (fish)
        self.draw_goal()

        # Draw player (cat)
        self.draw_player()

        # Draw blinking position indicator
        self.draw_position_indicator()

        # Draw win message
        if self.state == self.STATE_WIN:
            self.draw_win_message()

    def draw_maze(self):
        """Blit pre-rendered maze framebuf with camera offset applied."""
        self.renderer.display.blit(self._maze_fb, -self._camera_x, -self._camera_y, 0)

    def draw_path(self):
        """Draw breadcrumb trail connecting visited cells"""
        if len(self.path) < 2:
            return

        centers = self._cell_centers
        gw = self._grid_width
        cam_x = self._camera_x
        cam_y = self._camera_y
        draw_line = self.renderer.draw_line
        for i in range(len(self.path) - 1):
            x1, y1 = self.path[i]
            x2, y2 = self.path[i + 1]
            px1, py1 = centers[y1 * gw + x1]
            px2, py2 = centers[y2 * gw + x2]
            draw_line(px1 - cam_x, py1 - cam_y, px2 - cam_x, py2 - cam_y)

    def draw_reward(self):
        """Draw bobbing hollow diamonds at uncollected reward cells."""
        if not self._reward_cells:
            return
        bob = int(math.sin(self._anim_t * 3.14) * 2)
        gw = self._grid_width
        cam_x = self._camera_x
        cam_y = self._camera_y
        r = 2
        draw_line = self.renderer.draw_line
        for cell in self._reward_cells:
            if cell in self._reward_collected:
                continue
            rx, ry = cell
            cx, cy = self._cell_centers[ry * gw + rx]
            cx -= cam_x
            cy -= cam_y + bob
            draw_line(cx,     cy - r, cx + r, cy    )
            draw_line(cx + r, cy,     cx,     cy + r)
            draw_line(cx,     cy + r, cx - r, cy    )
            draw_line(cx - r, cy,     cx,     cy - r)

    def draw_goal(self):
        """Draw fish at goal position (may be off-screen until player scrolls there)"""
        self.renderer.draw_sprite_obj(FISH1, 108 + self._max_camera_x - self._camera_x, 4 - self._camera_y)

    def draw_player(self):
        """Draw cat at the start-area position, offset by camera"""
        self.renderer.draw_sprite_obj(SITCAT1, 3 - self._camera_x, 44 + self._max_camera_y - self._camera_y)

    def draw_position_indicator(self):
        """Draw blinking 3x3 square at player's current cell center"""
        if self._anim_t % 0.5 >= 0.25:
            return

        cx, cy = self._cell_centers[self.player_y * self._grid_width + self.player_x]
        self.renderer.draw_rect(cx - 1 - self._camera_x, cy - 1 - self._camera_y, 3, 3)

    def draw_win_message(self):
        """Draw win screen overlay"""
        self.win_popup.set_text("Found it!", wrap=False, center=True)
        self.win_popup.draw(show_scroll_indicators=False)

    def handle_input(self):
        if self.state == self.STATE_WIN:
            if self.input.was_just_pressed('a'):
                self.reset_game()
            return None

        if self.input.was_just_pressed('b'):
            self._rewind_step()
            self._b_held_time = 0.0
            self._b_rewind_timer = 0.0
        elif self.input.was_just_pressed('up'):
            self.move_player(0, -1)
        elif self.input.was_just_pressed('down'):
            self.move_player(0, 1)
        elif self.input.was_just_pressed('left'):
            self.move_player(-1, 0)
        elif self.input.was_just_pressed('right'):
            self.move_player(1, 0)

        return None
