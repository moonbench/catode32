"""Store scene - buy food and toys with coins."""

from scene import Scene
from menu import Menu, MenuItem
from ui import Popup
from assets.store import (
    SHINGLE1, SHINGLE2, SHADOW1, COUNTER_STRUT, PAW_LOGO, STORE_TEXT, ITEMS1, ITEMS2, ITEMS3
)

# Coins per purchase (all food grants this many uses)
_FOOD_USES = 5

# (display_name, food_stock_key, cost)
_MEAL_ITEMS = (
    ("Chicken",  "chicken",  8),
    ("Salmon",   "salmon",   7),
    ("Tuna",     "tuna",     6),
    ("Shrimp",   "shrimp",   5),
    ("Turkey",   "turkey",   6),
    ("Kibble",   "kibble",   4),
)

_SNACK_ITEMS = (
    ("Nuggets",    "nugget",      2),
    ("Cream",      "cream",       4),
    ("Milk",       "milk",        3),
    ("Sticks",     "chew_stick",  3),
    ("Bytes",      "fish_bite",   2),
)

# (store_label, full_name, variant, cost)
_TOY_ITEMS = (
    ("Yarn",    "Yarn Ball",      "ball",   10),
    ("Laser",   "Laser Pointer",  "laser",  15),
    ("Feather", "Feather",        "toy",     8),
    ("String",  "String",         "string",  5),
)

# Width of the menu panel (left half of 128px screen)
_MENU_WIDTH = 60
# X position of the store art panel
_ART_X = 64


class StoreScene(Scene):
    MODULES_TO_KEEP = []

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.menu = Menu(
            renderer, input,
            content_width=_MENU_WIDTH,
            scrollbar_x=_MENU_WIDTH,
            show_submenu_arrow=False,
        )
        self._popup = Popup(renderer, x=14, y=20, width=100, height=24, padding=4)
        self._purchase_msg = None

    def enter(self):
        self.menu.open(self._build_menu())

    def exit(self):
        pass

    # ------------------------------------------------------------------
    # Menu construction
    # ------------------------------------------------------------------

    def _build_menu(self):
        snack_items = [self._food_item(name, key, cost)
                       for name, key, cost in _SNACK_ITEMS]
        food_items = [self._food_item(name, key, cost)
                      for name, key, cost in _MEAL_ITEMS]
        food_items.append(MenuItem("Snacks", submenu=snack_items))
        toy_items  = [self._toy_item(label, full_name, variant, cost)
                      for label, full_name, variant, cost in _TOY_ITEMS]
        return [
            MenuItem("Food",  submenu=food_items),
            MenuItem("Toys",  submenu=toy_items),
            MenuItem("Exit",  action=("leave",)),
        ]

    def _food_item(self, name, key, cost):
        label = f"{name}"
        if self.context.coins >= cost:
            return MenuItem(label, action=("buy_food", key, cost),
                            confirm=f"{name}({_FOOD_USES}): {cost}c")
        return MenuItem(label, action=("no_funds",), confirm="Can't afford!")

    def _toy_item(self, label, full_name, variant, cost):
        owned = any(t["name"] == full_name for t in self.context.inventory.get("toys", []))
        if owned:
            return MenuItem(label + "*", action=("already_owned",),
                            confirm="Already owned!")
        if self.context.coins >= cost:
            return MenuItem(label, action=("buy_toy", full_name, variant, cost),
                            confirm=f"{label}: {cost}c")
        return MenuItem(label, action=("no_funds",), confirm="Can't afford!")

    # ------------------------------------------------------------------
    # Update / draw / input
    # ------------------------------------------------------------------

    def update(self, dt):
        pass

    def draw(self):
        self.draw_store()

        # Coin display
        self.renderer.draw_text(f"{self.context.coins}c", 84, 29)

        # Menu on the left half
        self.menu.draw()

        if self._purchase_msg is not None:
            self._popup.draw()
    
    def draw_store(self):
        # Shop roof
        for i in range(7):
            self.renderer.draw_sprite(SHINGLE1, 10, 4, 128 - 62 + (i*9), 0)
        
        # Shop sign
        self.renderer.draw_line(128, 5, 66, 5)
        self.renderer.draw_line(128, 17, 66, 17)
        self.renderer.draw_line(128, 17, 66, 17)
        self.renderer.draw_pixel(66, 6)
        self.renderer.draw_pixel(66, 16)
        self.renderer.draw_line(65, 6, 65, 16)
        self.renderer.draw_rect(68, 7, 64, 9, filled=True)
        self.renderer.draw_line(67, 8, 67, 14)
        self.renderer.draw_sprite(PAW_LOGO, 7, 7, 73, 8, invert=True, transparent=True, transparent_color=1)
        self.renderer.draw_sprite(STORE_TEXT, 19, 5, 86, 9, invert=True, transparent=True, transparent_color=1)
        self.renderer.draw_sprite(PAW_LOGO, 7, 7, 113, 8, invert=True, transparent=True, transparent_color=1)

        for i in range(9):
            self.renderer.draw_sprite(SHINGLE2, 11, 8, 128 - 68 + (i*8), 19)

        # Pillar
        self.renderer.draw_rect(65, 29, 2, 15)
        self.renderer.draw_sprite(SHADOW1, 3, 4, 68, 31)
        self.renderer.draw_rect(68, 35, 3, 9, filled=True)
        
        # Counter
        self.renderer.draw_rect(59, 45, 69, 2)
        self.renderer.draw_pixel(58, 45)

        for i in range(4):
            self.renderer.draw_sprite(COUNTER_STRUT, 5, 4, 128 - 66 + (i*18), 48)

        # Items
        self.renderer.draw_sprite(ITEMS1, 13, 7, 79, 37)
        self.renderer.draw_sprite(ITEMS2, 14, 5, 110, 39)
        self.renderer.draw_sprite(ITEMS3, 9, 9, 73, 27)
        
        # Under counter
        self.renderer.draw_line(63, 53, 63, 63)
        self.renderer.draw_line(63, 63, 128, 63)

        for i in range(13):
            self.renderer.draw_rect(65 + (i * 5), 53, 4, 9, filled=True)

    def handle_input(self):
        if self._purchase_msg is not None:
            if self.input.was_just_pressed('a') or self.input.was_just_pressed('b'):
                self._purchase_msg = None
                self.menu.open(self._build_menu())
            return None
        result = self.menu.handle_input()
        if result == 'closed':
            return ('change_scene', self.context.last_main_scene)
        if result is not None:
            return self._handle_action(result)
        return None

    # ------------------------------------------------------------------
    # Purchase logic
    # ------------------------------------------------------------------

    def _handle_action(self, action):
        if not action:
            return None

        kind = action[0]

        if kind == "leave":
            return ('change_scene', self.context.last_main_scene)

        elif kind == "buy_food":
            _, key, cost = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.food_stock[key] = self.context.food_stock.get(key, 0) + _FOOD_USES
                print(f"[Store] Bought {key} for {cost}c (+{_FOOD_USES} uses)")
                self._purchase_msg = f"{key[0].upper() + key[1:]} purchased!"
                self._popup.set_text(self._purchase_msg, center=True)
            else:
                self.menu.open(self._build_menu())

        elif kind == "buy_toy":
            _, name, variant, cost = action
            if self.context.coins >= cost:
                owned = any(t["name"] == name for t in self.context.inventory.get("toys", []))
                if not owned:
                    self.context.coins -= cost
                    self.context.inventory.setdefault("toys", []).append(
                        {"name": name, "variant": variant}
                    )
                    print(f"[Store] Bought toy {name} for {cost}c")
                    self._purchase_msg = f"{name} purchased!"
                    self._popup.set_text(self._purchase_msg, center=True)
                    return None
            self.menu.open(self._build_menu())

        elif kind in ("no_funds", "already_owned"):
            # Confirmation was shown; just reopen the menu
            self.menu.open(self._build_menu())

        return None
