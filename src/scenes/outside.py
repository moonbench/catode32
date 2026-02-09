from scene import Scene
from entities.character import CharacterEntity
from entities.butterfly import ButterflyEntity
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS, SUN_ICON
from assets.nature import PLANT1, PLANTER1, PLANT2, CLOUD1, CLOUD2


class OutsideScene(Scene):
    """Outside scene - similar to NormalScene but outdoors"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.cloud_x = -10.0
        self.cloud_x2 = 30.0
        self.cloud_x3 = 60.0
        self.menu_active = False
        self.character = None
        self.butterfly = None
        self.butterfly2 = None

    def load(self):
        super().load()
        self.character = CharacterEntity(64, 60)
        self.butterfly = ButterflyEntity(110, 20)
        self.butterfly2 = ButterflyEntity(50, 30)
        self.butterfly2.anim_speed = 10
        self.menu = Menu(self.renderer, self.input)

    def unload(self):
        super().unload()

    def enter(self):
        pass

    def exit(self):
        pass

    def update(self, dt):
        self.character.update(dt)
        self.butterfly.update(dt)
        self.butterfly2.update(dt)

        self.cloud_x += dt * 0.8
        self.cloud_x2 += dt * 2.4
        self.cloud_x3 += dt * 1.2

        if self.cloud_x > 128:
            self.cloud_x = -65
        if self.cloud_x2 > 128:
            self.cloud_x2 = -65
        if self.cloud_x3 > 128:
            self.cloud_x3 = -65

    def draw(self):
        """Draw the scene"""
        if self.menu_active:
            self.menu.draw()
            return

        self.renderer.clear()

        # Draw some simple grass tufts
        for x in [10, 35, 80, 110]:
            self.renderer.draw_line(x, 64, x - 2, 60)
            self.renderer.draw_line(x, 64, x, 60)
            self.renderer.draw_line(x, 64, x + 2, 60)

        # Draw a simple sun in corner
        self.renderer.draw_sprite(SUN_ICON, 13, 13, 110, 5)
        self.renderer.draw_sprite_obj(CLOUD1, int(self.cloud_x), -7)
        self.renderer.draw_sprite_obj(CLOUD2, int(self.cloud_x3), 0)
        self.renderer.draw_sprite_obj(CLOUD1, int(self.cloud_x2), -17)

        self.renderer.draw_sprite_obj(PLANTER1, 10, 63 - PLANTER1["height"])
        self.renderer.draw_sprite_obj(PLANT1, 9, 63 - PLANTER1["height"] - PLANT1["height"])

        self.butterfly.draw(self.renderer)
        self.butterfly2.draw(self.renderer)
        self.character.draw(self.renderer)

        self.renderer.draw_sprite_obj(PLANTER1, 94, 63 - PLANTER1["height"])
        self.renderer.draw_sprite_obj(PLANT2, 90, 63 - PLANTER1["height"] - PLANT2["height"])

        self.renderer.show()

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
