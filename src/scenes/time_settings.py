from scene import Scene
from settings import Settings, SettingItem


class TimeSettingsScene(Scene):
    """Scene for editing time speed settings."""

    MODULES_TO_KEEP = ['settings']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.settings = Settings(renderer, input)

    def enter(self):
        items = [
            SettingItem(
                "Speed", "time_speed",
                min_val=0.5,
                max_val=20.0,
                step=0.5,
                value=getattr(self.context, 'time_speed', 1.0)
            ),
        ]
        self.settings.open(items)

    def draw(self):
        self.settings.draw()

    def handle_input(self):
        result = self.settings.handle_input()
        if result is not None:
            self.context.time_speed = result.get('time_speed', 1.0)
            return ('change_scene', 'normal')
        return None
