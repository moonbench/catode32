import config
from scene import Scene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from entities.butterfly import ButterflyEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, FISH_ICON, CHICKEN_ICON, MEAL_ICON
from assets.items import FISH1, BOX_SMALL_1, PLANTER_SMALL_1, FOOD_BOWL, TREAT_PILE
from assets.nature import PLANT1, PLANTER1, PLANT2
from sky import SkyRenderer


class OutsideScene(Scene):
    """Outside scene with parallax scrolling environment"""

    ASSETS_TO_KEEP = ['assets.nature']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None

        # Sky renderer handles celestial body, stars, clouds
        self.sky = SkyRenderer()

    def load(self):
        super().load()

        # Create environment with wider world for panning
        self.environment = Environment(world_width=256)

        # Add grass drawing
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_grass)

        # Add plants to foreground
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=10, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=9, y=63 - PLANTER1["height"] - PLANT1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=94, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT2,
            x=90, y=63 - PLANTER1["height"] - PLANT2["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=180, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=179, y=63 - PLANTER1["height"] - PLANT1["height"]
        )

        # Set movement bounds for behaviors like zoomies (world coordinates)
        self.context.scene_x_min = 10
        self.context.scene_x_max = 246

        # Create character with context for behavior management
        self.character = CharacterEntity(64, 64, context=self.context)
        self.character.mirror = True  # default facing left outdoors
        butterfly1 = ButterflyEntity(110, 20)
        butterfly2 = ButterflyEntity(50, 30)
        butterfly2.anim_speed = 10
        butterfly1.bounds_right = 200
        butterfly2.bounds_right = 200

        self.environment.add_entity(butterfly1)
        self.environment.add_entity(butterfly2)

        self.menu = Menu(self.renderer, self.input)

    def _draw_grass(self, renderer, camera_x, parallax):
        """Draw procedural grass tufts"""
        camera_offset = int(camera_x * parallax)
        for world_x in [10, 35, 80, 110, 150, 190, 230]:
            screen_x = world_x - camera_offset
            if screen_x < -5 or screen_x > config.DISPLAY_WIDTH + 5:
                continue
            renderer.draw_line(screen_x, 64, screen_x - 2, 60)
            renderer.draw_line(screen_x, 64, screen_x, 60)
            renderer.draw_line(screen_x, 64, screen_x + 2, 60)

    def unload(self):
        super().unload()

    def enter(self):
        # Configure and add sky objects when entering scene
        env_settings = getattr(self.context, 'environment', {})
        self.sky.configure(env_settings, world_width=self.environment.world_width)
        self.sky.add_to_environment(self.environment, LAYER_BACKGROUND)

        self.environment.add_custom_draw(LAYER_MIDGROUND, self.sky.make_precipitation_drawer(0.6, 1))
        self.environment.add_custom_draw(LAYER_FOREGROUND, self.sky.make_precipitation_drawer(1.0, 2))

        # Restart idle if behavior was stopped when scene was cached
        if self.character and not self.character.current_behavior.active:
            self.character.behavior_manager.trigger('idle')

    def exit(self):
        # Stop active behavior so its module is unloaded while scene is cached
        if self.character:
            self.character.behavior_manager.stop_current()
        # Remove sky objects when leaving scene
        self.sky.remove_from_environment(self.environment, LAYER_BACKGROUND)

    def update(self, dt):
        # Update sky (stars, clouds, celestial animation)
        self.sky.update(dt)

        # Update character
        self.character.update(dt)

        # Update environment entities (butterflies)
        self.environment.update(dt)

    def draw(self):
        """Draw the scene"""
        if self.menu_active:
            self.menu.draw()
            return

        self.renderer.clear()

        # Draw environment with all layers and parallax
        self.environment.draw(self.renderer)

        # Draw character separately (needs mirror control)
        camera_offset = int(self.environment.camera_x)
        self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)

        # Apply lightning inversion (hardware-level, affects display after show())
        self.renderer.invert(self.sky.get_lightning_invert_state())

    def handle_input(self):
        """Process input"""
        if self.menu_active:
            result = self.menu.handle_input()
            if result == 'closed':
                self.menu_active = False
            elif result is not None:
                self.menu_active = False
                self._handle_menu_action(result)
            return None

        if self.input.was_just_pressed('menu2'):
            self.menu_active = True
            self.menu.open(self._build_menu_items())
            return None

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
            MenuItem("Patience", icon=HAND_ICON, action=("train",)),
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
