import gc
import sys
from scene import Scene


class DebugMemoryScene(Scene):
    """Debug scene that displays memory usage"""

    LINES_VISIBLE = 8  # 64px / 8px per line
    LINE_HEIGHT = 8

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
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
        """Build display lines from memory info"""
        gc.collect()
        self.lines = []

        free = gc.mem_free()
        alloc = gc.mem_alloc()
        total = free + alloc

        self.lines.append("Memory:")
        self.lines.append(f" Free: {free}")
        self.lines.append(f" Used: {alloc}")
        self.lines.append(f" Total: {total}")
        self.lines.append("")

        # List loaded modules
        self.lines.append("Modules loaded:")
        modules = sorted(sys.modules.keys())
        for mod in modules:
            self.lines.append(f" {mod}")

    def update(self, dt):
        self._build_lines()

    def draw(self):
        """Draw the memory info"""
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
        indicator_x = 126
        track_height = 64

        thumb_height = max(4, int(track_height * self.LINES_VISIBLE / len(self.lines)))
        scroll_range = len(self.lines) - self.LINES_VISIBLE

        if scroll_range > 0:
            thumb_y = int(self.scroll_offset / scroll_range * (track_height - thumb_height))
        else:
            thumb_y = 0

        self.renderer.draw_rect(indicator_x, thumb_y, 2, thumb_height, filled=True)

    def handle_input(self):
        """Handle scrolling input"""
        max_scroll = max(0, len(self.lines) - self.LINES_VISIBLE)

        if self.input.was_just_pressed('up'):
            self.scroll_offset = max(0, self.scroll_offset - 1)

        if self.input.was_just_pressed('down'):
            self.scroll_offset = min(max_scroll, self.scroll_offset + 1)

        if self.input.was_just_pressed('b'):
            from scenes.normal import NormalScene
            return ('change_scene', NormalScene)

        return None
