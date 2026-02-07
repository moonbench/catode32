from scene import Scene
from assets.character_renderer import CharacterRenderer
from menu import Menu, MenuItem

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
        self.context.char["blink"] = (self.context.char["blink"] + dt) % 10.0
        self.context.char["tail"] = (self.context.char["tail"] + dt * 4) % 16.0
    
    def draw(self):
        """Draw the scene"""
        if self.menu_active:
            self.menu.draw()
            return

        self.renderer.clear()

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

        # Normal input handling
        move_x, move_y = self.input.get_direction()
        self.x += move_x
        self.y += move_y

        if self.input.was_just_pressed('a'):
            self.say_meow = not self.say_meow

        return None

    def _build_menu_items(self):
        """Build context-aware menu items"""
        items = [
            MenuItem("Give pets", action=("pets",)),
            MenuItem("Psst psst", action=("psst",)),
            MenuItem("Give kiss", action=("kiss",)),
        ]

        # Build snacks submenu from inventory
        snack_items = [
            MenuItem(snack, action=("snack", snack))
            for snack in self.context.inventory.get("snacks", [])
        ]
        if snack_items:
            items.append(MenuItem("Give snacks", submenu=snack_items))

        # Build toys submenu from inventory
        toy_items = [
            MenuItem(toy, action=("toy", toy))
            for toy in self.context.inventory.get("toys", [])
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
