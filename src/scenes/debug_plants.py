"""Debug scene for testing plant and pot sprite rendering."""

from scene import Scene
from menu import Menu, MenuItem
from assets.plants import PLANT_SPRITES, POT_SPRITES
from assets.icons import TREES_ICON


_STAGES     = ('seedling', 'young', 'growing', 'mature', 'thriving')
_HEALTH     = ('healthy', 'wilted', 'dead')
_POT_TYPES  = ('small', 'medium', 'large', 'planter')
_PLANT_TYPES = ('cat_grass', 'freesia', 'rose', 'sunflower')

_FLOOR_Y = 63   # bottom of display


class DebugPlantsScene(Scene):
    """Debug scene for previewing plant and pot sprites.

    menu2   → open pot-type / plant-type submenus
    up/down → cycle growth stage (seedling → thriving)
    left/right → cycle health state (healthy / wilted / dead)
    B       → return to last main scene
    """

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._pot_idx   = 0
        self._plant_idx = 0
        self._stage_idx = 0
        self._health_idx = 0
        self._menu_active = False
        self._menu = None

    def load(self):
        super().load()
        self._menu = Menu(self.renderer, self.input)

    def enter(self):
        self._menu_active = False

    def exit(self):
        pass

    def update(self, dt):
        pass

    # ------------------------------------------------------------------
    # Drawing
    # ------------------------------------------------------------------

    def draw(self):
        if self._menu_active:
            self._menu.draw()
            return

        self._draw_floor()
        self._draw_sprites()
        self._draw_labels()

    def _draw_floor(self):
        self.renderer.draw_line(0, _FLOOR_Y, 128, _FLOOR_Y)

    def _draw_sprites(self):
        pot_type   = _POT_TYPES[self._pot_idx]
        plant_type = _PLANT_TYPES[self._plant_idx]
        stage_key  = self._stage_key()

        pot_spr   = POT_SPRITES[pot_type]
        plant_spr = PLANT_SPRITES[(plant_type, stage_key)]

        cx = 64  # horizontal centre of display

        pot_y = _FLOOR_Y
        if pot_spr:
            pot_y = _FLOOR_Y - pot_spr['height']
            pot_x = cx - pot_spr['width'] // 2
            self.renderer.draw_sprite_obj(pot_spr, pot_x, pot_y)

        if plant_spr:
            plant_x = cx - plant_spr['width'] // 2
            plant_y = pot_y - plant_spr['height']
            self.renderer.draw_sprite_obj(plant_spr, plant_x, plant_y)

    def _draw_labels(self):
        pot_type   = _POT_TYPES[self._pot_idx]
        plant_type = _PLANT_TYPES[self._plant_idx]
        stage      = _STAGES[self._stage_idx]
        health     = _HEALTH[self._health_idx]

        # Two text rows at the top of the screen
        self.renderer.draw_text(f"{plant_type}  {pot_type}", 1, 0)
        self.renderer.draw_text(f"{stage}  {health}", 1, 8)

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    def _stage_key(self):
        base = _STAGES[self._stage_idx]
        h    = _HEALTH[self._health_idx]
        if h == 'healthy':
            return base
        elif h == 'wilted':
            return base + '_wilted'
        else:
            return base + '_dead'

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self._menu_active:
            result = self._menu.handle_input()
            if result == 'closed':
                self._menu_active = False
            elif result is not None:
                self._menu_active = False
                self._handle_menu_action(result)
            return None

        if self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')

        if self.input.was_just_pressed('menu2'):
            self._menu_active = True
            self._menu.open(self._build_menu())
            return None

        if self.input.was_just_pressed('up'):
            self._stage_idx = (self._stage_idx + 1) % len(_STAGES)
        elif self.input.was_just_pressed('down'):
            self._stage_idx = (self._stage_idx - 1) % len(_STAGES)

        if self.input.was_just_pressed('left'):
            self._health_idx = (self._health_idx - 1) % len(_HEALTH)
        elif self.input.was_just_pressed('right'):
            self._health_idx = (self._health_idx + 1) % len(_HEALTH)

        return None

    def _build_menu(self):
        pot_items = [
            MenuItem(pt, icon=TREES_ICON, action=('set_pot', i))
            for i, pt in enumerate(_POT_TYPES)
        ]
        plant_items = [
            MenuItem(pt, icon=TREES_ICON, action=('set_plant', i))
            for i, pt in enumerate(_PLANT_TYPES)
        ]
        return [
            MenuItem("Pot type",      icon=TREES_ICON, submenu=pot_items),
            MenuItem("Plant type",    icon=TREES_ICON, submenu=plant_items),
            MenuItem("Reset Plants",  icon=TREES_ICON, action=('reset_plants',), confirm="Reset all plants?"),
        ]

    def _handle_menu_action(self, action):
        if action[0] == 'set_pot':
            self._pot_idx = action[1]
        elif action[0] == 'set_plant':
            self._plant_idx = action[1]
        elif action[0] == 'reset_plants':
            self.context.reset_plants()
