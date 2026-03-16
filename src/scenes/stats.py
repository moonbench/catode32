from scene import Scene
from ui import Popup, Scrollbar

# Stats configuration with display info and descriptions
STATS_CONFIG = [
    # Vitals
    {"type": "header", "text": "Vitals"},
    {"type": "stat", "name": "Health", "key": "health",
     "desc": "Overall wellbeing, derived from all other stats. No single action raises it directly. Keep fitness, fullness, energy, comfort, and affection balanced for the best results."},
    {"type": "stat", "name": "Fullness", "key": "fullness",
     "desc": "How satisfied your pet's belly is. Feed treats and meals to fill it. Activity burns through it, especially sleep, play, and exercise."},
    {"type": "stat", "name": "Energy", "key": "energy",
     "desc": "How rested and ready your pet is. Restored by sleeping and napping. Burns down during active behaviors like playing, zoomies, training, and hunting."},

    # Physical
    {"type": "header", "text": ""},
    {"type": "header", "text": "Physical"},
    {"type": "stat", "name": "Comfort", "key": "comfort",
     "desc": "Physical ease with surroundings. Restored by sleep, stretching, kneading, grooming, and affection. Worn down by startles, pacing, and prolonged inactivity."},
    {"type": "stat", "name": "Cleanliness", "key": "cleanliness",
     "desc": "How clean and fresh your pet is. Your pet self-grooms regularly, and grooming them gives a bigger boost. Activity (especially sleeping and hunting) gradually dirties them."},
    {"type": "stat", "name": "Fitness", "key": "fitness",
     "desc": "Athletic conditioning and endurance. Built through training, hunting, zoomies, and movement. Fades slowly during rest and lounging."},

    # Mental
    {"type": "header", "text": ""},
    {"type": "header", "text": "Mental"},
    {"type": "stat", "name": "Focus", "key": "focus",
     "desc": "Ability to concentrate. Restored by sleep and affection. Scattered by mischief, grooming sessions, and active exploration."},
    {"type": "stat", "name": "Intelligence", "key": "intelligence",
     "desc": "Problem-solving ability and aptitude for learning. Developed through training and hunting. Fades very slowly when left to idle and passive behaviors."},
    {"type": "stat", "name": "Curiosity", "key": "curiosity",
     "desc": "Drive to explore and investigate. Sparked by rest, surprises, and your attention. Gradually satisfied by investigating and observing."},

    # Emotional
    {"type": "header", "text": ""},
    {"type": "header", "text": "Emotional"},
    {"type": "stat", "name": "Playfulness", "key": "playfulness",
     "desc": "Desire to play and have fun. Restored by sleep and affection. Spent naturally through play, training, and zoomies."},
    {"type": "stat", "name": "Affection", "key": "affection",
     "desc": "How loved your pet feels. Filled by kisses, petting, grooming, and treats. Quietly drains when your pet sulks, hides, or paces."},
    {"type": "stat", "name": "Fulfillment", "key": "fulfillment",
     "desc": "Sense of purpose and satisfaction. Grows through training, grooming, affection, hunting, and investigating. Fades when your pet is stuck idling or lounging too long."},
    {"type": "stat", "name": "Serenity", "key": "serenity",
     "desc": "Inner peace and calm. Restored gradually by rest, kneading, and grooming. Worn slightly by vocalizing, hunting, and active exploration."},

    # Social
    {"type": "header", "text": ""},
    {"type": "header", "text": "Social"},
    {"type": "stat", "name": "Sociability", "key": "sociability",
     "desc": "Eagerness to interact and connect. Boosted by training, affection, grooming, and gift-bringing. Falls when your pet hides, sulks, or causes mischief."},

    # Character
    {"type": "header", "text": ""},
    {"type": "header", "text": "Character"},
    {"type": "stat", "name": "Courage", "key": "courage",
     "desc": "Boldness in unfamiliar or scary situations. Strengthened through training, affection, and positive interactions. Chipped away by startles, hiding, and sulking."},
    {"type": "stat", "name": "Loyalty", "key": "loyalty",
     "desc": "Strength of bond and devotion. Grows through training and frequent affection. Eroded by mischief and sulking."},
    {"type": "stat", "name": "Mischievousness", "key": "mischievousness",
     "desc": "Tendency toward playful trouble. Rises naturally during mischief and hunting. Channeled and reduced by affection, grooming, and training."},
    {"type": "stat", "name": "Maturity", "key": "maturity",
     "desc": "Behavioral sophistication and self-control. Develops through training, grooming, investigating, and observing. Set back by mischief and zoomies."},
]


class StatsScene(Scene):
    """Scene displaying pet stats with bar graphs"""

    HEADER_HEIGHT = 10
    STAT_HEIGHT = 16  # 8px name + 8px bar
    BAR_WIDTH = 100
    BAR_HEIGHT = 6
    VISIBLE_HEIGHT = 64
    CONTENT_WIDTH = 124  # Leave room for scrollbar

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.selected_index = 0
        self.scroll_offset = 0
        self.showing_detail = False

        # Build list of just the stats (for selection indexing)
        self.stats_list = [item for item in STATS_CONFIG if item["type"] == "stat"]

        # UI components
        self.scrollbar = Scrollbar(renderer)
        self.popup = Popup(renderer, x=4, y=8, width=120, height=48)


    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.selected_index = 0
        self.scroll_offset = 0
        self.showing_detail = False
        self.context.debug_print_stats()

    def exit(self):
        pass

    def update(self, dt):
        pass

    def _get_item_y(self, item_index):
        """Calculate the y position for an item in STATS_CONFIG"""
        y = 0
        for i, item in enumerate(STATS_CONFIG):
            if i == item_index:
                return y
            if item["type"] == "header":
                y += self.HEADER_HEIGHT
            else:
                y += self.STAT_HEIGHT
        return y

    def _get_total_height(self):
        """Calculate total content height"""
        height = 0
        for item in STATS_CONFIG:
            if item["type"] == "header":
                height += self.HEADER_HEIGHT
            else:
                height += self.STAT_HEIGHT
        return height

    def _get_selected_config_index(self):
        """Get the index in STATS_CONFIG for the currently selected stat"""
        stat_count = 0
        for i, item in enumerate(STATS_CONFIG):
            if item["type"] == "stat":
                if stat_count == self.selected_index:
                    return i
                stat_count += 1
        return 0

    def _ensure_selection_visible(self):
        """Adjust scroll to keep selected item visible"""
        config_index = self._get_selected_config_index()
        item_y = self._get_item_y(config_index)
        item_height = self.STAT_HEIGHT

        # If item is above visible area, scroll up
        if item_y < self.scroll_offset:
            self.scroll_offset = item_y

        # If item is below visible area, scroll down
        if item_y + item_height > self.scroll_offset + self.VISIBLE_HEIGHT:
            self.scroll_offset = item_y + item_height - self.VISIBLE_HEIGHT

        # Clamp scroll offset
        max_scroll = max(0, self._get_total_height() - self.VISIBLE_HEIGHT)
        self.scroll_offset = max(0, min(max_scroll, self.scroll_offset))

    def draw(self):
        """Draw the stats list"""

        # Draw all items with scroll offset
        current_stat_index = 0
        y = -self.scroll_offset

        for item in STATS_CONFIG:
            if item["type"] == "header":
                item_height = self.HEADER_HEIGHT
                # Only draw if visible
                if y + item_height > 0 and y < self.VISIBLE_HEIGHT:
                    self._draw_header(item["text"], y)
                y += item_height
            else:
                item_height = self.STAT_HEIGHT
                # Only draw if visible
                if y + item_height > 0 and y < self.VISIBLE_HEIGHT:
                    is_selected = (current_stat_index == self.selected_index)
                    value = getattr(self.context, item["key"], 0)
                    self._draw_stat(item["name"], value, y, is_selected)
                current_stat_index += 1
                y += item_height

        # Draw scrollbar
        self._draw_scrollbar()

        # Draw detail popup if showing
        if self.showing_detail:
            self._draw_detail_popup()

    def _draw_header(self, text, y):
        """Draw a section header"""
        # Skip if completely off screen
        if y < -self.HEADER_HEIGHT or y >= self.VISIBLE_HEIGHT:
            return

        if text != "":
            # Center the header text with decoration
            header_text = f"== {text} =="
            text_width = len(header_text) * 8
            x = (self.CONTENT_WIDTH - text_width) // 2
            self.renderer.draw_text(header_text, max(0, x), int(y + 1))

    def _draw_stat(self, name, value, y, is_selected):
        """Draw a stat with name and bar"""
        # Skip if completely off screen
        if y < -self.STAT_HEIGHT or y >= self.VISIBLE_HEIGHT:
            return

        name_y = int(y)
        bar_y = int(y + 8)

        # Draw name (highlighted if selected)
        if is_selected:
            # Draw inverted background for selection
            self.renderer.draw_rect(0, name_y, self.CONTENT_WIDTH, 8, filled=True)
            self.renderer.draw_text(name, 2, name_y, color=0)
        else:
            self.renderer.draw_text(name, 2, name_y)

        # Draw bar (only if bar_y is on screen)
        if bar_y >= 0 and bar_y < self.VISIBLE_HEIGHT:
            self._draw_bar(2, bar_y, self.BAR_WIDTH, value)

    def _draw_bar(self, x, y, width, value, max_value=100):
        """Draw a horizontal bar graph"""
        # Draw outline
        self.renderer.draw_rect(x, y, width, self.BAR_HEIGHT, filled=False)

        # Draw fill
        fill_width = int((value / max_value) * (width - 2))
        if fill_width > 0:
            self.renderer.draw_rect(x + 1, y + 1, fill_width, self.BAR_HEIGHT - 2, filled=True)

    def _draw_scrollbar(self):
        """Draw scrollbar on right side"""
        self.scrollbar.draw(
            self._get_total_height(), self.VISIBLE_HEIGHT, self.scroll_offset
        )

    def _draw_detail_popup(self):
        """Draw the detail popup for selected stat"""
        self.popup.draw()

    def handle_input(self):
        """Handle navigation and selection"""
        # If showing detail popup
        if self.showing_detail:
            # Scroll up/down in popup
            if self.input.was_just_pressed('up'):
                self.popup.scroll_up()

            if self.input.was_just_pressed('down'):
                self.popup.scroll_down()

            # Close popup
            if self.input.was_just_pressed('b') or self.input.was_just_pressed('a'):
                self.showing_detail = False
            return None

        # Navigation in stats list
        if self.input.was_just_pressed('up'):
            if self.selected_index > 0:
                self.selected_index -= 1
                self._ensure_selection_visible()
            elif self.scroll_offset > 0:
                # Already at first stat, but can scroll up to show header
                self.scroll_offset = 0

        if self.input.was_just_pressed('down'):
            if self.selected_index < len(self.stats_list) - 1:
                self.selected_index += 1
                self._ensure_selection_visible()

        # Show detail popup
        if self.input.was_just_pressed('a'):
            stat = self.stats_list[self.selected_index]
            self.popup.set_text(stat["desc"])
            self.showing_detail = True
            return None

        # Exit scene
        if self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')

        return None
