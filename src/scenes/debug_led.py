import machine
import neopixel
from scene import Scene
from settings import Settings, SettingItem

_LED_PIN = 8
_STEP = 16


class DebugLedScene(Scene):
    """Debug scene to control the onboard WS2812 RGB LED on GPIO8."""



    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.np = None
        self.settings = Settings(renderer, input)

    def load(self):
        super().load()
        try:
            pin = machine.Pin(_LED_PIN, machine.Pin.OUT)
            self.np = neopixel.NeoPixel(pin, 1)
        except Exception as e:
            print(f"LED init error: {e}")
            self.np = None

    def unload(self):
        self._apply()
        super().unload()

    def enter(self):
        items = [
            SettingItem("Toggle", "enabled", options=["ON", "OFF"], value="ON"),
            SettingItem("R", "r", min_val=0, max_val=255, step=_STEP, value=255),
            SettingItem("G", "g", min_val=0, max_val=255, step=_STEP, value=255),
            SettingItem("B", "b", min_val=0, max_val=255, step=_STEP, value=255),
        ]
        self.settings.open(items, transition=False)

    def _apply(self):
        if self.np is None:
            return
        vals = self.settings.get_values()
        if vals.get("enabled") == "ON":
            self.np[0] = (vals.get("r", 255), vals.get("g", 255), vals.get("b", 255))
        else:
            self.np[0] = (0, 0, 0)
        self.np.write()

    def update(self, dt):
        self._apply()

    def draw(self):
        self.settings.draw()

    def handle_input(self):
        result = self.settings.handle_input()
        if result is not None:
            return ('change_scene', 'last_main')
        return None
