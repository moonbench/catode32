import config
from scene import Scene
from environment import Environment, LAYER_BACKGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from entities.butterfly import ButterflyEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, SUN_ICON
from assets.nature import PLANT1, PLANTER1, PLANT2, CLOUD1, CLOUD2


class OutsideScene(Scene):
    """Outside scene with parallax scrolling environment"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None

        # Cloud animation state (stored separately, positions updated each frame)
        self.cloud1 = None
        self.cloud2 = None
        self.cloud3 = None

    def load(self):
        super().load()

        # Create environment with wider world for panning
        self.environment = Environment(world_width=256)

        # Add sun to background (static)
        # With 0.3x parallax, when camera is at 128, offset is ~38
        # So sun at x=140 appears at screen_x = 140-38 = 102 when fully panned
        self.environment.add_object(
            LAYER_BACKGROUND,
            {"width": 13, "height": 13, "frames": [SUN_ICON]},
            x=140,
            y=5
        )

        # Add clouds to background - store references so we can animate them
        # Clouds are positioned in world coordinates and get parallax applied
        self.cloud1 = {"sprite": CLOUD1, "x": -10.0, "y": -7}
        self.cloud2 = {"sprite": CLOUD1, "x": 30.0, "y": -17}
        self.cloud3 = {"sprite": CLOUD2, "x": 60.0, "y": 0}
        self.environment.layers[LAYER_BACKGROUND].append(self.cloud1)
        self.environment.layers[LAYER_BACKGROUND].append(self.cloud2)
        self.environment.layers[LAYER_BACKGROUND].append(self.cloud3)

        # Add grass drawing as custom draw function
        self.environment.add_custom_draw(LAYER_FOREGROUND, self._draw_grass)

        # Add plants to foreground (static objects at world positions)
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

        # Add more plants further right (visible when panned)
        self.environment.add_object(
            LAYER_FOREGROUND, PLANTER1,
            x=180, y=63 - PLANTER1["height"]
        )
        self.environment.add_object(
            LAYER_FOREGROUND, PLANT1,
            x=179, y=63 - PLANTER1["height"] - PLANT1["height"]
        )

        # Create character and butterflies as entities
        self.character = CharacterEntity(64, 60)
        butterfly1 = ButterflyEntity(110, 20)
        butterfly2 = ButterflyEntity(50, 30)
        butterfly2.anim_speed = 10
        # Adjust butterfly bounds for wider world
        butterfly1.bounds_right = 200
        butterfly2.bounds_right = 200

        self.environment.add_entity(butterfly1)
        self.environment.add_entity(butterfly2)

        self.menu = Menu(self.renderer, self.input)

    def _draw_grass(self, renderer, camera_x, parallax):
        """Draw procedural grass tufts"""
        camera_offset = int(camera_x * parallax)
        # Grass positions in world coordinates
        for world_x in [10, 35, 80, 110, 150, 190, 230]:
            screen_x = world_x - camera_offset
            # Skip if off-screen
            if screen_x < -5 or screen_x > config.DISPLAY_WIDTH + 5:
                continue
            renderer.draw_line(screen_x, 64, screen_x - 2, 60)
            renderer.draw_line(screen_x, 64, screen_x, 60)
            renderer.draw_line(screen_x, 64, screen_x + 2, 60)

    def unload(self):
        super().unload()

    def enter(self):
        pass

    def exit(self):
        pass

    def update(self, dt):
        # Update cloud positions (they animate on their own)
        self.cloud1["x"] += dt * 2.5
        self.cloud2["x"] += dt * 8.0
        self.cloud3["x"] += dt * 4.0

        # Wrap clouds around (in world coordinates for the full world width)
        wrap_point = self.environment.world_width + 65
        if self.cloud1["x"] > wrap_point:
            self.cloud1["x"] = -65
        if self.cloud2["x"] > wrap_point:
            self.cloud2["x"] = -65
        if self.cloud3["x"] > wrap_point:
            self.cloud3["x"] = -65

        # Update character (not in environment - we draw it separately for mirror control)
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

        # Draw character separately (needs mirror control, draws with foreground parallax)
        camera_offset = int(self.environment.camera_x)
        self.character.draw(self.renderer, mirror=True, camera_offset=camera_offset)

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
        """Build context-aware menu items for outside"""
        items = [
            MenuItem("Give pets", icon=HAND_ICON, action=("pets",)),
            MenuItem("Point at bird", icon=HAND_ICON, action=("point_bird",)),
            MenuItem("Throw stick", icon=HAND_ICON, action=("throw_stick",)),
            MenuItem("Give treat", icon=KIBBLE_ICON, action=("treat",)),
        ]

        # Build toys submenu - only outdoor-appropriate toys
        toy_items = [
            MenuItem(toy, icon=TOY_ICONS.get(toy), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
            if toy in ["Feather", "Laser"]  # Only some toys work outside
        ]
        if toy_items:
            items.append(MenuItem("Play with toy", icon=TOYS_ICON, submenu=toy_items))

        return items

    def _handle_menu_action(self, action):
        """Handle menu selection"""
        if not action:
            return

        action_type = action[0]

        if action_type == "pets":
            self.context.affection = min(100, self.context.affection + 5)
        elif action_type == "point_bird":
            self.context.curiosity = min(100, self.context.curiosity + 10)
            self.context.stimulation = min(100, self.context.stimulation + 5)
        elif action_type == "throw_stick":
            self.context.playfulness = min(100, self.context.playfulness + 15)
            self.context.energy = max(0, self.context.energy - 10)
        elif action_type == "treat":
            self.context.fullness = min(100, self.context.fullness + 5)
            self.context.affection = min(100, self.context.affection + 3)
        elif action_type == "toy":
            self.context.playfulness = min(100, self.context.playfulness + 15)
            self.context.stimulation = min(100, self.context.stimulation + 10)
