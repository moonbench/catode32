from lang import t
import machine
from scene import Scene

_ACTIONS = ["Reboot", "Deep Sleep"]

_ROW_H = 16
_ACTIONS_START_Y = 24


class DebugPowerScene(Scene):
    """Debug scene for power control: reboot and deep sleep."""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.selected = 0

    def enter(self):
        self.selected = 0

    def draw(self):
        r = self.renderer
        r.draw_text(t("Power Control"), 2, 4)
        r.draw_line(0, 16, 127, 16)

        for i, label in enumerate(_ACTIONS):
            y = _ACTIONS_START_Y + i * _ROW_H
            if i == self.selected:
                r.draw_rect(0, y, 128, _ROW_H, filled=True)
                r.draw_text(label, 4, y + 4, color=0)
            else:
                r.draw_text(label, 4, y + 4, color=1)

    def handle_input(self):
        if (self.input.was_just_pressed('b')
                or self.input.was_just_pressed('menu1')
                or self.input.was_just_pressed('menu2')):
            return ('change_scene', 'last_main')

        if self.input.was_just_pressed('up'):
            self.selected = max(0, self.selected - 1)

        if self.input.was_just_pressed('down'):
            self.selected = min(len(_ACTIONS) - 1, self.selected + 1)

        if self.input.was_just_pressed('a'):
            if self.selected == 0:
                machine.reset()
            elif self.selected == 1:
                try:
                    for pin in self.input.buttons.values():
                        pin.irq(trigger=machine.Pin.IRQ_FALLING, wake=machine.DEEPSLEEP)
                except Exception as e:
                    print(f"Wake pin setup failed: {e}")
                self.renderer.clear()
                self.renderer.show()
                machine.deepsleep()

        return None
