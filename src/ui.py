"""ui.py - Reusable UI components"""

from assets.icons import UP_ICON, DOWN_ICON


class OverlayManager:
    """Manages a stack of UI overlays (menus, settings, dialogs).

    Overlays are drawn on top of the scene and intercept input.
    The topmost overlay receives input and is drawn.
    """

    def __init__(self):
        """Initialize the overlay manager."""
        self._stack = []  # Stack of (overlay, metadata) tuples
        self._result_handlers = {}  # overlay -> callback mapping

    @property
    def active(self):
        """Whether any overlay is currently active."""
        return len(self._stack) > 0

    @property
    def current(self):
        """The topmost overlay, or None if no overlays."""
        return self._stack[-1][0] if self._stack else None

    def push(self, overlay, on_result=None, metadata=None):
        """Push an overlay onto the stack.

        Args:
            overlay: The overlay object (must have handle_input and draw methods)
            on_result: Optional callback when overlay returns a result
            metadata: Optional metadata dict associated with this overlay
        """
        self._stack.append((overlay, metadata or {}))
        if on_result:
            self._result_handlers[id(overlay)] = on_result

    def pop(self):
        """Pop the topmost overlay from the stack.

        Returns:
            Tuple of (overlay, metadata) that was removed, or (None, None)
        """
        if not self._stack:
            return None, None
        overlay, metadata = self._stack.pop()
        self._result_handlers.pop(id(overlay), None)
        return overlay, metadata

    def handle_input(self):
        """Route input to the topmost overlay.

        Returns:
            True if an overlay consumed the input, False otherwise
        """
        if not self._stack:
            return False

        overlay, metadata = self._stack[-1]
        result = overlay.handle_input()

        if result is not None:
            # Overlay returned a result
            handler = self._result_handlers.get(id(overlay))
            self.pop()

            if handler:
                handler(result, metadata)

        return True

    def draw(self):
        """Draw the topmost overlay.

        Returns:
            True if an overlay was drawn, False otherwise
        """
        if not self._stack:
            return False

        overlay, _ = self._stack[-1]
        overlay.draw()
        return True

    def clear(self):
        """Clear all overlays from the stack."""
        self._stack.clear()
        self._result_handlers.clear()


def adjust_scroll_offset(selected_index, scroll_offset, visible_items):
    """Calculate new scroll offset to keep selected item visible.

    Args:
        selected_index: The currently selected item index
        scroll_offset: The current scroll offset
        visible_items: Number of items visible at once

    Returns:
        The new scroll offset
    """
    if selected_index < scroll_offset:
        return selected_index
    elif selected_index >= scroll_offset + visible_items:
        return selected_index - visible_items + 1
    return scroll_offset


class Scrollbar:
    """A reusable scrollbar component with optional state management."""

    def __init__(self, renderer, x=126, y=0, track_height=64, min_thumb_height=4, visible_items=None):
        """Initialize the scrollbar.

        Args:
            renderer: The renderer instance for drawing
            x: X position of the scrollbar
            y: Y position (top of track)
            track_height: Height of the scrollbar track
            min_thumb_height: Minimum height of the thumb
            visible_items: If provided, enables stateful mode with this many visible items
        """
        self.renderer = renderer
        self.x = x
        self.y = y
        self.track_height = track_height
        self.min_thumb_height = min_thumb_height

        # Optional state management
        self._visible_items = visible_items
        self._scroll_offset = 0

    @property
    def scroll_offset(self):
        """Current scroll offset (stateful mode only)."""
        return self._scroll_offset

    @scroll_offset.setter
    def scroll_offset(self, value):
        self._scroll_offset = max(0, value)

    def reset(self):
        """Reset scroll offset to 0."""
        self._scroll_offset = 0

    def adjust_for_selection(self, selected_index, visible_items=None):
        """Adjust scroll offset to keep selected item visible.

        Args:
            selected_index: The currently selected item index
            visible_items: Override visible_items if not using stateful mode

        Returns:
            The new scroll offset
        """
        vis = visible_items if visible_items is not None else self._visible_items
        if vis is None:
            return self._scroll_offset

        self._scroll_offset = adjust_scroll_offset(selected_index, self._scroll_offset, vis)
        return self._scroll_offset

    def draw(self, total_items, visible_items=None, scroll_offset=None):
        """Draw the scrollbar if content exceeds visible area.

        Args:
            total_items: Total number of items (or total pixel height)
            visible_items: Number of visible items (uses internal state if not provided)
            scroll_offset: Current scroll position (uses internal state if not provided)
        """
        # Use internal state if not provided
        if visible_items is None:
            visible_items = self._visible_items
        if scroll_offset is None:
            scroll_offset = self._scroll_offset

        if visible_items is None or total_items <= visible_items:
            return

        # Calculate thumb size proportional to visible/total ratio
        thumb_height = max(
            self.min_thumb_height,
            int(self.track_height * visible_items / total_items)
        )

        # Calculate thumb position
        scroll_range = total_items - visible_items
        if scroll_range > 0:
            thumb_y = self.y + int(
                scroll_offset / scroll_range * (self.track_height - thumb_height)
            )
        else:
            thumb_y = self.y

        # Draw thumb (2px wide filled rectangle)
        self.renderer.draw_rect(self.x, thumb_y, 2, thumb_height, filled=True)


class Popup:
    """A reusable popup window component.

    Displays text in a bordered window with optional scrolling.
    """

    def __init__(self, renderer, x=4, y=8, width=120, height=48, padding=4):
        """Initialize the popup.

        Args:
            renderer: The renderer instance for drawing
            x: X position of the popup
            y: Y position of the popup
            width: Width of the popup
            height: Height of the popup
            padding: Internal padding for text
        """
        self.renderer = renderer
        self.x = x
        self.y = y
        self.width = width
        self.height = height
        self.padding = padding
        self.lines = []
        self.scroll_offset = 0
        self.center = False

    @property
    def visible_lines(self):
        """Number of text lines that fit in the popup."""
        content_height = self.height - self.padding * 2
        return content_height // 8

    @property
    def can_scroll(self):
        """Whether the content exceeds visible area."""
        return len(self.lines) > self.visible_lines

    @property
    def max_scroll(self):
        """Maximum scroll offset."""
        return max(0, len(self.lines) - self.visible_lines)

    def set_text(self, text, wrap=True, center=False):
        """Set the popup text content.

        Args:
            text: The text to display
            wrap: If True, word-wrap the text. If False, split on newlines.
            center: If True, center each line horizontally.
        """
        self.center = center
        self.scroll_offset = 0

        if wrap:
            self.lines = self._wrap_text(text)
        else:
            self.lines = text.split('\n')

    def scroll_up(self):
        """Scroll up one line if possible."""
        if self.scroll_offset > 0:
            self.scroll_offset -= 1

    def scroll_down(self):
        """Scroll down one line if possible."""
        if self.scroll_offset < self.max_scroll:
            self.scroll_offset += 1

    def draw(self, show_scroll_indicators=True):
        """Draw the popup.

        Args:
            show_scroll_indicators: Whether to show scroll arrows when scrollable.
        """
        # Draw background (black)
        self.renderer.draw_rect(
            self.x, self.y, self.width, self.height, filled=True, color=0
        )
        # Draw border (white)
        self.renderer.draw_rect(
            self.x, self.y, self.width, self.height, filled=False, color=1
        )

        # Draw visible lines
        visible = self.lines[self.scroll_offset:self.scroll_offset + self.visible_lines]

        for i, line in enumerate(visible):
            if self.center:
                text_width = len(line) * 8
                line_x = self.x + (self.width - text_width) // 2
            else:
                line_x = self.x + self.padding
            line_y = self.y + self.padding + i * 8
            self.renderer.draw_text(line, line_x, line_y)

        # Draw scroll indicators if needed
        if show_scroll_indicators and self.can_scroll:
            icon_x = self.x + self.width - 12
            # Up arrow if can scroll up
            if self.scroll_offset > 0:
                self.renderer.draw_sprite(UP_ICON, 8, 8, icon_x, self.y + 2)
            # Down arrow if can scroll down
            if self.scroll_offset < self.max_scroll:
                self.renderer.draw_sprite(
                    DOWN_ICON, 8, 8, icon_x, self.y + self.height - 10
                )

    def _wrap_text(self, text):
        """Word-wrap text to fit within popup width.

        Returns:
            List of wrapped lines.
        """
        chars_per_line = (self.width - self.padding * 2) // 8
        words = text.split(' ')
        lines = []
        current_line = ""

        for word in words:
            test_line = current_line + (" " if current_line else "") + word
            if len(test_line) <= chars_per_line:
                current_line = test_line
            else:
                if current_line:
                    lines.append(current_line)
                current_line = word

        if current_line:
            lines.append(current_line)

        return lines
