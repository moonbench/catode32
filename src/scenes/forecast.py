from scene import Scene
from weather_system import WeatherSystem
from assets.icons import (
    WEATHER_CLEAR, WEATHER_CLOUDY, WEATHER_OVERCAST,
    WEATHER_RAIN, WEATHER_STORM, WEATHER_SNOW, WEATHER_WINDY,
    WEATHER_CLEAR_NIGHT, WEATHER_CLOUDY_NIGHT, WEATHER_OVERCAST_NIGHT,
)

_WEATHER_SYSTEM = WeatherSystem()

_COL_W = 26            # pixels per column
_LABEL_Y = 13          # y of time label
_ICON_Y = 25           # y of 15x15 weather icon
_ICON_X_OFF = (_COL_W - 15) // 2   # = 5, centers icon horizontally in column
_NUM_SLOTS = 24        # 24 slots × 3h = 72-hour forecast
_INTERVAL_H = 3        # hours between slots
_VIS_COLS = 128 // _COL_W           # = 4 full columns visible


def _fmt_hour(h):
    if h == 0:
        return "12A"
    elif h < 12:
        return "%dA" % h
    elif h == 12:
        return "12P"
    else:
        return "%dP" % (h - 12)


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
    return _DAY_ICONS.get(weather, WEATHER_CLEAR)


def _build_slots(forecast, cur_hour, cur_min):
    """Convert (weather, duration_minutes) forecast into _NUM_SLOTS 3-hour slots."""
    slots = []
    for i in range(_NUM_SLOTS):
        min_offset = i * _INTERVAL_H * 60
        hour = (cur_hour + (cur_min + min_offset) // 60) % 24
        # Walk the forecast timeline to find the weather at this offset
        cumul = 0
        weather = forecast[0][0] if forecast else "Clear"
        for w, dur in forecast:
            cumul += dur
            weather = w
            if min_offset < cumul:
                break
        slots.append((hour, weather))
    return slots


# Highlight rectangle bounds (covers label + icon)
_HL_Y = 11
_HL_H = _ICON_Y + 17 - _HL_Y   # top of rect to bottom of icon


class ForecastScene(Scene):
    """Horizontally-scrollable 3-hourly weather forecast (72 hours)."""

    MODULES_TO_KEEP = ['weather_system']

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
            return ('change_scene', 'normal')
        return None

    def draw(self):
        self.renderer.clear()

        # Header: name of the highlighted slot's weather
        _, sel_weather = self._slots[self._cursor]
        self.renderer.draw_text(sel_weather, 0, 0)
        self.renderer.draw_line(0, 9, 127, 9)

        # Hourly columns — one extra to fill partial column at the right edge
        for i, (hour, weather) in enumerate(
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

        # Highlight rectangle around the cursor column
        hl_x = (self._cursor - self._scroll) * _COL_W
        self.renderer.draw_rect(hl_x, _HL_Y, _COL_W, _HL_H)

        # Scroll indicators at bottom of screen
        if self._scroll > 0:
            self.renderer.draw_text("<", 0, 55)
        if self._scroll + _VIS_COLS < len(self._slots):
            self.renderer.draw_text(">", 120, 55)
