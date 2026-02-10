"""
renderer.py - Display rendering logic
"""

from machine import Pin, I2C
import ssd1306
import config
import framebuf
import math

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

    def mirror_byte(self, b):
        """Reverse bits in a byte using parallel bit swapping"""
        b = (b & 0xF0) >> 4 | (b & 0x0F) << 4
        b = (b & 0xCC) >> 2 | (b & 0x33) << 2
        b = (b & 0xAA) >> 1 | (b & 0x55) << 1
        return b

    def mirror_sprite_h(self, byte_array, width, height):
        """Mirror a MONO_HLSB sprite horizontally, returns a new bytearray"""
        bytes_per_row = (width + 7) // 8
        result = bytearray(len(byte_array))
        padding = (8 - (width % 8)) % 8  # unused bits on the right of last byte

        for row in range(height):
            row_start = row * bytes_per_row
            # Reverse byte order within row and mirror bits in each byte
            for col in range(bytes_per_row):
                src_byte = byte_array[row_start + (bytes_per_row - 1 - col)]
                result[row_start + col] = self.mirror_byte(src_byte)

            # Shift row left to move padding from left side back to right
            if padding > 0:
                for col in range(bytes_per_row):
                    current = result[row_start + col]
                    next_byte = result[row_start + col + 1] if col + 1 < bytes_per_row else 0
                    result[row_start + col] = ((current << padding) | (next_byte >> (8 - padding))) & 0xFF

        return result

    def mirror_sprite_v(self, byte_array, width, height):
        """Mirror a MONO_HLSB sprite vertically, returns a new bytearray"""
        bytes_per_row = (width + 7) // 8
        result = bytearray(len(byte_array))

        for row in range(height):
            src_start = row * bytes_per_row
            dst_start = (height - 1 - row) * bytes_per_row
            result[dst_start:dst_start + bytes_per_row] = byte_array[src_start:src_start + bytes_per_row]

        return result

    def rotate_sprite(self, byte_array, width, height, angle):
        """Rotate a MONO_HLSB sprite by the given angle in degrees.

        Uses naive nearest-neighbor rotation. Pixel-perfect for 90° increments.

        Returns (rotated_bytearray, new_width, new_height)
        """
        # Convert to radians
        rad = math.radians(angle)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        # Calculate new bounding box size
        new_width = int(abs(width * cos_a) + abs(height * sin_a) + 0.5)
        new_height = int(abs(width * sin_a) + abs(height * cos_a) + 0.5)

        # Ensure minimum size of 1
        new_width = max(1, new_width)
        new_height = max(1, new_height)

        # Source and destination centers
        src_cx = width / 2
        src_cy = height / 2
        dst_cx = new_width / 2
        dst_cy = new_height / 2

        # Create result bytearray
        src_bytes_per_row = (width + 7) // 8
        dst_bytes_per_row = (new_width + 7) // 8
        result = bytearray(dst_bytes_per_row * new_height)

        # For each destination pixel, find source pixel (inverse mapping)
        for dy in range(new_height):
            for dx in range(new_width):
                # Translate to center-relative
                rx = dx - dst_cx
                ry = dy - dst_cy

                # Inverse rotation (rotate by -angle to find source)
                sx = rx * cos_a + ry * sin_a + src_cx
                sy = -rx * sin_a + ry * cos_a + src_cy

                # Round to nearest integer
                sx_int = int(sx + 0.5)
                sy_int = int(sy + 0.5)

                # Check bounds
                if 0 <= sx_int < width and 0 <= sy_int < height:
                    # Get source pixel (MONO_HLSB: MSB is leftmost)
                    src_byte_idx = sy_int * src_bytes_per_row + sx_int // 8
                    src_bit = 7 - (sx_int % 8)
                    pixel = (byte_array[src_byte_idx] >> src_bit) & 1

                    if pixel:
                        # Set destination pixel
                        dst_byte_idx = dy * dst_bytes_per_row + dx // 8
                        dst_bit = 7 - (dx % 8)
                        result[dst_byte_idx] |= (1 << dst_bit)

        return result, new_width, new_height

    def skew_sprite(self, byte_array, width, height, skew_x=0.0, skew_y=0.0):
        """Skew a MONO_HLSB sprite.

        skew_x: horizontal skew factor (pixels shifted per row)
        skew_y: vertical skew factor (pixels shifted per column)

        Returns (skewed_bytearray, new_width, new_height)
        """
        # Calculate transformed corners to find bounding box
        # Transform: dx = sx + skew_x * sy, dy = sy + skew_y * sx
        corners = [
            (0, 0),
            (width - 1, 0),
            (0, height - 1),
            (width - 1, height - 1)
        ]

        transformed = []
        for sx, sy in corners:
            dx = sx + skew_x * sy
            dy = sy + skew_y * sx
            transformed.append((dx, dy))

        min_x = min(p[0] for p in transformed)
        max_x = max(p[0] for p in transformed)
        min_y = min(p[1] for p in transformed)
        max_y = max(p[1] for p in transformed)

        new_width = int(max_x - min_x + 1.5)
        new_height = int(max_y - min_y + 1.5)

        # Ensure minimum size
        new_width = max(1, new_width)
        new_height = max(1, new_height)

        # Offset to translate bounding box to origin
        offset_x = -min_x
        offset_y = -min_y

        # Create result
        src_bytes_per_row = (width + 7) // 8
        dst_bytes_per_row = (new_width + 7) // 8
        result = bytearray(dst_bytes_per_row * new_height)

        # Inverse transform denominator
        denom = 1 - skew_x * skew_y
        if abs(denom) < 0.001:
            # Degenerate case - skew factors cancel out, return empty
            return result, new_width, new_height

        # For each destination pixel, find source pixel (inverse mapping)
        for dy in range(new_height):
            for dx in range(new_width):
                # Remove offset to get transformed coordinates
                tx = dx - offset_x
                ty = dy - offset_y

                # Inverse transform
                sx = (tx - skew_x * ty) / denom
                sy = (ty - skew_y * tx) / denom

                # Round to nearest integer
                sx_int = int(sx + 0.5)
                sy_int = int(sy + 0.5)

                # Check bounds
                if 0 <= sx_int < width and 0 <= sy_int < height:
                    # Get source pixel (MONO_HLSB: MSB is leftmost)
                    src_byte_idx = sy_int * src_bytes_per_row + sx_int // 8
                    src_bit = 7 - (sx_int % 8)
                    pixel = (byte_array[src_byte_idx] >> src_bit) & 1

                    if pixel:
                        # Set destination pixel
                        dst_byte_idx = dy * dst_bytes_per_row + dx // 8
                        dst_bit = 7 - (dx % 8)
                        result[dst_byte_idx] |= (1 << dst_bit)

        return result, new_width, new_height

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
            byte_array = self.mirror_sprite_h(byte_array, width, height)

        # Mirror vertically if requested
        if mirror_v:
            byte_array = self.mirror_sprite_v(byte_array, width, height)

        # Rotate if requested
        if rotate != 0:
            # Adjust position so sprite rotates around its center
            old_cx = x + width // 2
            old_cy = y + height // 2
            byte_array, width, height = self.rotate_sprite(byte_array, width, height, rotate)
            x = old_cx - width // 2
            y = old_cy - height // 2

        # Skew if requested
        if skew_x != 0 or skew_y != 0:
            # Adjust position so sprite skews around its center
            old_cx = x + width // 2
            old_cy = y + height // 2
            byte_array, width, height = self.skew_sprite(byte_array, width, height, skew_x, skew_y)
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

    def draw_transition_fade(self, progress):
        """Draw dither pattern overlay for fade transition.

        Args:
            progress: 0.0 = fully clear, 1.0 = fully black
        """
        if progress <= 0:
            return
        if progress >= 1:
            self.display.fill(0)
            return

        # Use different dither patterns based on progress
        # Pattern density increases with progress
        for y in range(config.DISPLAY_HEIGHT):
            for x in range(config.DISPLAY_WIDTH):
                draw_pixel = False

                if progress < 0.25:
                    # Sparse: every 4th pixel in a grid pattern
                    threshold = progress / 0.25
                    draw_pixel = (x % 4 == 0 and y % 4 == 0) and ((x + y) % 8 < threshold * 8)
                elif progress < 0.5:
                    # Quarter fill: 2x2 grid, one pixel per cell
                    threshold = (progress - 0.25) / 0.25
                    base = (x % 2 == 0 and y % 2 == 0)
                    extra = (x % 2 == 1 and y % 2 == 1) and ((x + y) % 4 < threshold * 4)
                    draw_pixel = base or extra
                elif progress < 0.75:
                    # Checkerboard: half the pixels
                    threshold = (progress - 0.5) / 0.25
                    base = (x + y) % 2 == 0
                    extra = (x % 2 == 0 and y % 2 == 1) and ((x + y) % 4 < threshold * 4)
                    draw_pixel = base or extra
                else:
                    # Dense: three-quarters to full
                    threshold = (progress - 0.75) / 0.25
                    # Start with 3/4 filled, progress to full
                    skip = (x + y) % 2 == 1 and (x % 2 == 0) and ((x + y) % 4 >= threshold * 4)
                    draw_pixel = not skip

                if draw_pixel:
                    self.display.pixel(x, y, 0)

    def draw_transition_wipe(self, progress, direction='right'):
        """Draw wipe transition.

        Args:
            progress: 0.0 = no wipe, 1.0 = fully wiped
            direction: 'left', 'right', 'up', 'down'
        """
        if progress <= 0:
            return
        if progress >= 1:
            self.display.fill(0)
            return

        if direction == 'right':
            width = int(progress * config.DISPLAY_WIDTH)
            self.display.fill_rect(0, 0, width, config.DISPLAY_HEIGHT, 0)
        elif direction == 'left':
            width = int(progress * config.DISPLAY_WIDTH)
            x = config.DISPLAY_WIDTH - width
            self.display.fill_rect(x, 0, width, config.DISPLAY_HEIGHT, 0)
        elif direction == 'down':
            height = int(progress * config.DISPLAY_HEIGHT)
            self.display.fill_rect(0, 0, config.DISPLAY_WIDTH, height, 0)
        elif direction == 'up':
            height = int(progress * config.DISPLAY_HEIGHT)
            y = config.DISPLAY_HEIGHT - height
            self.display.fill_rect(0, y, config.DISPLAY_WIDTH, height, 0)

    def draw_transition_iris(self, progress):
        """Draw iris (circle) transition.

        Args:
            progress: 0.0 = fully open (no black), 1.0 = fully closed (all black)
        """
        if progress <= 0:
            return
        if progress >= 1:
            self.display.fill(0)
            return

        # Center of screen
        cx = config.DISPLAY_WIDTH // 2
        cy = config.DISPLAY_HEIGHT // 2

        # Max radius is distance from center to corner
        max_radius = int(math.sqrt(cx * cx + cy * cy)) + 1

        # Current hole radius (shrinks as progress increases)
        hole_radius = int(max_radius * (1 - progress))
        hole_radius_sq = hole_radius * hole_radius

        # Draw black pixels outside the circle
        for y in range(config.DISPLAY_HEIGHT):
            dy = y - cy
            dy_sq = dy * dy
            for x in range(config.DISPLAY_WIDTH):
                dx = x - cx
                dist_sq = dx * dx + dy_sq
                if dist_sq > hole_radius_sq:
                    self.display.pixel(x, y, 0)
