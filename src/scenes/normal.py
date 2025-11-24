import assets.character_renderer

from scene import Scene
from assets.character_renderer import CharacterRenderer

class NormalScene(Scene):
    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.x = 100
        self.y = 60
        self.say_meow = False
    
    def load(self):
        super().load()
        self.character_renderer = CharacterRenderer(self.renderer)
    
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
        self.renderer.clear()

        if self.say_meow:
            self.renderer.draw_text("Meow", self.x - 50, self.y - 30)

        self.character_renderer.draw_character(self.context.char, int(self.x), int(self.y))

        self.renderer.show()
        pass
    
    def handle_input(self):
        """Process input - can also return scene change instructions"""
        move_x, move_y = self.input.get_direction()
        self.x += move_x
        self.y += move_y

        if self.input.was_just_pressed('a'):
            self.say_meow = not self.say_meow

        return None
