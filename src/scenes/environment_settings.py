from scene import Scene
from settings import Settings, SettingItem


class EnvironmentSettingsScene(Scene):
    """Scene for editing environment settings (time of day, season, weather, etc.)"""

    MODULES_TO_KEEP = ['settings']

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.settings = Settings(renderer, input)

    def enter(self):
        env = getattr(self.context, 'environment', {})
        items = [
            SettingItem(
                "Time", "time_of_day",
                options=["Dawn", "Morning", "Noon", "Afternoon", "Dusk", "Evening", "Night", "Late Night"],
                value=env.get('time_of_day', "Noon")
            ),
            SettingItem(
                "Season", "season",
                options=["Spring", "Summer", "Fall", "Winter"],
                value=env.get('season', "Summer")
            ),
            SettingItem(
                "Moon", "moon_phase",
                options=["New", "Wax Cres", "1st Qtr", "Wax Gib",
                         "Full", "Wan Gib", "3rd Qtr", "Wan Cres"],
                value=env.get('moon_phase', "Full")
            ),
            SettingItem(
                "Weather", "weather",
                options=["Clear", "Cloudy", "Overcast", "Rain", "Storm", "Snow", "Windy"],
                value=env.get('weather', "Clear")
            ),
        ]
        self.settings.open(items)

    def draw(self):
        self.settings.draw()

    def handle_input(self):
        result = self.settings.handle_input()
        if result is not None:
            self.context.environment = result
            return ('change_scene', 'normal')
        return None
