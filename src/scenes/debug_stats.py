from lang import t
from scene import Scene
from settings import Settings, SettingItem


STATS = [
    (t("Fullness"),      "fullness"),
    (t("Energy"),        "energy"),
    (t("Comfort"),       "comfort"),
    (t("Playfulness"),   "playfulness"),
    (t("Focus"),         "focus"),
    (t("Health"),        "health"),
    (t("Fulfillment"),   "fulfillment"),
    (t("Cleanliness"),   "cleanliness"),
    (t("Curiosity"),     "curiosity"),
    (t("Sociability"),   "sociability"),
    (t("Intelligence"),  "intelligence"),
    (t("Maturity"),      "maturity"),
    (t("Affection"),     "affection"),
    (t("Fitness"),       "fitness"),
    (t("Serenity"),      "serenity"),
    (t("Courage"),       "courage"),
    (t("Loyalty"),       "loyalty"),
    (t("Mischief"),      "mischievousness"),
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
            return ('change_scene', 'last_main')
        return None
