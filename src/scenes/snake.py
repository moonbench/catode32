"""
Snake minigame - guide the cat to eat spots on a 4x4 grid
"""
import random
from scene import Scene
from assets.minigame_assets import CAT_THIN, SPOT

CELL = 4
GRID_W = 32   # 128 // CELL
GRID_H = 16   # 64 // CELL
TOTAL_CELLS = GRID_W * GRID_H  # 512

SPEED = 6.0  # cells per second

# Per-direction head draw params: (frame_index, w, h, mirror_h, mirror_v, ox, oy)
# ox/oy center the sprite on the 4x4 cell: (CELL - w) // 2, (CELL - h) // 2
_HEAD = {
    (0,  1): (0, 6, 8, False, False, -1, -2),  # down  - natural orientation
    (0, -1): (0, 6, 8, False, True,  -1, -2),  # up    - mirror_v
    (1,  0): (1, 8, 6, False, False, -2, -1),  # right - natural orientation
    (-1, 0): (1, 8, 6, True,  False, -2, -1),  # left  - mirror_h
}

STATE_READY   = 0
STATE_PLAYING = 1
STATE_WIN     = 2
STATE_LOSE    = 3


class SnakeScene(Scene):
    def enter(self):
        self._init_game()

    def unload(self):
        super().unload()

    def exit(self):
        session = getattr(self, '_session_score', 0) + getattr(self, 'score', 0)
        if session > 0:
            progress = (session / 50.0) ** 0.5
            print(f"Reward progress: {progress}")
            self.context.apply_stat_changes({
                'focus':       5 * progress,
                'playfulness': 4 * progress,
                'fitness':      5 * progress,
                'sociability':  3 * progress + 0.5,
                'loyalty':      0.5 * progress,
            })
            coins = int(5 * progress)
            if coins > 0:
                self.context.coins += coins
                print(f"[Snake] Awarded {coins} coins (total: {self.context.coins})")

    def _generate_dirt(self):
        W, H = 128, 64
        EDGE = 13  # zone thickness for edge bias
        dirt = []
        for _ in range(random.randint(4, 7)):
            edge = random.randrange(4)
            if edge == 0:    cx, cy = random.randint(4, W-5), random.randint(1, EDGE)
            elif edge == 1:  cx, cy = random.randint(4, W-5), random.randint(H-EDGE-1, H-2)
            elif edge == 2:  cx, cy = random.randint(1, EDGE), random.randint(4, H-5)
            else:            cx, cy = random.randint(W-EDGE-1, W-2), random.randint(4, H-5)
            placed = 0
            attempts = 0
            while placed < random.randint(2, 6) and attempts < 20:
                attempts += 1
                px = max(0, min(W-1, cx + random.randint(-4, 4)))
                py = max(0, min(H-1, cy + random.randint(-4, 4)))
                if all(abs(px-ex) > 2 or abs(py-ey) > 2 for ex, ey in dirt):
                    dirt.append((px, py))
                    placed += 1
        self.dirt = dirt

    def _init_game(self):
        # Accumulate score from the run that just ended before resetting
        self._session_score = getattr(self, '_session_score', 0) + getattr(self, 'score', 0)
        self._generate_dirt()

        mid_x = GRID_W // 2
        mid_y = GRID_H // 2
        self.snake = [[mid_x, mid_y], [mid_x - 1, mid_y], [mid_x - 2, mid_y]]
        self.occupied = bytearray(TOTAL_CELLS)
        for seg in self.snake:
            self.occupied[seg[1] * GRID_W + seg[0]] = 1
        self.direction = (1, 0)
        self.next_dir = (1, 0)
        self.move_progress = 0.0
        self.food = None
        self._place_food()
        self.score = 0
        self.state = STATE_READY

    def _place_food(self):
        empty_count = TOTAL_CELLS - len(self.snake)
        if empty_count == 0:
            self.food = None
            return
        k = random.randrange(empty_count)
        for i in range(TOTAL_CELLS):
            if not self.occupied[i]:
                if k == 0:
                    self.food = (i % GRID_W, i // GRID_W)
                    return
                k -= 1

    def handle_input(self):
        inp = self.input
        dx, dy = self.direction

        if inp.was_just_pressed('up') and dy == 0:
            self.next_dir = (0, -1)
        elif inp.was_just_pressed('down') and dy == 0:
            self.next_dir = (0, 1)
        elif inp.was_just_pressed('left') and dx == 0:
            self.next_dir = (-1, 0)
        elif inp.was_just_pressed('right') and dx == 0:
            self.next_dir = (1, 0)

        if self.state == STATE_READY:
            if inp.was_just_pressed('a'):
                self.state = STATE_PLAYING
        elif self.state in (STATE_WIN, STATE_LOSE):
            if inp.was_just_pressed('a'):
                self._init_game()
                self.state = STATE_PLAYING

    def update(self, dt):
        if self.state != STATE_PLAYING:
            return
        self.move_progress += SPEED * dt
        while self.move_progress >= 1.0:
            self.move_progress -= 1.0
            self._step()

    def _step(self):
        self.direction = self.next_dir
        dx, dy = self.direction
        hx, hy = self.snake[0]
        nx = (hx + dx) % GRID_W
        ny = (hy + dy) % GRID_H
        idx = ny * GRID_W + nx

        if self.occupied[idx]:
            self.state = STATE_LOSE
            return

        self.snake.insert(0, [nx, ny])
        self.occupied[idx] = 1

        food = self.food
        if food is not None and nx == food[0] and ny == food[1]:
            self.score += 1
            if self.score > self.context.snake_high_score:
                self.context.snake_high_score = self.score
            if len(self.snake) == TOTAL_CELLS:
                self.state = STATE_WIN
                self.food = None
                return
            self._place_food()
        else:
            tail = self.snake.pop()
            self.occupied[tail[1] * GRID_W + tail[0]] = 0

    def draw(self):
        r = self.renderer

        if self.state == STATE_READY:
            r.draw_text("SNAKE", 44, 12)
            r.draw_text("A: START", 32, 28)
            return

        if self.state == STATE_WIN:
            r.draw_text("YOU WIN!", 32, 16)
            r.draw_text(f"SCORE:{self.score}", 28, 30)
            r.draw_text("A: RETRY", 32, 44)
            return

        if self.state == STATE_LOSE:
            r.draw_text("GAME OVER", 28, 16)
            r.draw_text(f"SCORE:{self.score}", 28, 30)
            r.draw_text("A: RETRY", 32, 44)
            return

        # Dirt pixels
        for px, py in self.dirt:
            r.draw_rect(px, py, 1, 1, filled=True)

        # Food
        if self.food is not None:
            fx, fy = self.food
            r.draw_sprite_obj(SPOT, fx * CELL, fy * CELL)

        # Body segments (between head and tail)
        slen = len(self.snake)
        for i in range(1, slen - 1):
            gx, gy = self.snake[i]
            r.draw_rect(gx * CELL, gy * CELL, CELL, CELL, filled=True)

        # Tail: 4x2 (horizontal) or 2x4 (vertical)
        if slen > 1:
            tx, ty = self.snake[-1]
            px, py = self.snake[-2]
            tdx = px - tx
            tdy = py - ty
            if tdx > 1:   tdx = -1
            elif tdx < -1: tdx = 1
            if tdy > 1:   tdy = -1
            elif tdy < -1: tdy = 1
            if tdx != 0:
                r.draw_rect(tx * CELL, ty * CELL + 1, CELL, 2, filled=True)
            else:
                r.draw_rect(tx * CELL + 1, ty * CELL, 2, CELL, filled=True)

        # Head with sub-cell interpolation
        hx, hy = self.snake[0]
        dx, dy = self.direction
        prog = int(self.move_progress * CELL)
        fi, w, h, mh, mv, ox, oy = _HEAD[(dx, dy)]
        r.draw_sprite(
            CAT_THIN["frames"][fi], w, h,
            hx * CELL + dx * prog + ox,
            hy * CELL + dy * prog + oy,
            mirror_h=mh, mirror_v=mv,
        )
