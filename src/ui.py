"""ui.py - Reusable UI components"""

from assets.icons import UP_ICON, DOWN_ICON


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
