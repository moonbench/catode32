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
FPS = 12  # Target frames per second
FRAME_TIME_MS = 1000 // FPS  # Milliseconds per frame

# Transition Settings
TRANSITION_TYPE = 'fade'        # 'fade', 'wipe', 'iris'
TRANSITION_DURATION = 0.4       # seconds per half-transition (total is 2x this)

# Panning Settings
PAN_SPEED = 2  # pixels per frame when D-pad held