"""espnow_handler.py - Dispatch incoming ESP-NOW messages and manage the heard-bubble overlay.

The heard-bubble HUD is drawn at the game level (not inside a behavior) so it always
appears when a signal arrives, even if the cat is too busy or overwhelmed to react.
"""

import math
from ui import draw_heard_bubble


class EspNowHandler:
    """Processes incoming ESP-NOW messages, triggers cat reactions, and draws the heard-bubble overlay."""

    # How long the heard bubble stays visible, in seconds.
    # Generous enough to cover max noticing (1s) + max listening (4s).
    _FLASH_DURATION = 5.5

    def __init__(self, espnow, scene_manager):
        self._espnow = espnow
        self._scene_manager = scene_manager
        self._heard_flash = None  # (icon, corner, timer) or None

    def dispatch(self):
        """Poll ESP-NOW and handle all queued messages."""
        self._espnow.poll()
        if not self._espnow.messages:
            return
        for mac, msg_type, payload in self._espnow.messages:
            mac_str = ':'.join('%02x' % b for b in mac)
            print('[ESPNow] %s from %s: %s' % (msg_type, mac_str, payload))
            if msg_type == 'vocalize':
                self._handle_heard_vocalize(payload)
        self._espnow.messages.clear()

    def update(self, dt):
        """Tick the heard-bubble overlay timer."""
        if self._heard_flash is not None:
            icon, corner, timer = self._heard_flash
            timer -= dt
            if timer <= 0:
                self._heard_flash = None
            else:
                self._heard_flash = (icon, corner, timer)

    def draw(self, renderer):
        """Draw the heard-bubble overlay if active."""
        if self._heard_flash is None:
            return
        icon, corner, timer = self._heard_flash
        elapsed = self._FLASH_DURATION - timer
        y_offset = int(math.sin(elapsed * 9.42) * 3)
        draw_heard_bubble(renderer, icon, corner, y_offset=y_offset)

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _handle_heard_vocalize(self, payload):
        icon = payload.get('i', 'exclaim')
        scene = self._scene_manager.current_scene

        # Always show the HUD bubble regardless of what the cat does
        self._heard_flash = (icon, self._compute_corner(scene), self._FLASH_DURATION)

        if not hasattr(scene, 'character'):
            return
        character = scene.character
        # Don't re-trigger if hearing is already in progress
        cb = character.current_behavior
        if cb and cb.NAME == 'hearing':
            return
        # Crowd protection: cat tunes out after too many hearings in a row
        if character.context.recent_behaviors.count('hearing') >= 3:
            return
        character.behavior_manager.trigger('hearing', icon=icon)

    def _compute_corner(self, scene):
        """Place the bubble on the opposite side of the screen from the cat."""
        if not hasattr(scene, 'character'):
            return 'left'
        ctx = getattr(scene.character, 'context', None)
        if ctx is None:
            return 'left'
        mid = (ctx.scene_x_min + ctx.scene_x_max) / 2
        return 'right' if scene.character.x < mid else 'left'
