"""time_system.py - Simulated in-game time and calendar management."""

_MOON_PHASES = (
    "New", "Wax Cres", "1st Qtr", "Wax Gib",
    "Full", "Wan Gib", "3rd Qtr", "Wan Cres",
)


class TimeSystem:
    """Advances simulated in-game time and keeps the moon phase current."""

    def __init__(self, game_minutes_per_second=1.0):
        self._accumulator = 0.0
        self.game_minutes_per_second = game_minutes_per_second

    def advance(self, dt, environment, weather_system=None):
        """Advance time by dt real seconds. Calls weather_system.update if provided."""
        self._accumulator += dt * self.game_minutes_per_second
        if self._accumulator < 1.0:
            return

        mins = int(self._accumulator)
        self._accumulator -= mins

        total_minutes = environment.get('time_minutes', 0) + mins
        old_hours = environment.get('time_hours', 12)
        new_hours_raw = old_hours + total_minutes // 60
        environment['time_hours'] = new_hours_raw % 24
        environment['time_minutes'] = total_minutes % 60

        if new_hours_raw >= 24:
            environment['day_number'] = environment.get('day_number', 0) + (new_hours_raw // 24)
            self.update_moon_phase(environment)

        if weather_system:
            weather_system.update(mins, environment)

    def update_moon_phase(self, environment):
        """Recompute and store the current moon phase in the environment dict."""
        day = environment.get('day_number', 0)
        environment['moon_phase'] = _MOON_PHASES[(day // 6 + 2) % 8]
