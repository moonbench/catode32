"""
input_desktop.py - Keyboard/gamepad input for desktop/PC
Replaces input.py when running catode32 on a PC.

Uses keys that are in the same physical position on both QWERTY and AZERTY:

  Arrow keys  →  D-pad (Up / Down / Left / Right)
  T           →  A button
  Y           →  B button
  U           →  Menu 1
  I           →  Menu 2
  Escape      →  Quit
"""

import time
import pygame
import config


class InputHandler:
    """Handles keyboard input.
    API-compatible with the ESP32 InputHandler.
    """

    BUTTON_KEYS = ('up', 'down', 'left', 'right', 'a', 'b', 'menu1', 'menu2')

    _KEY_MAP = {
        'up':    pygame.K_UP,
        'down':  pygame.K_DOWN,
        'left':  pygame.K_LEFT,
        'right': pygame.K_RIGHT,
        'a':     pygame.K_t,
        'b':     pygame.K_y,
        'menu1': pygame.K_u,
        'menu2': pygame.K_i,
    }

    def __init__(self):
        self.button_states    = {k: False for k in self.BUTTON_KEYS}
        self.last_press_time  = {k: 0     for k in self.BUTTON_KEYS}
        self.debounce_time_ms = 50
        self._keys = pygame.key.get_pressed()

    def pump(self):
        """Refresh internal key state. Call once per game loop iteration."""
        self._keys = pygame.key.get_pressed()

    def is_pressed(self, button_name):
        key = self._KEY_MAP.get(button_name)
        if key is None:
            return False
        return bool(self._keys[key])

    def was_just_pressed(self, button_name):
        now = int(time.time() * 1000)
        currently = self.is_pressed(button_name)
        previously = self.button_states[button_name]
        elapsed = now - self.last_press_time[button_name]

        if currently and not previously:
            if elapsed > self.debounce_time_ms:
                self.button_states[button_name] = True
                self.last_press_time[button_name] = now
                return True

        if not currently and previously:
            self.button_states[button_name] = False

        return False

    def get_direction(self):
        dx, dy = 0, 0
        if self.is_pressed('up'):    dy -= 1
        if self.is_pressed('down'):  dy += 1
        if self.is_pressed('left'):  dx -= 1
        if self.is_pressed('right'): dx += 1
        return (dx, dy)

    def any_button_pressed(self):
        return any(self.is_pressed(b) for b in self.BUTTON_KEYS)

    def get_pressed_buttons(self):
        return [b for b in self.BUTTON_KEYS if self.is_pressed(b)]

    def consume_all(self):
        now = int(time.time() * 1000)
        for b in self.BUTTON_KEYS:
            self.button_states[b] = self.is_pressed(b)
            self.last_press_time[b] = now
