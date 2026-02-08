from scene import Scene
from assets.character_renderer import CharacterRenderer
from menu import Menu, MenuItem
from assets.icons import TOYS_ICON, HEART_ICON, HEART_BUBBLE_ICON, HAND_ICON, KIBBLE_ICON, TOY_ICONS, SNACK_ICONS
from assets.furniture import BOOKSHELF

class NormalScene(Scene):
    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.x = 100
        self.y = 60
        self.say_meow = False
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
        self.character_renderer.update_animation(self.context.char, dt)
    
    def draw(self):
        """Draw the scene"""
        if self.menu_active:
            self.menu.draw()
            return

        self.renderer.clear()

        self.renderer.draw_sprite_obj(BOOKSHELF, 0, 63-BOOKSHELF["height"])

        if self.say_meow:
            self.renderer.draw_text("Meow", self.x - 50, self.y - 30)

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
