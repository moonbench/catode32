from scene import Scene
from settings import Settings, SettingItem


STATS = [
    ("Fullness",      "fullness"),
    ("Energy",        "energy"),
    ("Comfort",       "comfort"),
    ("Playfulness",   "playfulness"),
    ("Focus",         "focus"),
    ("Health",        "health"),
    ("Fulfillment",   "fulfillment"),
    ("Cleanliness",   "cleanliness"),
    ("Curiosity",     "curiosity"),
    ("Independence",  "independence"),
    ("Sociability",   "sociability"),
    ("Routine",       "routine"),
    ("Intelligence",  "intelligence"),
    ("Resilience",    "resilience"),
    ("Maturity",      "maturity"),
    ("Affection",     "affection"),
    ("Fitness",       "fitness"),
    ("Appetite",      "appetite"),
    ("Patience",      "patience"),
    ("Charisma",      "charisma"),
    ("Craftiness",    "craftiness"),
    ("Serenity",      "serenity"),
    ("Courage",       "courage"),
    ("Loyalty",       "loyalty"),
    ("Mischief",      "mischievousness"),
    ("Dignity",       "dignity"),
]


class DebugStatsScene(Scene):
    """Debug scene for directly editing pet stats"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.settings = Settings(renderer, input)

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        items = [
            SettingItem(
                name, key,
                min_val=0, max_val=100, step=1,
                value=int(getattr(self.context, key, 50))
            )
            for name, key in STATS
        ]
        self.settings.open(items, transition=False)

    def exit(self):
        pass

    def update(self, dt):
        pass

    def draw(self):
        self.settings.draw()

    def handle_input(self):
        result = self.settings.handle_input()
        if result is not None:
            for key, value in result.items():
                setattr(self.context, key, float(value))
            return ('change_scene', 'normal')
        return None
