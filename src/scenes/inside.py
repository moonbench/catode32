import config
from scene import Scene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, FISH_ICON, CHICKEN_ICON, MEAL_ICON
from assets.furniture import BOOKSHELF
from assets.nature import PLANTER1, PLANT1, PLANT3
from assets.items import FISH1, BOX_SMALL_1, PLANTER_SMALL_1, FOOD_BOWL, TREAT_PILE
from sky import SkyRenderer
from clock import ClockWidget


class InsideScene(Scene):
    MODULES_TO_KEEP = ['assets.furniture', 'assets.nature', 'sky', 'clock']

    # Window position and size (world x, screen y, width, height)
    WINDOW_WORLD_X = 100
    WINDOW_Y = -10
    WINDOW_W = 56
    WINDOW_H = 36

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None

        self.sky = SkyRenderer()
        self._last_weather = None
        self.clock = None

    def load(self):
        super().load()

        # Create environment - indoor room with some panning room
        self.environment = Environment(world_width=192)

        # Plant on window sill
        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER1,
            x=110, y=26 - PLANTER1["height"] + 3
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANT1,
            x=110, y=26 - PLANTER1["height"] + 3 - PLANT1["height"]
        )
        self.environment.add_object(
            LAYER_MIDGROUND, PLANTER_SMALL_1,
            x=130, y=26 - PLANTER_SMALL_1["height"] + 3
        )

        # Add furniture to foreground layer
        self.environment.add_object(
            LAYER_FOREGROUND, BOOKSHELF,
            x=0, y=63 - BOOKSHELF["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, BOX_SMALL_1,
            x=2, y=63 - BOOKSHELF["height"] - BOX_SMALL_1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=15, y=63 - BOOKSHELF["height"] - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT3,
            x=16, y=63 - BOOKSHELF["height"] - PLANTER1["height"] - PLANT3["height"]
        )

        # Add more furniture on the right side (visible when panned)
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=140, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT3,
            x=141, y=63 - PLANTER1["height"] - PLANT3["height"]
        )

        # Set movement bounds for behaviors like zoomies (world coordinates)
        self.context.scene_x_min = 10
        self.context.scene_x_max = 182

        # Create character with context for behavior management
        self.character = CharacterEntity(64, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

        self.clock = ClockWidget(world_x=36, world_y=0)

        self.menu = Menu(self.renderer, self.input)

    def unload(self):
        super().unload()

    def enter(self):
        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)
        self._last_weather = env_settings.get('weather', 'Clear')
        self.environment.add_custom_draw(LAYER_MIDGROUND, self.sky.make_precipitation_drawer(0.3, 0))
        self.environment.add_custom_draw(LAYER_MIDGROUND, self._draw_window)
        self.environment.add_custom_draw(LAYER_MIDGROUND, self.clock.draw)

        # Restart prior behavior (or idle) if behavior was stopped when scene was cached
        if self.character and not self.character.current_behavior.active:
            self.character.behavior_manager.resume_prior_behavior()

    def exit(self):
        # Stop active behavior so its module is unloaded while scene is cached
        if self.character:
            self.character.behavior_manager.stop_current()
        self.environment.custom_draws.clear()
        self.sky.remove_from_environment(self.environment, LAYER_BACKGROUND)

    def update(self, dt):
        env = self.context.environment
        hours = env.get('time_hours', 12)
        minutes = env.get('time_minutes', 0)
        self.sky.set_time(hours, minutes)
        self.clock.set_time(hours, minutes)

        # Re-enter if weather changed so clouds, precipitation, etc. all rebuild
        current_weather = env.get('weather', 'Clear')
        if current_weather != self._last_weather:
            self.exit()
            self.enter()

        self.sky.update(dt)

        # Update character
        prev_x = self.character.x
        self.character.update(dt)

        # Auto-pan only when pet moved and d-pad isn't held
        if not (self.input.is_pressed('left') or self.input.is_pressed('right')):
            if int(prev_x) != int(self.character.x):
                margin = 32
                screen_x = int(self.character.x) - int(self.environment.camera_x)
                if screen_x < margin:
                    self.environment.set_camera(int(self.character.x) - margin)
                elif screen_x > config.DISPLAY_WIDTH - margin:
                    self.environment.set_camera(int(self.character.x) - (config.DISPLAY_WIDTH - margin))

    def _draw_window(self, renderer, camera_x, parallax):
        """Draw window mask and frame on the midground layer (0.6x parallax)."""
        win_sx = self.WINDOW_WORLD_X - int(camera_x * parallax)
        wall_bottom = self.WINDOW_Y + self.WINDOW_H
        screen_left = max(0, win_sx)
        screen_right = min(config.DISPLAY_WIDTH, win_sx + self.WINDOW_W)

        # Mask everything outside the window opening. Full DISPLAY_HEIGHT on the sides
        # so tall sprites (balloon, plane) that extend below the window bottom are covered.
        # Foreground furniture draws on top of these rects afterward.
        if screen_left > 0:
            renderer.draw_rect(0, 0, screen_left, config.DISPLAY_HEIGHT, filled=True, color=0)
        if screen_right < config.DISPLAY_WIDTH:
            renderer.draw_rect(screen_right, 0, config.DISPLAY_WIDTH - screen_right, config.DISPLAY_HEIGHT, filled=True, color=0)
        if self.WINDOW_Y > 0:
            renderer.draw_rect(screen_left, 0, screen_right - screen_left, self.WINDOW_Y, filled=True, color=0)
        if wall_bottom < config.DISPLAY_HEIGHT and screen_right > screen_left:
            renderer.draw_rect(screen_left, wall_bottom, screen_right - screen_left, config.DISPLAY_HEIGHT - wall_bottom, filled=True, color=0)

        # Window frame and sill
        renderer.draw_rect(win_sx, self.WINDOW_Y, self.WINDOW_W, self.WINDOW_H)
        renderer.draw_rect(win_sx - 4, self.WINDOW_Y - 4, self.WINDOW_W + 8, self.WINDOW_H + 8)
        renderer.draw_rect(win_sx - 6, self.WINDOW_Y + self.WINDOW_H + 4, self.WINDOW_W + 12, 3, filled=True)

    def draw(self):
        """Draw the scene"""
        if self.menu_active:
            self.menu.draw()
            return

        # Sky render rect must match the window's midground position (0.6x parallax)
        # so the clip region aligns with the frame when environment.draw() runs.
        win_sx = self.WINDOW_WORLD_X - int(self.environment.camera_x * 0.6)
        self.sky._render_rect = (win_sx, self.WINDOW_Y, self.WINDOW_W, self.WINDOW_H)

        self.renderer.clear()

        # Background: sky (clipped to window rect)
        # Midground: window mask + frame (_draw_window)
        # Foreground: furniture
        self.environment.draw(self.renderer)

        # Draw character (with foreground parallax)
        camera_offset = int(self.environment.camera_x)
        self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)

        # Lightning outside flashes the whole room
        self.renderer.invert(self.sky.get_lightning_invert_state())

    def handle_input(self):
        """Process input - can also return scene change instructions"""
        # Handle menu input when active
        if self.menu_active:
            result = self.menu.handle_input()
            if result == 'closed':
                self.menu_active = False
            elif result is not None:
                self.menu_active = False
                self._handle_menu_action(result)
            return None

        # Open menu on menu2 button
        if self.input.was_just_pressed('menu2'):
            self.menu_active = True
            self.menu.open(self._build_menu_items())
            return None

        # D-pad pans camera
        dx, dy = self.input.get_direction()
        if dx != 0:
            self.environment.pan(dx * config.PAN_SPEED)

        return None

    def _build_menu_items(self):
        """Build context-aware menu items"""

        # Affection submenu
        affection_items = [
            MenuItem("Pets", icon=HAND_ICON, action=("pets",)),
            MenuItem("Kiss", icon=HEART_ICON, action=("kiss",)),
            MenuItem("Psst psst", icon=HEART_BUBBLE_ICON, action=("psst",)),
            MenuItem("Groom", icon=HAND_ICON, action=("groom",))
        ]

        # Feed submenu
        meal_items = [
            MenuItem("Chicken", icon=CHICKEN_ICON, action=("meal", "chicken")),
            MenuItem("Fish", icon=FISH_ICON, action=("meal", "fish")),
        ]
        snack_items = [
            MenuItem(snack["name"], icon=SNACK_ICONS.get(snack["name"]), action=("snack", snack))
            for snack in self.context.inventory.get("snacks", [])
        ]
        feed_items = [
            MenuItem("Meals", icon=MEAL_ICON, submenu=meal_items),
            MenuItem("Snacks", icon=KIBBLE_ICON, submenu=snack_items),
        ]

        # Toys submenu
        toy_items = [
            MenuItem(toy["name"], icon=TOY_ICONS.get(toy["name"]), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
        ]

        # Train submenu
        train_items = [
            MenuItem("Intelligence", icon=HAND_ICON, action=("train",)),
            MenuItem("Behavior", icon=HAND_ICON, action=("train",)),
            MenuItem("Fitness", icon=HAND_ICON, action=("train",)),
            MenuItem("Sociability", icon=HAND_ICON, action=("train",)),
        ]

        # Build parent menu
        items = [
            MenuItem("Affection", icon=HEART_ICON, submenu=affection_items),
            MenuItem("Train", icon=HAND_ICON, submenu=train_items),
            MenuItem("Feed", icon=MEAL_ICON, submenu=feed_items),
        ]
        
        if toy_items:
            items.append(MenuItem("Play", icon=TOYS_ICON, submenu=toy_items))

        return items

    def _handle_menu_action(self, action):
        """Handle menu selection"""
        if not action:
            return

        action_type = action[0]

        if action_type == "meal":
            self.character.trigger('eating', food_sprite=FOOD_BOWL, food_type=action[1])
        elif action_type == "kiss":
            self.character.trigger('affection', variant='kiss')
        elif action_type == "pets":
            self.character.trigger('affection', variant='pets')
        elif action_type == "psst":
            self.character.trigger('attention', variant='psst')
        elif action_type == "snack":
            self.character.trigger('eating', food_sprite=TREAT_PILE, food_type='treat')
        elif action_type == "toy":
            self.character.trigger('playing', variant=action[1]['variant'])
        elif action_type == "groom":
            self.character.trigger('being_groomed')
        elif action_type == "train":
            self.character.trigger('training')
