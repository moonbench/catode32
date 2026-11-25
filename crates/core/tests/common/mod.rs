//! Shared test helpers. Each integration test file is its own crate, so
//! include this with `mod common;` and allow unused items.
#![allow(dead_code)]

pub mod maxed;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use catode32_core::{
    context::GameContext,
    ring::{Flash, SECTOR_SIZE, WORD},
};

/// Serialises tests that change the process working directory, and runs
/// each in a fresh temp dir so the real save files are never touched.
pub struct CwdGuard {
    prev: PathBuf,
    tmp: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CwdGuard {
    pub fn enter() -> Self {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _lock = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = env::current_dir().expect("get cwd");
        let mut tmp = env::temp_dir();
        tmp.push(format!(
            "catode32-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&tmp).expect("create tmp");
        env::set_current_dir(&tmp).expect("set cwd");
        Self { prev, tmp, _lock }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.prev);
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// RAM-backed NOR flash. Enforces the esp-storage alignment rules, counts
/// erases per sector, and can simulate power loss after a byte budget.
#[derive(Clone)]
pub struct MemFlash {
    pub data: Vec<u8>,
    pub erases: Vec<u32>,
    /// Bytes that may still be programmed before "power is lost". After
    /// that every write and erase fails, and the write in progress stops
    /// part-way through, as on real hardware.
    pub budget: Option<usize>,
}

impl MemFlash {
    pub fn new(sectors: usize) -> Self {
        Self {
            data: vec![0xFF; sectors * SECTOR_SIZE],
            erases: vec![0; sectors],
            budget: None,
        }
    }

    pub fn from_bytes(data: Vec<u8>) -> Self {
        let sectors = data.len() / SECTOR_SIZE;
        Self { data, erases: vec![0; sectors], budget: None }
    }

    fn dead(&self) -> bool {
        self.budget == Some(0)
    }
}

impl Flash for MemFlash {
    fn sectors(&self) -> usize {
        self.data.len() / SECTOR_SIZE
    }

    fn read(&mut self, offset: usize, buf: &mut [u8]) -> bool {
        assert_eq!(offset % WORD, 0, "unaligned read offset {offset}");
        assert_eq!(buf.len() % WORD, 0, "unaligned read length {}", buf.len());
        match self.data.get(offset..offset + buf.len()) {
            Some(src) => {
                buf.copy_from_slice(src);
                true
            }
            None => false,
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> bool {
        assert_eq!(offset % WORD, 0, "unaligned write offset {offset}");
        assert_eq!(data.len() % WORD, 0, "unaligned write length {}", data.len());
        if self.dead() {
            return false;
        }
        let allowed = self.budget.map_or(data.len(), |b| b.min(data.len()));
        let Some(dst) = self.data.get_mut(offset..offset + allowed) else {
            return false;
        };
        for (d, s) in dst.iter_mut().zip(data) {
            *d &= *s;
        }
        if let Some(b) = self.budget.as_mut() {
            *b -= allowed;
        }
        allowed == data.len()
    }

    fn erase(&mut self, sector: usize) -> bool {
        if self.dead() {
            return false;
        }
        let start = sector * SECTOR_SIZE;
        let Some(dst) = self.data.get_mut(start..start + SECTOR_SIZE) else {
            return false;
        };
        dst.fill(0xFF);
        self.erases[sector] += 1;
        true
    }
}

/// Assert every saved field of two contexts matches. `water_tol` covers
/// plant water debt and fertilizer, which v1 rounds (water to half hours,
/// fertilizer to whole units).
pub fn assert_same_cat(a: &GameContext, b: &GameContext, water_tol: f32) {
    assert_eq!(a.pet_seed, b.pet_seed, "pet_seed");
    assert_eq!(a.pet_name, b.pet_name, "pet_name");
    assert_eq!(a.pet_gender, b.pet_gender, "pet_gender");
    assert_eq!(a.star_sign, b.star_sign, "star_sign");
    assert_eq!(a.fav_weather, b.fav_weather, "fav_weather");
    assert_eq!(a.fav_meal, b.fav_meal, "fav_meal");
    assert_eq!(a.least_fav_meal, b.least_fav_meal, "least_fav_meal");
    assert_eq!(a.fav_snack, b.fav_snack, "fav_snack");
    assert_eq!(a.least_fav_snack, b.least_fav_snack, "least_fav_snack");
    assert_eq!(a.fav_toy, b.fav_toy, "fav_toy");
    assert_eq!(a.least_fav_toy, b.least_fav_toy, "least_fav_toy");
    assert_eq!(a.fav_location, b.fav_location, "fav_location");
    assert_eq!(a.least_fav_location, b.least_fav_location, "least_fav_location");

    let sa = [a.fullness, a.energy, a.comfort, a.playfulness, a.focus, a.fulfillment,
        a.cleanliness, a.curiosity, a.sociability, a.intelligence, a.maturity, a.affection,
        a.fitness, a.serenity, a.courage, a.loyalty, a.mischievousness];
    let sb = [b.fullness, b.energy, b.comfort, b.playfulness, b.focus, b.fulfillment,
        b.cleanliness, b.curiosity, b.sociability, b.intelligence, b.maturity, b.affection,
        b.fitness, b.serenity, b.courage, b.loyalty, b.mischievousness];
    assert_eq!(sa.map(f32::to_bits), sb.map(f32::to_bits), "stats (bit-exact)");
    assert_eq!(a.sickness.to_bits(), b.sickness.to_bits(), "sickness");
    assert_eq!(a.coins, b.coins, "coins");
    assert_eq!(a.time_speed.to_bits(), b.time_speed.to_bits(), "time_speed");

    assert_eq!(a.recent_meals, b.recent_meals, "recent_meals");
    assert_eq!(
        [a.milestone_fed, a.milestone_groomed, a.milestone_played, a.milestone_petted, a.milestone_store],
        [b.milestone_fed, b.milestone_groomed, b.milestone_played, b.milestone_petted, b.milestone_store],
        "milestones"
    );
    assert_eq!(a.food_stock, b.food_stock, "food_stock");
    assert_eq!(
        a.toys.iter().map(|t| (t.variant, t.durability)).collect::<Vec<_>>(),
        b.toys.iter().map(|t| (t.variant, t.durability)).collect::<Vec<_>>(),
        "toys"
    );
    assert_eq!(a.pots, b.pots, "pots");
    assert_eq!(a.seeds, b.seeds, "seeds");
    assert_eq!(a.tools, b.tools, "tools");
    assert_eq!(a.fertilizer, b.fertilizer, "fertilizer");
    assert_eq!(a.medicine, b.medicine, "medicine");
    assert_eq!(a.medicine_pending, b.medicine_pending, "medicine_pending");
    assert_eq!(
        a.friends.iter().map(|f| (f.mac, f.name.clone(), f.total_secs.to_bits(), f.visits)).collect::<Vec<_>>(),
        b.friends.iter().map(|f| (f.mac, f.name.clone(), f.total_secs.to_bits(), f.visits)).collect::<Vec<_>>(),
        "friends"
    );

    assert_eq!(
        (a.time_hours, a.time_minutes, a.day_number, a.season_offset, a.season, a.moon_phase),
        (b.time_hours, b.time_minutes, b.day_number, b.season_offset, b.season, b.moon_phase),
        "clock"
    );
    assert_eq!(a.weather, b.weather, "weather");
    assert_eq!(
        [a.temperature, a.weather_timer, a.meteor_shower_timer].map(f32::to_bits),
        [b.temperature, b.weather_timer, b.meteor_shower_timer].map(f32::to_bits),
        "weather floats"
    );
    assert_eq!(a.weather_step, b.weather_step, "weather_step");

    assert_eq!(a.plants.len(), b.plants.len(), "plant count");
    for (i, (p, q)) in a.plants.iter().zip(b.plants.iter()).enumerate() {
        assert_eq!(
            (p.id, p.seed, p.scene, p.layer, p.x, p.y_snap, p.pot, p.stage, p.age_hours, p.planted_day, p.mirror, p.aged),
            (q.id, q.seed, q.scene, q.layer, q.x, q.y_snap, q.pot, q.stage, q.age_hours, q.planted_day, q.mirror, q.aged),
            "plant {i}"
        );
        assert!((p.water_debt - q.water_debt).abs() <= water_tol, "plant {i} water {} vs {}", p.water_debt, q.water_debt);
        assert!((p.fertilizer - q.fertilizer).abs() <= water_tol, "plant {i} fert {} vs {}", p.fertilizer, q.fertilizer);
    }
    assert_eq!(a.next_plant_id, b.next_plant_id, "next_plant_id");

    assert_eq!(
        [a.zoomies_high_score, a.maze_best_time, a.snake_high_score, a.memory_best_score, a.meowltiply_high_score],
        [b.zoomies_high_score, b.maze_best_time, b.snake_high_score, b.memory_best_score, b.meowltiply_high_score],
        "scores"
    );
    assert_eq!(a.sudokat_difficulty, b.sudokat_difficulty, "sudokat_difficulty");
    assert_eq!(a.sudokat_givens, b.sudokat_givens, "sudokat_givens");
    assert_eq!(a.sudokat_entries, b.sudokat_entries, "sudokat_entries");

    let wifi = |c: &GameContext| {
        (
            c.wifi_familiar.iter().map(|e| (e.bssid, e.ssid.clone(), e.count.to_bits())).collect::<Vec<_>>(),
            c.wifi_recent.iter().map(|e| (e.bssid, e.ssid.clone(), e.count.to_bits())).collect::<Vec<_>>(),
        )
    };
    assert_eq!(wifi(a), wifi(b), "wifi");
    assert_eq!(a.scan_started, b.scan_started, "scan_started");
}
