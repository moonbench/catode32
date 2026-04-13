from scene import Scene
from settings import Settings, SettingItem


class EnvironmentSettingsScene(Scene):
    """Scene for editing environment settings (time of day, season, weather, etc.)"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.settings = Settings(renderer, input)

    def enter(self):
        env = getattr(self.context, 'environment', {})
        items = [
            SettingItem(
                "Day", "day_number",
                min_val=0, max_val=9999999, step=1,
                value=env.get('day_number', 0)
            ),
            SettingItem(
                "Hour", "time_hours",
                min_val=0, max_val=23, step=1,
                value=env.get('time_hours', 12)
            ),
            SettingItem(
                "Min", "time_minutes",
                min_val=0, max_val=55, step=5,
                value=env.get('time_minutes', 0)
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
            self.context.environment.update(result)
            self.context.environment['weather_timer'] = 60.0
            return ('change_scene', 'last_main')
        return None
