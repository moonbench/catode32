"""
character.py - Character class with position and movement
"""

import config

class Character:
    """Represents a character with position and movement logic"""
    
    def __init__(self, x=None, y=None):
        """
        Initialize character at given position
        If no position given, starts at center of screen
        """
        if x is None:
            x = (config.DISPLAY_WIDTH - config.CHAR_SIZE) // 2
        if y is None:
            y = (config.DISPLAY_HEIGHT - config.CHAR_SIZE) // 2
        
        self.x = x
        self.y = y
        self.size = config.CHAR_SIZE
        self.speed = config.CHAR_SPEED
        
        # Animation state (for future use)
        self.facing = 'down'  # 'up', 'down', 'left', 'right'
        self.is_moving = False
    
    def move(self, dx, dy):
        """
        Move character by delta x and y
        Respects world boundaries
        """
        if dx != 0 or dy != 0:
            self.is_moving = True
            
            # Update facing direction
            if dx < 0:
                self.facing = 'left'
            elif dx > 0:
                self.facing = 'right'
            elif dy < 0:
                self.facing = 'up'
            elif dy > 0:
                self.facing = 'down'
        else:
            self.is_moving = False
        
        # Apply movement with speed multiplier
        new_x = self.x + (dx * self.speed)
        new_y = self.y + (dy * self.speed)
        
        # Clamp to world boundaries
        self.x = max(config.WORLD_MIN_X, min(config.WORLD_MAX_X, new_x))
        self.y = max(config.WORLD_MIN_Y, min(config.WORLD_MAX_Y, new_y))
    
    def set_position(self, x, y):
        """Set character position directly"""
        self.x = max(config.WORLD_MIN_X, min(config.WORLD_MAX_X, x))
        self.y = max(config.WORLD_MIN_Y, min(config.WORLD_MAX_Y, y))
    
    def get_position(self):
        """Get character position as tuple"""
        return (int(self.x), int(self.y))
    
    def get_rect(self):
        """Get character bounding box as (x, y, width, height)"""
        return (int(self.x), int(self.y), self.size, self.size)
    
    def collides_with(self, other):
        """
        Check collision with another character or rect
        other should have get_rect() method or be (x, y, w, h) tuple
        """
        if hasattr(other, 'get_rect'):
            ox, oy, ow, oh = other.get_rect()
        else:
            ox, oy, ow, oh = other
        
        x, y, w, h = self.get_rect()
        
        return (x < ox + ow and
                x + w > ox and
                y < oy + oh and
                y + h > oy)