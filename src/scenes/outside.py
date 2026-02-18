import config
from scene import Scene
from environment import Environment, LAYER_BACKGROUND, LAYER_MIDGROUND, LAYER_FOREGROUND
from entities.character import CharacterEntity
from entities.butterfly import ButterflyEntity
from menu import Menu, MenuItem
from actions import ReactionManager, apply_action
from assets.icons import TOYS_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS
from assets.nature import PLANT1, PLANTER1, PLANT2
from sky import SkyRenderer


class OutsideScene(Scene):
    """Outside scene with parallax scrolling environment"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu_active = False
        self.environment = None
        self.character = None

        # Sky renderer handles celestial body, stars, clouds
        self.sky = SkyRenderer()

        # Reaction state
        self.default_pose = "sitting.side.neutral"
        self.reactions = ReactionManager()

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

        # Create character and butterflies
        self.character = CharacterEntity(64, 62)
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

    def exit(self):
        # Remove sky objects when leaving scene
        self.sky.remove_from_environment(self.environment, LAYER_BACKGROUND)

    def update(self, dt):
        # Update sky (stars, clouds, celestial animation)
        self.sky.update(dt)

        # Update character
        self.character.update(dt)

        # Update reactions
        self.reactions.update(dt, self.character, self.default_pose)

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
        self.character.draw(self.renderer, mirror=True, camera_offset=camera_offset)

        # Draw reaction bubble if active
        char_screen_x = int(self.character.x) - camera_offset
        self.reactions.draw(self.renderer, char_screen_x, self.character.y, mirror=True)

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
        """Build context-aware menu items for outside"""
        items = [
            MenuItem("Give pets", icon=HAND_ICON, action=("pets",)),
            MenuItem("Point at bird", icon=HAND_ICON, action=("point_bird",)),
            MenuItem("Throw stick", icon=HAND_ICON, action=("throw_stick",)),
            MenuItem("Give treat", icon=KIBBLE_ICON, action=("treat",)),
        ]

        toy_items = [
            MenuItem(toy, icon=TOY_ICONS.get(toy), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
            if toy in ["Feather", "Laser"]
        ]
        if toy_items:
            items.append(MenuItem("Play with toy", icon=TOYS_ICON, submenu=toy_items))

        return items

    def _handle_menu_action(self, action):
        """Handle menu selection"""
        if action:
            apply_action(action[0], self.context, self.character, self.reactions)
