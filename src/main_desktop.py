"""
main_desktop.py - Desktop entry point for catode32
Run from the src/ directory:  python main_desktop.py

Controls:
  Arrow keys  →  D-pad (Up / Down / Left / Right)
  Z           →  A button
  X           →  B button
  A           →  Menu 1
  S           →  Menu 2
  Escape      →  Quit
"""

import sys
import os
import time

# Make sure we're running from the src/ directory so all imports resolve
os.chdir(os.path.dirname(os.path.abspath(__file__)))

# ── Boot pygame first so config_desktop can reference pygame.K_* constants ──
import pygame
pygame.init()

# ── Swap in desktop versions before anything else imports the originals ──
# We rename the desktop modules to the names the game expects.
import config_desktop
sys.modules['config'] = config_desktop          # replaces config.py

import renderer_desktop
sys.modules['renderer'] = renderer_desktop      # replaces renderer.py
# Patch so  `from renderer import Renderer`  works
renderer_desktop.Renderer = renderer_desktop.Renderer

import input_desktop
sys.modules['input'] = input_desktop            # replaces input.py

# ── Stub out MicroPython-only modules ──────────────────────────────────────

import types

# machine — stub Pin/I2C so modules that import them don't crash
machine_stub = types.ModuleType('machine')
machine_stub.reset = lambda: sys.exit(0)   # crash handler calls machine.reset()
machine_stub.soft_reset = lambda: print("[Desktop] Save complete (no reboot needed)")

class _Pin:
    IN = 1; OUT = 0; PULL_UP = 1
    def __init__(self, *a, **k): pass
    def value(self): return 1             # buttons read as "not pressed"

machine_stub.Pin  = _Pin
machine_stub.I2C  = None
sys.modules['machine'] = machine_stub

# stub WiFi/ESPNow modules — not used on desktop
for _mod in ('espnow_manager', 'espnow_handler', 'visit_manager',
             'sleep_manager', 'wifi_tracker'):
    sys.modules[_mod] = types.ModuleType(_mod)

# uos  (used by context.py for save-file access → use standard os instead)
sys.modules['uos'] = os

# ujson (used by main.py intent handling → use standard json)
import json
sys.modules['ujson'] = json

# time.ticks_ms / ticks_diff — polyfill for MicroPython time functions
_time_origin = time.perf_counter()

def _ticks_ms():
    return int((time.perf_counter() - _time_origin) * 1000)

def _ticks_diff(a, b):
    return a - b

time.ticks_ms   = _ticks_ms
time.ticks_diff = _ticks_diff
time.sleep_ms   = lambda ms: time.sleep(ms / 1000)

# framebuf — MicroPython built-in; install the pure-Python port if needed
try:
    import framebuf
except ImportError:
    print("ERROR: 'framebuf' module not found.")
    print("Install it with:  pip install micropython-framebuf")
    sys.exit(1)

# ── Now import the real game ───────────────────────────────────────────────
from renderer_desktop import Renderer
from input_desktop import InputHandler
from context import GameContext
from scene_manager import SceneManager
from weather_system import WeatherSystem
from time_system import TimeSystem
from splash import show_splash
import config

# ── Game class (trimmed version of main.py — no ESP-Now, no deep sleep) ───

class Game:
    def __init__(self):
        print("==> Catode32 Desktop Starting...")

        self.renderer = Renderer()
        show_splash(self.renderer)

        self.input   = InputHandler()
        self.context = GameContext()
        self.context.load()
        self.context.input = self.input

        self.scene_manager = SceneManager(self.context, self.renderer, self.input)
        self.scene_manager.change_scene_by_name('inside')

        self.weather_system = WeatherSystem()
        if 'weather' not in self.context.environment:
            self.weather_system.init_environment(
                self.context.environment, self.context.pet_seed)

        self.time_system = TimeSystem()
        self.time_system.update_moon_phase(self.context.environment)
        self.time_system.game_minutes_per_second = 1 / 15

        self.last_frame_time = _ticks_ms()

    def run(self):
        print("==> Game loop running  (Esc to quit)")
        clock = pygame.time.Clock()

        while True:
            # ── pygame event handling ──────────────────────────────────
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    self._quit()
                if event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                    self._quit()

            # Refresh key state for InputHandler
            self.input.pump()

            # ── game tick ─────────────────────────────────────────────
            now      = _ticks_ms()
            delta_ms = _ticks_diff(now, self.last_frame_time)
            dt       = (delta_ms / 1000.0) * self.context.time_speed

            self.scene_manager.handle_input()
            self.time_system.advance(dt, self.context.environment, self.weather_system)
            self.scene_manager.update(dt)

            # ── draw ──────────────────────────────────────────────────
            self.renderer.clear()
            self.scene_manager.draw()
            self.renderer.show()

            self.last_frame_time = now
            clock.tick(config.FPS)

    def _quit(self):
        print("==> Saving and quitting...")
        try:
            self.context.save()
        except Exception as e:
            print(f"[Quit] Save failed: {e}")
        pygame.quit()
        sys.exit(0)


if __name__ == "__main__":
    game = Game()
    game.run()
