"""
config_desktop.py - Desktop/PC configuration for catode32.
Loaded by main_desktop.py and injected as the 'config' module.
Imports base config then overrides desktop-specific values.
"""

import pygame
import os as _os

from config import *

IS_DESKTOP = True

# Save file location. When launched via run.py, CATODE32_SRC points to src/
# so the save lands there regardless of which build directory is active.
_src_dir  = _os.environ.get('CATODE32_SRC', _os.path.dirname(_os.path.abspath(__file__)))
SAVE_PATH = _os.path.join(_src_dir, 'save.json')

BOARD_TYPE = "DESKTOP"

# Desktop display extras — base DISPLAY_WIDTH/HEIGHT inherited from config
DISPLAY_SCALE = 8             # Each pixel becomes an 8x8 block → 1024x512 window
DISPLAY_COLOR = (35, 165, 204)
DISPLAY_BG    = (10, 10, 10)
# Set True only if your physical OLED has SEG remap enabled and text appears
# mirrored. For the desktop PC port this should be False.
DISPLAY_MIRROR_H = False

# Key mappings (pygame key constants) — override GPIO pin numbers from config
BTN_UP    = pygame.K_UP
BTN_DOWN  = pygame.K_DOWN
BTN_LEFT  = pygame.K_LEFT
BTN_RIGHT = pygame.K_RIGHT
BTN_A     = pygame.K_a
BTN_B     = pygame.K_s
BTN_MENU1 = pygame.K_q
BTN_MENU2 = pygame.K_w

# WiFi / ESPNow features — disabled on desktop
WIFI_ENABLED = False

# Sleep disabled on desktop
SLEEP_MODE = None

# Append suffix to base version
VERSION = VERSION + "-desktop"
