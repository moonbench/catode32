"""
renderer.py - Display rendering logic
"""

from machine import Pin, I2C
import ssd1306
import config
import framebuf

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

    def draw_sprite(self, byte_array, width, height, x, y, transparent=True):
        """Draw a sprite at the given position
        
        Args:
            byte_array: bytearray containing the sprite bitmap
            width: sprite width in pixels
            height: sprite height in pixels
            x: x position on display
            y: y position on display
        """
        
        # Create a framebuffer from the sprite data
        sprite_fb = framebuf.FrameBuffer(
            byte_array, 
            width, 
            height, 
            framebuf.MONO_HLSB  # or MONO_VLSB
        )
        
        if transparent:
            # Draw with transparency - black pixels (0) are transparent
            self.display.blit(sprite_fb, x, y, 0)
        else:
            # Draw without transparency (overwrites everything)
            self.display.blit(sprite_fb, x, y)
