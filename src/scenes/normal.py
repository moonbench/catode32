import config
from scene import Scene
from environment import Environment, LAYER_FOREGROUND
from entities.character import CharacterEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, FISH_ICON, CHICKEN_ICON, MEAL_ICON
from assets.furniture import BOOKSHELF
from assets.nature import PLANTER1, PLANT3
from assets.items import FISH1, BOX_SMALL_1, PLANTER_SMALL_1, FOOD_BOWL


class NormalScene(Scene):
    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None
        self.fish_angle = 0

        # Reference to fish object for animation
        self.fish_obj = None

        # Eating state
        self.food_bowl_obj = None

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

        # Create character with context for behavior management
        self.character = CharacterEntity(100, 63, context=self.context)
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

        # Sync food bowl with character's eating progress
        if self.food_bowl_obj and self.character.eating.active:
            bowl_x, bowl_y = self.character.eating.get_bowl_position(
                self.character.x, self.character.y, mirror=False
            )
            self.food_bowl_obj["x"] = bowl_x
            self.food_bowl_obj["y"] = bowl_y
            self.food_bowl_obj["frame"] = self.character.eating.get_bowl_frame()

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
            else:
                # Menu is open - check if user wants to switch to big menu
                if self.input.was_just_pressed('menu1'):
                    # Close context menu and open big menu
                    self.menu_active = False
                    return ('open_big_menu',)
            return None

        # Check MENU1 button release for short press FIRST (before was_long_pressed resets state)
        hold_time = self.input.was_released_after_hold('menu1')
        if hold_time >= 0:
            # Button was released before long press threshold
            if hold_time < self.input.hold_time_ms:
                # Short press - open context menu
                self.menu_active = True
                self.menu.open(self._build_menu_items())
                return None
            # If hold_time >= threshold, long press already triggered, ignore release

        # Check for long press (instant at 500ms threshold)
        if self.input.was_long_pressed('menu1'):
            # Long press reached - open big menu immediately
            return ('open_big_menu',)

        # D-pad pans camera
        dx, dy = self.input.get_direction()
        if dx != 0:
            self.environment.pan(dx * config.PAN_SPEED)

        return None

    def _build_menu_items(self):
        """Build context-aware menu items"""
        items = [
            MenuItem("Give pets", icon=HAND_ICON, action=("pets",)),
            MenuItem("Psst psst", icon=HEART_BUBBLE_ICON, action=("psst",)),
            MenuItem("Give kiss", icon=HEART_ICON, action=("kiss",)),
        ]

        # Meals submenu
        meal_items = [
            MenuItem("Chicken", icon=CHICKEN_ICON, action=("meal", "chicken")),
            MenuItem("Fish", icon=FISH_ICON, action=("meal", "fish")),
        ]
        items.append(MenuItem("Meals", icon=MEAL_ICON, submenu=meal_items))

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

    def open_context_menu(self):
        """Open the context menu (called when switching from big menu)"""
        self.menu_active = True
        self.menu.open(self._build_menu_items())

    def _handle_menu_action(self, action):
        """Handle menu selection"""
        if not action:
            return

        action_type = action[0]
        manager = self.character.behavior_manager

        if action_type == "meal":
            self._start_eating(action[1])
        elif action_type == "kiss":
            manager.trigger("affection", variant="kiss", context=self.context)
        elif action_type == "pets":
            manager.trigger("affection", variant="pets", context=self.context)
        elif action_type == "psst":
            manager.trigger("attention", variant="psst", context=self.context)
        elif action_type == "snack":
            manager.trigger("snacking", variant="snack", context=self.context)
        elif action_type == "toy":
            manager.trigger("playing", trigger="toy", context=self.context)

    def _start_eating(self, meal_type):
        """Start the eating sequence.

        Args:
            meal_type: "chicken" or "fish"
        """
        # Start eating first so bowl sprite is set
        self.character.eating.start(FOOD_BOWL, meal_type, on_complete=self._on_eating_complete)

        # Add food bowl to environment at character's calculated position
        bowl_x, bowl_y = self.character.eating.get_bowl_position(
            self.character.x, self.character.y, mirror=False
        )
        self.food_bowl_obj = {
            "sprite": FOOD_BOWL,
            "x": bowl_x,
            "y": bowl_y,
            "frame": 0,
        }
        self.environment.layers[LAYER_FOREGROUND].append(self.food_bowl_obj)

    def _on_eating_complete(self, completed, progress):
        """Called when eating finishes. Handles visual cleanup."""
        # Remove food bowl from environment
        if self.food_bowl_obj and self.food_bowl_obj in self.environment.layers[LAYER_FOREGROUND]:
            self.environment.layers[LAYER_FOREGROUND].remove(self.food_bowl_obj)
        self.food_bowl_obj = None
