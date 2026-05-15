"""
config_desktop.py - Desktop/PC configuration for catode32
Drop-in replacement for config.py when running on PC with pygame.
Copy or symlink this as config.py to run on desktop.
"""

import pygame

import os as _os

# Save file location — stored next to config_desktop.py so it's easy to find.
# Change this to any path you prefer, e.g.:
#   _os.path.expanduser("~/catode32_save.json")
SAVE_PATH = _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), 'save.json')

BOARD_TYPE = "DESKTOP"

# Display Configuration — matches original OLED resolution
# Increase DISPLAY_SCALE to make the window larger (4 = 512x256 window)
DISPLAY_WIDTH  = 128
DISPLAY_HEIGHT = 64
DISPLAY_SCALE  = 6      # Each pixel becomes a 6x6 block → 768x384 window
DISPLAY_COLOR  = (0, 230, 0)   # Pixel-on colour  (green, like a classic LCD)
DISPLAY_BG     = (10, 10, 10)  # Pixel-off / background colour
# Set True only if your physical OLED has SEG remap enabled and text appears
# mirrored. For the desktop PC port this should be False.
DISPLAY_MIRROR_H = False

# Key mappings  (pygame key constants)
BTN_UP    = pygame.K_UP
BTN_DOWN  = pygame.K_DOWN
BTN_LEFT  = pygame.K_LEFT
BTN_RIGHT = pygame.K_RIGHT
BTN_A     = pygame.K_z
BTN_B     = pygame.K_x
BTN_MENU1 = pygame.K_a
BTN_MENU2 = pygame.K_s

# Dummy I2C values (not used on desktop, but some modules import these)
I2C_SDA  = 0
I2C_SCL  = 0
I2C_FREQ = 400000

# WiFi / ESPNow features — disabled on desktop
WIFI_ENABLED = False

# Debug menus
SHOW_DEBUG_MENUS = True

# Software version
VERSION = "0.4.0-desktop"

# Game timing
FPS            = 12
FRAME_TIME_MS  = 1000 // FPS

# Transition settings
TRANSITION_DURATION = 0.25

# Panning
PAN_SPEED = 4

# Sleep / power saving — disable on desktop
SLEEP_MODE        = None
SLEEP_TIMEOUT_SEC = 900
SLEEP_FPS         = 2
SLEEP_FRAME_TIME_MS = 1000 // SLEEP_FPS
