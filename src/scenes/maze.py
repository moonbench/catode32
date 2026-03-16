"""
Maze scene - Find the fish minigame
"""
import random
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

    MODULES_TO_KEEP = ['assets.minigame_character']

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
        # Generate maze with reserved open areas for sprites
        self.maze = self.generate_maze()

        # Precompute flat draw list: (px, py, walls_bitmask) per cell, row-major
        self._maze_draw_data = []
        for cy in range(self.GRID_HEIGHT):
            for cx in range(self.GRID_WIDTH):
                px = self.GRID_OFFSET_X + cx * self.CELL_WIDTH
                py = self.GRID_OFFSET_Y + cy * self.CELL_HEIGHT
                self._maze_draw_data.append((px, py, self.maze[cy][cx]))

        # Precompute cell center pixel coords (flat, row-major) for path/indicator
        cx_base = self.GRID_OFFSET_X + self.CELL_WIDTH // 2
        cy_base = self.GRID_OFFSET_Y + self.CELL_HEIGHT // 2
        gw = self.GRID_WIDTH
        self._cell_centers = [
            (cx_base + (i % gw) * self.CELL_WIDTH,
             cy_base + (i // gw) * self.CELL_HEIGHT)
            for i in range(gw * self.GRID_HEIGHT)
        ]

        # Player state (bottom-left)
        self.player_x = 4
        self.player_y = self.GRID_HEIGHT - 1

        # Goal state (top-right)
        self.goal_x = self.GRID_WIDTH - 1
        self.goal_y = 0

        # Path tracking
        self.path = [(self.player_x, self.player_y)]

        # Timer
        self.elapsed_time = 0.0
        self.win_display_timer = 0.0
        self.is_new_best = False

        # Game state
        self.state = self.STATE_PLAYING

    def generate_maze(self):
        """Generate a perfect maze using Prim's algorithm (more branching, harder mazes)"""
        # Initialize grid: each cell is a bitmask of present walls (all walls = 15)
        all_walls = WALL_N | WALL_S | WALL_E | WALL_W
        maze = [[all_walls] * self.GRID_WIDTH for _ in range(self.GRID_HEIGHT)]

        visited = [[False] * self.GRID_WIDTH for _ in range(self.GRID_HEIGHT)]

        # Define start area (bottom-left) and goal area (top-right)
        start_area = {
            'x': 0, 'y': self.GRID_HEIGHT - self.START_CLEAR_HEIGHT,
            'w': self.START_CLEAR_WIDTH, 'h': self.START_CLEAR_HEIGHT
        }
        goal_area = {
            'x': self.GRID_WIDTH - self.GOAL_CLEAR_WIDTH, 'y': 0,
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
        start_gen_y = self.GRID_HEIGHT - 1
        visited[start_gen_y][start_gen_x] = True

        # Connect the start area to the maze
        maze[start_gen_y][start_gen_x] &= ~WALL_W
        maze[start_gen_y][start_gen_x - 1] &= ~WALL_E

        # Frontier: list of (x, y, nx, ny, wall, opposite)
        frontier = []

        def add_frontier(x, y):
            for dx, dy, wall, opposite in directions:
                nx, ny = x + dx, y + dy
                if 0 <= nx < self.GRID_WIDTH and 0 <= ny < self.GRID_HEIGHT:
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

    def move_player(self, dx, dy):
        """Move player and update path"""
        if not self.can_move(dx, dy):
            return

        new_x = self.player_x + dx
        new_y = self.player_y + dy
        new_pos = (new_x, new_y)

        # Check for backtracking
        if new_pos in self.path:
            # Unwind path to that position
            while self.path[-1] != new_pos:
                self.path.pop()
        else:
            # Add to path
            self.path.append(new_pos)

        self.player_x = new_x
        self.player_y = new_y

        # Check win - player enters the goal area (top-right clear zone)
        in_goal_x = self.player_x >= self.GRID_WIDTH - self.GOAL_CLEAR_WIDTH
        in_goal_y = self.player_y < self.GOAL_CLEAR_HEIGHT
        if in_goal_x and in_goal_y:
            self.state = self.STATE_WIN
            self.win_display_timer = 0.0
            # Update best time
            if not hasattr(self.context, 'maze_best_time'):
                self.context.maze_best_time = 0
            if self.context.maze_best_time == 0 or self.elapsed_time < self.context.maze_best_time:
                self.context.maze_best_time = self.elapsed_time
                self.is_new_best = True
            else:
                self.is_new_best = False

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.reset_game()

    def exit(self):
        pass

    def update(self, dt):
        if self.state == self.STATE_PLAYING:
            self.elapsed_time += dt
        elif self.state == self.STATE_WIN:
            self.win_display_timer += dt
            if self.win_display_timer >= self.WIN_DISPLAY_DURATION:
                self.reset_game()

    def draw(self):

        # Draw maze walls
        self.draw_maze()

        # Draw path trail
        self.draw_path()

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
        """Draw maze walls using precomputed draw data"""
        draw_line = self.renderer.draw_line
        cw1 = self.CELL_WIDTH - 1
        ch1 = self.CELL_HEIGHT - 1
        for px, py, walls in self._maze_draw_data:
            if walls & WALL_N:
                draw_line(px, py, px + cw1, py)
            if walls & WALL_S:
                draw_line(px, py + ch1, px + cw1, py + ch1)
            if walls & WALL_E:
                draw_line(px + cw1, py, px + cw1, py + ch1)
            if walls & WALL_W:
                draw_line(px, py, px, py + ch1)

    def draw_path(self):
        """Draw breadcrumb trail connecting visited cells"""
        if len(self.path) < 2:
            return

        centers = self._cell_centers
        gw = self.GRID_WIDTH
        draw_line = self.renderer.draw_line
        for i in range(len(self.path) - 1):
            x1, y1 = self.path[i]
            x2, y2 = self.path[i + 1]
            px1, py1 = centers[y1 * gw + x1]
            px2, py2 = centers[y2 * gw + x2]
            draw_line(px1, py1, px2, py2)

    def draw_goal(self):
        """Draw fish at goal position"""
        self.renderer.draw_sprite_obj(FISH1, 108, 4)

    def draw_player(self):
        """Draw cat at start position (fixed marker)"""
        self.renderer.draw_sprite_obj(SITCAT1, 3, 44)

    def draw_position_indicator(self):
        """Draw blinking 3x3 square at player's current cell center"""
        if self.elapsed_time % 0.5 >= 0.25:
            return

        cx, cy = self._cell_centers[self.player_y * self.GRID_WIDTH + self.player_x]
        self.renderer.draw_rect(cx - 1, cy - 1, 3, 3)

    def draw_win_message(self):
        """Draw win screen overlay"""
        if self.is_new_best:
            title = "NEW BEST!"
        else:
            title = "Found it!"
        time_text = f"Time: {self.elapsed_time:.1f}s"
        self.win_popup.set_text(f"{title}\n{time_text}", wrap=False, center=True)
        self.win_popup.draw(show_scroll_indicators=False)

    def handle_input(self):
        if self.state == self.STATE_WIN:
            if self.input.was_just_pressed('a'):
                self.reset_game()
            return None

        if self.input.was_just_pressed('up'):
            self.move_player(0, -1)
        elif self.input.was_just_pressed('down'):
            self.move_player(0, 1)
        elif self.input.was_just_pressed('left'):
            self.move_player(-1, 0)
        elif self.input.was_just_pressed('right'):
            self.move_player(1, 0)

        return None
