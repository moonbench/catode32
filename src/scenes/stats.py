from scene import Scene
from ui import Popup

# Grid layout
COLS = 6
ROWS = 3
CELL_W = 17           # box width  (border + 2px pad + 13px icon + 2px pad)
CELL_H = 20           # box height (border + 1px gap + 13px icon + 1px gap + separator + 2px bar + border)
H_GAP = 4             # horizontal gap between boxes
V_GAP = 2             # vertical gap between boxes
CELL_STEP_X = CELL_W + H_GAP   # 21
CELL_STEP_Y = CELL_H + V_GAP   # 22
# 6*17 + 5*4 = 122px wide  →  (128-122)//2 = 3px each side
# 3*20 + 2*2 = 64px tall   →  exactly fills the screen, flush top and bottom
GRID_X = (128 - (COLS * CELL_W + (COLS - 1) * H_GAP)) // 2   # 3
GRID_Y = 0

# Stat icons (13x13), in STATS_CONFIG order
ICON_HEALTH        = bytearray([0x07, 0x00, 0x07, 0x00, 0x07, 0x00, 0x07, 0x00, 0x07, 0x00, 0xff, 0xf8, 0xff, 0xf8, 0xff, 0xf8, 0x07, 0x00, 0x07, 0x00, 0x07, 0x00, 0x07, 0x00, 0x07, 0x00])
ICON_FULLNESS      = bytearray([0x30, 0x00, 0x38, 0x00, 0xf0, 0x00, 0xf8, 0x00, 0x5f, 0xc0, 0x0d, 0x60, 0x0a, 0xb0, 0x0f, 0x58, 0x0f, 0xe8, 0x0f, 0xf8, 0x07, 0xf8, 0x03, 0xf0, 0x00, 0xe0])
ICON_ENERGY        = bytearray([0x0f, 0xe0, 0x0f, 0xc0, 0x1f, 0x80, 0x1f, 0x80, 0x1f, 0x00, 0x3e, 0x00, 0x3f, 0xc0, 0x0f, 0x80, 0x0f, 0x00, 0x0e, 0x00, 0x1c, 0x00, 0x18, 0x00, 0x10, 0x00])
ICON_COMFORT       = bytearray([0x00, 0x00, 0x00, 0x08, 0x00, 0x08, 0x00, 0xe8, 0x00, 0x08, 0xff, 0xe8, 0xff, 0xe8, 0xff, 0xe8, 0x00, 0x08, 0xff, 0xf8, 0xff, 0xf8, 0x80, 0x08, 0x00, 0x00])
ICON_CLEANLINESS   = bytearray([0x10, 0x00, 0x10, 0x20, 0x28, 0x70, 0xc6, 0x20, 0x28, 0x00, 0x10, 0x40, 0x10, 0x40, 0x00, 0xa0, 0x0b, 0x18, 0x08, 0xa0, 0x36, 0x40, 0x08, 0x40, 0x08, 0x00])
ICON_FITNESS       = bytearray([0x00, 0x00, 0x00, 0x00, 0x20, 0x20, 0x60, 0x30, 0x60, 0x30, 0xff, 0xf8, 0xff, 0xf8, 0xff, 0xf8, 0x60, 0x30, 0x60, 0x30, 0x20, 0x20, 0x00, 0x00, 0x00, 0x00])
ICON_FOCUS         = bytearray([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x70, 0x70, 0xf8, 0xf8, 0x6d, 0xb0, 0x25, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
ICON_INTELLIGENCE  = bytearray([0x00, 0x00, 0x07, 0x00, 0x3f, 0xe0, 0xff, 0xf8, 0x3f, 0xe0, 0x07, 0x08, 0x30, 0x68, 0x3f, 0xe8, 0x3f, 0xe8, 0x1f, 0xc0, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00])
ICON_CURIOSITY     = bytearray([0x1f, 0xc0, 0x20, 0x20, 0x46, 0x10, 0x29, 0x10, 0x12, 0x20, 0x04, 0x40, 0x08, 0x80, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x08, 0x80, 0x08, 0x80, 0x07, 0x00])
ICON_PLAYFULNESS   = bytearray([0x00, 0x78, 0x00, 0xa8, 0x01, 0x48, 0x02, 0x90, 0x05, 0x20, 0x0a, 0x40, 0x14, 0x80, 0x29, 0x00, 0x32, 0x00, 0x6c, 0x00, 0x70, 0x00, 0xc0, 0x00, 0x80, 0x00])
ICON_AFFECTION     = bytearray([0x38, 0xe0, 0x6d, 0xb0, 0xd7, 0x78, 0xaf, 0xf8, 0xff, 0xf8, 0xff, 0xf8, 0xff, 0xf8, 0x7f, 0xf0, 0x3f, 0xe0, 0x1f, 0xc0, 0x0f, 0x80, 0x07, 0x00, 0x02, 0x00])
ICON_FULFILLMENT   = bytearray([0xff, 0xe8, 0x88, 0x88, 0xaa, 0xe8, 0xaa, 0x88, 0xaa, 0xb8, 0xa2, 0x08, 0xef, 0xe8, 0x81, 0x08, 0xfd, 0x78, 0x85, 0x08, 0xf5, 0xe8, 0x01, 0x08, 0xff, 0xf8])
ICON_SERENITY      = bytearray([0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x05, 0x00, 0x08, 0x80, 0x68, 0xb0, 0x98, 0xc8, 0x88, 0x88, 0x45, 0x10, 0xa5, 0x28, 0x95, 0x48, 0x6a, 0xb0, 0x00, 0x00])
ICON_SOCIABILITY   = bytearray([0x84, 0x00, 0xcc, 0x00, 0xfc, 0x00, 0xb4, 0x00, 0xb4, 0x00, 0xfd, 0x08, 0x79, 0x98, 0x01, 0xf8, 0x01, 0x68, 0x01, 0x68, 0x01, 0xf8, 0x00, 0xf0, 0x00, 0x00])
ICON_COURAGE       = bytearray([0x02, 0x00, 0x07, 0x00, 0x07, 0x00, 0x0f, 0x80, 0xff, 0xf8, 0x7f, 0xf0, 0x3f, 0xe0, 0x1f, 0xc0, 0x1f, 0xc0, 0x3d, 0xe0, 0x38, 0xe0, 0x60, 0x30, 0x00, 0x00])
ICON_LOYALTY       = bytearray([0x84, 0x50, 0xcc, 0xf8, 0xfc, 0xf8, 0xb4, 0x70, 0xb4, 0x20, 0xfc, 0x00, 0x78, 0x00, 0x7f, 0xe0, 0x7f, 0xd0, 0x7f, 0xc8, 0x51, 0x48, 0x51, 0x48, 0x51, 0x40])
ICON_MISCHIEVOUS   = bytearray([0x00, 0x00, 0x00, 0x00, 0x30, 0x60, 0x08, 0x80, 0xe0, 0x38, 0xf0, 0x78, 0xd8, 0xd8, 0x48, 0x90, 0x38, 0xe0, 0x00, 0x00, 0x0a, 0x80, 0x05, 0x00, 0x00, 0x00])
ICON_MATURITY      = bytearray([0x15, 0x40, 0x0f, 0x80, 0x0f, 0x80, 0x07, 0x00, 0x02, 0x00, 0x02, 0x00, 0x7a, 0x00, 0x46, 0x00, 0x26, 0xf0, 0x1b, 0x10, 0x03, 0x20, 0x02, 0xc0, 0x02, 0x00])

# Stats in grid order (row-major: left→right, top→bottom)
STATS_CONFIG = [
    # Row 0 — Vitals & Physical
    {"name": "Health",        "key": "health",        "icon": ICON_HEALTH,
     "desc": "Overall wellbeing, derived from all other stats. No single action raises it directly. Keep fitness, fullness, energy, comfort, and affection balanced for the best results."},
    {"name": "Fullness",      "key": "fullness",      "icon": ICON_FULLNESS,
     "desc": "How satisfied your pet's belly is. Feed treats and meals to fill it. Activity burns through it, especially sleep, play, and exercise."},
    {"name": "Energy",        "key": "energy",        "icon": ICON_ENERGY,
     "desc": "How rested and ready your pet is. Restored by sleeping and napping. Burns down during active behaviors like playing, zoomies, training, and hunting."},
    {"name": "Comfort",       "key": "comfort",       "icon": ICON_COMFORT,
     "desc": "Physical ease with surroundings. Restored by sleep, stretching, kneading, grooming, and affection. Worn down by startles, pacing, and prolonged inactivity."},
    {"name": "Cleanliness",   "key": "cleanliness",   "icon": ICON_CLEANLINESS,
     "desc": "How clean and fresh your pet is. Your pet self-grooms regularly, and grooming them gives a bigger boost. Activity (especially sleeping and hunting) gradually dirties them."},
    {"name": "Fitness",       "key": "fitness",       "icon": ICON_FITNESS,
     "desc": "Athletic conditioning and endurance. Built through training, hunting, zoomies, and movement. Fades slowly during rest and lounging."},

    # Row 1 — Mental & Emotional
    {"name": "Focus",         "key": "focus",         "icon": ICON_FOCUS,
     "desc": "Ability to concentrate. Restored by sleep and affection. Scattered by mischief, grooming sessions, and active exploration."},
    {"name": "Intelligence",  "key": "intelligence",  "icon": ICON_INTELLIGENCE,
     "desc": "Problem solving ability and aptitude for learning. Developed through training and hunting. Fades very slowly when left to idle and passive behaviors."},
    {"name": "Curiosity",     "key": "curiosity",     "icon": ICON_CURIOSITY,
     "desc": "Drive to explore and investigate. Sparked by rest, surprises, and your attention. Gradually satisfied by investigating and observing."},
    {"name": "Playfulness",   "key": "playfulness",   "icon": ICON_PLAYFULNESS,
     "desc": "Desire to play and have fun. Restored by sleep and affection. Spent naturally through play, training, and zoomies."},
    {"name": "Affection",     "key": "affection",     "icon": ICON_AFFECTION,
     "desc": "How loved your pet feels. Filled by kisses, petting, grooming, and treats. Quietly drains when your pet sulks, hides, or paces."},
    {"name": "Fulfillment",   "key": "fulfillment",   "icon": ICON_FULFILLMENT,
     "desc": "Sense of purpose and satisfaction. Grows through training, grooming, affection, hunting, and investigating. Fades when your pet is stuck idling or lounging too long."},

    # Row 2 — Emotional (cont.) & Character
    {"name": "Serenity",         "key": "serenity",         "icon": ICON_SERENITY,
     "desc": "Inner peace and calm. Restored gradually by rest, kneading, and grooming. Worn slightly by vocalizing, hunting, and active exploration."},
    {"name": "Sociability",      "key": "sociability",      "icon": ICON_SOCIABILITY,
     "desc": "Eagerness to interact and connect. Boosted by training, affection, grooming, and gift-bringing. Falls when your pet hides, sulks, or causes mischief."},
    {"name": "Courage",          "key": "courage",          "icon": ICON_COURAGE,
     "desc": "Boldness in unfamiliar or scary situations. Strengthened through training, affection, and positive interactions. Chipped away by startles, hiding, and sulking."},
    {"name": "Loyalty",          "key": "loyalty",          "icon": ICON_LOYALTY,
     "desc": "Strength of bond and devotion. Grows through training and frequent affection. Eroded by mischief and sulking."},
    {"name": "Mischievousness",  "key": "mischievousness",  "icon": ICON_MISCHIEVOUS,
     "desc": "Tendency toward playful trouble. Rises naturally during mischief and hunting. Channeled and reduced by affection, grooming, and training."},
    {"name": "Maturity",         "key": "maturity",         "icon": ICON_MATURITY,
     "desc": "Behavioral sophistication and self-control. Develops through training, grooming, investigating, and observing. Set back by mischief and zoomies."},
]


class StatsScene(Scene):
    """Grid-based stats scene: 6x3 icon boxes, D-pad navigation, A for details."""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.sel_col = 0
        self.sel_row = 0
        self.showing_detail = False
        self.popup = Popup(renderer, x=0, y=6, width=128, height=48)

    @property
    def selected_index(self):
        return self.sel_row * COLS + self.sel_col

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.sel_col = 0
        self.sel_row = 0
        self.showing_detail = False
        self.context.debug_print_stats()

    def exit(self):
        pass

    def update(self, dt):
        pass

    # ------------------------------------------------------------------
    # Drawing
    # ------------------------------------------------------------------

    def draw(self):
        for row in range(ROWS):
            for col in range(COLS):
                idx = row * COLS + col
                stat = STATS_CONFIG[idx]
                bx = GRID_X + col * CELL_STEP_X
                by = GRID_Y + row * CELL_STEP_Y
                is_selected = (col == self.sel_col and row == self.sel_row)
                value = getattr(self.context, stat["key"], 0)
                self._draw_cell(bx, by, stat["icon"], value, is_selected)

        if self.showing_detail:
            self.popup.draw()

    def _draw_cell(self, bx, by, icon, value, is_selected):
        """Draw a single stat cell (CELL_W x CELL_H).

        Layout inside the cell (y-offsets from by):
          +0      : top border
          +1      : 1px gap above icon
          +2..+14 : 13x13 icon  (x offset +2 for horizontal centering)
          +15     : 1px gap below icon
          +16     : separator line (full cell width)
          +17..+18: 2px progress bar
          +19     : bottom border
        """
        # Border (same for selected and unselected)
        self.renderer.draw_rect(bx, by, CELL_W, CELL_H, filled=False, color=1)
        # Separator line
        self.renderer.draw_line(bx, by + 16, bx + CELL_W - 1, by + 16, color=1)

        if is_selected:
            # White fill behind icon only, then draw icon inverted (black on white)
            self.renderer.draw_rect(bx + 2, by + 2, 13, 13, filled=True, color=1)
            self.renderer.draw_sprite(
                icon, 13, 13, bx + 2, by + 2,
                transparent=True, invert=True, transparent_color=1
            )
        else:
            self.renderer.draw_sprite(
                icon, 13, 13, bx + 2, by + 2,
                transparent=True, invert=False, transparent_color=0
            )

        # Progress bar (same for selected and unselected)
        bar_w = int(value / 100 * (CELL_W - 2))
        if bar_w > 0:
            self.renderer.draw_rect(bx + 1, by + 17, bar_w, 2, filled=True, color=1)

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self.showing_detail:
            if self.input.was_just_pressed('up'):
                self.popup.scroll_up()
            if self.input.was_just_pressed('down'):
                self.popup.scroll_down()
            if self.input.was_just_pressed('b') or self.input.was_just_pressed('a'):
                self.showing_detail = False
            return None

        # Grid navigation
        if self.input.was_just_pressed('left'):
            if self.sel_col > 0:
                self.sel_col -= 1
            elif self.sel_row > 0:
                # Wrap to end of previous row
                self.sel_row -= 1
                self.sel_col = COLS - 1

        if self.input.was_just_pressed('right'):
            if self.sel_col < COLS - 1:
                self.sel_col += 1
            elif self.sel_row < ROWS - 1:
                # Wrap to start of next row
                self.sel_row += 1
                self.sel_col = 0

        if self.input.was_just_pressed('up'):
            if self.sel_row > 0:
                self.sel_row -= 1

        if self.input.was_just_pressed('down'):
            if self.sel_row < ROWS - 1:
                self.sel_row += 1

        # Show detail popup
        if self.input.was_just_pressed('a'):
            stat = STATS_CONFIG[self.selected_index]
            value = int(getattr(self.context, stat["key"], 0))
            self.popup.set_text(f"{stat["name"]}\n---- {value}% ----\n{stat["desc"]}")
            self.showing_detail = True
            return None

        # Exit scene
        if self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')

        return None
