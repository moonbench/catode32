"""espnow_handler.py - Dispatch incoming ESP-NOW messages and manage the heard-bubble overlay.

The heard-bubble HUD is drawn at the game level (not inside a behavior) so it always
appears when a signal arrives, even if the cat is too busy or overwhelmed to react.

Visit protocol messages handled here:
  vst  - remote cat state update (pose/position/mirror); updates visitor_cat on current scene
  vbye - remote player ended the visit; clears context.visit and stops radio if appropriate

Handshake messages (hello, vreq, vok, vno) are forwarded to the current scene via
on_espnow_msg() so SocialScene can own that state machine.
"""

import math
from ui import draw_heard_bubble

# Seconds with no vst from the peer before we consider the visit dropped.
_VISIT_TIMEOUT = 10.0


class EspNowHandler:
    """Processes incoming ESP-NOW messages, triggers cat reactions, and draws the heard-bubble overlay."""

    # How long the heard bubble stays visible, in seconds.
    # Generous enough to cover max noticing (1s) + max listening (4s).
    _FLASH_DURATION = 5.5

    def __init__(self, espnow, scene_manager):
        self._espnow = espnow
        self._scene_manager = scene_manager
        self._heard_flash = None  # (icon, corner, timer) or None
        self._visit_timeout = 0.0  # seconds since last vst from peer

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
            elif msg_type == 'vst':
                self._handle_vst(mac, payload)
            elif msg_type == 'vbye':
                self._handle_vbye(mac)
            elif msg_type == 'vloc':
                self._handle_vloc(mac, payload)
            elif msg_type == 'venv':
                self._handle_venv(mac, payload)
            elif msg_type == 'vss':
                self._handle_vss(mac, payload)
            elif msg_type == 'vse':
                self._handle_vse(mac, payload)
            elif msg_type in ('hello', 'vreq', 'vok', 'vno'):
                scene = self._scene_manager.current_scene
                if hasattr(scene, 'on_espnow_msg'):
                    scene.on_espnow_msg(mac, msg_type, payload)
        self._espnow.messages.clear()

    def update(self, dt):
        """Tick the heard-bubble overlay timer and check visit keepalive."""
        if self._heard_flash is not None:
            icon, corner, timer = self._heard_flash
            timer -= dt
            if timer <= 0:
                self._heard_flash = None
            else:
                self._heard_flash = (icon, corner, timer)

        ctx = self._scene_manager.context
        if ctx.visit is not None:
            self._visit_timeout += dt
            if self._visit_timeout >= _VISIT_TIMEOUT:
                print('[ESPNow] Visit timed out - no packets from peer')
                self._end_visit()

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
        ctx = self._scene_manager.context
        # Suppress "nearby cat" HUD during playdates and when not outdoors
        if ctx.visit is not None:
            return
        scene = self._scene_manager.current_scene
        if getattr(scene, 'SCENE_NAME', None) not in ('outside', 'treehouse'):
            return

        icon = payload.get('i', 'exclaim')

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

    def _handle_vst(self, mac, payload):
        """Apply remote cat state to the visitor entity on the current scene."""
        ctx = self._scene_manager.context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        # Reset timeout counter on any vst from our peer
        self._visit_timeout = 0.0
        scene = self._scene_manager.current_scene
        visitor = getattr(scene, 'visitor_cat', None)
        if visitor is not None:
            visitor.apply_state(
                payload.get('x', 64),
                payload.get('p', 'sitting.side.neutral'),
                payload.get('m', 0),
                payload.get('vx', 0),
            )

    def _handle_vloc(self, mac, payload):
        """Peer switched to a different scene; follow if we're not already there."""
        ctx = self._scene_manager.context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        scene_name = payload.get('s')
        if not scene_name:
            return
        current = getattr(self._scene_manager.current_scene, 'SCENE_NAME', None)
        if scene_name != current:
            self._scene_manager.change_scene_by_name(scene_name)

    def _handle_venv(self, mac, payload):
        """Apply the inviter's environment snapshot so time-of-day/weather stay in sync."""
        ctx = self._scene_manager.context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        env = ctx.environment
        env['time_hours']   = payload.get('h', env.get('time_hours', 12))
        env['time_minutes'] = payload.get('mn', env.get('time_minutes', 0))
        env['weather']      = payload.get('w', env.get('weather', 'Clear'))
        env['season']       = payload.get('s', env.get('season', 'Spring'))
        env['moon_phase']   = payload.get('mp', env.get('moon_phase', 'Full'))

    def _handle_vss(self, mac, payload):
        """Apply an inviter-spawned shooting star to our sky."""
        ctx = self._scene_manager.context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        sky = getattr(self._scene_manager.current_scene, 'sky', None)
        if sky:
            sky.apply_shooting_star(
                payload.get('x', 30), payload.get('y', 10),
                payload.get('ml', 20), payload.get('sx', 28), payload.get('sy', 8),
            )

    def _handle_vse(self, mac, payload):
        """Apply an inviter-spawned sky event (balloon/plane) to our sky."""
        ctx = self._scene_manager.context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        sky = getattr(self._scene_manager.current_scene, 'sky', None)
        if sky:
            sky.apply_sky_event(
                payload.get('ei', 0),
                bool(payload.get('r', 0)),
                payload.get('y', 10),
                payload.get('sp', 4.0),
            )

    def _handle_vbye(self, mac):
        """Remote player ended the visit."""
        ctx = self._scene_manager.context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        print('[ESPNow] Peer ended visit')
        self._end_visit()

    def _end_visit(self):
        """Clear visit state and stop the radio if the current scene isn't outdoor."""
        ctx = self._scene_manager.context
        ctx.visit = None
        self._visit_timeout = 0.0
        scene_name = getattr(self._scene_manager.current_scene, 'SCENE_NAME', None)
        if scene_name not in ('outside', 'treehouse'):
            if ctx.espnow:
                ctx.espnow.stop()

    def _compute_corner(self, scene):
        """Place the bubble on the opposite side of the screen from the cat."""
        if not hasattr(scene, 'character'):
            return 'left'
        ctx = getattr(scene.character, 'context', None)
        if ctx is None:
            return 'left'
        mid = (ctx.scene_x_min + ctx.scene_x_max) / 2
        return 'right' if scene.character.x < mid else 'left'
