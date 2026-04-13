class Scene:
    def __init__(self, context, renderer, input_handler):
        self.context = context
        self.renderer = renderer
        self.input = input_handler
        self.is_loaded = False
    
    def load(self):
        """Called when scene is first created - load resources here"""
        self.is_loaded = True
    
    def unload(self):
        """Called when scene is being destroyed - cleanup here"""
        self.is_loaded = False
    
    def enter(self):
        """Called when transitioning TO this scene"""
        pass
    
    def exit(self):
        """Called when transitioning FROM this scene"""
        pass
    
    def update(self, dt):
        """Update game logic - return None to stay, or ('change_scene', SceneClass) to switch"""
        pass
    
    def draw(self):
        """Draw the scene"""
        pass
    
    def handle_input(self):
        """Process input - can also return scene change instructions"""
        pass
