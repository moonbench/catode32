from lang import t
"""
Breakout scene - Brick breaker minigame
"""
import random
import math
import config
from scene import Scene
from assets.minigame_character import CAT_AVATAR1
from assets.minigame_assets import PAW_SMALL1

# Brick type constants (int for fast comparison)
BRICK_EMPTY = 0
BRICK_NORMAL = 1
BRICK_SPECIAL = 2


class BreakoutScene(Scene):
    """Breakout/brick breaker minigame"""



    # Brick dimensions
    BRICK_WIDTH = 6
    BRICK_HEIGHT = 3
    BRICK_GAP = 1

    # Ball dimensions
    BALL_SIZE = 3

    # Paddle dimensions
    PADDLE_WIDTH = 20
    PADDLE_HEIGHT = 2
    PADDLE_Y = 60  # Near bottom of screen

    # Paddle physics
    PADDLE_MAX_SPEED = 120  # Maximum pixels per second
    PADDLE_ACCELERATION = 800  # Pixels per second squared
    PADDLE_FRICTION = 300  # Deceleration when not pressing

    # Cat avatar position (bottom-left)
    CAT_X = 0
    CAT_Y = config.DISPLAY_HEIGHT - CAT_AVATAR1["height"]  # 64 - 18 = 46

    # Ball physics
    BALL_SPEED = 36  # Pixels per second

    # Brick grid layout
    BRICK_ROWS = 5
    BRICK_COLS = 18
    BRICK_START_X = 2
    BRICK_START_Y = 2
    SPECIAL_BRICK_COUNT = 12  # Number of special bricks to place randomly

    # Falling paw settings
    PAW_FALL_SPEED = 40  # Pixels per second

    # Game states
    STATE_READY = 0
    STATE_PLAYING = 1
    STATE_WIN = 2
    STATE_LOSE = 3

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._session_bricks = 0
        self._session_paws = 0
        self._session_won = False
        self.reset_game()

    def reset_game(self, reset_score=True):
        """Reset all game state for a new game

        Args:
            reset_score: If True, reset score to 0. If False, preserve current score.
        """
        # Game state
        self.state = self.STATE_READY

        # Paddle state
        self.paddle_x = float((config.DISPLAY_WIDTH - self.PADDLE_WIDTH) // 2)
        self.paddle_vx = 0.0  # Paddle velocity

        # Ball state (starts on paddle)
        self.ball_x = 0.0
        self.ball_y = 0.0
        self.ball_vx = 0.0
        self.ball_vy = 0.0
        self._position_ball_on_paddle()

        # Bricks: flat list of int (BRICK_EMPTY/NORMAL/SPECIAL), row-major order
        self.bricks, self.bricks_remaining = self._create_bricks()

        # Precomputed brick pixel positions: list of (x, y) per cell, row-major
        self._brick_positions = [
            (self.BRICK_START_X + col * (self.BRICK_WIDTH + self.BRICK_GAP),
             self.BRICK_START_Y + row * (self.BRICK_HEIGHT + self.BRICK_GAP))
            for row in range(self.BRICK_ROWS)
            for col in range(self.BRICK_COLS)
        ]

        # Falling paws: list of [x, y] (list for fast index access)
        self.falling_paws = []

        # Cache static cat rect (never changes)
        self._cat_x = self.CAT_X
        self._cat_y = self.CAT_Y
        self._cat_w = CAT_AVATAR1["width"]
        self._cat_h = CAT_AVATAR1["height"]
        self._cat_right = self._cat_x + self._cat_w
        self._cat_bottom = self._cat_y + self._cat_h

        # Score - only reset if requested
        if reset_score:
            self.score = 0

    def _create_bricks(self):
        """Create the brick grid with randomly placed special bricks.
        Returns (flat_list, count) where flat_list is row-major BRICK_* ints."""
        total = self.BRICK_ROWS * self.BRICK_COLS
        bricks = [BRICK_NORMAL] * total

        special_count = min(self.SPECIAL_BRICK_COUNT, total)
        indices = list(range(total))
        for _ in range(special_count):
            idx = random.randint(0, len(indices) - 1)
            bricks[indices.pop(idx)] = BRICK_SPECIAL

        return bricks, total

    def _position_ball_on_paddle(self):
        """Position ball centered on top of paddle"""
        self.ball_x = float(self.paddle_x + self.PADDLE_WIDTH // 2 - self.BALL_SIZE // 2)
        self.ball_y = float(self.PADDLE_Y - self.BALL_SIZE)

    def _get_brick_rect(self, idx):
        """Get the rectangle (x, y, w, h) for a brick at flat index"""
        x, y = self._brick_positions[idx]
        return (x, y, self.BRICK_WIDTH, self.BRICK_HEIGHT)

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.reset_game()

    def exit(self):
        total_bricks = self.BRICK_ROWS * self.BRICK_COLS
        brick_reward = (self._session_bricks / 144.0) ** 0.5
        paw_reward = (self._session_paws / 20.0) ** 0.5
        print(f"Brick reward {brick_reward}")
        print(f"Paw reward {paw_reward}")
        if self._session_bricks > 0 or self._session_paws > 0:
            changes = {
                'playfulness': 5 * brick_reward,
                'focus':       3 * brick_reward + 4 * paw_reward,
                'fitness':      6 * brick_reward,
                'sociability':  3 * paw_reward,
                'loyalty':      1.0 * brick_reward,
            }
            if self._session_won:
                changes['fulfillment'] = 4 * brick_reward
            else:
                changes['fulfillment'] = 2 * brick_reward
            self.context.apply_stat_changes(changes)
            coins = int((2 * brick_reward) + (3 * paw_reward))
            if coins > 0:
                self.context.coins += coins
                print(f"[Breakout] Awarded {coins} coins (total: {self.context.coins})")

    def update(self, dt):
        # Update paddle physics (runs in all states for smooth feel)
        self._update_paddle(dt)

        if self.state != self.STATE_PLAYING:
            return

        # Sub-step physics to prevent tunneling at low framerates.
        # Ball speed is constant so we can use BALL_SPEED directly.
        max_step = self.BALL_SIZE - 1  # max pixels per sub-step (2px)
        steps = max(1, int(self.BALL_SPEED * dt / max_step) + 1)
        sub_dt = dt / steps

        for _ in range(steps):
            self.ball_x += self.ball_vx * sub_dt
            self.ball_y += self.ball_vy * sub_dt
            self._handle_wall_collisions()
            self._handle_cat_collision()
            self._handle_paddle_collision()
            self._handle_brick_collisions()

        # Update falling paws
        self._update_falling_paws(dt)

        # Check if ball fell below screen
        if self.ball_y > config.DISPLAY_HEIGHT:
            self.state = self.STATE_LOSE


    def _update_paddle(self, dt):
        """Update paddle position based on velocity"""
        # Apply friction/deceleration when not moving
        if self.paddle_vx > 0:
            self.paddle_vx = max(0, self.paddle_vx - self.PADDLE_FRICTION * dt)
        elif self.paddle_vx < 0:
            self.paddle_vx = min(0, self.paddle_vx + self.PADDLE_FRICTION * dt)

        # Update position
        self.paddle_x += self.paddle_vx * dt

        # Clamp to screen bounds (don't overlap cat avatar)
        min_x = CAT_AVATAR1["width"] + 1
        max_x = config.DISPLAY_WIDTH - self.PADDLE_WIDTH
        if self.paddle_x < min_x:
            self.paddle_x = min_x
            self.paddle_vx = 0
        elif self.paddle_x > max_x:
            self.paddle_x = max_x
            self.paddle_vx = 0

    def _update_falling_paws(self, dt):
        """Update falling paws - move down, check paddle/cat catch, remove if off-screen"""
        paw_w = PAW_SMALL1["width"]
        paw_h = PAW_SMALL1["height"]
        fall = self.PAW_FALL_SPEED * dt
        paddle_x = self.paddle_x
        paddle_right = paddle_x + self.PADDLE_WIDTH
        paddle_top = self.PADDLE_Y
        paddle_bottom = paddle_top + self.PADDLE_HEIGHT
        cat_x = self._cat_x
        cat_y = self._cat_y
        cat_right = self._cat_right
        cat_bottom = self._cat_bottom
        screen_h = config.DISPLAY_HEIGHT
        keep = []
        score = self.score
        paws_caught = 0

        for paw in self.falling_paws:
            paw[1] += fall
            py = paw[1]
            px = paw[0]
            pr = px + paw_w
            pb = py + paw_h

            if (pb >= paddle_top and py < paddle_bottom and
                    pr > paddle_x and px < paddle_right):
                score += 1
                paws_caught += 1
            elif (pb >= cat_y and py < cat_bottom and
                    pr > cat_x and px < cat_right):
                score += 1
                paws_caught += 1
            elif py <= screen_h:
                keep.append(paw)

        self.score = score
        self._session_paws += paws_caught
        self.falling_paws = keep

    def _handle_wall_collisions(self):
        """Handle ball bouncing off walls"""
        # Left wall
        if self.ball_x <= 0:
            self.ball_x = 0
            self.ball_vx = abs(self.ball_vx)

        # Right wall
        if self.ball_x + self.BALL_SIZE >= config.DISPLAY_WIDTH:
            self.ball_x = config.DISPLAY_WIDTH - self.BALL_SIZE
            self.ball_vx = -abs(self.ball_vx)

        # Top wall
        if self.ball_y <= 0:
            self.ball_y = 0
            self.ball_vy = abs(self.ball_vy)

    def _handle_cat_collision(self):
        """Handle ball bouncing off cat avatar"""
        cat_x = self._cat_x
        cat_y = self._cat_y
        cat_right = self._cat_right
        cat_bottom = self._cat_bottom
        ball_size = self.BALL_SIZE

        ball_right = self.ball_x + ball_size
        ball_bottom = self.ball_y + ball_size

        if (self.ball_x < cat_right and ball_right > cat_x and
                self.ball_y < cat_bottom and ball_bottom > cat_y):
            overlap_left = ball_right - cat_x
            overlap_right = cat_right - self.ball_x
            overlap_top = ball_bottom - cat_y
            overlap_bottom = cat_bottom - self.ball_y

            min_overlap = min(overlap_left, overlap_right, overlap_top, overlap_bottom)

            if min_overlap == overlap_left:
                self.ball_x = cat_x - ball_size
                self.ball_vx = -abs(self.ball_vx)
            elif min_overlap == overlap_right:
                self.ball_x = cat_right
                self.ball_vx = abs(self.ball_vx)
            elif min_overlap == overlap_top:
                self.ball_y = cat_y - ball_size
                self.ball_vy = -abs(self.ball_vy)
            else:
                self.ball_y = cat_bottom
                self.ball_vy = abs(self.ball_vy)

    def _handle_paddle_collision(self):
        """Handle ball bouncing off paddle with angle variation"""
        paddle_top = self.PADDLE_Y
        paddle_left = self.paddle_x
        paddle_right = self.paddle_x + self.PADDLE_WIDTH

        ball_bottom = self.ball_y + self.BALL_SIZE
        ball_center_x = self.ball_x + self.BALL_SIZE / 2

        # Check if ball is hitting paddle from above
        if (self.ball_vy > 0 and
                ball_bottom >= paddle_top and
                self.ball_y < paddle_top + self.PADDLE_HEIGHT and
                ball_center_x >= paddle_left and
                ball_center_x <= paddle_right):

            # Position ball on top of paddle
            self.ball_y = paddle_top - self.BALL_SIZE

            # Calculate bounce angle based on where ball hit paddle
            # -1 = left edge, 0 = center, 1 = right edge
            hit_position = (ball_center_x - paddle_left) / self.PADDLE_WIDTH
            hit_position = (hit_position - 0.5) * 2  # Convert to -1 to 1 range

            # Angle varies from -60 to +60 degrees based on hit position
            max_angle = math.pi / 3  # 60 degrees
            angle = hit_position * max_angle

            # Set velocity based on angle (ball always goes up after paddle hit)
            self.ball_vx = self.BALL_SPEED * math.sin(angle)
            self.ball_vy = -self.BALL_SPEED * math.cos(angle)

    def _handle_brick_collisions(self):
        """Handle ball hitting bricks"""
        bricks = self.bricks
        positions = self._brick_positions
        ball_x = self.ball_x
        ball_y = self.ball_y
        ball_size = self.BALL_SIZE
        ball_right = ball_x + ball_size
        ball_bottom = ball_y + ball_size
        bw = self.BRICK_WIDTH
        bh = self.BRICK_HEIGHT

        for idx in range(len(bricks)):
            if bricks[idx] == BRICK_EMPTY:
                continue

            bx, by = positions[idx]
            bx_right = bx + bw
            by_bottom = by + bh

            if (ball_x < bx_right and ball_right > bx and
                    ball_y < by_bottom and ball_bottom > by):

                if bricks[idx] == BRICK_SPECIAL:
                    paw_x = bx + bw // 2 - PAW_SMALL1["width"] // 2
                    self.falling_paws.append([float(paw_x), float(by_bottom)])

                bricks[idx] = BRICK_EMPTY
                self.bricks_remaining -= 1
                self._session_bricks += 1
                if self.bricks_remaining == 0:
                    self.state = self.STATE_WIN
                    self._session_won = True
                    return

                dx_left = ball_right - bx
                dx_right = bx_right - ball_x
                dy_top = ball_bottom - by
                dy_bottom = by_bottom - ball_y

                min_d = min(dx_left, dx_right, dy_top, dy_bottom)
                if min_d == dy_top or min_d == dy_bottom:
                    self.ball_vy = -self.ball_vy
                else:
                    self.ball_vx = -self.ball_vx

                return

    def draw(self):
        # Draw cat avatar
        self.renderer.draw_sprite_obj(CAT_AVATAR1, self.CAT_X, self.CAT_Y)

        # Draw bricks
        self._draw_bricks()

        # Draw paddle
        self.renderer.draw_rect(
            int(self.paddle_x), self.PADDLE_Y,
            self.PADDLE_WIDTH, self.PADDLE_HEIGHT,
            filled=True
        )

        # Draw ball
        self.renderer.draw_rect(
            int(self.ball_x), int(self.ball_y),
            self.BALL_SIZE, self.BALL_SIZE,
            filled=True
        )

        # Draw falling paws
        for paw in self.falling_paws:
            self.renderer.draw_sprite_obj(
                PAW_SMALL1, int(paw[0]), int(paw[1]), transparent=True
            )

        # Draw score above cat avatar
        if self.score > 0:
            score_str = str(self.score)
            # Position score above cat avatar, centered
            score_x = self.CAT_X + (CAT_AVATAR1["width"] - len(score_str) * 8) // 2
            score_y = self.CAT_Y - 10
            self.renderer.draw_text(score_str, score_x, score_y)

        # Draw state messages
        if self.state == self.STATE_READY:
            self.renderer.draw_text(t("A: Start"), 32, 30)
        elif self.state == self.STATE_WIN:
            self.renderer.draw_text(t("WIN!"), 50, 30)
        elif self.state == self.STATE_LOSE:
            self.renderer.draw_text(t("GAME OVER"), 34, 30)

    def _draw_bricks(self):
        """Draw all bricks"""
        bricks = self.bricks
        positions = self._brick_positions
        bw = self.BRICK_WIDTH
        bh = self.BRICK_HEIGHT
        draw_rect = self.renderer.draw_rect

        for idx in range(len(bricks)):
            bt = bricks[idx]
            if bt == BRICK_EMPTY:
                continue
            x, y = positions[idx]
            draw_rect(x, y, bw, bh, filled=(bt == BRICK_NORMAL))

    def handle_input(self):
        # Handle game state transitions
        if self.state == self.STATE_READY:
            if self.input.was_just_pressed('a'):
                self._launch_ball()
                self.state = self.STATE_PLAYING
        elif self.state == self.STATE_WIN:
            if self.input.was_just_pressed('a'):
                # Keep score when winning
                self.reset_game(reset_score=False)
            return None
        elif self.state == self.STATE_LOSE:
            if self.input.was_just_pressed('a'):
                # Reset score on game over
                self.reset_game(reset_score=True)
            return None

        # Handle paddle movement with acceleration
        dt = 1.0 / config.FPS  # Approximate dt for input handling
        if self.input.is_pressed('left'):
            self.paddle_vx -= self.PADDLE_ACCELERATION * dt
            self.paddle_vx = max(-self.PADDLE_MAX_SPEED, self.paddle_vx)

        if self.input.is_pressed('right'):
            self.paddle_vx += self.PADDLE_ACCELERATION * dt
            self.paddle_vx = min(self.PADDLE_MAX_SPEED, self.paddle_vx)

        # Keep ball on paddle when not launched
        if self.state == self.STATE_READY:
            self._position_ball_on_paddle()

        return None

    def _launch_ball(self):
        """Launch the ball from the paddle"""
        # Launch at a slight angle (not straight up)
        angle = math.pi / 6  # 30 degrees from vertical
        self.ball_vx = self.BALL_SPEED * math.sin(angle)
        self.ball_vy = -self.BALL_SPEED * math.cos(angle)
