from lang import t
from scene import Scene
from weather_system import WeatherSystem
from assets.icons import (
    WEATHER_CLEAR, WEATHER_CLOUDY, WEATHER_OVERCAST,
    WEATHER_RAIN, WEATHER_STORM, WEATHER_SNOW, WEATHER_WINDY,
    WEATHER_CLEAR_NIGHT, WEATHER_CLOUDY_NIGHT, WEATHER_OVERCAST_NIGHT,
)
try:
    from assets.icons import METEOR_SHOWER
except ImportError:
    METEOR_SHOWER = None

_WEATHER_SYSTEM = WeatherSystem()

_COL_W = 26            # pixels per column
_LABEL_Y = 13          # y of time label
_ICON_Y = 25           # y of 15x15 weather icon
_ICON_X_OFF = (_COL_W - 15) // 2   # = 5, centers icon horizontally in column
_METEOR_Y = 43         # y of 13x13 meteor shower icon (below weather icon)
_METEOR_X_OFF = (_COL_W - 13) // 2  # = 6, centers 13px icon in column
_NUM_SLOTS = 24        # 24 slots × 3h = 72-hour forecast
_INTERVAL_H = 3        # hours between slots
_VIS_COLS = 128 // _COL_W           # = 4 full columns visible


def _fmt_hour(h):
    if h == 0:
        return t("12A")
    elif h < 12:
        return t("{h}A", h=h)
    elif h == 12:
        return t("12P")
    else:
        return t("{h}P", h=(h - 12))


_NIGHT_ICONS = {
    "Clear":    WEATHER_CLEAR_NIGHT,
    "Cloudy":   WEATHER_CLOUDY_NIGHT,
    "Overcast": WEATHER_OVERCAST_NIGHT,
}

_DAY_ICONS = {
    "Clear":    WEATHER_CLEAR,
    "Cloudy":   WEATHER_CLOUDY,
    "Overcast": WEATHER_OVERCAST,
    "Rain":     WEATHER_RAIN,
    "Storm":    WEATHER_STORM,
    "Snow":     WEATHER_SNOW,
    "Windy":    WEATHER_WINDY,
}


def _get_icon(weather, hour):
    is_night = hour >= 20 or hour < 6
    if is_night and weather in _NIGHT_ICONS:
        return _NIGHT_ICONS[weather]
    return _DAY_ICONS[weather]


def _build_slots(forecast, cur_hour, cur_min):
    """Build _NUM_SLOTS slots aligned to absolute 3-hour boundaries (12a, 3a, 6a, ...).

    Slot 0 is the 3-hour bin containing the current time.  The shower flag is
    True if *any* forecast transition within the slot's window has a shower.
    """
    cur_total = cur_hour * 60 + cur_min
    slot0_start = (cur_total // (_INTERVAL_H * 60)) * (_INTERVAL_H * 60)

    slots = []
    for i in range(_NUM_SLOTS):
        # Offset in minutes from now to this slot's boundary
        slot_start = slot0_start + i * _INTERVAL_H * 60 - cur_total
        slot_end   = slot_start + _INTERVAL_H * 60
        eff_start  = max(0, slot_start)   # can't sample before the present
        slot_hour  = ((slot0_start // 60) + i * _INTERVAL_H) % 24

        cumul   = 0
        weather = forecast[0][0] if forecast else "Clear"
        shower  = False
        for w, dur, sh in forecast:
            next_cumul = cumul + dur
            if next_cumul > eff_start and cumul < slot_end:
                if cumul <= eff_start:
                    weather = w         # dominant weather at the slot's effective start
                if sh:
                    shower = True       # any shower active in this window counts
            cumul = next_cumul
            if cumul >= slot_end:
                break

        slots.append((slot_hour, weather, shower))
    return slots


# Highlight rectangle bounds (covers label + icon)
_HL_Y = 12
_HL_H = _ICON_Y + 17 - _HL_Y   # top of rect to bottom of icon


class ForecastScene(Scene):
    """Horizontally-scrollable 3-hourly weather forecast (72 hours)."""

    def __init__(self, context, renderer, input_handler):
        super().__init__(context, renderer, input_handler)
        self._slots = []
        self._cursor = 0   # index of the highlighted slot
        self._scroll = 0   # index of the leftmost visible slot

    def enter(self):
        forecast = _WEATHER_SYSTEM.get_forecast(self.context.environment, hours=72)
        cur_hour = self.context.environment.get('time_hours', 12)
        cur_min = int(self.context.environment.get('time_minutes', 0))
        self._slots = _build_slots(forecast, cur_hour, cur_min)
        self._cursor = 0
        self._scroll = 0

    def handle_input(self):
        if self.input.was_just_pressed('left'):
            if self._cursor > 0:
                self._cursor -= 1
                if self._cursor < self._scroll:
                    self._scroll = self._cursor
        elif self.input.was_just_pressed('right'):
            if self._cursor < len(self._slots) - 1:
                self._cursor += 1
                if self._cursor >= self._scroll + _VIS_COLS:
                    self._scroll = self._cursor - _VIS_COLS + 1
        elif self.input.was_just_pressed('a') or self.input.was_just_pressed('b'):
            return ('change_scene', 'last_main')
        return None

    def draw(self):

        # Header: name of the highlighted slot's weather
        _, sel_weather, _ = self._slots[self._cursor]
        self.renderer.draw_text(t(sel_weather), 0, 0)
        self.renderer.draw_line(0, 9, 127, 9)

        # Hourly columns — one extra to fill partial column at the right edge
        for i, (hour, weather, shower) in enumerate(
                self._slots[self._scroll: self._scroll + _VIS_COLS + 1]):
            col_x = i * _COL_W

            # Time label centred in column
            label = _fmt_hour(hour)
            label_x = col_x + (_COL_W - len(label) * 8) // 2
            self.renderer.draw_text(label, label_x, _LABEL_Y)

            # Weather icon (15×15)
            self.renderer.draw_sprite(
                _get_icon(weather, hour), 15, 15,
                col_x + _ICON_X_OFF, _ICON_Y,
            )

            # Meteor shower icon (13×13) — only during clear-sky night slots with a shower
            is_night = hour >= 20 or hour < 6
            sky_clear = weather in ("Clear", "Cloudy", "Windy")
            if shower and is_night and sky_clear and METEOR_SHOWER is not None:
                self.renderer.draw_sprite(
                    METEOR_SHOWER, 13, 13,
                    col_x + _METEOR_X_OFF, _METEOR_Y,
                )

        # Highlight rectangle around the cursor column
        hl_x = (self._cursor - self._scroll) * _COL_W
        self.renderer.draw_rect(hl_x, _HL_Y, _COL_W, _HL_H)

        # Scroll indicators at bottom of screen
        if self._scroll > 0:
            self.renderer.draw_text("<", 0, 55)
        if self._scroll + _VIS_COLS < len(self._slots):
            self.renderer.draw_text(">", 120, 55)
