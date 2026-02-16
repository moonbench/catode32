"""
sprite_transform.py - Sprite transformation utilities for MONO_HLSB bitmaps
"""

import math


def mirror_byte(b):
    """Reverse bits in a byte using parallel bit swapping."""
    b = (b & 0xF0) >> 4 | (b & 0x0F) << 4
    b = (b & 0xCC) >> 2 | (b & 0x33) << 2
    b = (b & 0xAA) >> 1 | (b & 0x55) << 1
    return b


def mirror_sprite_h(byte_array, width, height):
    """Mirror a MONO_HLSB sprite horizontally, returns a new bytearray."""
    bytes_per_row = (width + 7) // 8
    result = bytearray(len(byte_array))
    padding = (8 - (width % 8)) % 8  # unused bits on the right of last byte

    for row in range(height):
        row_start = row * bytes_per_row
        # Reverse byte order within row and mirror bits in each byte
        for col in range(bytes_per_row):
            src_byte = byte_array[row_start + (bytes_per_row - 1 - col)]
            result[row_start + col] = mirror_byte(src_byte)

        # Shift row left to move padding from left side back to right
        if padding > 0:
            for col in range(bytes_per_row):
                current = result[row_start + col]
                next_byte = result[row_start + col + 1] if col + 1 < bytes_per_row else 0
                result[row_start + col] = ((current << padding) | (next_byte >> (8 - padding))) & 0xFF

    return result


def mirror_sprite_v(byte_array, width, height):
    """Mirror a MONO_HLSB sprite vertically, returns a new bytearray."""
    bytes_per_row = (width + 7) // 8
    result = bytearray(len(byte_array))

    for row in range(height):
        src_start = row * bytes_per_row
        dst_start = (height - 1 - row) * bytes_per_row
        result[dst_start:dst_start + bytes_per_row] = byte_array[src_start:src_start + bytes_per_row]

    return result


def rotate_sprite(byte_array, width, height, angle):
    """Rotate a MONO_HLSB sprite by the given angle in degrees.

    Uses naive nearest-neighbor rotation. Pixel-perfect for 90 degree increments.

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


def skew_sprite(byte_array, width, height, skew_x=0.0, skew_y=0.0):
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
