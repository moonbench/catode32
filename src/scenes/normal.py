import config
from scene import Scene
from environment import Environment, LAYER_FOREGROUND
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS
from assets.furniture import BOOKSHELF
from assets.nature import PLANTER1, PLANT3
from assets.items import FISH1, BOX_SMALL_1, PLANTER_SMALL_1


class NormalScene(Scene):
    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.say_meow = False
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
            x=50, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT3,
            x=51, y=63 - PLANTER1["height"] - PLANT3["height"]
        )

        # Fish - store reference for rotation animation
        self.fish_obj = {"sprite": FISH1, "x": 60, "y": 20, "rotate": 0}
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

        # Create character
        self.character = CharacterEntity(100, 63)
        self.character.set_pose("sitting.forward.neutral")

        self.menu = Menu(self.renderer, self.input)

    def unload(self):
        super().unload()

    def enter(self):
        pass

    def exit(self):
        pass

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

        # Draw speech bubble if active (UI layer, doesn't scroll)
        if self.say_meow:
            # Draw relative to character's screen position
            camera_offset = int(self.environment.camera_x)
            char_screen_x = int(self.character.x) - camera_offset
            self.renderer.draw_text("Meow", char_screen_x - 50, int(self.character.y) - 30)

        # Draw character (with foreground parallax)
        camera_offset = int(self.environment.camera_x)
        self.character.draw(self.renderer, camera_offset=camera_offset)

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

        if self.input.was_just_pressed('a'):
            self.say_meow = not self.say_meow

        return None

    def _build_menu_items(self):
        """Build context-aware menu items"""
        items = [
            MenuItem("Give pets", icon=HAND_ICON, action=("pets",)),
            MenuItem("Psst psst", icon=HEART_BUBBLE_ICON, action=("psst",)),
            MenuItem("Give kiss", icon=HEART_ICON, action=("kiss",)),
        ]

        # Build snacks submenu from inventory
        snack_items = [
            MenuItem(snack, icon=SNACK_ICONS.get(snack), action=("snack", snack))
            for snack in self.context.inventory.get("snacks", [])
        ]
        if snack_items:
            items.append(MenuItem("Give snacks", icon=KIBBLE_ICON, submenu=snack_items))

        # Build toys submenu from inventory
        toy_items = [
            MenuItem(toy, icon=TOY_ICONS.get(toy), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
        ]
        if toy_items:
            items.append(MenuItem("Use toys", icon=TOYS_ICON, submenu=toy_items))

        return items

    def _handle_menu_action(self, action):
        """Handle menu selection"""
        if not action:
            return

        action_type = action[0]

        if action_type == "pets":
            self.context.affection = min(100, self.context.affection + 5)
        elif action_type == "psst":
            self.context.curiosity = min(100, self.context.curiosity + 3)
        elif action_type == "kiss":
            self.context.affection = min(100, self.context.affection + 10)
        elif action_type == "snack":
            snack_name = action[1]
            self.context.fullness = min(100, self.context.fullness + 10)
        elif action_type == "toy":
            toy_name = action[1]
            self.context.playfulness = min(100, self.context.playfulness + 15)
            self.context.stimulation = min(100, self.context.stimulation + 10)
