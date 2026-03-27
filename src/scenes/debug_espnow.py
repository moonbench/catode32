import gc
from scene import Scene
from ui import Scrollbar


class DebugEspnowScene(Scene):
    """Debug scene that shows ESP-NOW status and lets two devices exchange messages."""

    LINES_VISIBLE = 8
    LINE_HEIGHT = 8
    BROADCAST_MAC = b'\xff\xff\xff\xff\xff\xff'

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.scrollbar = Scrollbar(renderer)
        self.scroll_offset = 0
        self.lines = []
        self.wlan = None
        self.e = None
        self.own_mac = None
        self.send_count = 0

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.scroll_offset = 0
        self.send_count = 0
        self.lines = []
        self._start()

    def exit(self):
        self._stop()

    def _fmt_mac(self, mac_bytes):
        return ':'.join(f'{b:02x}' for b in mac_bytes)

    def _start(self):
        try:
            import network
            import espnow

            self.wlan = network.WLAN(network.STA_IF)
            self.wlan.active(True)

            self.own_mac = self.wlan.config('mac')
            self.lines.append("Own MAC (STA):")
            self.lines.append(f" {self._fmt_mac(self.own_mac)}")
            self.lines.append("")

            self.e = espnow.ESPNow()
            self.e.active(True)

            # Add broadcast peer so we can send without knowing the other device's MAC
            try:
                self.e.add_peer(self.BROADCAST_MAC)
                self.lines.append("Broadcast peer added")
            except Exception as ex:
                self.lines.append(f"Peer err: {ex}")

            self.lines.append("A=send  B=back")
            self.lines.append("")
        except Exception as ex:
            self.lines = [f"Start err: {ex}"]
            self.e = None
        gc.collect()

    def _stop(self):
        try:
            if self.e:
                self.e.active(False)
                self.e = None
        except Exception:
            pass
        try:
            if self.wlan:
                self.wlan.active(False)
                self.wlan = None
        except Exception:
            pass

    def _send_ping(self):
        if not self.e:
            self.lines.append("Not active")
            return
        try:
            self.send_count += 1
            msg = f"ping#{self.send_count}"
            self.e.send(self.BROADCAST_MAC, msg)
            self.lines.append(f">> {msg}")
        except Exception as ex:
            self.lines.append(f"Send err: {ex}")

    def update(self, dt):
        if not self.e:
            return None
        # Drain all pending received messages
        try:
            while True:
                host, msg = self.e.recv(0)
                if host is None:
                    break
                mac_str = self._fmt_mac(host)
                # Shorten MAC to last two octets for readability
                short_mac = mac_str[-5:]
                text = msg.decode('utf-8') if isinstance(msg, (bytes, bytearray)) else str(msg)
                self.lines.append(f"<{short_mac} {text}")
                # Auto-scroll to bottom when new message arrives
                max_scroll = max(0, len(self.lines) - self.LINES_VISIBLE)
                self.scroll_offset = max_scroll
        except Exception:
            pass
        return None

    def draw(self):
        visible_end = min(self.scroll_offset + self.LINES_VISIBLE, len(self.lines))
        for i, line in enumerate(self.lines[self.scroll_offset:visible_end]):
            self.renderer.draw_text(line[:21], 0, i * self.LINE_HEIGHT)
        if len(self.lines) > self.LINES_VISIBLE:
            self.scrollbar.draw(len(self.lines), self.LINES_VISIBLE, self.scroll_offset)

    def handle_input(self):
        max_scroll = max(0, len(self.lines) - self.LINES_VISIBLE)
        if self.input.was_just_pressed('up'):
            self.scroll_offset = max(0, self.scroll_offset - 1)
        if self.input.was_just_pressed('down'):
            self.scroll_offset = min(max_scroll, self.scroll_offset + 1)
        if self.input.was_just_pressed('a'):
            self._send_ping()
        if self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')
        return None
