from lang import t
"""
Zoomies scene - Chrome dino game clone
"""

import config
import random
from scene import Scene
from assets.minigame_character import RUNCAT1, SITCAT1, SMALL_BIRD1
from assets.nature import SMALLTREE1, CLOUD1, CLOUD2, CLOUD3, SUN, MOON
from assets.plants import PLANT1, PLANT2, PLANT6, SUNFLOWER_YOUNG, ROSE_GROWING, FREESIA_GROWING, ROSE_MATURE, FREESIA_MATURE, FREESIA_THRIVING
from ui import Popup

# Ground decor type constants (int for fast comparison)
DECOR_DOT = 0
DECOR_LINE = 1
DECOR_BUMP = 2


class ZoomiesSky:
    """Accelerated day/night sky for the zoomies minigame.

    One full arc (sun or moon crossing) takes ARC_DURATION real seconds.
    sky_progress: 0.0→1.0 = sun crossing, 1.0→2.0 = moon crossing, loops.
    Starts at 0.45 so the sun is near its peak when the game begins.
    """

    ARC_DURATION = 90.0   # real seconds per celestial arc
    STAR_SPEED   = 4.0    # px/sec stars drift left

    # Screen-space arc endpoints (sprite enters from left, exits right)
    _X_START   = -17      # off left edge (SUN/MOON widths are 17/16)
    _X_END     = 128      # just off right edge
    _Y_HORIZON = 12       # y where body appears at horizon
    _Y_PEAK    = 2        # y at zenith

    # Moon phase frame indices; None = New Moon (not drawn)
    _MOON_PHASES = [None, 0, 1, 2, 3, 4, 5, 6]

    _TWINKLE_PERIOD = 0.15  # seconds per twinkle step (12-step cycle)

    # Static star field: [x, y, twinkle_offset]
    # Spread across ~1.75 screen widths so wrap is seamless.
    # y range 2-50 fills the sky above the ground (GROUND_Y=54).
    _STAR_DATA = [
        [6,   4, 0],  [14, 31, 3],  [23, 14, 6],  [33, 42, 2],  [44,  8, 9],
        [52, 26, 5],  [61, 47, 1],  [70, 18, 8],  [79, 36, 4],  [90,  3, 7],
        [98, 50, 2],  [107, 22, 10], [118, 11, 3], [127, 40, 8], [9,  48, 1],
        [29, 34, 5],  [48,  6, 7],  [68, 44, 0],  [88, 17, 9],  [122, 29, 6],
    ]

    def __init__(self):
        self._moon_phase_idx = 1
        self._sun_anim_timer = 0.0
        self._sun_anim_frame = 0
        self._twinkle_timer  = 0.0
        self._twinkle_phase  = 0
        self._stars = [list(s) for s in self._STAR_DATA]
        self._shuffle_stars()
        self.sky_progress = 0.45  # set last so reset() can be called instead

    def _shuffle_stars(self):
        stars = self._stars
        for i in range(len(stars) - 1, 0, -1):
            j = random.randint(0, i)
            stars[i], stars[j] = stars[j], stars[i]

    def reset(self):
        self.sky_progress    = 0.45
        self._moon_phase_idx = 1
        self._sun_anim_timer = 0.0
        self._sun_anim_frame = 0
        self._twinkle_timer  = 0.0
        self._twinkle_phase  = 0
        self._stars = [list(s) for s in self._STAR_DATA]
        self._shuffle_stars()

    def update(self, dt):
        prev = self.sky_progress
        self.sky_progress += dt / self.ARC_DURATION

        # Moon just finished a crossing → advance lunar phase
        if prev < 2.0 and self.sky_progress >= 2.0:
            self._moon_phase_idx = (self._moon_phase_idx + 1) % len(self._MOON_PHASES)
        self.sky_progress %= 2.0

        # Sun ray animation at ~2 fps
        self._sun_anim_timer += dt
        if self._sun_anim_timer >= 0.5:
            self._sun_anim_timer -= 0.5
            self._sun_anim_frame = (self._sun_anim_frame + 1) % len(SUN["frames"])

        # Scroll stars left; wrap off-left back onto right
        scroll = self.STAR_SPEED * dt
        w = config.DISPLAY_WIDTH
        for star in self._stars:
            star[0] -= scroll
            if star[0] < -5:
                star[0] += w + 10

        # Advance 12-step twinkle phase
        self._twinkle_timer += dt
        if self._twinkle_timer >= self._TWINKLE_PERIOD:
            self._twinkle_timer -= self._TWINKLE_PERIOD
            self._twinkle_phase = (self._twinkle_phase + 1) % 12

    def draw(self, renderer):
        if self.sky_progress >= 1.0:
            self._draw_stars(renderer)
            self._draw_moon(renderer)
        else:
            self._draw_sun(renderer)

    # ------------------------------------------------------------------
    def _arc_xy(self, t):
        x = self._X_START + t * (self._X_END - self._X_START)
        y = self._Y_HORIZON - (self._Y_HORIZON - self._Y_PEAK) * 4.0 * t * (1.0 - t)
        return int(x), int(y)

    def _draw_sun(self, renderer):
        x, y = self._arc_xy(self.sky_progress)
        renderer.draw_sprite_obj(SUN, x, y, frame=self._sun_anim_frame, transparent=True)

    def _draw_moon(self, renderer):
        t = self.sky_progress - 1.0
        x, y = self._arc_xy(t)
        frame = self._MOON_PHASES[self._moon_phase_idx]
        if frame is not None:
            renderer.draw_sprite_obj(MOON, x, y, frame=frame, transparent=True)

    def _draw_stars(self, renderer):
        # Gradually reveal stars at dusk and hide them at dawn
        t = self.sky_progress - 1.0  # 0.0 -> 1.0 across the full night
        n = len(self._stars)
        fade = 0.15  # fraction of night used for fade in/out
        if t < fade:
            visible = int(t / fade * n)
        elif t > 1.0 - fade:
            visible = int((1.0 - t) / fade * n)
        else:
            visible = n

        phase = self._twinkle_phase
        w = config.DISPLAY_WIDTH
        for star in self._stars[:visible]:
            sx = int(star[0])
            if sx < 0 or sx >= w:
                continue
            sy = star[1]
            p = (phase + star[2]) % 12
            renderer.draw_pixel(sx, sy)
            if p == 10:
                # Large twinkle: 4-point cross
                renderer.draw_pixel(sx - 1, sy)
                renderer.draw_pixel(sx + 1, sy)
                renderer.draw_pixel(sx, sy - 1)
                renderer.draw_pixel(sx, sy + 1)
            elif p == 8 or p == 9 or p == 11:
                # Small twinkle: horizontal bar
                renderer.draw_pixel(sx - 1, sy)
                renderer.draw_pixel(sx + 1, sy)


class ZoomiesScene(Scene):
    """Endless runner minigame inspired by Chrome dino"""

    # Game constants
    GROUND_Y = 54  # Y position of the ground line
    PLAYER_X = 2  # Starting/minimum X position of player
    PLAYER_X_MAX = 89  # Maximum X position (~70% of screen width)
    PLAYER_MOVE_SPEED = 60  # Horizontal movement speed on ground (px/s)
    GRAVITY = 207  # Pixels per second squared
    JUMP_VELOCITY = -136  # Initial jump velocity (negative = up)
    BASE_SPEED = 48  # Starting speed (pixels per second)
    MAX_SPEED = 144  # Maximum speed
    SPEED_INCREASE_INTERVAL = 7  # Points between speed increases
    SPAWN_MIN = 0.75  # Minimum seconds between obstacles
    SPAWN_MAX = 2.25  # Maximum seconds between obstacles
    CLOUD_SPEED_RATIO = 0.3  # Cloud speed as ratio of ground speed
    BIRD_CHANCE = 0.22  # Chance to spawn a bird instead of ground obstacle
    BIRD_Y_LOW = 40  # Bird y position when low (jump over)
    BIRD_Y_HIGH = 25  # Bird y position when high (duck under / stay on ground)

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        # Popups - centered on screen
        self.hit_popup = Popup(renderer, x=14, y=10, width=100, height=24)
        self.start_popup = Popup(renderer, x=14, y=10, width=100, height=48)
        self._session_score = 0
        self.score = 0
        self.sky = ZoomiesSky()
        self.reset_game()

    def reset_game(self):
        """Reset all game state for a new game"""
        # Player state
        self.player_x = float(self.PLAYER_X)
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

        # Bonus texts: list of [x_float, y_float, timer]
        self.bonus_texts = []

        # Score and speed
        self.score = 0
        self.score_timer = 0.0
        self.current_speed = self.BASE_SPEED

        # Game state
        self.game_started = False

        # Sky
        self.sky.reset()

    def _init_ground_decor(self):
        """Initialize ground decoration elements"""
        self.ground_decor = []
        # Add initial decorations spread across the screen
        for x in range(0, config.DISPLAY_WIDTH + 20, 15):
            self._add_decor_at(x)

    def _add_decor_at(self, x):
        """Add a decoration element at the given x position"""
        self.ground_decor.append([float(x), random.randint(1, 4)])

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
        y = random.randint(-10, 10)
        self.clouds.append([sprite, float(x), y])

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        # Reset game when entering the scene
        self.reset_game()

    def exit(self):
        # Include score from any in-progress run at exit time
        session = self._session_score + self.score
        print(f"Applying zoomies changes with a session score of: {self._session_score}")
        if session > 0:
            progress = (session / 1000.0) ** 0.5  # asymptotic: 1000pts≈1x, 5000pts≈2.2x, 10000pts≈3.2x
            print(f"Reward scale: {progress}")
            self.context.apply_stat_changes({
                'energy':      -5 * progress,
                'fitness':      8 * progress,
                'fulfillment':  3 * progress,
                'fullness':    -3 * progress,
                'playfulness':  3 * progress,
                'sociability':   3 * progress,
                'loyalty':      1.0 * progress,
            })
            coins = int(5 * progress)
            if coins > 0:
                self.context.coins += coins
                print(f"[Zoomies] Awarded {coins} coins (total: {self.context.coins})")

    def update(self, dt):
        if not self.is_hit:
            self.sky.update(dt)

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
            if self.input.is_pressed('a') or self.input.is_pressed('up'):
                gravity_mult = 1.0
            elif self.input.is_pressed('down'):
                gravity_mult = 1.5
            else:
                gravity_mult = 2.0
            self.player_vy += self.GRAVITY * gravity_mult * dt
            self.player_y += self.player_vy * dt

            # Check if landed
            ground_level = self.GROUND_Y - RUNCAT1["height"]
            if self.player_y >= ground_level:
                self.player_y = ground_level
                self.player_vy = 0
                self.is_jumping = False
        else:
            # Horizontal movement on the ground
            if self.input.is_pressed('left'):
                self.player_x = max(float(self.PLAYER_X), self.player_x - self.PLAYER_MOVE_SPEED * dt)
            elif self.input.is_pressed('right'):
                self.player_x = min(float(self.PLAYER_X_MAX), self.player_x + self.PLAYER_MOVE_SPEED * dt)

        # Update running animation
        if not self.is_jumping:
            self.run_anim += dt * self.run_speed
            if self.run_anim >= len(RUNCAT1["frames"]):
                self.run_anim -= len(RUNCAT1["frames"])

        # Update obstacles: move, animate, detect bird jumps, filter off-screen
        speed_dt = self.current_speed * dt
        anim_dt = dt * 8
        keep = []
        for obs in self.obstacles:
            obs[1] -= speed_dt
            if obs[3] >= 0:  # bird
                obs[3] += anim_dt
                # Mark if player was jumping while bird overlapped player's x-range
                if not obs[4] and self.is_jumping:
                    bird_right = obs[1] + obs[0]["width"]
                    if obs[1] < self.player_x + RUNCAT1["width"] and bird_right > self.player_x:
                        obs[4] = True
            if obs[1] > -20:
                keep.append(obs)
            elif obs[3] >= 0 and obs[4]:  # bird cleared screen and was jumped over
                self._award_bird_jump_bonus()
        self.obstacles = keep

        # Spawn new obstacles
        self.spawn_timer -= dt
        if self.spawn_timer <= 0:
            self._spawn_obstacle()
            if self.current_speed < (self.BASE_SPEED + 40):
                self.spawn_timer = random.uniform(self.SPAWN_MIN * 1.5, self.SPAWN_MAX * 1.25)
            elif self.current_speed > (self.MAX_SPEED -20):
                self.spawn_timer = random.uniform(self.SPAWN_MIN * 1.0, self.SPAWN_MAX * 0.75)
            else:
                self.spawn_timer = random.uniform(self.SPAWN_MIN, self.SPAWN_MAX)

        # Update ground decorations: move and filter in one pass
        keep = []
        for decor in self.ground_decor:
            decor[0] -= speed_dt
            if decor[0] > -10:
                keep.append(decor)
        self.ground_decor = keep
        rightmost = self.ground_decor[-1][0] if self.ground_decor else 0
        if rightmost < config.DISPLAY_WIDTH:
            self._add_decor_at(rightmost + random.randint(12, 20))

        # Update ground bumps: move and filter in one pass
        keep = []
        for b in self.ground_bumps:
            b -= speed_dt
            if b > -10:
                keep.append(b)
        self.ground_bumps = keep
        rightmost = self.ground_bumps[-1] if self.ground_bumps else 0
        if rightmost < config.DISPLAY_WIDTH:
            self.ground_bumps.append(rightmost + 32)

        # Update clouds: move and filter in one pass
        cloud_speed_dt = cloud_speed * dt
        keep = []
        for cloud in self.clouds:
            cloud[1] -= cloud_speed_dt
            if cloud[1] > -70:
                keep.append(cloud)
        self.clouds = keep
        rightmost = self.clouds[-1][1] if self.clouds else 0
        if rightmost < config.DISPLAY_WIDTH:
            self._add_cloud_at(rightmost + random.randint(60, 100))

        # Update bonus texts: rise upward and expire
        keep = []
        for bt in self.bonus_texts:
            bt[1] -= 18.0 * dt
            bt[2] -= dt
            if bt[2] > 0:
                keep.append(bt)
        self.bonus_texts = keep

        # Check collisions
        self._check_collisions()

    def _spawn_obstacle(self):
        """Spawn a new obstacle on the right side.
        List format: [sprite, x, y, anim, jumped]  y=-1 = ground level, anim=-1 = no animation
        jumped: True if player was in the air while this bird overlapped the player (birds only)"""
        if random.random() < self.BIRD_CHANCE:
            y = float(random.choice([self.BIRD_Y_LOW, self.BIRD_Y_HIGH]))
            self.obstacles.append([SMALL_BIRD1, float(config.DISPLAY_WIDTH + 5), y, 0.0, False])
        else:
            options = [
                ('SMALLTREE1',     SMALLTREE1),
                ('PLANT1',         PLANT1),
                ('PLANT2',         PLANT2),
                ('SUNFLOWER_YOUNG', SUNFLOWER_YOUNG),
                ('ROSE_GROWING',   ROSE_GROWING),
                ('FREESIA_GROWING', FREESIA_GROWING),
            ]
            if self.current_speed > 110:
                options += [
                    ('PLANT6',          PLANT6),
                    ('ROSE_MATURE',     ROSE_MATURE),
                    ('FREESIA_MATURE',  FREESIA_MATURE),
                    ('FREESIA_THRIVING', FREESIA_THRIVING),
                ]
            name, chosen = random.choice(options)
            self.obstacles.append([chosen, float(config.DISPLAY_WIDTH + 5), -1.0, -1.0, False])

    def _check_collisions(self):
        """Check for collisions between player and obstacles"""
        player_left = int(self.player_x) + 4
        player_right = int(self.player_x) + RUNCAT1["width"] - 4
        player_top = int(self.player_y) + 2
        player_bottom = int(self.player_y) + RUNCAT1["height"]
        ground_y = self.GROUND_Y

        for obs in self.obstacles:
            obs_sprite = obs[0]
            obs_x = int(obs[1])
            y = obs[2]
            obs_y = ground_y - obs_sprite["height"] if y < 0 else int(y)

            obs_left = obs_x + 2
            obs_right = obs_x + obs_sprite["width"] - 2
            obs_top = obs_y + 2
            obs_bottom = obs_y + obs_sprite["height"]

            if (player_right > obs_left and
                    player_left < obs_right and
                    player_bottom > obs_top and
                    player_top < obs_bottom):
                self.is_hit = True
                # Accumulate score from the run
                self._session_score += self.score

                if self.score > self.context.zoomies_high_score:
                    self.context.zoomies_high_score = self.score
                    self.is_new_best = True
                else:
                    self.is_new_best = False
                return

    def _award_bird_jump_bonus(self):
        """Award +100 score bonus for jumping over a bird and spawn floating text"""
        self.score += 100
        # Center "+100" (4 chars * 8px = 32px wide) on the player
        text_x = self.player_x + RUNCAT1["width"] // 2 - 16
        self.bonus_texts.append([text_x, float(int(self.player_y) - 2), 0.8])

    def draw(self):

        # Draw sky (behind everything — sun/moon/stars)
        self.sky.draw(self.renderer)

        # Draw clouds (background, behind everything)
        self._draw_clouds()

        # Draw ground line with bumps
        self._draw_ground()

        # Draw ground decorations (below ground line)
        self._draw_ground_decor()

        # Draw obstacles
        ground_y = self.GROUND_Y
        for obs in self.obstacles:
            sprite = obs[0]
            x = int(obs[1])
            y = obs[2]
            if y < 0:
                y = ground_y - sprite["height"]
            frame = 0
            if obs[3] >= 0:
                frame = int(obs[3]) % len(sprite["frames"])
            self.renderer.draw_sprite_obj(sprite, x, int(y), frame=frame, transparent=True)

        # Draw player
        self._draw_player()

        # Draw floating bonus texts
        for bt in self.bonus_texts:
            self.renderer.draw_text(t("+100"), int(bt[0]), int(bt[1]))

        # Draw score
        self._draw_score()

        # Draw start/game over message
        if not self.game_started:
            self.start_popup.set_text("A to jump\n\nHold for\nhigh jumps!",  wrap=False, center=True)
            self.start_popup.draw(show_scroll_indicators=False)
        elif self.is_hit:
            if self.is_new_best:
                self.hit_popup.set_text(f"NEW BEST!\n{self.score}", wrap=False, center=True)
            else:
                self.hit_popup.set_text(t("Ooof!") + "\n" + t("Best: {n}", n=self.context.zoomies_high_score), wrap=False, center=True)
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
        ground_y = self.GROUND_Y
        for decor in self.ground_decor:
            x = int(decor[0])
            if x < 0 or x >= config.DISPLAY_WIDTH:
                continue
            decor_type = decor[1]
            if decor_type == DECOR_DOT:
                self.renderer.draw_pixel(x, ground_y + 3)
            elif decor_type == DECOR_LINE:
                self.renderer.draw_line(x, ground_y + 4, x + 3, ground_y + 4)
            else:  # DECOR_BUMP
                self.renderer.draw_pixel(x, ground_y + 2)
                self.renderer.draw_pixel(x + 1, ground_y + 3)

    def _draw_clouds(self):
        """Draw background clouds"""
        for cloud in self.clouds:
            self.renderer.draw_sprite_obj(cloud[0], int(cloud[1]), cloud[2], frame=0, transparent=True)

    def _draw_player(self):
        """Draw the player sprite based on state"""
        x = int(self.player_x)
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
