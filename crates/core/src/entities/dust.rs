//! Dust motes drifting in a beam of light from a window. The beam fans out
//! from a virtual light source behind the window, angled by where the sun or
//! moon is (`sky::SkyLight`). Motes that drift out of the beam vanish and new
//! ones drift in from its edges; how many there are follows the light's
//! strength, so clouds and a thin moon thin out the dust.
//!
//! Coordinates are world pixels in whatever layer the window is drawn on;
//! `draw` takes that layer's camera offset.

use heapless::Vec;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;

use crate::{
    rand::{rand_bool, rand_range_f32, xorshift32},
    render::Renderer,
    sky::SkyLight,
};

pub const MAX_DUST: usize = 28;

/// How far above the window's centre the virtual light source sits. Closer
/// means a wider fan.
const SOURCE_RISE: f32 = 48.0;
/// Horizontal run per pixel of drop when the light comes straight off the
/// horizon. Higher bodies cast steeper beams.
const MAX_SLANT: f32 = 2.0;

const SPEED_MIN: f32 = 0.25;
const SPEED_MAX: f32 = 0.7;
const DIR_MIN: f32 = 6.0;
const DIR_MAX: f32 = 14.0;
const WOBBLE_AMP: f32 = 0.6;
const WOBBLE_SPEED: f32 = 0.35;

/// Seconds between motes drifting in while the beam is under-populated.
const SPAWN_INTERVAL: f32 = 1.0;
/// Seconds between motes fading out while it is over-populated.
const CULL_INTERVAL: f32 = 1.0;

#[derive(Clone, Copy)]
struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    dir_timer: f32,
    wobble: f32,
}

/// The beam: every ray from (sx, sy) that passes through the window.
#[derive(Clone, Copy, Debug)]
struct Beam {
    sx: f32,
    sy: f32,
    /// dx/dy of the left- and right-most rays through the window.
    m_lo: f32,
    m_hi: f32,
}

pub struct DustBeam {
    win_x: f32,
    win_y: f32,
    win_w: f32,
    win_h: f32,
    floor_y: f32,
    beam: Option<Beam>,
    target: usize,
    motes: Vec<Mote, MAX_DUST>,
    rng: u32,
    spawn_timer: f32,
    cull_timer: f32,
}

impl DustBeam {
    /// A beam through the window at (`x`, `y`) of `w` x `h`, falling as far
    /// as `floor_y`.
    pub fn new(x: i32, y: i32, w: i32, h: i32, floor_y: i32, seed: u32) -> Self {
        Self {
            win_x: x as f32,
            win_y: y as f32,
            win_w: w as f32,
            win_h: h as f32,
            floor_y: floor_y as f32,
            beam: None,
            target: 0,
            motes: Vec::new(),
            rng: seed | 1,
            spawn_timer: 0.0,
            cull_timer: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.motes.len()
    }

    pub fn target(&self) -> usize {
        self.target
    }

    /// Re-aim the beam and set how many motes it should hold. `None` (night
    /// with a new moon, a storm) lets the existing motes fade away.
    pub fn set_light(&mut self, light: Option<SkyLight>) {
        let Some(light) = light else {
            self.target = 0;
            return;
        };
        self.target = ((MAX_DUST as f32 * light.strength) + 0.5) as usize;
        self.target = self.target.min(MAX_DUST);

        let slant = (light.dir_x * (1.0 - 0.6 * light.elevation) * MAX_SLANT)
            .clamp(-MAX_SLANT, MAX_SLANT);
        let cx = self.win_x + self.win_w / 2.0;
        let cy = self.win_y + self.win_h / 2.0;
        let sx = cx - slant * SOURCE_RISE;
        let sy = cy - SOURCE_RISE;
        let mut m_lo = f32::MAX;
        let mut m_hi = f32::MIN;
        for (px, py) in [
            (self.win_x, self.win_y),
            (self.win_x + self.win_w, self.win_y),
            (self.win_x, self.win_y + self.win_h),
            (self.win_x + self.win_w, self.win_y + self.win_h),
        ] {
            let m = (px - sx) / (py - sy);
            m_lo = m_lo.min(m);
            m_hi = m_hi.max(m);
        }
        self.beam = Some(Beam { sx, sy, m_lo, m_hi });
    }

    /// Scatter motes through the beam up to the target, so a room entered
    /// mid-day doesn't start with an empty beam.
    pub fn fill(&mut self) {
        self.motes.clear();
        while self.motes.len() < self.target {
            match self.random_inside() {
                Some(m) => {
                    let _ = self.motes.push(m);
                }
                None => break,
            }
        }
    }

    /// Whether (x, y) is lit: on or below the window, above the floor, and on
    /// a ray from the source that passed through the window to get there.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let Some(b) = self.beam else { return false };
        if y < self.win_y || y > self.floor_y {
            return false;
        }
        let dy = y - b.sy;
        if dy <= 0.0 {
            return false;
        }
        let m = (x - b.sx) / dy;
        // Where this ray crosses the window's rows, up to the point itself.
        let y0 = self.win_y;
        let y1 = (self.win_y + self.win_h).min(y);
        let x0 = b.sx + m * (y0 - b.sy);
        let x1 = b.sx + m * (y1 - b.sy);
        let (lo, hi) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        hi >= self.win_x && lo <= self.win_x + self.win_w
    }

    fn random_velocity(rng: &mut u32) -> (f32, f32) {
        let spd = rand_range_f32(rng, SPEED_MIN, SPEED_MAX);
        let a = rand_range_f32(rng, 0.0, core::f32::consts::PI * 2.0);
        (a.cos() * spd, a.sin() * spd)
    }

    fn make_mote(&mut self, x: f32, y: f32, vx: f32, vy: f32) -> Mote {
        Mote {
            x,
            y,
            vx,
            vy,
            dir_timer: rand_range_f32(&mut self.rng, DIR_MIN, DIR_MAX),
            wobble: rand_range_f32(&mut self.rng, 0.0, core::f32::consts::PI * 2.0),
        }
    }

    /// A point on the ray `m` at row `y`, if that point is lit.
    fn point_on_ray(&self, m: f32, y: f32) -> Option<(f32, f32)> {
        let b = self.beam?;
        let x = b.sx + m * (y - b.sy);
        self.contains(x, y).then_some((x, y))
    }

    fn random_inside(&mut self) -> Option<Mote> {
        let b = self.beam?;
        for _ in 0..8 {
            let m = rand_range_f32(&mut self.rng, b.m_lo, b.m_hi);
            let y = rand_range_f32(&mut self.rng, self.win_y, self.floor_y);
            if let Some((x, y)) = self.point_on_ray(m, y) {
                let (vx, vy) = Self::random_velocity(&mut self.rng);
                return Some(self.make_mote(x, y, vx, vy));
            }
        }
        None
    }

    /// A mote just inside one of the beam's edges, drifting inward.
    fn random_entering(&mut self) -> Option<Mote> {
        let b = self.beam?;
        let inset = (b.m_hi - b.m_lo) * 0.03;
        for _ in 0..8 {
            let left = rand_bool(&mut self.rng, 0.5);
            let m = if left { b.m_lo + inset } else { b.m_hi - inset };
            let y = rand_range_f32(&mut self.rng, self.win_y, self.floor_y);
            if let Some((x, y)) = self.point_on_ray(m, y) {
                let spd = rand_range_f32(&mut self.rng, SPEED_MIN, SPEED_MAX);
                let vx = if left { spd } else { -spd };
                let vy = rand_range_f32(&mut self.rng, -SPEED_MIN, SPEED_MIN);
                return Some(self.make_mote(x, y, vx, vy));
            }
        }
        None
    }

    pub fn update(&mut self, dt: f32) {
        // Drift, retarget, and drop whatever leaves the light.
        let mut i = self.motes.len();
        while i > 0 {
            i -= 1;
            let mut m = self.motes[i];
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.wobble += dt * WOBBLE_SPEED;
            m.dir_timer -= dt;
            if m.dir_timer <= 0.0 {
                let (vx, vy) = Self::random_velocity(&mut self.rng);
                m.vx = vx;
                m.vy = vy;
                m.dir_timer = rand_range_f32(&mut self.rng, DIR_MIN, DIR_MAX);
            }
            if self.contains(m.x, m.y) {
                self.motes[i] = m;
            } else {
                self.motes.swap_remove(i);
            }
        }

        // Dimming light: fade motes out one at a time.
        if self.motes.len() > self.target {
            self.cull_timer += dt;
            if self.cull_timer >= CULL_INTERVAL {
                self.cull_timer = 0.0;
                let idx = (xorshift32(&mut self.rng) as usize) % self.motes.len();
                self.motes.swap_remove(idx);
            }
        } else {
            self.cull_timer = 0.0;
        }

        // Too few: let one drift in from an edge.
        if self.motes.len() < self.target {
            self.spawn_timer += dt;
            if self.spawn_timer >= SPAWN_INTERVAL {
                self.spawn_timer = 0.0;
                if let Some(m) = self.random_entering() {
                    let _ = self.motes.push(m);
                }
            }
        } else {
            self.spawn_timer = 0.0;
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, camera_offset: i32) {
        use embedded_graphics::prelude::Point;
        for m in &self.motes {
            let x = m.x + m.wobble.sin() * WOBBLE_AMP;
            renderer.draw_pixel(Point::new(x as i32 - camera_offset, m.y as i32), true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sky::sky_light, time_system::Weather};

    const WX: i32 = 30;
    const WY: i32 = 4;
    const WW: i32 = 40;
    const WH: i32 = 22;
    const FLOOR: i32 = 60;

    fn beam_at(hours: f32, phase: u8, weather: Weather) -> DustBeam {
        let mut d = DustBeam::new(WX, WY, WW, WH, FLOOR, 0xC0FFEE);
        d.set_light(sky_light(hours, phase, weather));
        d.fill();
        d
    }

    /// Lit span of the floor row as (leftmost, rightmost) x.
    fn floor_span(d: &DustBeam) -> (i32, i32) {
        let y = (FLOOR - 1) as f32;
        let xs: std::vec::Vec<i32> =
            (-200..400).filter(|&x| d.contains(x as f32, y)).collect();
        (*xs.first().unwrap(), *xs.last().unwrap())
    }

    #[test]
    fn morning_beam_falls_to_the_right() {
        let d = beam_at(8.0, 0, Weather::Clear);
        let (lo, hi) = floor_span(&d);
        let centre = WX + WW / 2;
        assert!((lo + hi) / 2 > centre, "floor span {lo}..{hi}");
    }

    #[test]
    fn evening_beam_falls_to_the_left() {
        let d = beam_at(16.0, 0, Weather::Clear);
        let (lo, hi) = floor_span(&d);
        let centre = WX + WW / 2;
        assert!((lo + hi) / 2 < centre, "floor span {lo}..{hi}");
    }

    #[test]
    fn beam_fans_out_wider_than_the_window() {
        let d = beam_at(12.0, 0, Weather::Clear);
        let (lo, hi) = floor_span(&d);
        assert!(hi - lo > WW, "floor span {lo}..{hi}");
    }

    #[test]
    fn wall_beside_the_window_is_not_lit() {
        let d = beam_at(8.0, 0, Weather::Clear);
        // Light heads right, so the wall left of the window stays dark.
        assert!(!d.contains((WX - 3) as f32, (WY + WH / 2) as f32));
        assert!(!d.contains((WX + WW / 2) as f32, (WY - 2) as f32));
    }

    #[test]
    fn dark_skies_have_no_dust() {
        assert_eq!(beam_at(23.0, 0, Weather::Clear).len(), 0);
        assert_eq!(beam_at(12.0, 4, Weather::Storm).len(), 0);
    }

    #[test]
    fn full_moon_has_less_dust_than_clear_noon() {
        let moon = beam_at(23.0, 4, Weather::Clear);
        let noon = beam_at(12.0, 4, Weather::Clear);
        assert!(moon.len() > 0);
        assert!(moon.len() < noon.len());
        assert_eq!(noon.len(), MAX_DUST);
    }

    #[test]
    fn motes_stay_lit_and_count_holds() {
        let mut d = beam_at(9.5, 0, Weather::Clear);
        for _ in 0..3000 {
            d.update(1.0 / 30.0);
            assert!(d.len() <= d.target());
            for m in &d.motes {
                assert!(d.contains(m.x, m.y));
            }
        }
        // Spawning keeps the beam close to full despite motes drifting out.
        assert!(d.len() + 3 >= d.target(), "{} of {}", d.len(), d.target());
    }

    #[test]
    fn clouds_thin_out_the_dust() {
        let mut d = beam_at(12.0, 0, Weather::Clear);
        assert_eq!(d.len(), MAX_DUST);
        d.set_light(sky_light(12.0, 0, Weather::Overcast));
        for _ in 0..1800 {
            d.update(1.0 / 30.0);
        }
        assert!(d.len() <= d.target());
        d.set_light(None);
        for _ in 0..1800 {
            d.update(1.0 / 30.0);
        }
        assert_eq!(d.len(), 0);
    }
}
