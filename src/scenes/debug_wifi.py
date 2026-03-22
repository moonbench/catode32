import gc
import sys
import config
from scene import Scene
from ui import Scrollbar

_AUTH_MODES = {0: 'Open', 1: 'WEP', 2: 'WPA', 3: 'WPA2', 4: 'WPA/2', 6: 'WPA3'}


class DebugWifiScene(Scene):
    """Debug scene showing familiar/recent AP lists and live scan results."""

    LINES_VISIBLE = 8
    LINE_HEIGHT = 8

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.scrollbar = Scrollbar(renderer)
        self.scroll_offset = 0
        self.lines = []
        self._scan_countdown = None

    def load(self):
        super().load()

    def unload(self):
        super().unload()

    def enter(self):
        self.scroll_offset = 0
        if config.WIFI_ENABLED:
            self.lines = ["Scanning WiFi..."]
            self._scan_countdown = 0.1  # seconds before scan fires
        else:
            self.lines = ["WiFi disabled.", "Set WIFI_ENABLED=True", "in config.py to use."]
            self._scan_countdown = None

    def exit(self):
        pass

    def _scan(self):
        self.lines = ["Scanning WiFi..."]
        ctx = self.context
        try:
            import wifi_tracker
            raw_aps = wifi_tracker.scan_now(ctx)
            del sys.modules['wifi_tracker']
        except Exception as e:
            self.lines = ["WiFi error:", " " + str(e)]
            gc.collect()
            return

        fam = ctx.wifi_familiar
        rec = ctx.wifi_recent

        # BSSIDs visible in this scan (for marking entries)
        visible = set()
        for ap in raw_aps:
            try:
                visible.add(':'.join('%02x' % b for b in ap[1]))
            except Exception:
                pass

        fam_bssids = {e['b'] for e in fam}
        rec_bssids = {e['b'] for e in rec}
        loc = "Y" if ctx.in_familiar_location else "N"

        lines = []

        # --- Familiar ---
        lines.append("Home? %s" % loc)
        lines.append("")
        lines.append("Familiar:%d/%d" % (len(fam), 16))
        if fam:
            for e in sorted(fam, key=lambda x: -x['n']):
                vis = "*" if e['b'] in visible else " "
                label = (e['s'] or e['b'])[:10]
                lines.append("%s%-10s %4.1f" % (vis, label, e['n']))
        else:
            lines.append("(none yet)")

        # --- Recent ---
        lines.append("")
        lines.append("Recent:%d/%d" % (len(rec), 8))
        if rec:
            for e in sorted(rec, key=lambda x: -x['n']):
                vis = "*" if e['b'] in visible else " "
                label = (e['s'] or e['b'])[:10]
                lines.append("%s%-10s %4.1f" % (vis, label, e['n']))
        else:
            lines.append("(none yet)")

        # --- Live scan ---
        lines.append("")
        lines.append("Scan: %d APs" % len(raw_aps))
        for ap in sorted(raw_aps, key=lambda x: -x[3]):
            try:
                bssid = ':'.join('%02x' % b for b in ap[1])
                try:
                    ssid = ap[0].decode('utf-8') if ap[0] else '(hidden)'
                except Exception:
                    ssid = '(binary)'
                rssi = ap[3]
                auth = _AUTH_MODES.get(ap[4], '?')
                if bssid in fam_bssids:
                    marker = "F"
                elif bssid in rec_bssids:
                    marker = "R"
                else:
                    marker = "."
                lines.append("%s %-11s%4d %s" % (marker, ssid[:11], rssi, auth))
            except Exception:
                pass

        self.lines = lines

        # Mirror to terminal
        print("[WiFi Debug] " + "=" * 38)
        for line in lines:
            print("[WiFi Debug] " + line)
        print("[WiFi Debug] " + "=" * 38)

        gc.collect()

    def update(self, dt):
        if self._scan_countdown is not None:
            self._scan_countdown -= dt
            if self._scan_countdown <= 0:
                self._scan_countdown = None
                self._scan()
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
        if self.input.was_just_pressed('a') and config.WIFI_ENABLED:
            self.scroll_offset = 0
            self._scan()
        if self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')
        return None
