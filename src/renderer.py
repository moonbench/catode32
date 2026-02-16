"""
renderer.py - Display rendering logic
"""

from machine import Pin, I2C
import ssd1306
import config
import framebuf
import math
from sprite_transform import mirror_sprite_h, mirror_sprite_v, rotate_sprite, skew_sprite

class Renderer:
    """Handles all display rendering operations"""
    
    def __init__(self):
        """Initialize display and rendering system"""
        # Initialize I2C
        self.i2c = I2C(0, scl=Pin(config.I2C_SCL), sda=Pin(config.I2C_SDA), 
                      freq=config.I2C_FREQ)
        
        # Initialize OLED display
        self.display = ssd1306.SSD1306_I2C(config.DISPLAY_WIDTH, 
                                           config.DISPLAY_HEIGHT, 
                                           self.i2c)
        
        # Clear display
        self.clear()
        self.show()
    
    def clear(self):
        """Clear the display buffer"""
        self.display.fill(0)
    
    def show(self):
        """Update the physical display with buffer contents"""
        self.display.show()
    
    def draw_character(self, character):
        """
        Draw a character on screen
        For now, draws as a simple filled rectangle
        """
        x, y = character.get_position()
        size = character.size
        
        # Draw filled rectangle for character
        self.display.fill_rect(x, y, size, size, 1)
        
        # Optional: Draw a border to make it look more distinct
        self.display.rect(x, y, size, size, 1)
    
    def draw_text(self, text, x, y, color=1):
        """Draw text at given position

        Args:
            color: 1 for white (default), 0 for black
        """
        self.display.text(text, x, y, color)
    
    def draw_rect(self, x, y, width, height, filled=False, color=1):
        """Draw a rectangle

        Args:
            color: 1 for white (default), 0 for black
        """
        if filled:
            self.display.fill_rect(x, y, width, height, color)
        else:
            self.display.rect(x, y, width, height, color)
    
    def draw_line(self, x1, y1, x2, y2, color=1):
        """Draw a line between two points

        Args:
            color: 1 for white (default), 0 for black
        """
        self.display.line(x1, y1, x2, y2, color)
    
    def draw_pixel(self, x, y, color=1):
        """Draw a single pixel

        Args:
            color: 1 for white (default), 0 for black
        """
        self.display.pixel(x, y, color)

    def draw_polygon(self, points, color=1):
        """Draw a polygon outline

        Args:
            points: list of (x, y) tuples defining vertices
            color: 1 for white (default), 0 for black
        """
        if len(points) < 2:
            return
        for i in range(len(points)):
            x1, y1 = points[i]
            x2, y2 = points[(i + 1) % len(points)]
            self.display.line(int(x1), int(y1), int(x2), int(y2), color)

    def fill_polygon(self, points, color=1, pattern=None):
        """Fill a polygon using scanline algorithm

        Args:
            points: list of (x, y) tuples defining vertices
            color: 1 for white (default), 0 for black
            pattern: optional pattern name or function
                Built-in patterns: 'solid', 'checkerboard', 'horizontal',
                    'vertical', 'diagonal', 'dots'
                Or pass a function: pattern(x, y) -> bool
        """
        if len(points) < 3:
            return

        # Built-in patterns
        patterns = {
            'solid': lambda x, y: True,
            'checkerboard': lambda x, y: (x + y) % 2 == 0,
            'horizontal': lambda x, y: y % 2 == 0,
            'vertical': lambda x, y: x % 2 == 0,
            'diagonal': lambda x, y: (x + y) % 3 == 0,
            'dots': lambda x, y: x % 2 == 0 and y % 2 == 0,
        }

        # Resolve pattern
        if pattern is None or pattern == 'solid':
            pattern_fn = None  # Solid fill, skip pattern check for speed
        elif callable(pattern):
            pattern_fn = pattern
        elif pattern in patterns:
            pattern_fn = patterns[pattern]
        else:
            pattern_fn = None

        # Find bounding box
        min_y = int(min(p[1] for p in points))
        max_y = int(max(p[1] for p in points))

        # Build edge list: each edge is (x1, y1, x2, y2) with y1 <= y2
        edges = []
        n = len(points)
        for i in range(n):
            x1, y1 = points[i]
            x2, y2 = points[(i + 1) % n]
            if y1 != y2:  # Skip horizontal edges
                if y1 > y2:
                    x1, y1, x2, y2 = x2, y2, x1, y1
                edges.append((x1, y1, x2, y2))

        # Scanline fill
        for y in range(min_y, max_y + 1):
            # Find intersections with all edges
            intersections = []
            for x1, y1, x2, y2 in edges:
                if y1 <= y < y2:  # Edge crosses this scanline
                    # Calculate x intersection using linear interpolation
                    t = (y - y1) / (y2 - y1)
                    x = x1 + t * (x2 - x1)
                    intersections.append(x)

            # Sort intersections
            intersections.sort()

            # Fill between pairs of intersections (even-odd rule)
            for i in range(0, len(intersections) - 1, 2):
                x_start = int(intersections[i] + 0.5)
                x_end = int(intersections[i + 1] + 0.5)
                for x in range(x_start, x_end + 1):
                    if pattern_fn is None or pattern_fn(x, y):
                        self.display.pixel(x, y, color)

    def draw_ui_frame(self):
        """Draw a UI frame around the screen (optional border)"""
        self.display.rect(0, 0, config.DISPLAY_WIDTH, config.DISPLAY_HEIGHT, 1)
    
    def draw_fps(self, fps):
        """Draw FPS counter in top-right corner"""
        fps_text = f"{fps:.1f}"
        # Clear small area for FPS
        self.display.fill_rect(config.DISPLAY_WIDTH - 25, 0, 25, 8, 0)
        self.display.text(fps_text, config.DISPLAY_WIDTH - 24, 0)

    def draw_debug_info(self, info_dict, start_y=0):
        """
        Draw debug information on screen
        info_dict: dictionary of label->value pairs
        """
        y = start_y
        for label, value in info_dict.items():
            text = f"{label}:{value}"
            self.display.text(text, 0, y)
            y += 8
            if y >= config.DISPLAY_HEIGHT:
                break

    def draw_sprite(self, byte_array, width, height, x, y, transparent=True, invert=False, transparent_color=0, mirror_h=False, mirror_v=False, rotate=0, skew_x=0, skew_y=0):
        """Draw a sprite at the given position

        Args:
            byte_array: bytearray containing the sprite bitmap
            width: sprite width in pixels
            height: sprite height in pixels
            x: x position on display
            y: y position on display
            transparent: if True, pixels matching transparent_color are transparent
            invert: if True, flip all pixel colors (white becomes black, etc.)
            transparent_color: which color to treat as transparent (0=black, 1=white)
            mirror_h: if True, flip the sprite horizontally
            mirror_v: if True, flip the sprite vertically
            rotate: rotation angle in degrees (clockwise)
            skew_x: horizontal skew factor (pixels shifted per row)
            skew_y: vertical skew factor (pixels shifted per column)
        """

        # Mirror horizontally if requested
        if mirror_h:
            byte_array = mirror_sprite_h(byte_array, width, height)

        # Mirror vertically if requested
        if mirror_v:
            byte_array = mirror_sprite_v(byte_array, width, height)

        # Rotate if requested
        if rotate != 0:
            # Adjust position so sprite rotates around its center
            old_cx = x + width // 2
            old_cy = y + height // 2
            byte_array, width, height = rotate_sprite(byte_array, width, height, rotate)
            x = old_cx - width // 2
            y = old_cy - height // 2

        # Skew if requested
        if skew_x != 0 or skew_y != 0:
            # Adjust position so sprite skews around its center
            old_cx = x + width // 2
            old_cy = y + height // 2
            byte_array, width, height = skew_sprite(byte_array, width, height, skew_x, skew_y)
            x = old_cx - width // 2
            y = old_cy - height // 2

        # Invert colors if requested
        if invert:
            byte_array = bytearray(b ^ 0xFF for b in byte_array)

        # Create a framebuffer from the sprite data
        sprite_fb = framebuf.FrameBuffer(
            byte_array,
            width,
            height,
            framebuf.MONO_HLSB  # or MONO_VLSB
        )

        if transparent:
            # Draw with transparency - pixels matching transparent_color are not drawn
            self.display.blit(sprite_fb, x, y, transparent_color)
        else:
            # Draw without transparency (overwrites everything)
            self.display.blit(sprite_fb, x, y)

    def draw_sprite_obj(self, sprite, x, y, frame=0, transparent=True, invert=False, mirror_h=False, mirror_v=False, rotate=0, skew_x=0, skew_y=0):
        """Draw a sprite object at the given position

        Args:
            sprite: dict with 'width', 'height', and 'frames' keys
                    optionally includes 'fill_frames' for solid fill behind outline
            x: x position on display
            y: y position on display
            frame: which frame to draw (default 0)
            transparent: if True, black pixels (0) are transparent
            invert: if True, flip all pixel colors
            mirror_h: if True, flip the sprite horizontally
            mirror_v: if True, flip the sprite vertically
            rotate: rotation angle in degrees (clockwise)
            skew_x: horizontal skew factor (pixels shifted per row)
            skew_y: vertical skew factor (pixels shifted per column)
        """
        # If sprite has fill_frames, draw the fill first (in black)
        # Invert so white fill becomes black, use white as transparent color
        if "fill_frames" in sprite:
            self.draw_sprite(
                sprite["fill_frames"][frame],
                sprite["width"],
                sprite["height"],
                x, y,
                transparent=True,
                invert=True,
                transparent_color=1,
                mirror_h=mirror_h,
                mirror_v=mirror_v,
                rotate=rotate,
                skew_x=skew_x,
                skew_y=skew_y
            )

        self.draw_sprite(
            sprite["frames"][frame],
            sprite["width"],
            sprite["height"],
            x, y,
            transparent,
            invert,
            mirror_h=mirror_h,
            mirror_v=mirror_v,
            rotate=rotate,
            skew_x=skew_x,
            skew_y=skew_y
        )
