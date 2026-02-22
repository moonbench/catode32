"""
menu.py - Composable menu component for UI navigation
"""

from ui import Scrollbar, adjust_scroll_offset


class MenuItem:
    """Represents a single menu item"""

    def __init__(self, label, icon=None, submenu=None, action=None, confirm=None):
        """
        Args:
            label: Display text for the menu item
            icon: Optional 13x13 sprite bytearray
            submenu: Optional list of MenuItem for nested menus
            action: Value returned when this item is selected (ignored if submenu)
            confirm: Optional confirmation message to show before executing action
        """
        self.label = label
        self.icon = icon
        self.submenu = submenu
        self.action = action
        self.confirm = confirm


class Menu:
    """Composable menu component with navigation, submenus, and confirmation dialogs"""

    VISIBLE_ITEMS = 4
    ROW_HEIGHT = 16
    ICON_SIZE = 13
    CONTENT_WIDTH = 120  # 128 - scrollbar area

    def __init__(self, renderer, input_handler):
        self.renderer = renderer
        self.input = input_handler
        self.scrollbar = Scrollbar(renderer)

        self.active = False
        self.items = []
        self.selected_index = 0
        self.scroll_offset = 0
        self.menu_stack = []  # Stack of {items, index, scroll} for submenu navigation
        self.pending_confirmation = None  # MenuItem awaiting confirmation

    def open(self, items):
        """Open the menu with the given items"""
        self.items = items
        self.selected_index = 0
        self.scroll_offset = 0
        self.menu_stack = []
        self.pending_confirmation = None
        self.active = True

    def close(self):
        """Close the menu and reset state"""
        self.active = False
        self.items = []
        self.menu_stack = []
        self.pending_confirmation = None

    def handle_input(self):
        """Process input and return result

        Returns:
            None: Still navigating, no action taken
            'closed': Menu was closed without selection
            any other value: The action from the selected MenuItem
        """
        # Handle confirmation dialog if active
        if self.pending_confirmation:
            return self._handle_confirmation_input()

        # Navigation
        if self.input.was_just_pressed('up'):
            self.selected_index = max(0, self.selected_index - 1)
            self._adjust_scroll()

        if self.input.was_just_pressed('down'):
            self.selected_index = min(len(self.items) - 1, self.selected_index + 1)
            self._adjust_scroll()

        # Back buttons
        if self.input.was_just_pressed('b'):
            if self.menu_stack:
                return self._exit_submenu()
            else:
                # At top level, close menu
                self.close()
                return 'closed'

        if self.input.was_just_pressed('left'):
            if self.menu_stack:
                return self._exit_submenu()

        # "Go into sub menu" buttons
        if self.input.was_just_pressed('right'):
            selected = self.items[self.selected_index]
            if selected.submenu:
                return self._enter_submenu(selected)

        # Select button
        if self.input.was_just_pressed('a'):
            if not self.items:
                return None

            selected = self.items[self.selected_index]

            if selected.submenu:
                return self._enter_submenu(selected)
            elif selected.confirm:
                # Show confirmation dialog
                self.pending_confirmation = selected
            else:
                # Execute action
                self.close()
                return selected.action

        return None

    def _exit_submenu(self):
        # Return to parent menu
        parent = self.menu_stack.pop()
        self.items = parent['items']
        self.selected_index = parent['index']
        self.scroll_offset = parent['scroll']

    def _enter_submenu(self, selected):
        self.menu_stack.append({
            'items': self.items,
            'index': self.selected_index,
            'scroll': self.scroll_offset
        })
        self.items = selected.submenu
        self.selected_index = 0
        self.scroll_offset = 0
        return None

    def _handle_confirmation_input(self):
        """Handle input during confirmation dialog"""
        if self.input.was_just_pressed('a'):
            # Confirmed - execute action
            action = self.pending_confirmation.action
            self.close()
            return action

        if self.input.was_just_pressed('b'):
            # Cancelled - return to menu
            self.pending_confirmation = None

        return None

    def _adjust_scroll(self):
        """Keep selected item visible in the scroll window"""
        self.scroll_offset = adjust_scroll_offset(
            self.selected_index, self.scroll_offset, self.VISIBLE_ITEMS
        )

    def draw(self):
        """Render the menu to screen"""
        self.renderer.clear()

        if self.pending_confirmation:
            self._draw_confirmation()
        else:
            self._draw_menu_list()

    def _draw_menu_list(self):
        """Draw the menu items with scrollbar"""
        visible_start = self.scroll_offset
        visible_end = min(visible_start + self.VISIBLE_ITEMS, len(self.items))

        for i, item in enumerate(self.items[visible_start:visible_end]):
            y = i * self.ROW_HEIGHT
            actual_index = visible_start + i
            is_selected = actual_index == self.selected_index

            self._draw_menu_item(item, y, is_selected)

        self._draw_scrollbar()

    def _draw_menu_item(self, item, y, is_selected):
        """Draw a single menu item row"""
        # Draw selection background (inverted)
        if is_selected:
            self.renderer.draw_rect(0, y, self.CONTENT_WIDTH, self.ROW_HEIGHT,
                                    filled=True, color=1)

        text_color = 0 if is_selected else 1
        x_offset = 2

        # Draw icon if present
        if item.icon:
            # Center icon vertically in row
            icon_y = y + (self.ROW_HEIGHT - self.ICON_SIZE) // 2
            # Invert icon colors when selected to match inverted text
            # Disable transparency when inverted (black pixels would be transparent)
            self.renderer.draw_sprite(item.icon, self.ICON_SIZE, self.ICON_SIZE,
                                      x_offset, icon_y,
                                      transparent=not is_selected, invert=is_selected)
            x_offset = 2 + self.ICON_SIZE + 3  # icon + gap

        # Draw label - center text vertically (8px font height)
        text_y = y + (self.ROW_HEIGHT - 8) // 2
        self.renderer.draw_text(item.label, x_offset, text_y, text_color)

        # Draw submenu indicator if has submenu
        if item.submenu:
            arrow_x = self.CONTENT_WIDTH - 10
            self.renderer.draw_text(">", arrow_x, text_y, text_color)

    def _draw_scrollbar(self):
        """Draw scrollbar if items exceed visible area"""
        self.scrollbar.draw(len(self.items), self.VISIBLE_ITEMS, self.scroll_offset)

    def _draw_confirmation(self):
        """Draw confirmation dialog"""
        # Draw border
        self.renderer.draw_rect(4, 12, 120, 40, filled=False)

        # Clear inside
        self.renderer.draw_rect(5, 13, 118, 38, filled=True, color=0)

        # Draw confirmation message (centered-ish)
        message = self.pending_confirmation.confirm
        # Simple text wrapping - just truncate for now
        if len(message) > 18:
            line1 = message[:18]
            line2 = message[18:36]
            self.renderer.draw_text(line1, 8, 18)
            self.renderer.draw_text(line2, 8, 28)
        else:
            self.renderer.draw_text(message, 8, 22)

        # Draw button hints
        self.renderer.draw_text("[A]Yes [B]No", 20, 42)
