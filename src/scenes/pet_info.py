"""pet_info.py - View and edit pet name and genome seed."""

from scene import Scene
from menu import Menu, MenuItem
from ui_keyboard import OnScreenKeyboard


class PetInfoScene(Scene):
    SCENE_NAME = 'pet_info'

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._menu    = Menu(renderer, input)
        self._editing = None
        self._kb_name = OnScreenKeyboard(renderer, input, charset='full', max_len=12)
        self._kb_seed = OnScreenKeyboard(renderer, input, charset='hex',  max_len=16)

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self._editing = None
        self._rebuild_menu()

    def exit(self):
        pass

    def update(self, dt):
        return None

    def draw(self):
        if self._editing == 'name':
            self._kb_name.draw()
        elif self._editing == 'seed':
            self._kb_seed.draw()
        else:
            self._menu.draw()

    def handle_input(self):
        if self._editing is not None:
            kb = self._kb_name if self._editing == 'name' else self._kb_seed
            result = kb.handle_input()
            if result is not None:
                self._apply(self._editing, result)
                self._editing = None
                self._rebuild_menu()
            return None

        result = self._menu.handle_input()
        if result == 'closed':
            return ('change_scene', 'last_main')
        if result is not None:
            _, field = result
            if field == 'name':
                self._kb_name.open('', self.context.pet_name or '')
                self._editing = 'name'
            else:
                seed_hex = ('%016X' % self.context.pet_seed) if self.context.pet_seed else ''
                self._kb_seed.open('', seed_hex)
                self._editing = 'seed'
        return None

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _rebuild_menu(self):
        seed_hex = ('%016X' % self.context.pet_seed) if self.context.pet_seed else '?'
        self._menu.open([
            MenuItem('Name: ' + (self.context.pet_name or '?'), action=('edit', 'name')),
            MenuItem('Seed: ' + seed_hex,                       action=('edit', 'seed')),
        ])

    def _apply(self, field, value):
        value = value.strip()
        if not value:
            return
        if field == 'name':
            self.context.pet_name = value
        elif field == 'seed':
            try:
                self.context.pet_seed = int(value, 16)
            except ValueError:
                pass
