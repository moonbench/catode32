"""
config.py - Hardware configuration and game constants
"""

# ============================================================================
# BOARD SELECTION - Change this to match your ESP32 board
# ============================================================================
# Supported boards: "ESP32-C6", "ESP32-C3"
BOARD_TYPE = "ESP32-C6"  # Change to "ESP32-C3" if using ESP32-C3 board

# ============================================================================
# Board-Specific Pin Configurations
# ============================================================================

# ESP32-C6 Pin Configuration (SuperMini)
_ESP32_C6_CONFIG = {
    'I2C_SDA': 4,
    'I2C_SCL': 7,
    'BTN_UP': 14,
    'BTN_DOWN': 18,
    'BTN_LEFT': 20,
    'BTN_RIGHT': 19,
    'BTN_A': 1,
    'BTN_B': 0,
    'BTN_MENU1': 3,
}

# ESP32-C3 Pin Configuration
# Uses lower GPIO pins that are commonly available on ESP32-C3 boards
# Avoids strapping pins (GPIO2, GPIO8, GPIO9)
_ESP32_C3_CONFIG = {
    'I2C_SDA': 6,
    'I2C_SCL': 7,
    'BTN_UP': 0,
    'BTN_DOWN': 1,
    'BTN_LEFT': 2,
    'BTN_RIGHT': 3,
    'BTN_A': 4,
    'BTN_B': 5,
    'BTN_MENU1': 10,
}

# Select configuration based on board type
if BOARD_TYPE == "ESP32-C3":
    _CONFIG = _ESP32_C3_CONFIG
elif BOARD_TYPE == "ESP32-C6":
    _CONFIG = _ESP32_C6_CONFIG
else:
    raise ValueError(f"Unknown BOARD_TYPE: {BOARD_TYPE}. Supported: 'ESP32-C6', 'ESP32-C3'")

# Display Configuration
DISPLAY_WIDTH = 128
DISPLAY_HEIGHT = 64
I2C_SDA = _CONFIG['I2C_SDA']
I2C_SCL = _CONFIG['I2C_SCL']
I2C_FREQ = 400000

# Button Pin Mappings
BTN_UP = _CONFIG['BTN_UP']
BTN_DOWN = _CONFIG['BTN_DOWN']
BTN_LEFT = _CONFIG['BTN_LEFT']
BTN_RIGHT = _CONFIG['BTN_RIGHT']
BTN_A = _CONFIG['BTN_A']
BTN_B = _CONFIG['BTN_B']
BTN_MENU1 = _CONFIG['BTN_MENU1']

# Game Constants
FPS = 12  # Target frames per second
FRAME_TIME_MS = 1000 // FPS  # Milliseconds per frame

# Transition Settings
TRANSITION_TYPE = 'fade'        # 'fade', 'wipe', 'iris'
TRANSITION_DURATION = 0.4       # seconds per half-transition (total is 2x this)

# Panning Settings
PAN_SPEED = 2  # pixels per frame when D-pad held