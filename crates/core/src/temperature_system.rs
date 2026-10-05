// Deterministic temperature simulation
//
// Formula: seasonal_base + diurnal + weather_mod + slow_noise + day_noise + slot_noise
//   seasonal_base: cosine wave, trough at day 0 (winter), peak at day 182 (summer)
//   diurnal:       cosine wave, peak at 14:00, trough at 02:00
//   weather_mod:   per-weather adjustment (clouds/precip cool things down)
//   slow_noise:    seeded week-level variation (+/-3 C) for warm/cold spells
//   day_noise:     seeded day-level variation (+/-2 C)
//   slot_noise:    seeded 3h-slot variation (+/-1 C)
// Final value is clamped to [-5, 35] C.

use core::f32::consts::PI;

#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::time_system::Weather;

const SEASONAL_MEAN: f32 = 13.0;
const SEASONAL_AMP: f32 = 13.0;
const DIURNAL_AMP: f32 = 5.0;

const SLOW_NOISE_AMP: f32 = 3.0;
const DAY_NOISE_AMP: f32 = 2.0;
const SLOT_NOISE_AMP: f32 = 1.0;

const MIN_TEMP: f32 = -5.0;
const MAX_TEMP: f32 = 35.0;

fn weather_mod(weather: Weather) -> f32 {
    match weather {
        Weather::Clear => 2.0,
        Weather::Cloudy => -0.5,
        Weather::Overcast => -1.5,
        Weather::Windy => -1.0,
        Weather::Rain => -3.0,
        Weather::Storm => -4.0,
        Weather::Snow => -5.0,
    }
}

fn xorshift32(mut x: u32) -> u32 {
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

fn noise(seed: u32, amplitude: f32) -> f32 {
    let v = xorshift32(seed.wrapping_mul(2_654_435_761).wrapping_add(1));
    (v % 10001) as f32 / 10000.0 * 2.0 * amplitude - amplitude
}

pub fn get_temperature(
    day_number: u32,
    season_offset: u16,
    hour: u8,
    weather: Weather,
    pet_seed: u32,
) -> f32 {
    let d = (day_number + season_offset as u32) % 365;

    let seasonal = SEASONAL_MEAN - SEASONAL_AMP * (2.0 * PI * d as f32 / 365.0).cos();
    let diurnal = DIURNAL_AMP * (2.0 * PI * (hour as f32 - 14.0) / 24.0).cos();
    let wmod = weather_mod(weather);

    let week = day_number / 7;
    let slow_seed = pet_seed ^ week.wrapping_mul(1_234_567_891);
    let slow = noise(slow_seed, SLOW_NOISE_AMP);

    let day_seed = pet_seed ^ day_number.wrapping_mul(2_654_435_761);
    let day_noise = noise(day_seed, DAY_NOISE_AMP);

    let slot = (hour / 3) as u32;
    let slot_seed = day_seed ^ slot.wrapping_mul(1_234_567);
    let slot_noise = noise(slot_seed, SLOT_NOISE_AMP);

    let temp = seasonal + diurnal + wmod + slow + day_noise + slot_noise;
    temp.clamp(MIN_TEMP, MAX_TEMP)
}
