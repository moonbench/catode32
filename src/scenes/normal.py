import config
from scene import Scene
from environment import Environment, LAYER_FOREGROUND
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, FISH_ICON, CHICKEN_ICON, MEAL_ICON
from assets.furniture import BOOKSHELF
from assets.nature import PLANTER1, PLANT3
from assets.items import FISH1, BOX_SMALL_1, PLANTER_SMALL_1, FOOD_BOWL, TREAT_PILE


class NormalScene(Scene):
    MODULES_TO_KEEP = ['assets.furniture', 'assets.nature']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None
        self.fish_angle = 0

        # Reference to fish object for animation
        self.fish_obj = None

    def load(self):
        super().load()

        # Create environment - indoor room with some panning room
        self.environment = Environment(world_width=192)

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
            LAYER_FOREGROUND, PLANTER_SMALL_1,
            x=14, y=63 - BOOKSHELF["height"] - PLANTER_SMALL_1["height"]
        )

        # Plants in the middle
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=42, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT3,
            x=43, y=63 - PLANTER1["height"] - PLANT3["height"]
        )

        # Fish - store reference for rotation animation
        self.fish_obj = {"sprite": FISH1, "x": 160, "y": 20, "rotate": 0}
        self.environment.layers[LAYER_FOREGROUND].append(self.fish_obj)

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
        self.character = CharacterEntity(100, 63, context=self.context)
        self.character.set_pose("sitting.forward.neutral")

        self.menu = Menu(self.renderer, self.input)

    def unload(self):
        super().unload()

    def enter(self):
        # Restart idle if behavior was stopped when scene was cached
        if self.character and not self.character.current_behavior.active:
            self.character.behavior_manager.trigger('idle')

    def exit(self):
        # Stop active behavior so its module is unloaded while scene is cached
        if self.character:
            self.character.behavior_manager.stop_current()

    def update(self, dt):
        # Update character
        self.character.update(dt)

        # Update fish rotation
        self.fish_angle = (self.fish_angle + (dt * 25)) % 360
        self.fish_obj["rotate"] = self.fish_angle

    def draw(self):
        """Draw the scene"""
        if self.menu_active:
            self.menu.draw()
            return

        self.renderer.clear()

        # Draw environment with all layers
        self.environment.draw(self.renderer)

        # Draw character (with foreground parallax)
        camera_offset = int(self.environment.camera_x)
        self.character.draw(self.renderer, mirror=self.character.mirror, camera_offset=camera_offset)

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
