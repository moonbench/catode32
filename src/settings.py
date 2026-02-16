"""
settings.py - Composable settings component for option selection
"""


class SettingItem:
    """Represents a single setting with selectable options or numeric range"""

    def __init__(self, name, key, options=None, min_val=None, max_val=None, step=1, value=None):
        """
        Args:
            name: Display label (left-aligned)
            key: Key used in returned dict
            options: For bool/enum - list of possible values
            min_val: For numeric - minimum value
            max_val: For numeric - maximum value
            step: For numeric - increment amount
            value: Current value
        """
        self.name = name
        self.key = key
        self.options = options
        self.min_val = min_val
        self.max_val = max_val
        self.step = step
        self.value = value

        # Set default value if not provided
        if self.value is None:
            if self.options:
                self.value = self.options[0]
            elif self.min_val is not None:
                self.value = self.min_val

    def is_numeric(self):
        """Check if this is a numeric range setting"""
        return self.min_val is not None and self.max_val is not None

    def cycle_next(self):
        """Move to next option or increment numeric value"""
        if self.is_numeric():
            self.value = min(self.max_val, self.value + self.step)
        elif self.options:
            idx = self.options.index(self.value)
            self.value = self.options[(idx + 1) % len(self.options)]

    def cycle_prev(self):
        """Move to previous option or decrement numeric value"""
        if self.is_numeric():
            self.value = max(self.min_val, self.value - self.step)
        elif self.options:
            idx = self.options.index(self.value)
            self.value = self.options[(idx - 1) % len(self.options)]

    def get_display_value(self):
        """Get the string representation of the current value"""
        if isinstance(self.value, bool):
            return "On" if self.value else "Off"
        return str(self.value)


class Settings:
    """Composable settings component with option cycling"""

    VISIBLE_ITEMS = 4
    ROW_HEIGHT = 16
    SCROLLBAR_WIDTH = 4
    CONTENT_WIDTH = 124  # 128 - scrollbar width

    def __init__(self, renderer, input_handler):
        self.renderer = renderer
        self.input = input_handler

        self.active = False
        self.items = []
        self.selected_index = 0
        self.scroll_offset = 0

    def open(self, items, transition=True):
        """Open the settings with the given items

        Args:
            items: List of SettingItem objects
            transition: Whether to run a transition animation (unused for now)
        """
        self.items = items
        self.selected_index = 0
        self.scroll_offset = 0
        self.active = True

    def close(self):
        """Close settings and return current values as dict"""
        self.active = False
        result = self.get_values()
        self.items = []
        return result

    def get_values(self):
        """Get current values as a dictionary"""
        return {item.key: item.value for item in self.items}

    def handle_input(self):
        """Process input and return result

        Returns:
            None: Still navigating
            dict: Settings values when closed (via B or menu button)
        """
        # Menu button closes
        if self.input.was_just_pressed('menu2') or self.input.was_just_pressed('menu1'):
            return self.close()

        # B button closes
        if self.input.was_just_pressed('b'):
            return self.close()

        # Navigation
        if self.input.was_just_pressed('up'):
            self.selected_index = max(0, self.selected_index - 1)
            self._adjust_scroll()

        if self.input.was_just_pressed('down'):
            self.selected_index = min(len(self.items) - 1, self.selected_index + 1)
            self._adjust_scroll()

        # Cycle options
        if self.input.was_just_pressed('right'):
            if self.items:
                self.items[self.selected_index].cycle_next()

        if self.input.was_just_pressed('left'):
            if self.items:
                self.items[self.selected_index].cycle_prev()

        return None

    def _adjust_scroll(self):
        """Keep selected item visible in the scroll window"""
        if self.selected_index < self.scroll_offset:
            self.scroll_offset = self.selected_index
        elif self.selected_index >= self.scroll_offset + self.VISIBLE_ITEMS:
            self.scroll_offset = self.selected_index - self.VISIBLE_ITEMS + 1

    def draw(self):
        """Render the settings to screen"""
        self.renderer.clear()
        self._draw_settings_list()

    def _draw_settings_list(self):
        """Draw the setting items with scrollbar"""
        visible_start = self.scroll_offset
        visible_end = min(visible_start + self.VISIBLE_ITEMS, len(self.items))

        for i, item in enumerate(self.items[visible_start:visible_end]):
            y = i * self.ROW_HEIGHT
            actual_index = visible_start + i
            is_selected = actual_index == self.selected_index

            self._draw_setting_item(item, y, is_selected)

        self._draw_scrollbar()

    def _draw_setting_item(self, item, y, is_selected):
        """Draw a single setting row"""
        # Draw selection background (inverted)
        if is_selected:
            self.renderer.draw_rect(0, y, self.CONTENT_WIDTH, self.ROW_HEIGHT,
                                    filled=True, color=1)

        text_color = 0 if is_selected else 1

        # Draw name (left-aligned) - center text vertically (8px font height)
        text_y = y + (self.ROW_HEIGHT - 8) // 2
        self.renderer.draw_text(item.name, 2, text_y, text_color)

        # Draw value (right-aligned with padding for selection highlight)
        value_str = item.get_display_value()
        # Approximate character width of 6px for right alignment
        padding = 16 if is_selected else 12
        value_x = self.CONTENT_WIDTH - padding - (len(value_str) * 6)
        self.renderer.draw_text(value_str, value_x, text_y, text_color)

    def _draw_scrollbar(self):
        """Draw scrollbar if items exceed visible area"""
        if len(self.items) <= self.VISIBLE_ITEMS:
            return

        scrollbar_x = 124
        track_height = 64

        # Calculate thumb size proportional to visible/total ratio
        thumb_height = max(8, int(track_height * self.VISIBLE_ITEMS / len(self.items)))

        # Calculate thumb position
        scroll_range = len(self.items) - self.VISIBLE_ITEMS
        if scroll_range > 0:
            thumb_y = int(self.scroll_offset / scroll_range * (track_height - thumb_height))
        else:
            thumb_y = 0

        # Draw track (thin line)
        self.renderer.draw_line(scrollbar_x + 2, 0, scrollbar_x + 2, 63)

        # Draw thumb (filled rectangle)
        self.renderer.draw_rect(scrollbar_x, thumb_y, self.SCROLLBAR_WIDTH,
                                thumb_height, filled=True)
