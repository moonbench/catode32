"""
Breakout scene - Brick breaker minigame
"""
import random
import config
from scene import Scene
from assets.minigame_character import CAT_AVATAR1


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
    PADDLE_ACCELERATION = 600  # Pixels per second squared
    PADDLE_FRICTION = 400  # Deceleration when not pressing

    # Cat avatar position (bottom-left)
    CAT_X = 0
    CAT_Y = config.DISPLAY_HEIGHT - CAT_AVATAR1["height"]  # 64 - 18 = 46

    # Ball physics
    BALL_SPEED = 60  # Pixels per second

    # Brick grid layout
    BRICK_ROWS = 5
    BRICK_COLS = 18
    BRICK_START_X = 2
    BRICK_START_Y = 2
    POWERUP_BRICK_COUNT = 12  # Number of powerup bricks to place randomly

    # Game states
    STATE_READY = 0
    STATE_PLAYING = 1
    STATE_WIN = 2
    STATE_LOSE = 3

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.reset_game()

    def reset_game(self):
        """Reset all game state for a new game"""
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

        # Bricks: 2D list, None = destroyed, 'normal' = filled, 'powerup' = unfilled
        self.bricks = self._create_bricks()

    def _create_bricks(self):
        """Create the brick grid with randomly placed powerup bricks"""
        # Start with all normal bricks
        bricks = [['normal' for _ in range(self.BRICK_COLS)]
                  for _ in range(self.BRICK_ROWS)]

        # Randomly select positions for powerup bricks
        total_bricks = self.BRICK_ROWS * self.BRICK_COLS
        powerup_count = min(self.POWERUP_BRICK_COUNT, total_bricks)

        # Create list of all positions
        positions = [(r, c) for r in range(self.BRICK_ROWS)
                     for c in range(self.BRICK_COLS)]

        # Randomly pick positions without using shuffle (not in MicroPython)
        for _ in range(powerup_count):
            idx = random.randint(0, len(positions) - 1)
            row, col = positions.pop(idx)
            bricks[row][col] = 'powerup'

        return bricks

    def _position_ball_on_paddle(self):
        """Position ball centered on top of paddle"""
        self.ball_x = float(self.paddle_x + self.PADDLE_WIDTH // 2 - self.BALL_SIZE // 2)
        self.ball_y = float(self.PADDLE_Y - self.BALL_SIZE)

    def _get_brick_rect(self, row, col):
        """Get the rectangle (x, y, w, h) for a brick at given row/col"""
        x = self.BRICK_START_X + col * (self.BRICK_WIDTH + self.BRICK_GAP)
        y = self.BRICK_START_Y + row * (self.BRICK_HEIGHT + self.BRICK_GAP)
        return (x, y, self.BRICK_WIDTH, self.BRICK_HEIGHT)

    def _get_cat_rect(self):
        """Get the cat avatar collision rectangle"""
        return (self.CAT_X, self.CAT_Y, CAT_AVATAR1["width"], CAT_AVATAR1["height"])

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.reset_game()

    def exit(self):
        pass

    def update(self, dt):
        # Update paddle physics (runs in all states for smooth feel)
        self._update_paddle(dt)

        if self.state != self.STATE_PLAYING:
            return

        # Update ball position
        self.ball_x += self.ball_vx * dt
        self.ball_y += self.ball_vy * dt

        # Wall collisions
        self._handle_wall_collisions()

        # Cat avatar collision
        self._handle_cat_collision()

        # Paddle collision
        self._handle_paddle_collision()

        # Brick collisions
        self._handle_brick_collisions()

        # Check if ball fell below screen
        if self.ball_y > config.DISPLAY_HEIGHT:
            self.state = self.STATE_LOSE

        # Check win condition
        if self._all_bricks_destroyed():
            self.state = self.STATE_WIN

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
        cat_x, cat_y, cat_w, cat_h = self._get_cat_rect()

        # Check if ball overlaps with cat
        ball_right = self.ball_x + self.BALL_SIZE
        ball_bottom = self.ball_y + self.BALL_SIZE

        if (self.ball_x < cat_x + cat_w and ball_right > cat_x and
                self.ball_y < cat_y + cat_h and ball_bottom > cat_y):
            # Calculate how far the ball penetrated from each side
            overlap_left = ball_right - cat_x      # Ball entered from left
            overlap_right = (cat_x + cat_w) - self.ball_x  # Ball entered from right
            overlap_top = ball_bottom - cat_y      # Ball entered from top
            overlap_bottom = (cat_y + cat_h) - self.ball_y  # Ball entered from bottom

            min_overlap = min(overlap_left, overlap_right, overlap_top, overlap_bottom)

            # Push ball out and reverse velocity based on smallest penetration
            if min_overlap == overlap_left:
                self.ball_x = cat_x - self.BALL_SIZE
                self.ball_vx = -abs(self.ball_vx)
            elif min_overlap == overlap_right:
                self.ball_x = cat_x + cat_w
                self.ball_vx = abs(self.ball_vx)
            elif min_overlap == overlap_top:
                self.ball_y = cat_y - self.BALL_SIZE
                self.ball_vy = -abs(self.ball_vy)
            else:  # overlap_bottom
                self.ball_y = cat_y + cat_h
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
            import math
            max_angle = math.pi / 3  # 60 degrees
            angle = hit_position * max_angle

            # Set velocity based on angle (ball always goes up after paddle hit)
            self.ball_vx = self.BALL_SPEED * math.sin(angle)
            self.ball_vy = -self.BALL_SPEED * math.cos(angle)

    def _handle_brick_collisions(self):
        """Handle ball hitting bricks"""
        ball_cx = self.ball_x + self.BALL_SIZE / 2
        ball_cy = self.ball_y + self.BALL_SIZE / 2

        for row in range(self.BRICK_ROWS):
            for col in range(self.BRICK_COLS):
                if self.bricks[row][col] is None:
                    continue

                bx, by, bw, bh = self._get_brick_rect(row, col)

                # Check if ball overlaps brick
                if (self.ball_x < bx + bw and
                        self.ball_x + self.BALL_SIZE > bx and
                        self.ball_y < by + bh and
                        self.ball_y + self.BALL_SIZE > by):

                    # Destroy brick
                    self.bricks[row][col] = None

                    # Calculate which side was hit for bounce direction
                    # Find the closest edge
                    dx_left = abs(self.ball_x + self.BALL_SIZE - bx)
                    dx_right = abs(self.ball_x - (bx + bw))
                    dy_top = abs(self.ball_y + self.BALL_SIZE - by)
                    dy_bottom = abs(self.ball_y - (by + bh))

                    min_d = min(dx_left, dx_right, dy_top, dy_bottom)

                    if min_d == dx_left or min_d == dx_right:
                        self.ball_vx = -self.ball_vx
                    else:
                        self.ball_vy = -self.ball_vy

                    # Only handle one brick collision per frame
                    return

    def _all_bricks_destroyed(self):
        """Check if all bricks are destroyed"""
        for row in self.bricks:
            for brick in row:
                if brick is not None:
                    return False
        return True

    def draw(self):
        self.renderer.clear()

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

        # Draw state messages
        if self.state == self.STATE_READY:
            self.renderer.draw_text("A:Start", 80, 56)
        elif self.state == self.STATE_WIN:
            self.renderer.draw_text("WIN!", 50, 30)
        elif self.state == self.STATE_LOSE:
            self.renderer.draw_text("GAME OVER", 35, 30)

    def _draw_bricks(self):
        """Draw all bricks"""
        for row in range(self.BRICK_ROWS):
            for col in range(self.BRICK_COLS):
                brick_type = self.bricks[row][col]
                if brick_type is None:
                    continue

                x, y, w, h = self._get_brick_rect(row, col)

                if brick_type == 'normal':
                    # Filled brick
                    self.renderer.draw_rect(x, y, w, h, filled=True)
                else:
                    # Unfilled brick (powerup)
                    self.renderer.draw_rect(x, y, w, h, filled=False)

    def handle_input(self):
        # Handle game state transitions
        if self.state == self.STATE_READY:
            if self.input.was_just_pressed('a'):
                self._launch_ball()
                self.state = self.STATE_PLAYING
        elif self.state in (self.STATE_WIN, self.STATE_LOSE):
            if self.input.was_just_pressed('a'):
                self.reset_game()
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
        import math
        # Launch at a slight angle (not straight up)
        angle = math.pi / 6  # 30 degrees from vertical
        self.ball_vx = self.BALL_SPEED * math.sin(angle)
        self.ball_vy = -self.BALL_SPEED * math.cos(angle)
