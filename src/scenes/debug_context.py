from scene import Scene
from ui import Scrollbar


class DebugContextScene(Scene):
    """Debug scene that displays all context values"""

    LINES_VISIBLE = 8  # 64px / 8px per line
    LINE_HEIGHT = 8

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.scrollbar = Scrollbar(renderer)
        self.scroll_offset = 0
        self.lines = []

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.scroll_offset = 0
        self._build_lines()

    def exit(self):
        pass

    def _build_lines(self):
        """Build display lines from context"""
        self.lines = []

        # Pet stats
        self.lines.append("Pet stats:")
        stats = [
            ('fullness', self.context.fullness),
            ('stimulation', self.context.stimulation),
            ('energy', self.context.energy),
            ('vigor', self.context.vigor),
            ('comfort', self.context.comfort),
            ('playfulness', self.context.playfulness),
            ('focus', self.context.focus),
            ('affection', self.context.affection),
            ('health', self.context.health),
            ('fulfillment', self.context.fulfillment),
            ('cleanliness', self.context.cleanliness),
            ('curiosity', self.context.curiosity),
            ('confidence', self.context.confidence),
        ]
        for name, value in stats:
            self.lines.append(f"{name}: {value}")

        self.lines.append("")

        # Inventory
        self.lines.append("Inventory:")
        for category, items in self.context.inventory.items():
            self.lines.append(f"{category}:")
            for item in items:
                self.lines.append(f"  {item}")

    def update(self, dt):
        # Refresh values periodically
        self._build_lines()

    def draw(self):
        """Draw the debug info"""
        self.renderer.clear()

        visible_end = min(self.scroll_offset + self.LINES_VISIBLE, len(self.lines))

        for i, line in enumerate(self.lines[self.scroll_offset:visible_end]):
            y = i * self.LINE_HEIGHT
            self.renderer.draw_text(line[:21], 0, y)  # Truncate to fit screen

        # Draw scroll indicator if needed
        if len(self.lines) > self.LINES_VISIBLE:
            self._draw_scroll_indicator()

    def _draw_scroll_indicator(self):
        """Draw a simple scroll indicator on the right"""
        self.scrollbar.draw(len(self.lines), self.LINES_VISIBLE, self.scroll_offset)

    def handle_input(self):
        """Handle scrolling input"""
        max_scroll = max(0, len(self.lines) - self.LINES_VISIBLE)

        if self.input.was_just_pressed('up'):
            self.scroll_offset = max(0, self.scroll_offset - 1)

        if self.input.was_just_pressed('down'):
            self.scroll_offset = min(max_scroll, self.scroll_offset + 1)

        # B button or menu1 to go back (will be caught by SceneManager for menu1)
        if self.input.was_just_pressed('b'):
            return ('change_scene', 'normal')

        return None
