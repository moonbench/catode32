"""
Zoomies scene - Chrome dino game clone
"""

import config
import random
from scene import Scene
from assets.minigame_character import RUNCAT1, SITCAT1, SMALL_BIRD1
from assets.nature import SMALLTREE1, PLANT1, PLANT2, PLANT6, CLOUD1, CLOUD2, CLOUD3
from ui import Popup


class ZoomiesScene(Scene):
    """Endless runner minigame inspired by Chrome dino"""

    # Game constants
    GROUND_Y = 54  # Y position of the ground line
    PLAYER_X = 4  # Fixed X position of player
    GRAVITY = 260  # Pixels per second squared
    JUMP_VELOCITY = -140  # Initial jump velocity (negative = up)
    BASE_SPEED = 48  # Starting speed (pixels per second)
    MAX_SPEED = 120  # Maximum speed
    SPEED_INCREASE_INTERVAL = 5  # Points between speed increases
    SPAWN_MIN = 1.3  # Minimum seconds between obstacles
    SPAWN_MAX = 3.2  # Maximum seconds between obstacles
    CLOUD_SPEED_RATIO = 0.2  # Cloud speed as ratio of ground speed
    BIRD_CHANCE = 0.2  # Chance to spawn a bird instead of ground obstacle
    BIRD_Y_LOW = 38  # Bird y position when low (jump over)
    BIRD_Y_HIGH = 20  # Bird y position when high (duck under / stay on ground)

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        # Popups - centered on screen
        self.hit_popup = Popup(renderer, x=14, y=10, width=100, height=24)
        self.start_popup = Popup(renderer, x=14, y=10, width=100, height=24)
        self.reset_game()

    def reset_game(self):
        """Reset all game state for a new game"""
        # Player state
        self.player_y = self.GROUND_Y - RUNCAT1["height"]
        self.player_vy = 0  # Vertical velocity
        self.is_jumping = False
        self.is_hit = False
        self.is_new_best = False

        # Animation
        self.run_anim = 0.0
        self.run_speed = 12  # Frames per second

        # Obstacles: list of {"sprite": sprite, "x": float}
        self.obstacles = []
        self.spawn_timer = 1.0  # Start with a delay before first obstacle

        # Ground decoration: list of {"x": float, "type": str}
        self.ground_decor = []
        self._init_ground_decor()

        # Ground bumps (on the ground line itself)
        self.ground_bumps = []
        self._init_ground_bumps()

        # Clouds: list of {"sprite": sprite, "x": float, "y": int}
        self.clouds = []
        self._init_clouds()

        # Score and speed
        self.score = 0
        self.score_timer = 0.0
        self.current_speed = self.BASE_SPEED

        # Game state
        self.game_started = False

    def _init_ground_decor(self):
        """Initialize ground decoration elements"""
        self.ground_decor = []
        # Add initial decorations spread across the screen
        for x in range(0, config.DISPLAY_WIDTH + 20, 15):
            self._add_decor_at(x)

    def _add_decor_at(self, x):
        """Add a decoration element at the given x position"""
        decor_type = random.choice(["dot", "line", "bump"])
        self.ground_decor.append({"x": float(x), "type": decor_type})

    def _init_ground_bumps(self):
        """Initialize ground bumps on the ground line"""
        self.ground_bumps = []
        for x in range(16, config.DISPLAY_WIDTH + 40, 32):
            self.ground_bumps.append(float(x))

    def _init_clouds(self):
        """Initialize background clouds"""
        self.clouds = []
        # Add a few clouds at random positions
        self._add_cloud_at(20)
        self._add_cloud_at(90)
        self._add_cloud_at(160)

    def _add_cloud_at(self, x):
        """Add a cloud at the given x position"""
        sprite = random.choice([CLOUD1, CLOUD2, CLOUD3])
        # Random y position in upper portion of screen
        y = random.randint(-10, 10)
        self.clouds.append({"sprite": sprite, "x": float(x), "y": y})

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        # Reset game when entering the scene
        self.reset_game()

    def exit(self):
        pass

    def update(self, dt):
        if not self.game_started or self.is_hit:
            return

        # Update score (1 point per ~0.1 seconds)
        self.score_timer += dt
        if self.score_timer >= 0.1:
            self.score += 1
            self.score_timer -= 0.1
            # Update speed when crossing a SPEED_INCREASE_INTERVAL boundary
            if self.score % self.SPEED_INCREASE_INTERVAL == 0:
                self.current_speed = min(self.current_speed + 1, self.MAX_SPEED)

        # Calculate cloud speed from current speed
        cloud_speed = self.current_speed * self.CLOUD_SPEED_RATIO

        # Update player physics
        if self.is_jumping:
            self.player_vy += self.GRAVITY * dt
            self.player_y += self.player_vy * dt

            # Check if landed
            ground_level = self.GROUND_Y - RUNCAT1["height"]
            if self.player_y >= ground_level:
                self.player_y = ground_level
                self.player_vy = 0
                self.is_jumping = False

        # Update running animation
        if not self.is_jumping:
            self.run_anim += dt * self.run_speed
            if self.run_anim >= len(RUNCAT1["frames"]):
                self.run_anim -= len(RUNCAT1["frames"])

        # Update obstacles
        for obstacle in self.obstacles:
            obstacle["x"] -= self.current_speed * dt
            # Animate birds
            if "anim" in obstacle:
                obstacle["anim"] += dt * 8  # 8 fps animation

        # Remove off-screen obstacles
        self.obstacles = [o for o in self.obstacles if o["x"] > -20]

        # Spawn new obstacles
        self.spawn_timer -= dt
        if self.spawn_timer <= 0:
            self._spawn_obstacle()
            if self.current_speed < 80:
                self.spawn_timer = random.uniform(self.SPAWN_MIN, self.SPAWN_MAX)
            else:
                self.spawn_timer = random.uniform(self.SPAWN_MIN * 0.6, self.SPAWN_MAX * 0.8)

        # Update ground decorations
        for decor in self.ground_decor:
            decor["x"] -= self.current_speed * dt

        # Remove off-screen decorations and add new ones
        self.ground_decor = [d for d in self.ground_decor if d["x"] > -10]
        # Add new decorations on the right
        if len(self.ground_decor) == 0 or self.ground_decor[-1]["x"] < config.DISPLAY_WIDTH:
            rightmost = max((d["x"] for d in self.ground_decor), default=0)
            if rightmost < config.DISPLAY_WIDTH + 10:
                self._add_decor_at(rightmost + random.randint(12, 20))

        # Update ground bumps
        for i in range(len(self.ground_bumps)):
            self.ground_bumps[i] -= self.current_speed * dt

        # Remove off-screen bumps and add new ones
        self.ground_bumps = [b for b in self.ground_bumps if b > -10]
        if len(self.ground_bumps) == 0 or max(self.ground_bumps) < config.DISPLAY_WIDTH:
            rightmost = max(self.ground_bumps, default=0)
            self.ground_bumps.append(rightmost + 32)

        # Update clouds (slower parallax)
        for cloud in self.clouds:
            cloud["x"] -= cloud_speed * dt

        # Remove off-screen clouds and add new ones
        self.clouds = [c for c in self.clouds if c["x"] > -70]
        if len(self.clouds) == 0 or max(c["x"] for c in self.clouds) < config.DISPLAY_WIDTH:
            rightmost = max((c["x"] for c in self.clouds), default=0)
            self._add_cloud_at(rightmost + random.randint(60, 100))

        # Check collisions
        self._check_collisions()

    def _spawn_obstacle(self):
        """Spawn a new obstacle on the right side"""
        if random.random() < self.BIRD_CHANCE:
            # Spawn a bird at random height
            y = random.choice([self.BIRD_Y_LOW, self.BIRD_Y_HIGH])
            self.obstacles.append({
                "sprite": SMALL_BIRD1,
                "x": float(config.DISPLAY_WIDTH + 5),
                "y": y,
                "anim": 0.0  # Bird animation counter
            })
        else:
            # Spawn a ground obstacle
            options = [SMALLTREE1, PLANT1, PLANT2]
            if self.current_speed > 110:
                options.append(PLANT6)
            sprite = random.choice(options)
            self.obstacles.append({
                "sprite": sprite,
                "x": float(config.DISPLAY_WIDTH + 5),
                "y": None  # None means ground level
            })

    def _check_collisions(self):
        """Check for collisions between player and obstacles"""
        # Player hitbox (slightly smaller than sprite for fairness)
        player_left = self.PLAYER_X + 4
        player_right = self.PLAYER_X + RUNCAT1["width"] - 4
        player_top = int(self.player_y) + 2
        player_bottom = int(self.player_y) + RUNCAT1["height"]

        for obstacle in self.obstacles:
            obs_sprite = obstacle["sprite"]
            obs_x = int(obstacle["x"])
            # Use custom y if set, otherwise ground level
            if obstacle["y"] is not None:
                obs_y = obstacle["y"]
            else:
                obs_y = self.GROUND_Y - obs_sprite["height"]

            # Obstacle hitbox
            obs_left = obs_x + 2
            obs_right = obs_x + obs_sprite["width"] - 2
            obs_top = obs_y + 2
            obs_bottom = obs_y + obs_sprite["height"]

            # AABB collision
            if (player_right > obs_left and
                player_left < obs_right and
                player_bottom > obs_top and
                player_top < obs_bottom):
                self.is_hit = True
                # Update high score
                if self.score > self.context.zoomies_high_score:
                    self.context.zoomies_high_score = self.score
                    self.is_new_best = True
                else:
                    self.is_new_best = False
                return

    def draw(self):
        self.renderer.clear()

        # Draw clouds (background, behind everything)
        self._draw_clouds()

        # Draw ground line with bumps
        self._draw_ground()

        # Draw ground decorations (below ground line)
        self._draw_ground_decor()

        # Draw obstacles
        for obstacle in self.obstacles:
            sprite = obstacle["sprite"]
            x = int(obstacle["x"])
            # Use custom y if set, otherwise ground level
            if obstacle["y"] is not None:
                y = obstacle["y"]
            else:
                y = self.GROUND_Y - sprite["height"]
            # Get animation frame for birds
            frame = 0
            if "anim" in obstacle:
                frame = int(obstacle["anim"]) % len(sprite["frames"])
            self.renderer.draw_sprite_obj(sprite, x, y, frame=frame, transparent=True)

        # Draw player
        self._draw_player()

        # Draw score
        self._draw_score()

        # Draw start/game over message
        if not self.game_started:
            if self.context.zoomies_high_score > 0:
                self.start_popup.set_text(f"A to start\nBest: {self.context.zoomies_high_score}", wrap=False, center=True)
            else:
                self.start_popup.set_text("Press A to start", center=True)
            self.start_popup.draw(show_scroll_indicators=False)
        elif self.is_hit:
            if self.is_new_best:
                self.hit_popup.set_text(f"NEW BEST!\n{self.score}", wrap=False, center=True)
            else:
                self.hit_popup.set_text(f"Ooof!\nBest: {self.context.zoomies_high_score}", wrap=False, center=True)
            self.hit_popup.draw(show_scroll_indicators=False)

    def _draw_ground(self):
        """Draw the ground line with scrolling bumps"""
        y = self.GROUND_Y

        # Draw the main ground line
        self.renderer.draw_line(0, y, config.DISPLAY_WIDTH, y)

        # Draw scrolling bumps on top of the line
        for bump_x in self.ground_bumps:
            x = int(bump_x)
            if 0 <= x < config.DISPLAY_WIDTH - 4:
                # Small bump (draw black over the line, then the bump shape)
                self.renderer.draw_line(x, y, x + 4, y, color=0)
                self.renderer.draw_line(x, y, x + 2, y - 2)
                self.renderer.draw_line(x + 2, y - 2, x + 4, y)

    def _draw_ground_decor(self):
        """Draw scrolling ground decorations below the ground line"""
        for decor in self.ground_decor:
            x = int(decor["x"])
            if x < 0 or x >= config.DISPLAY_WIDTH:
                continue

            if decor["type"] == "dot":
                self.renderer.draw_pixel(x, self.GROUND_Y + 3)
            elif decor["type"] == "line":
                self.renderer.draw_line(x, self.GROUND_Y + 4, x + 3, self.GROUND_Y + 4)
            elif decor["type"] == "bump":
                self.renderer.draw_pixel(x, self.GROUND_Y + 2)
                self.renderer.draw_pixel(x + 1, self.GROUND_Y + 3)

    def _draw_clouds(self):
        """Draw background clouds"""
        for cloud in self.clouds:
            x = int(cloud["x"])
            y = cloud["y"]
            sprite = cloud["sprite"]
            self.renderer.draw_sprite_obj(sprite, x, y, frame=0, transparent=True)

    def _draw_player(self):
        """Draw the player sprite based on state"""
        x = self.PLAYER_X
        y = int(self.player_y)

        if self.is_hit:
            # Draw sitting cat when hit
            # Adjust y position since SITCAT1 is taller
            sit_y = self.GROUND_Y - SITCAT1["height"]
            self.renderer.draw_sprite_obj(SITCAT1, x, sit_y, frame=0, transparent=True)
        elif self.is_jumping:
            # First frame when jumping
            self.renderer.draw_sprite_obj(RUNCAT1, x, y, frame=0, transparent=True)
        else:
            # Animate through frames when running
            frame = int(self.run_anim) % len(RUNCAT1["frames"])
            self.renderer.draw_sprite_obj(RUNCAT1, x, y, frame=frame, transparent=True)

    def _draw_score(self):
        """Draw score in top-right corner"""
        score_str = str(self.score)
        # Each character is 8 pixels wide in the default font
        x = config.DISPLAY_WIDTH - (len(score_str) * 8) - 2
        self.renderer.draw_text(score_str, x, 2)

    def _draw_centered_text(self, text, y):
        """Draw text centered horizontally"""
        # Each character is 8 pixels wide
        x = (config.DISPLAY_WIDTH - len(text) * 8) // 2
        self.renderer.draw_text(text, x, y)

    def handle_input(self):
        # Check for jump/start/restart
        if self.input.was_just_pressed('a') or self.input.was_just_pressed('up'):
            if not self.game_started:
                self.game_started = True
            elif self.is_hit:
                self.reset_game()
                self.game_started = True
            elif not self.is_jumping:
                # Jump
                self.is_jumping = True
                self.player_vy = self.JUMP_VELOCITY

        return None
