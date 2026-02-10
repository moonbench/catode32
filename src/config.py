"""
config.py - Hardware configuration and game constants
"""

# Display Configuration
DISPLAY_WIDTH = 128
DISPLAY_HEIGHT = 64
I2C_SDA = 4
I2C_SCL = 7
I2C_FREQ = 400000

# Button Pin Mappings
BTN_UP = 14
BTN_DOWN = 18
BTN_LEFT = 20
BTN_RIGHT = 19
BTN_A = 1
BTN_B = 0
BTN_MENU1 = 3
BTN_MENU2 = 2

# Game Constants
FPS = 30  # Target frames per second
FRAME_TIME_MS = 1000 // FPS  # Milliseconds per frame

# Character Constants
CHAR_SIZE = 8  # Character is 8x8 pixels
CHAR_SPEED = 2  # Pixels per frame when moving

# World Boundaries
WORLD_MIN_X = 0
WORLD_MAX_X = DISPLAY_WIDTH - CHAR_SIZE
WORLD_MIN_Y = 0
WORLD_MAX_Y = DISPLAY_HEIGHT - CHAR_SIZE

# Transition Settings
TRANSITION_TYPE = 'fade'        # 'fade', 'wipe', 'iris'
TRANSITION_DURATION = 0.5       # seconds per half-transition (total is 2x this)

# Panning Settings
PAN_SPEED = 2  # pixels per frame when D-pad held