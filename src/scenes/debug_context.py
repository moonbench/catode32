from lang import t
"""debug_context.py - Debug scene for editing key context values."""

from scene import Scene
from settings import Settings, SettingItem
from ui_keyboard import OnScreenKeyboard
from menu import Menu, MenuItem


class DebugContextScene(Scene):

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._settings = Settings(renderer, input)
        self._kb_seed  = OnScreenKeyboard(renderer, input, charset='hex', max_len=16)
        self._menu     = Menu(renderer, input)
        self._mode     = 'settings'

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self._mode = 'settings'
        self._open_settings()

    def exit(self):
        pass

    def update(self, dt):
        pass

    def draw(self):
        if self._mode == 'seed':
            self._kb_seed.draw()
        elif self._mode == 'confirm_reset':
            self._menu.draw()
        else:
            self._settings.draw()

    def handle_input(self):
        if self._mode == 'seed':
            result = self._kb_seed.handle_input()
            if result is not None:
                value = result.strip()
                if value:
                    try:
                        self.context.pet_seed = int(value, 16)
                    except ValueError:
                        pass
                self._mode = 'settings'
                self._open_settings()
            return None

        if self._mode == 'confirm_reset':
            result = self._menu.handle_input()
            if result == ('reset_plants',):
                self.context.reset_plants()
            if result is not None or not self._menu.pending_confirmation:
                self._mode = 'settings'
            return None

        # Settings mode — intercept A for non-numeric items before passing to Settings
        if self.input.was_just_pressed('a') and self._settings.items:
            item = self._settings.items[self._settings.selected_index]
            if item.key == 'seed':
                seed_hex = ('%016X' % self.context.pet_seed) if self.context.pet_seed else ''
                self._kb_seed.open('', seed_hex)
                self._mode = 'seed'
                return None
            if item.key == 'reset_plants':
                reset_item = MenuItem(t("Reset Plants"), action=('reset_plants',),
                                      confirm=t("Reset all plants?"))
                self._menu.open([reset_item])
                self._menu.pending_confirmation = reset_item
                self._mode = 'confirm_reset'
                return None

        result = self._settings.handle_input()
        if result is not None:
            self.context.coins = result['coins']
            return ('change_scene', 'last_main')
        return None

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _open_settings(self):
        seed_hex = ('%016X' % self.context.pet_seed) if self.context.pet_seed else '?'
        self._settings.open([
            SettingItem(t("Coins"),        "coins",        min_val=0, max_val=99999, step=1,
                        value=int(self.context.coins)),
            SettingItem(t("Reset Plants"), "reset_plants", value=""),
            SettingItem(t("Seed"),         "seed",         value=""),
        ])
