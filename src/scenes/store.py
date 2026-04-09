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
    ("Kibble",   "kibble",    5),
    ("Cod",      "cod",       6),
    ("Haddock",  "haddock",   7),
    ("Trout",    "trout",     8),
    ("Shrimp",   "shrimp",    9),
    ("Herring",  "herring",  10),
    ("Turkey",   "turkey",   10),
    ("Tuna",     "tuna",     12),
    ("Salmon",   "salmon",   12),
    ("Chicken",  "chicken",  13),
    ("Liver",    "liver",    14),
    ("Beef",     "beef",     14),
    ("Lamb",     "lamb",     15),
)

_SNACK_ITEMS = (
    ("Carrots",    "carrots",     2),
    ("Pumpkin",    "pumpkin",     2),
    ("Treats",     "treats",      3),
    ("Bytes",      "fish_bite",   4),
    ("Eggs",       "eggs",        5),
    ("Nuggets",    "nugget",      5),
    ("Milk",       "milk",        6),
    ("Sticks",     "chew_stick",  6),
    ("Puree",      "puree",       8),
)

# (store_label, full_name, variant, cost)
_TOY_ITEMS = (
    ("String",  "String",         "string",  20),
    ("Feather", "Feather",        "toy",     35),
    ("Yarn",    "Yarn Ball",      "ball",    50),
    ("Laser",   "Laser Pointer",  "laser",   75),
)

# (display_name, full_name, inventory_key, cost)
_POT_ITEMS = (
    ("Small",   "Small pot",   "small",    15),
    ("Medium",  "Medium pot",  "medium",   25),
    ("Large",   "Large pot",   "large",    40),
    ("Planter", "Planter box", "planter",  55),
)

_SEEDS_PER_PACK = 3

# (display_name, full_name, inventory_key, cost per pack)
_SEED_ITEMS = (
    ("Grass",   "Cat Grass",  "cat_grass",   4),
    ("Freesia", "Freesia",    "freesia",    10),
    ("Sun",     "Sunflower",  "sunflower",  12),
    ("Rose",    "Rose",       "rose",       15),
)

_FERTILIZER_COST = 25

# (display_name, full_name, inventory_key, cost)
_TOOL_ITEMS = (
    ("Spade",  "Spade",        "spade",        40),
    ("W. Can", "Watering Can", "watering_can", 50),
)

# (display_name, stat_changes, cost, completion_msg)
_SERVICE_ITEMS = (
    ("Groom", {"cleanliness": 40, "sociability": 8, "courage": 6},   50,  "A luxurious spa day!"),
    ("Train", {"maturity": 5, "sociability": 5, "intelligence": 8, "fitness": 6, "mischievousness": -8}, 100, "Professional training done!"),
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
        food_items  = [self._food_item(name, key, cost)
                       for name, key, cost in _MEAL_ITEMS]
        snack_items = [self._food_item(name, key, cost)
                       for name, key, cost in _SNACK_ITEMS]
        toy_items   = [self._toy_item(label, full_name, variant, cost)
                       for label, full_name, variant, cost in _TOY_ITEMS]
        pot_items   = [self._pot_item(label, full, key, cost)
                       for label, full, key, cost in _POT_ITEMS]
        seed_items  = [self._seed_item(label, full, key, cost)
                       for label, full, key, cost in _SEED_ITEMS]
        tool_items  = [self._tool_item(label, full, key, cost)
                       for label, full, key, cost in _TOOL_ITEMS]
        gardening_items = [
            MenuItem("Pots",       submenu=pot_items),
            MenuItem("Seeds",      submenu=seed_items),
            MenuItem("Tools",      submenu=tool_items),
            self._fertilizer_item(),
        ]
        service_items = [self._service_item(name, stats, cost, msg)
                         for name, stats, cost, msg in _SERVICE_ITEMS]
        return [
            MenuItem("Food",    submenu=food_items),
            MenuItem("Snacks",  submenu=snack_items),
            MenuItem("Toys",    submenu=toy_items),
            MenuItem("Garden",  submenu=gardening_items),
            MenuItem("Service", submenu=service_items),
            MenuItem("Exit",    action=("leave",)),
        ]

    def _food_item(self, name, key, cost):
        label = f"{name}"
        if self.context.coins >= cost:
            return MenuItem(label, action=("buy_food", name, key, cost),
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

    def _pot_item(self, label, full, key, cost):
        if self.context.coins >= cost:
            return MenuItem(label, action=("buy_pot", full, key, cost),
                            confirm=f"{full}: {cost}c")
        return MenuItem(label, action=("no_funds",), confirm="Can't afford!")

    def _seed_item(self, label, full, key, cost):
        if self.context.coins >= cost:
            return MenuItem(label, action=("buy_seeds", full, key, cost),
                            confirm=f"{full}x{_SEEDS_PER_PACK}: {cost}c")
        return MenuItem(label, action=("no_funds",), confirm="Can't afford!")

    def _fertilizer_item(self):
        cost = _FERTILIZER_COST
        if self.context.coins >= cost:
            return MenuItem("Fertilizer", action=("buy_fertilizer", cost),
                            confirm=f"Fertilizer: {cost}c")
        return MenuItem("Fertilizer", action=("no_funds",), confirm="Can't afford!")

    def _service_item(self, name, stats, cost, msg):
        if self.context.coins >= cost:
            return MenuItem(name, action=("buy_service", name, stats, cost, msg),
                            confirm=f"{name}: {cost}c")
        return MenuItem(name, action=("no_funds",), confirm="Can't afford!")

    def _tool_item(self, label, full, key, cost):
        owned = self.context.inventory['tools'].get(key, False)
        if owned:
            return MenuItem(label + "*", action=("already_owned",),
                            confirm="Already owned!")
        if self.context.coins >= cost:
            return MenuItem(label, action=("buy_tool", full, key, cost),
                            confirm=f"{full}: {cost}c")
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
            _, name, key, cost = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.food_stock[key] = self.context.food_stock.get(key, 0) + _FOOD_USES
                print(f"[Store] Bought {key} for {cost}c (+{_FOOD_USES} uses)")
                self._purchase_msg = f"{name} purchased!"
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

        elif kind == "buy_pot":
            _, full, key, cost = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.inventory['pots'][key] = self.context.inventory['pots'].get(key, 0) + 1
                print(f"[Store] Bought pot {key} for {cost}c")
                self._purchase_msg = f"{full} bought!"
                self._popup.set_text(self._purchase_msg, center=True)
            else:
                self.menu.open(self._build_menu())

        elif kind == "buy_seeds":
            _, full, key, cost = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.inventory['seeds'][key] = self.context.inventory['seeds'].get(key, 0) + _SEEDS_PER_PACK
                print(f"[Store] Bought {key} seeds x{_SEEDS_PER_PACK} for {cost}c")
                self._purchase_msg = f"{full} x{_SEEDS_PER_PACK}!"
                self._popup.set_text(self._purchase_msg, center=True)
            else:
                self.menu.open(self._build_menu())

        elif kind == "buy_tool":
            _, full, key, cost = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.inventory['tools'][key] = True
                print(f"[Store] Bought tool {key} for {cost}c")
                self._purchase_msg = f"{full} bought!"
                self._popup.set_text(self._purchase_msg, center=True)
            else:
                self.menu.open(self._build_menu())

        elif kind == "buy_fertilizer":
            _, cost = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.inventory['fertilizer'] = self.context.inventory.get('fertilizer', 0) + 1
                print(f"[Store] Bought fertilizer for {cost}c")
                self._purchase_msg = "Fertilizer bought!"
                self._popup.set_text(self._purchase_msg, center=True)
            else:
                self.menu.open(self._build_menu())

        elif kind == "buy_service":
            _, name, stats, cost, msg = action
            if self.context.coins >= cost:
                self.context.coins -= cost
                self.context.apply_stat_changes(stats)
                print(f"[Store] Used service {name} for {cost}c")
                self._purchase_msg = msg
                self._popup.set_text(self._purchase_msg, center=True)
            else:
                self.menu.open(self._build_menu())

        elif kind in ("no_funds", "already_owned"):
            # Confirmation was shown; just reopen the menu
            self.menu.open(self._build_menu())

        return None
