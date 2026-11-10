use heapless::Vec;

use crate::{
    context::GameContext,
    rand::xorshift32,
    time_system::{Season, Weather},
};

pub const FORECAST_MAX_ENTRIES: usize = 96;

#[derive(Clone, Copy)]
pub struct ForecastEntry {
    pub weather: Weather,
    pub duration_minutes: u32,
    pub meteor_shower: bool,
}

const SNOW_TEMP_THRESHOLD: f32 = 4.0;
const METEOR_SEED_OFFSET: u32 = 0x100000;
const METEOR_SHOWER_MIN_DURATION: u32 = 180;
const METEOR_SHOWER_MAX_DURATION: u32 = 300;

fn seeded_rand(step: u32) -> u32 {
    let mut x = step.wrapping_mul(2_654_435_761).wrapping_add(1);
    xorshift32(&mut x)
}

fn base_transitions(weather: Weather) -> &'static [Weather] {
    match weather {
        Weather::Clear => &[Weather::Clear, Weather::Cloudy, Weather::Windy],
        Weather::Cloudy => &[Weather::Cloudy, Weather::Clear, Weather::Overcast, Weather::Windy],
        Weather::Overcast => &[Weather::Overcast, Weather::Cloudy, Weather::Rain, Weather::Windy],
        Weather::Windy => &[Weather::Windy, Weather::Clear, Weather::Cloudy, Weather::Overcast],
        Weather::Rain => &[Weather::Rain, Weather::Overcast, Weather::Storm],
        Weather::Storm => &[Weather::Storm, Weather::Rain, Weather::Overcast],
        Weather::Snow => &[Weather::Snow, Weather::Cloudy, Weather::Overcast],
    }
}

fn seasonal_transitions(season: Season, weather: Weather) -> Option<&'static [Weather]> {
    use Season::*;
    use Weather::*;
    let opts: &'static [Weather] = match (season, weather) {
        (Spring, Clear) => &[Clear, Cloudy, Windy, Windy],
        (Spring, Cloudy) => &[Cloudy, Clear, Overcast, Overcast, Windy, Windy],
        (Spring, Overcast) => &[Overcast, Cloudy, Rain, Rain, Windy],
        (Spring, Rain) => &[Rain, Rain, Overcast, Storm],
        (Spring, Windy) => &[Windy, Windy, Clear, Cloudy, Overcast],

        (Summer, Clear) => &[Clear, Clear, Cloudy, Windy],
        (Summer, Cloudy) => &[Cloudy, Cloudy, Clear, Clear, Overcast, Windy],
        (Summer, Overcast) => &[Overcast, Cloudy, Cloudy, Rain, Windy],

        (Fall, Clear) => &[Clear, Cloudy, Cloudy, Windy, Windy],
        (Fall, Cloudy) => &[Cloudy, Cloudy, Clear, Overcast, Overcast, Windy, Windy],
        (Fall, Overcast) => &[Overcast, Overcast, Cloudy, Rain, Windy],
        (Fall, Windy) => &[Windy, Windy, Clear, Cloudy, Overcast],

        (Winter, Clear) => &[Clear, Cloudy, Cloudy, Windy],
        (Winter, Cloudy) => &[Cloudy, Cloudy, Clear, Overcast, Overcast, Windy],
        (Winter, Overcast) => &[Overcast, Overcast, Cloudy, Rain, Windy],

        _ => return None,
    };
    Some(opts)
}

fn duration_range(weather: Weather) -> (u32, u32) {
    match weather {
        Weather::Clear => (120, 300),
        Weather::Cloudy => (90, 240),
        Weather::Overcast => (60, 180),
        Weather::Windy => (60, 150),
        Weather::Rain => (60, 180),
        Weather::Storm => (30, 90),
        Weather::Snow => (90, 240),
    }
}

fn cold_season(season: Season) -> bool {
    matches!(season, Season::Fall | Season::Winter)
}

fn meteor_shower_chance(season: Season) -> u32 {
    match season {
        Season::Summer => 8,
        Season::Spring => 4,
        Season::Fall => 4,
        Season::Winter => 2,
    }
}

fn compute_meteor_shower(step: u32, season: Season) -> (bool, u32) {
    let mut x = seeded_rand(step.wrapping_add(METEOR_SEED_OFFSET));
    let chance = meteor_shower_chance(season);
    if (x % 100) < chance {
        let dur_rand = xorshift32(&mut x);
        let span = METEOR_SHOWER_MAX_DURATION - METEOR_SHOWER_MIN_DURATION + 1;
        (true, METEOR_SHOWER_MIN_DURATION + dur_rand % span)
    } else {
        (false, 0)
    }
}

fn compute_transition(
    step: u32,
    current: Weather,
    season: Season,
    temperature: f32,
) -> (Weather, u32) {
    let mut x = seeded_rand(step);

    let base = seasonal_transitions(season, current).unwrap_or_else(|| base_transitions(current));

    let cold_enough = temperature <= SNOW_TEMP_THRESHOLD;
    let extra_snow = current == Weather::Overcast && cold_season(season) && cold_enough;
    let total_len = base.len() + if extra_snow { 1 } else { 0 };
    let pick_idx = (x as usize) % total_len;
    let mut next = if pick_idx < base.len() {
        base[pick_idx]
    } else {
        Weather::Snow
    };

    if cold_enough && matches!(next, Weather::Rain | Weather::Storm) {
        next = Weather::Snow;
    }

    let dur_rand = xorshift32(&mut x);
    let (min_d, max_d) = duration_range(next);
    let duration = min_d + dur_rand % (max_d - min_d + 1);

    (next, duration)
}

/// Start this pet's weather at a seed-derived point in the shared sequence.
pub fn init_from_seed(ctx: &mut GameContext) {
    let step = (ctx.pet_seed >> 8) as u32;
    let (weather, duration) = compute_transition(step, Weather::Clear, ctx.season, ctx.temperature);
    ctx.weather = weather;
    ctx.weather_step = step.wrapping_add(1);
    ctx.weather_timer = duration as f32;
    ctx.meteor_shower_timer = 0.0;
}

pub struct WeatherSystem;

impl WeatherSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn update(&mut self, game_minutes: u32, ctx: &mut GameContext) {
        if game_minutes == 0 {
            return;
        }
        let gm = game_minutes as f32;
        let mut timer = ctx.weather_timer - gm;
        let mut shower_timer = (ctx.meteor_shower_timer - gm).max(0.0);

        while timer <= 0.0 {
            let step = ctx.weather_step;
            let (shower_start, shower_dur) = compute_meteor_shower(step, ctx.season);
            if shower_start {
                shower_timer = shower_timer.max(shower_dur as f32);
            }
            let (next_weather, duration) =
                compute_transition(step, ctx.weather, ctx.season, ctx.temperature);
            ctx.weather = next_weather;
            ctx.weather_step = step.wrapping_add(1);
            timer += duration as f32;
        }

        ctx.weather_timer = timer;
        ctx.meteor_shower_timer = shower_timer;
    }

    /// Build a deterministic forecast covering at least `hours` of future
    /// in-game time, starting with the current weather and its remaining
    /// duration.
    pub fn get_forecast(&self, ctx: &GameContext, hours: u32) -> Vec<ForecastEntry, FORECAST_MAX_ENTRIES> {
        let mut out: Vec<ForecastEntry, FORECAST_MAX_ENTRIES> = Vec::new();

        let mut current = ctx.weather;
        let mut step = ctx.weather_step;
        let season = ctx.season;
        let temperature = ctx.temperature;
        let remaining = ctx.weather_timer.max(0.0);
        let mut shower_timer = ctx.meteor_shower_timer.max(0.0);

        let _ = out.push(ForecastEntry {
            weather: current,
            duration_minutes: remaining as u32,
            meteor_shower: shower_timer > 0.0,
        });

        let mut total_minutes = remaining;
        let target_minutes = (hours * 60) as f32;

        // Consume the shower timer over the current weather's remaining window.
        shower_timer = (shower_timer - remaining).max(0.0);

        while total_minutes < target_minutes && !out.is_full() {
            let (shower_start, shower_dur) = compute_meteor_shower(step, season);
            if shower_start {
                shower_timer = shower_timer.max(shower_dur as f32);
            }
            let (next_weather, duration) =
                compute_transition(step, current, season, temperature);
            let _ = out.push(ForecastEntry {
                weather: next_weather,
                duration_minutes: duration,
                meteor_shower: shower_timer > 0.0,
            });
            total_minutes += duration as f32;
            current = next_weather;
            step = step.wrapping_add(1);
            shower_timer = (shower_timer - duration as f32).max(0.0);
        }

        out
    }
}
