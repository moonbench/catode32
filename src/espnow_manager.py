"""EspNowManager - game-level ESP-NOW controller.

Activated when the cat enters an outdoor scene (outside/treehouse).
Deactivated when the cat goes indoors.

Usage:
    manager.start()                        # called by outdoor scenes on enter
    manager.stop()                         # called by outdoor scenes on exit
    manager.send('vocalize', {'i': 'lonely'})  # broadcast an event
    manager.poll()                         # drain receive buffer each game frame
    manager.messages                       # list of (mac, type, payload) since last clear
"""

import gc

try:
    import ujson as json
except ImportError:
    import json

_BROADCAST = b'\xff\xff\xff\xff\xff\xff'


class EspNowManager:

    def __init__(self):
        self._wlan = None
        self._e = None
        self.own_mac = None
        # Incoming messages accumulated by poll(). Consumer clears after reading.
        self.messages = []

    @property
    def active(self):
        return self._e is not None

    def start(self):
        """Activate WLAN and ESP-NOW. No-op if already active."""
        if self._e is not None:
            return
        try:
            import network
            import espnow
            self._wlan = network.WLAN(network.STA_IF)
            self._wlan.active(True)
            self.own_mac = self._wlan.config('mac')
            self._e = espnow.ESPNow()
            self._e.active(True)
            try:
                self._e.add_peer(_BROADCAST)
            except Exception:
                pass  # peer may already exist
            mac_str = ':'.join('%02x' % b for b in self.own_mac)
            print('[ESPNow] Started. MAC: ' + mac_str)
        except Exception as ex:
            print('[ESPNow] Start failed: ' + str(ex))
            self._e = None
        gc.collect()

    def stop(self):
        """Deactivate ESP-NOW and WLAN. No-op if already stopped."""
        if self._e is None:
            return
        try:
            self._e.active(False)
        except Exception:
            pass
        self._e = None
        try:
            if self._wlan:
                self._wlan.active(False)
        except Exception:
            pass
        self._wlan = None
        self.own_mac = None
        self.messages.clear()
        gc.collect()
        print('[ESPNow] Stopped')

    def send(self, msg_type, payload=None):
        """Broadcast a message to all nearby devices.

        Args:
            msg_type: Short string identifying the event (e.g. 'vocalize').
            payload:  Optional dict of extra fields to include.
        """
        if self._e is None:
            return
        try:
            msg = {'t': msg_type}
            if payload:
                msg.update(payload)
            self._e.send(_BROADCAST, json.dumps(msg))
        except Exception as ex:
            print('[ESPNow] Send error: ' + str(ex))

    def poll(self):
        """Drain the ESP-NOW receive buffer into self.messages.

        Call once per game frame. The caller is responsible for reading
        and clearing self.messages after processing.
        """
        if self._e is None:
            return
        try:
            while len(self.messages) < 8:
                host, data = self._e.recv(0)
                if host is None:
                    break
                if host == self.own_mac:
                    continue  # ignore our own broadcasts
                try:
                    msg = json.loads(data)
                    msg_type = msg.pop('t', None)
                    if msg_type:
                        self.messages.append((host, msg_type, msg))
                except Exception:
                    pass  # malformed packet
        except Exception as ex:
            print('[ESPNow] Poll error: ' + str(ex))
