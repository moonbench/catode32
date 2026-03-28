"""social.py - Discovery, invite, and playdate management scene.

Both players must be on this scene for the flow to work. While here:
  - Radio is always on (for discovery broadcasts and invite exchange)
  - Nearby cats are discovered via periodic 'hello' broadcasts
  - Player selects a cat and sends a visit invite ('vreq')
  - Other player sees a popup and accepts ('vok') or declines ('vno')
  - On acceptance both devices transition to the inside scene with visit active

During an active visit, returning to this scene shows a "leave" option.
Leaving sends 'vbye' and clears the visit state.

Handshake messages are delivered by EspNowHandler.dispatch() via on_espnow_msg().
"""

import time
from scene import Scene
from menu import Menu, MenuItem

_HELLO_INTERVAL = 2.0   # seconds between hello broadcasts
_NEARBY_TIMEOUT = 6.0   # seconds before removing a peer from the nearby list
_INVITE_TIMEOUT = 10.0  # seconds waiting for the other player to respond

# Scene states
_ST_BROWSING = 0   # showing nearby cat list
_ST_INVITING = 1   # sent vreq, awaiting response
_ST_INVITED  = 2   # received vreq, showing accept/decline popup
_ST_VISITING = 3   # visit active; showing leave option


class SocialScene(Scene):
    SCENE_NAME = 'social'
    MODULES_TO_KEEP = []

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._state = _ST_BROWSING
        self._nearby = {}        # mac_bytes -> {'n': name, 'ts': ticks_ms}
        self._selected = 0       # cursor index into nearby list
        self._invite_mac = None  # MAC involved in current invite (either direction)
        self._invite_name = None
        self._invite_timer = 0.0
        self._hello_timer = 0.0
        self._confirm_menu = Menu(self.renderer, self.input)

    # ------------------------------------------------------------------
    # Scene lifecycle
    # ------------------------------------------------------------------

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        # Always turn radio on here (needed for discovery)
        if self.context.espnow:
            self.context.espnow.start()
            # Derive a display name from MAC the first time we have one
            if self.context.pet_name is None and self.context.espnow.own_mac:
                mac = self.context.espnow.own_mac
                self.context.pet_name = 'CAT-%02x%02x' % (mac[-2], mac[-1])

        if self.context.visit is not None:
            self._state = _ST_VISITING
        else:
            self._state = _ST_BROWSING
            self._nearby = {}
            self._selected = 0
            self._invite_mac = None
            self._invite_name = None
            self._invite_timer = 0.0

        self._hello_timer = 0.0

    def exit(self):
        # Leave radio on if a visit is still active; the visit keeps it alive
        if self.context.espnow and self.context.visit is None:
            self.context.espnow.stop()

    # ------------------------------------------------------------------
    # Update
    # ------------------------------------------------------------------

    def update(self, dt):
        if not (self.context.espnow and self.context.espnow.active):
            return None

        # Broadcast our presence periodically
        self._hello_timer -= dt
        if self._hello_timer <= 0:
            self._hello_timer = _HELLO_INTERVAL
            self._send_hello()

        # Prune stale nearby entries
        now = time.ticks_ms()
        stale = [m for m, d in self._nearby.items()
                 if time.ticks_diff(now, d['ts']) > int(_NEARBY_TIMEOUT * 1000)]
        for m in stale:
            del self._nearby[m]
        # Clamp cursor if list shrank
        nearby_list = list(self._nearby.items())
        if nearby_list:
            self._selected = min(self._selected, len(nearby_list) - 1)

        # Invite timeout
        if self._state == _ST_INVITING:
            self._invite_timer -= dt
            if self._invite_timer <= 0:
                self._state = _ST_BROWSING
                self._invite_mac = None
                self._invite_name = None

        return None

    # ------------------------------------------------------------------
    # Draw
    # ------------------------------------------------------------------

    def draw(self):
        r = self.renderer
        if self._state == _ST_BROWSING:
            r.draw_text('Social', 0, 0)
            nearby_list = list(self._nearby.items())
            if not nearby_list:
                r.draw_text('No cats nearby...', 0, 18)
            else:
                for i, (mac, data) in enumerate(nearby_list):
                    prefix = '>' if i == self._selected else ' '
                    r.draw_text((prefix + data['n'])[:21], 0, 16 + i * 9)
            r.draw_text('A=invite  B=back', 0, 56)

        elif self._state == _ST_INVITING:
            r.draw_text('Inviting...', 0, 0)
            r.draw_text((self._invite_name or '?')[:21], 0, 16)
            r.draw_text('Waiting...', 0, 32)
            r.draw_text('B=cancel', 0, 56)

        elif self._state == _ST_INVITED:
            self._confirm_menu.draw()

        elif self._state == _ST_VISITING:
            visit = self.context.visit
            name = visit['peer_name'] if visit else '?'
            r.draw_text('Playing with:', 0, 8)
            r.draw_text(name[:21], 0, 20)
            r.draw_text('B=leave', 0, 56)

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def handle_input(self):
        if self._state == _ST_BROWSING:
            nearby_list = list(self._nearby.items())
            if self.input.was_just_pressed('up') and nearby_list:
                self._selected = max(0, self._selected - 1)
            elif self.input.was_just_pressed('down') and nearby_list:
                self._selected = min(len(nearby_list) - 1, self._selected + 1)
            elif self.input.was_just_pressed('a') and nearby_list:
                mac, data = nearby_list[self._selected]
                self._invite_mac = mac
                self._invite_name = data['n']
                self._invite_timer = _INVITE_TIMEOUT
                self._state = _ST_INVITING
                self._send_invite(mac)
            elif self.input.was_just_pressed('b'):
                return ('change_scene', 'last_main')

        elif self._state == _ST_INVITING:
            if self.input.was_just_pressed('b'):
                if self.context.espnow and self._invite_mac:
                    self.context.espnow.send_to(self._invite_mac, 'vno')
                self._state = _ST_BROWSING
                self._invite_mac = None
                self._invite_name = None

        elif self._state == _ST_INVITED:
            was_confirming = self._confirm_menu.pending_confirmation is not None
            result = self._confirm_menu.handle_input()
            if result == 'accept':
                if self.context.espnow and self._invite_mac:
                    name = self.context.pet_name or '?'
                    self.context.espnow.send_to(self._invite_mac, 'vok', {'n': name})
                self._start_visit(self._invite_mac, self._invite_name, 'invitee')
                return ('change_scene', 'inside')
            elif result == 'closed' or (was_confirming and not self._confirm_menu.pending_confirmation):
                # B pressed during confirmation, or menu button closed it — treat as decline
                if self.context.espnow and self._invite_mac:
                    self.context.espnow.send_to(self._invite_mac, 'vno')
                self._state = _ST_BROWSING
                self._invite_mac = None
                self._invite_name = None

        elif self._state == _ST_VISITING:
            if self.input.was_just_pressed('b'):
                self._end_visit()
                return ('change_scene', 'last_main')

        return None

    # ------------------------------------------------------------------
    # ESP-NOW message handler (called by EspNowHandler.dispatch)
    # ------------------------------------------------------------------

    def on_espnow_msg(self, mac, msg_type, payload):
        """Receive a handshake message from EspNowHandler."""
        if msg_type == 'hello':
            self._nearby[mac] = {'n': payload.get('n', '?'), 'ts': time.ticks_ms()}

        elif msg_type == 'vreq':
            if self._state == _ST_BROWSING:
                if self.context.espnow:
                    self.context.espnow.add_peer(mac)
                self._invite_mac = mac
                self._invite_name = payload.get('n', '?')
                self._show_invite_confirm(self._invite_name)

        elif msg_type == 'vok':
            if self._state == _ST_INVITING and mac == self._invite_mac:
                peer_name = payload.get('n', '?')
                self._start_visit(mac, peer_name, 'inviter')
                # Trigger scene change via context so it happens in the update loop
                self.context.pending_scene = 'inside'

        elif msg_type == 'vno':
            if self._state == _ST_INVITING and mac == self._invite_mac:
                self._state = _ST_BROWSING
                self._invite_mac = None
                self._invite_name = None

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _show_invite_confirm(self, name):
        """Open the reusable Menu confirmation dialog for an incoming invite."""
        msg = (name + ' wants to play!')[:36]
        item = MenuItem('', confirm=msg, action='accept')
        self._confirm_menu.open([item])
        self._confirm_menu.pending_confirmation = item
        self._state = _ST_INVITED

    def _send_hello(self):
        name = self.context.pet_name or '?'
        self.context.espnow.send('hello', {'n': name})

    def _send_invite(self, mac):
        if self.context.espnow:
            self.context.espnow.add_peer(mac)
            name = self.context.pet_name or '?'
            self.context.espnow.send_to(mac, 'vreq', {'n': name})

    def _start_visit(self, peer_mac, peer_name, role):
        self.context.visit = {
            'peer_mac': peer_mac,
            'peer_name': peer_name,
            'role': role,
            'greeted': False,   # becomes True after first greeting ritual fires
            'play_time': 0.0,   # accumulated seconds; written to friends on end
        }
        self._state = _ST_VISITING

    def _end_visit(self):
        if self.context.espnow and self.context.visit:
            self.context.espnow.send_to(self.context.visit['peer_mac'], 'vbye')
        self.context.record_visit_end()  # saves playtime to friends and clears visit
        self._state = _ST_BROWSING
        if self.context.espnow:
            self.context.espnow.stop()
