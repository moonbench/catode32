"""
renderer_desktop.py - pygame-based renderer for desktop/PC
Replaces renderer.py when running catode32 on a PC.

The API is identical to the original Renderer class so all scenes,
sprites and UI code work without any changes.
"""

import pygame
import config
import framebuf
import math
from sprite_transform import mirror_sprite_h, mirror_sprite_v, rotate_sprite, skew_sprite

_FILL_PATTERNS = {
    'solid':        lambda x, y: True,
    'checkerboard': lambda x, y: (x + y) % 2 == 0,
    'horizontal':   lambda x, y: y % 2 == 0,
    'vertical':     lambda x, y: x % 2 == 0,
    'diagonal':     lambda x, y: (x + y) % 3 == 0,
    'dots':         lambda x, y: x % 2 == 0 and y % 2 == 0,
}

# ---------------------------------------------------------------------------
# Tiny SSD1306-compatible framebuffer backed by a bytearray
# This lets us keep the same blit / pixel / text / rect calls as MicroPython.
# ---------------------------------------------------------------------------

class _FakeDisplay:
    """
    Minimal emulation of the ssd1306.SSD1306_I2C interface used by Renderer.
    All drawing goes into a bytearray framebuffer; show() copies it to the
    pygame window.
    """

    def __init__(self, width, height, scale, surface):
        self.width  = width
        self.height = height
        self.scale  = scale
        self._surface = surface          # pygame window surface
        self._buf = bytearray(width * height // 8)
        self._fb  = framebuf.FrameBuffer(self._buf, width, height, framebuf.MONO_HLSB)

    # --- low-level drawing (delegate to framebuf) ---

    def fill(self, c):        self._fb.fill(c)
    def pixel(self, x, y, c=1): self._fb.pixel(x, y, c)
    def text(self, s, x, y, c=1): self._fb.text(s, x, y, c)
    def rect(self, x, y, w, h, c): self._fb.rect(x, y, w, h, c)
    def fill_rect(self, x, y, w, h, c): self._fb.fill_rect(x, y, w, h, c)
    def line(self, x1, y1, x2, y2, c): self._fb.line(x1, y1, x2, y2, c)
    def hline(self, x, y, w, c): self._fb.hline(x, y, w, c)
    def vline(self, x, y, h, c): self._fb.vline(x, y, h, c)

    def blit(self, src_fb, x, y, key=-1):
        self._fb.blit(src_fb, x, y, key)

    def invert(self, state):
        if state:
            for i in range(len(self._buf)):
                self._buf[i] ^= 0xFF
        # non-trivial to undo; keep a flag if needed
        self._inverted = bool(state)

    def poweroff(self): pass   # no-op on desktop
    def poweron(self):  pass

    # --- flush to pygame window ---

    def show(self):
        s = self.scale
        on_color  = config.DISPLAY_COLOR
        off_color = config.DISPLAY_BG
        surf = self._surface
        surf.fill(off_color)
        # The real SSD1306 hardware uses SEG remap (0xA1) which mirrors
        # the display horizontally at the hardware level. The game content
        # was designed with this mirror active, so we compensate here.
        # Set DISPLAY_MIRROR_H = False in config_desktop.py to disable.
        mirror = getattr(config, 'DISPLAY_MIRROR_H', True)
        w = self.width
        for y in range(self.height):
            for x in range(w):
                if self._fb.pixel(x, y):
                    draw_x = (w - 1 - x) if mirror else x
                    pygame.draw.rect(surf, on_color, (draw_x * s, y * s, s, s))
        pygame.display.flip()


# ---------------------------------------------------------------------------
# Renderer — identical public API to the ESP32 version
# ---------------------------------------------------------------------------

class Renderer:
    """Handles all display rendering operations (desktop/pygame version)"""

    def __init__(self):
        pygame.display.set_caption("Catode32 — Desktop")
        w = config.DISPLAY_WIDTH  * config.DISPLAY_SCALE
        h = config.DISPLAY_HEIGHT * config.DISPLAY_SCALE
        self._surface = pygame.display.set_mode((w, h))
        self.display  = _FakeDisplay(
            config.DISPLAY_WIDTH,
            config.DISPLAY_HEIGHT,
            config.DISPLAY_SCALE,
            self._surface,
        )
        self.clear()
        self.show()

    def reinit(self):
        pass  # nothing to reinitialise on desktop

    def power_off(self):
        pass

    def power_on(self):
        pass

    def clear(self):
        self.display.fill(0)

    def show(self):
        self.display.show()

    def invert(self, state):
        self.display.invert(state)

    def draw_character(self, character):
        x, y = character.get_position()
        size = character.size
        self.display.fill_rect(x, y, size, size, 1)
        self.display.rect(x, y, size, size, 1)

    def draw_text(self, text, x, y, color=1):
        self.display.text(text, x, y, color)

    def draw_rect(self, x, y, width, height, filled=False, color=1):
        if filled:
            self.display.fill_rect(x, y, width, height, color)
        else:
            self.display.rect(x, y, width, height, color)

    def draw_line(self, x1, y1, x2, y2, color=1):
        self.display.line(x1, y1, x2, y2, color)

    def draw_pixel(self, x, y, color=1):
        self.display.pixel(x, y, color)

    def draw_circle(self, cx, cy, radius, filled=False, color=1):
        x, y, err = radius, 0, 1 - radius
        while x >= y:
            if filled:
                self.display.hline(cx - x, cy + y, 2 * x + 1, color)
                self.display.hline(cx - x, cy - y, 2 * x + 1, color)
                self.display.hline(cx - y, cy + x, 2 * y + 1, color)
                self.display.hline(cx - y, cy - x, 2 * y + 1, color)
            else:
                for px, py in [
                    (cx+x, cy+y), (cx-x, cy+y), (cx+x, cy-y), (cx-x, cy-y),
                    (cx+y, cy+x), (cx-y, cy+x), (cx+y, cy-x), (cx-y, cy-x),
                ]:
                    self.display.pixel(px, py, color)
            y += 1
            if err < 0:
                err += 2 * y + 1
            else:
                x -= 1
                err += 2 * (y - x) + 1

    def draw_polygon(self, points, color=1):
        if len(points) < 2:
            return
        for i in range(len(points)):
            x1, y1 = points[i]
            x2, y2 = points[(i + 1) % len(points)]
            self.display.line(int(x1), int(y1), int(x2), int(y2), color)

    def fill_polygon(self, points, color=1, pattern=None):
        if len(points) < 3:
            return
        if pattern is None or pattern == 'solid':
            pattern_fn = None
        elif callable(pattern):
            pattern_fn = pattern
        elif pattern in _FILL_PATTERNS:
            pattern_fn = _FILL_PATTERNS[pattern]
        else:
            pattern_fn = None

        min_y = int(min(p[1] for p in points))
        max_y = int(max(p[1] for p in points))
        edges = []
        n = len(points)
        for i in range(n):
            x1, y1 = points[i]
            x2, y2 = points[(i + 1) % n]
            if y1 != y2:
                if y1 > y2:
                    x1, y1, x2, y2 = x2, y2, x1, y1
                edges.append((x1, y1, x2, y2))

        for y in range(min_y, max_y + 1):
            intersections = []
            for x1, y1, x2, y2 in edges:
                if y1 <= y < y2:
                    t = (y - y1) / (y2 - y1)
                    intersections.append(x1 + t * (x2 - x1))
            intersections.sort()
            for i in range(0, len(intersections) - 1, 2):
                x_start = int(intersections[i] + 0.5)
                x_end   = int(intersections[i + 1] + 0.5)
                for x in range(x_start, x_end + 1):
                    if pattern_fn is None or pattern_fn(x, y):
                        self.display.pixel(x, y, color)

    def draw_ui_frame(self):
        self.display.rect(0, 0, config.DISPLAY_WIDTH, config.DISPLAY_HEIGHT, 1)

    def draw_fps(self, fps):
        fps_text = f"{fps:.1f}"
        self.display.fill_rect(config.DISPLAY_WIDTH - 25, 0, 25, 8, 0)
        self.display.text(fps_text, config.DISPLAY_WIDTH - 24, 0)

    def draw_debug_info(self, info_dict, start_y=0):
        y = start_y
        for label, value in info_dict.items():
            self.display.text(f"{label}:{value}", 0, y)
            y += 8
            if y >= config.DISPLAY_HEIGHT:
                break

    def draw_sprite(self, byte_array, width, height, x, y,
                    transparent=True, invert=False, transparent_color=0,
                    mirror_h=False, mirror_v=False, rotate=0, skew_x=0, skew_y=0):
        if mirror_h:
            byte_array = mirror_sprite_h(byte_array, width, height)
        if mirror_v:
            byte_array = mirror_sprite_v(byte_array, width, height)
        if rotate != 0:
            old_cx = x + width  // 2
            old_cy = y + height // 2
            byte_array, width, height = rotate_sprite(byte_array, width, height, rotate)
            x = old_cx - width  // 2
            y = old_cy - height // 2
        if skew_x != 0 or skew_y != 0:
            old_cx = x + width  // 2
            old_cy = y + height // 2
            byte_array, width, height = skew_sprite(byte_array, width, height, skew_x, skew_y)
            x = old_cx - width  // 2
            y = old_cy - height // 2
        if invert:
            byte_array = bytearray(b ^ 0xFF for b in byte_array)
        if not isinstance(byte_array, bytearray):
            byte_array = bytearray(byte_array)
        sprite_fb = framebuf.FrameBuffer(byte_array, width, height, framebuf.MONO_HLSB)
        if transparent:
            self.display.blit(sprite_fb, x, y, transparent_color)
        else:
            self.display.blit(sprite_fb, x, y)

    def draw_sprite_obj(self, sprite, x, y, frame=0, transparent=True, invert=False,
                        mirror_h=False, mirror_v=False, rotate=0, skew_x=0, skew_y=0,
                        transparent_color=0):
        if "fill_frames" in sprite:
            fill_frames = sprite["fill_frames"]
            fill_frame  = fill_frames[frame if frame < len(fill_frames) else 0]
            self.draw_sprite(fill_frame, sprite["width"], sprite["height"], x, y,
                             transparent=True, invert=True, transparent_color=1,
                             mirror_h=mirror_h, mirror_v=mirror_v,
                             rotate=rotate, skew_x=skew_x, skew_y=skew_y)
        self.draw_sprite(sprite["frames"][frame], sprite["width"], sprite["height"], x, y,
                         transparent, invert,
                         mirror_h=mirror_h, mirror_v=mirror_v,
                         rotate=rotate, skew_x=skew_x, skew_y=skew_y,
                         transparent_color=transparent_color)
