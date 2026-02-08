from scene import Scene
from assets.character_renderer import CharacterRenderer
from menu import Menu, MenuItem
from assets.icons import TOY_ICONS, SUN_ICON
from assets.nature import PLANT1, PLANTER1


class OutsideScene(Scene):
    """Outside scene - similar to NormalScene but outdoors"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.x = 64
        self.y = 60
        self.menu_active = False

    def load(self):
        super().load()
        self.character_renderer = CharacterRenderer(self.renderer)
        self.menu = Menu(self.renderer, self.input)

    def unload(self):
        super().unload()

    def enter(self):
        pass

    def exit(self):
        pass

    def update(self, dt):
        self.context.char["blink"] = (self.context.char["blink"] + dt) % 10.0
        self.context.char["tail"] = (self.context.char["tail"] + dt * 4) % 16.0

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

        self.renderer.draw_sprite_obj(PLANTER1, 10, 63 - PLANTER1["height"])
        self.renderer.draw_sprite_obj(PLANT1, 9, 63 - PLANTER1["height"] - PLANT1["height"])

        self.character_renderer.draw_character(self.context.char, int(self.x), int(self.y))

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

        # Normal input handling
        move_x, move_y = self.input.get_direction()
        self.x += move_x
        self.y += move_y

        return None

    def _build_menu_items(self):
        """Build context-aware menu items for outside"""
        items = [
            MenuItem("Give pets", action=("pets",)),
            MenuItem("Point at bird", action=("point_bird",)),
            MenuItem("Throw stick", action=("throw_stick",)),
            MenuItem("Give treat", action=("treat",)),
        ]

        # Build toys submenu - only outdoor-appropriate toys
        toy_items = [
            MenuItem(toy, icon=TOY_ICONS.get(toy), action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
            if toy in ["Feather", "Laser"]  # Only some toys work outside
        ]
        if toy_items:
            items.append(MenuItem("Play with toy", submenu=toy_items))

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
