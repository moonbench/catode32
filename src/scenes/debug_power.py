import machine
from lang import t
from scene import Scene
from menu import Menu, MenuItem


class DebugPowerScene(Scene):
    """Debug scene for power control: reboot and deep sleep."""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._menu = Menu(renderer, input)

    def enter(self):
        self._menu.open([
            MenuItem(t("Reboot"),      action=('reboot',)),
            MenuItem(t("Light Sleep"), action=('light_sleep',)),
            MenuItem(t("Deep Sleep"),  action=('deep_sleep',)),
        ])

    def draw(self):
        self._menu.draw()

    def handle_input(self):
        result = self._menu.handle_input()
        if result == 'closed':
            return ('change_scene', 'last_main')
        if result == ('reboot',):
            machine.reset()
        elif result == ('light_sleep',):
            self.context.pending_light_sleep = True
            return ('change_scene', 'last_main')
        elif result == ('deep_sleep',):
            try:
                for pin in self.input.buttons.values():
                    pin.irq(trigger=machine.Pin.IRQ_FALLING, wake=machine.DEEPSLEEP)
            except Exception as e:
                print(f"Wake pin setup failed: {e}")
            self.renderer.clear()
            self.renderer.show()
            machine.deepsleep()
        return None
