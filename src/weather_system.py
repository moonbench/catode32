# weather_system.py - Deterministic weather progression via seeded Markov chain


def _xorshift32(x):
    """Simple xorshift PRNG for deterministic pseudo-random values"""
    x ^= (x << 13) & 0xFFFFFFFF
    x ^= (x >> 17)
    x ^= (x << 5) & 0xFFFFFFFF
    return x & 0xFFFFFFFF


def _seeded_rand(step):
    """Mix step into a well-distributed seed, then xorshift."""
    x = (step * 2654435761 + 1) & 0xFFFFFFFF
    return _xorshift32(x)


# Markov chain: each state lists its possible successors (including itself).
# Order matters for the weighted pick — all options are equally likely.
_TRANSITIONS = {
    "Clear":    ("Clear", "Cloudy", "Windy"),
    "Cloudy":   ("Cloudy", "Clear", "Overcast", "Windy"),
    "Overcast": ("Overcast", "Cloudy", "Rain", "Windy"),  # Snow appended in Fall/Winter
    "Windy":    ("Windy", "Clear", "Cloudy", "Overcast"),
    "Rain":     ("Rain", "Overcast", "Storm"),
    "Storm":    ("Storm", "Rain", "Overcast"),
    "Snow":     ("Snow", "Cloudy", "Overcast"),
}

# How long each weather state lasts, in in-game minutes (min, max inclusive).
_DURATION_RANGES = {
    "Clear":    (120, 300),
    "Cloudy":   ( 90, 240),
    "Overcast": ( 60, 180),
    "Windy":    ( 60, 150),
    "Rain":     ( 60, 180),
    "Storm":    ( 30,  90),
    "Snow":     ( 90, 240),
}

_COLD_SEASONS = ("Fall", "Winter")

# Meteor shower constants
# Large offset so shower PRNG doesn't correlate with weather PRNG at the same step
_METEOR_SEED_OFFSET = 0x100000

# Chance (%) that a meteor shower starts at any given weather transition, by season
_METEOR_SHOWER_CHANCE = {
    "Summer": 8,
    "Spring": 4,
    "Fall":   4,
    "Winter": 2,
}

# Shower duration range in in-game minutes (min >= one 3h forecast slot)
_METEOR_SHOWER_MIN_DURATION = 180
_METEOR_SHOWER_MAX_DURATION = 300


def _compute_meteor_shower(step, season):
    """
    Deterministically decide whether a meteor shower starts at this transition step.

    Returns (shower_active: bool, duration_minutes: int).
    Fully deterministic for a given (step, season).
    """
    x = _seeded_rand(step + _METEOR_SEED_OFFSET)
    chance = _METEOR_SHOWER_CHANCE.get(season, 4)
    if (x % 100) < chance:
        x = _xorshift32(x)
        span = _METEOR_SHOWER_MAX_DURATION - _METEOR_SHOWER_MIN_DURATION + 1
        duration = _METEOR_SHOWER_MIN_DURATION + (x % span)
        return True, duration
    return False, 0


def _compute_transition(step, current_weather, season):
    """
    Given a transition step index, current weather, and season, return
    (next_weather, duration_minutes) using the seeded PRNG.

    The result is fully deterministic for a given (step, current_weather, season).
    """
    x = _seeded_rand(step)

    options = _TRANSITIONS.get(current_weather, ("Clear",))
    if current_weather == "Overcast" and season in _COLD_SEASONS:
        # Extend with a mutable copy so Snow becomes possible
        options = options + ("Snow",)

    next_weather = options[x % len(options)]

    x = _xorshift32(x)
    min_d, max_d = _DURATION_RANGES.get(next_weather, (60, 180))
    duration = min_d + (x % (max_d - min_d + 1))

    return next_weather, duration


class WeatherSystem:
    """
    Manages automatic weather transitions over in-game time.

    State is stored entirely in context.environment so it persists across saves:
      - 'weather'       : current weather string
      - 'weather_step'  : int, global transition counter (seed for PRNG)
      - 'weather_timer' : float, in-game minutes remaining in current state

    Usage:
        ws = WeatherSystem()
        ws.update(game_minutes_elapsed, context.environment)
        forecast = ws.get_forecast(context.environment, hours=72)
    """

    def init_environment(self, environment, pet_seed):
        """
        Seed a fresh environment dict for a new game using pet_seed.

        Sets an initial weather_step offset so each pet's entire weather trajectory
        is unique, then derives the starting weather and timer from that step.
        The step range (0-16383) keeps us safely within the Markov chain's valid inputs.
        """
        # Use bits 8-21 of the seed as the starting step offset
        step = (pet_seed >> 8) & 0x3FFF
        season = environment.get('season', 'Summer')
        weather, duration = _compute_transition(step, 'Clear', season)
        environment['weather'] = weather
        environment['weather_step'] = step + 1
        environment['weather_timer'] = float(duration)
        environment['meteor_shower_timer'] = 0.0

    def update(self, game_minutes, environment):
        """
        Advance the weather simulation by game_minutes in-game minutes.

        If the current state's timer expires, transitions to the next state
        (possibly multiple times if game_minutes is large, e.g. after a save load).
        """
        if game_minutes <= 0:
            return

        timer = environment.get('weather_timer', 0.0)
        shower_timer = environment.get('meteor_shower_timer', 0.0) - game_minutes
        if shower_timer < 0:
            shower_timer = 0.0
        timer -= game_minutes

        while timer <= 0:
            step = environment.get('weather_step', 0)
            current = environment.get('weather', 'Clear')
            season = environment.get('season', 'Summer')
            shower_start, shower_dur = _compute_meteor_shower(step, season)
            if shower_start:
                shower_timer = max(shower_timer, float(shower_dur))
            next_weather, duration = _compute_transition(step, current, season)
            environment['weather'] = next_weather
            environment['weather_step'] = step + 1
            timer += duration

        environment['weather_timer'] = timer
        environment['meteor_shower_timer'] = shower_timer

    def get_forecast(self, environment, hours=72):
        """
        Return a deterministic weather forecast for the next `hours` in-game hours.

        Returns a list of (weather, duration_minutes, meteor_shower) tuples. The first
        entry is the current weather with its remaining time; subsequent entries are
        future states. The list covers at least `hours * 60` minutes of future time.
        """
        current = environment.get('weather', 'Clear')
        step = environment.get('weather_step', 0)
        remaining = environment.get('weather_timer', 60.0)
        season = environment.get('season', 'Summer')
        shower_timer = float(environment.get('meteor_shower_timer', 0.0))

        forecast = [(current, int(remaining), shower_timer > 0)]
        total_minutes = remaining
        target_minutes = hours * 60

        # Advance shower timer past the current weather's remaining duration
        shower_timer = max(0.0, shower_timer - remaining)

        while total_minutes < target_minutes:
            shower_start, shower_dur = _compute_meteor_shower(step, season)
            if shower_start:
                shower_timer = max(shower_timer, float(shower_dur))
            next_weather, duration = _compute_transition(step, current, season)
            forecast.append((next_weather, duration, shower_timer > 0))
            total_minutes += duration
            current = next_weather
            step += 1
            shower_timer = max(0.0, shower_timer - duration)

        return forecast
