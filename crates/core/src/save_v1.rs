//! Save format v1: compact, tagged, upgrade-safe.
//!
//! A snapshot payload is a sequence of sections:
//!
//! ```text
//! [section id u8][body len u16 LE][crc16 u16 LE][body]
//! ```
//!
//! The CRC covers the id, the length and the body, so a damaged section is
//! detected on its own and the loader can take that section from an older
//! snapshot instead. A body is a sequence of tagged fields:
//!
//! ```text
//! [tag u8][value len u16 LE][value]
//! ```
//!
//! Lists are a value of `[count u8]` followed by `count` records, each
//! `[record len u8][record bytes]`.
//!
//! # Compatibility rules (never break an existing cat)
//!
//! 1. Never reuse or renumber a section id, tag, or enum id below. Retired
//!    numbers stay reserved forever.
//! 2. Never change what a field means or how it is encoded. Add a new tag
//!    instead.
//! 3. Fixed-layout values (stats, clock, records) only ever grow at the end.
//!    Readers stop at the end of the bytes they were given and leave any
//!    field they didn't get at its `GameContext::new` default; older readers
//!    ignore trailing bytes they don't know.
//! 4. Unknown sections and tags are skipped, so older firmware can still
//!    load a newer save.
//! 5. Missing fields keep the `GameContext::new` value, so those defaults
//!    must always be safe (never a starving or dead cat).
//!
//! Plant water debt is stored in half hours and fertilizer in whole units,
//! rounded to nearest. That's off by at most 15 game minutes of water debt
//! and 0.5 fertilizer (about 2 game hours of decay), and keeps both to one
//! byte for most plants. Everything else is stored exactly.

use heapless::{String, Vec};

use crate::{
    assets::plants::{PlantStage, PotKind},
    context::{
        FavWeather, FoodItem, FriendEntry, GameContext, MealEntry, PotSize, SeedKind, ToolKind,
        ToyEntry, ToyVariant, WifiEntry, PET_NAME_MAX,
    },
    pet_seed::{PetGender, StarSign},
    plant_system::{Plant, PlantLayer},
    ring::crc16_update,
    scene::SceneId,
    time_system::{Season, Weather},
};

// ---------------------------------------------------------------------------
// Section ids and tags. FROZEN: see the compatibility rules above.
// ---------------------------------------------------------------------------

pub const SECTION_PET: u8 = 1;
pub const SECTION_WORLD: u8 = 2;

mod pet {
    pub const SEED: u8 = 1;
    pub const NAME: u8 = 2;
    pub const STATS: u8 = 3;
    pub const SICKNESS: u8 = 4;
    pub const COINS: u8 = 5;
    pub const IDENTITY: u8 = 6;
    pub const RECENT_MEALS: u8 = 7;
    pub const MILESTONES: u8 = 8;
    pub const FOOD_STOCK: u8 = 9;
    pub const TOYS: u8 = 10;
    pub const POTS: u8 = 11;
    pub const SEEDS: u8 = 12;
    pub const TOOLS: u8 = 13;
    pub const FERTILIZER: u8 = 14;
    pub const MEDICINE: u8 = 15;
    pub const MEDICINE_PENDING: u8 = 16;
    pub const FRIENDS: u8 = 17;
}

mod world {
    pub const CLOCK: u8 = 1;
    pub const WEATHER: u8 = 2;
    pub const TIME_SPEED: u8 = 3;
    pub const PLANTS: u8 = 4;
    pub const NEXT_PLANT_ID: u8 = 5;
    pub const SCORES: u8 = 6;
    pub const SUDOKAT: u8 = 7;
    pub const WIFI_FAMILIAR: u8 = 8;
    pub const WIFI_RECENT: u8 = 9;
    pub const SCAN_STARTED: u8 = 10;
}

/// "None" for optional enum ids.
const NONE: u8 = 0xFF;
/// `MealEntry::CaughtSnack` in the recent-meals list.
const CAUGHT_SNACK: u8 = 0xFE;
/// Plant water debt is stored in half hours.
const WATER_SCALE: f32 = 2.0;
/// Plant fertilizer is stored in whole units.
const FERT_SCALE: f32 = 1.0;

// ---------------------------------------------------------------------------
// Stable enum ids. FROZEN: append new variants with new ids, never renumber.
// ---------------------------------------------------------------------------

macro_rules! save_ids {
    ($to:ident, $from:ident, $Enum:ident; $($Variant:ident => $id:literal),+ $(,)?) => {
        fn $to(e: $Enum) -> u8 {
            match e { $($Enum::$Variant => $id,)+ }
        }
        fn $from(v: u8) -> Option<$Enum> {
            match v { $($id => Some($Enum::$Variant),)+ _ => None }
        }
    };
}

save_ids! { food_id, food_from_id, FoodItem;
    Kibble => 0, Cod => 1, Haddock => 2, Trout => 3, Shrimp => 4, Herring => 5,
    Turkey => 6, Tuna => 7, Salmon => 8, Chicken => 9, Liver => 10, Beef => 11,
    Lamb => 12, Mackerel => 13, Carrots => 14, Pumpkin => 15, Treats => 16,
    FishBite => 17, Eggs => 18, Nugget => 19, Milk => 20, ChewStick => 21, Puree => 22,
}

save_ids! { toy_id, toy_from_id, ToyVariant;
    String_ => 0, Feather => 1, Mouse => 2, Ball => 3, Bubbles => 4, Laser => 5,
    FeatherBall => 6,
}

save_ids! { pot_id, pot_from_id, PotKind;
    Small => 0, Medium => 1, Large => 2, Planter => 3, Ground => 4,
}

save_ids! { seed_id, seed_from_id, SeedKind;
    CatGrass => 0, Freesia => 1, Sunflower => 2, Rose => 3,
}

save_ids! { layer_id, layer_from_id, PlantLayer;
    Background => 0, Midground => 1, Foreground => 2,
}

save_ids! { stage_id, stage_from_id, PlantStage;
    EmptyPot => 0, Seedling => 1, Young => 2, Growing => 3, Mature => 4, Thriving => 5,
    SeedlingWilted => 6, YoungWilted => 7, GrowingWilted => 8, MatureWilted => 9,
    ThrivingWilted => 10, SeedlingDead => 11, YoungDead => 12, GrowingDead => 13,
    MatureDead => 14, ThrivingDead => 15, Dead => 16, Dormant => 17,
}

save_ids! { weather_id, weather_from_id, Weather;
    Clear => 0, Cloudy => 1, Overcast => 2, Windy => 3, Rain => 4, Storm => 5, Snow => 6,
}

save_ids! { season_id, season_from_id, Season;
    Winter => 0, Spring => 1, Summer => 2, Fall => 3,
}

save_ids! { gender_id, gender_from_id, PetGender;
    Tom => 0, Queen => 1,
}

save_ids! { fav_weather_id, fav_weather_from_id, FavWeather;
    Sunny => 0, Rainy => 1, Snowy => 2, Overcast => 3,
}

save_ids! { star_sign_id, star_sign_from_id, StarSign;
    Aries => 0, Taurus => 1, Gemini => 2, Cancer => 3, Leo => 4, Virgo => 5,
    Libra => 6, Scorpio => 7, Sagittarius => 8, Capricorn => 9, Aquarius => 10, Pisces => 11,
}

/// Rooms. Only rooms are stored (plant locations, favourite places), so
/// `SceneId` doesn't go through `save_ids!`: most of its variants are
/// minigames and menus that never reach a save.
fn scene_id(s: SceneId) -> u8 {
    match s {
        SceneId::Outside => 1,
        SceneId::Bedroom => 2,
        SceneId::Kitchen => 3,
        SceneId::Treehouse => 4,
        SceneId::Garage => 5,
        // Inside, and anything that isn't a room.
        _ => 0,
    }
}

fn scene_from_id(v: u8) -> Option<SceneId> {
    Some(match v {
        0 => SceneId::Inside,
        1 => SceneId::Outside,
        2 => SceneId::Bedroom,
        3 => SceneId::Kitchen,
        4 => SceneId::Treehouse,
        5 => SceneId::Garage,
        _ => return None,
    })
}

/// Food-stock bytes are indexed by food id; this lists the ids in order.
const FOOD_BY_ID: [FoodItem; 23] = [
    FoodItem::Kibble, FoodItem::Cod, FoodItem::Haddock, FoodItem::Trout, FoodItem::Shrimp,
    FoodItem::Herring, FoodItem::Turkey, FoodItem::Tuna, FoodItem::Salmon, FoodItem::Chicken,
    FoodItem::Liver, FoodItem::Beef, FoodItem::Lamb, FoodItem::Mackerel, FoodItem::Carrots,
    FoodItem::Pumpkin, FoodItem::Treats, FoodItem::FishBite, FoodItem::Eggs, FoodItem::Nugget,
    FoodItem::Milk, FoodItem::ChewStick, FoodItem::Puree,
];

const POTS_ORDER: [PotSize; 4] = [PotSize::Small, PotSize::Medium, PotSize::Large, PotSize::Planter];
const SEEDS_ORDER: [SeedKind; 4] =
    [SeedKind::CatGrass, SeedKind::Freesia, SeedKind::Sunflower, SeedKind::Rose];

// ---------------------------------------------------------------------------
// Byte writer / reader.
// ---------------------------------------------------------------------------

struct Writer<'a> {
    buf: &'a mut [u8],
    pos: usize,
    ok: bool,
}

impl<'a> Writer<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0, ok: true }
    }

    fn bytes(&mut self, b: &[u8]) {
        match self.buf.get_mut(self.pos..self.pos + b.len()) {
            Some(dst) => {
                dst.copy_from_slice(b);
                self.pos += b.len();
            }
            None => self.ok = false,
        }
    }

    fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.bytes(&v.to_le_bytes());
    }

    fn varint(&mut self, mut v: u32) {
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                self.u8(byte);
                return;
            }
            self.u8(byte | 0x80);
        }
    }

    fn zigzag(&mut self, v: i32) {
        self.varint(((v << 1) ^ (v >> 31)) as u32);
    }

    fn short_str(&mut self, s: &str) {
        let b = s.as_bytes();
        let n = b.len().min(255);
        self.u8(n as u8);
        self.bytes(&b[..n]);
    }

    /// Reserve `n` bytes to patch later; returns their position.
    fn reserve(&mut self, n: usize) -> usize {
        let at = self.pos;
        for _ in 0..n {
            self.u8(0);
        }
        at
    }

    fn patch(&mut self, at: usize, b: &[u8]) {
        if self.ok {
            self.buf[at..at + b.len()].copy_from_slice(b);
        }
    }

    fn field(&mut self, tag: u8, f: impl FnOnce(&mut Self)) {
        self.u8(tag);
        let at = self.reserve(2);
        let start = self.pos;
        f(self);
        let len = self.pos - start;
        if len > u16::MAX as usize {
            self.ok = false;
        }
        self.patch(at, &(len as u16).to_le_bytes());
    }

    fn record(&mut self, f: impl FnOnce(&mut Self)) {
        let at = self.reserve(1);
        let start = self.pos;
        f(self);
        let len = self.pos - start;
        if len > u8::MAX as usize {
            self.ok = false;
        }
        self.patch(at, &[len as u8]);
    }

    fn section(&mut self, id: u8, f: impl FnOnce(&mut Self)) {
        let head = self.pos;
        self.u8(id);
        let len_at = self.reserve(2);
        let crc_at = self.reserve(2);
        let start = self.pos;
        f(self);
        if !self.ok {
            return;
        }
        let len = self.pos - start;
        if len > u16::MAX as usize {
            self.ok = false;
            return;
        }
        self.patch(len_at, &(len as u16).to_le_bytes());
        let crc = crc16_update(crc16_update(0xFFFF, &self.buf[head..head + 3]), &self.buf[start..self.pos]);
        self.patch(crc_at, &crc.to_le_bytes());
    }
}

#[derive(Clone)]
struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }

    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let out = self.b.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(out)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.bytes(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.array()?))
    }

    fn varint(&mut self) -> Option<u32> {
        let mut out = 0u32;
        for shift in (0..35).step_by(7) {
            let b = self.u8()?;
            out |= ((b & 0x7F) as u32).checked_shl(shift)?;
            if b & 0x80 == 0 {
                return Some(out);
            }
        }
        None
    }

    fn zigzag(&mut self) -> Option<i32> {
        let v = self.varint()?;
        Some(((v >> 1) as i32) ^ -((v & 1) as i32))
    }

    fn short_str(&mut self) -> Option<&'a str> {
        let n = self.u8()? as usize;
        core::str::from_utf8(self.bytes(n)?).ok()
    }

    /// Next `[tag][len u16][value]` field.
    fn field(&mut self) -> Option<(u8, &'a [u8])> {
        let tag = self.u8()?;
        let len = self.u16()? as usize;
        Some((tag, self.bytes(len)?))
    }

    /// Iterate a list value's records, each as its own reader.
    fn records(mut self, mut f: impl FnMut(Reader<'a>)) {
        let Some(count) = self.u8() else {
            return;
        };
        for _ in 0..count {
            let Some(len) = self.u8() else {
                return;
            };
            let Some(rec) = self.bytes(len as usize) else {
                return;
            };
            f(Reader::new(rec));
        }
    }
}

/// Copy as much of `s` as fits, on char boundaries.
fn push_truncated<const N: usize>(dst: &mut String<N>, s: &str) {
    dst.clear();
    for c in s.chars() {
        if dst.push(c).is_err() {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Encode.
// ---------------------------------------------------------------------------

/// Encode `ctx` into `buf`. Returns the payload length, or `None` if it
/// didn't fit (can't happen with the context's capacity limits).
pub fn encode(ctx: &GameContext, buf: &mut [u8]) -> Option<usize> {
    let mut w = Writer::new(buf);
    w.section(SECTION_PET, |w| encode_pet(ctx, w));
    w.section(SECTION_WORLD, |w| encode_world(ctx, w));
    w.ok.then_some(w.pos)
}

fn stats(ctx: &GameContext) -> [f32; 17] {
    [
        ctx.fullness, ctx.energy, ctx.comfort, ctx.playfulness, ctx.focus, ctx.fulfillment,
        ctx.cleanliness, ctx.curiosity, ctx.sociability, ctx.intelligence, ctx.maturity,
        ctx.affection, ctx.fitness, ctx.serenity, ctx.courage, ctx.loyalty, ctx.mischievousness,
    ]
}

fn stats_mut(ctx: &mut GameContext) -> [&mut f32; 17] {
    [
        &mut ctx.fullness, &mut ctx.energy, &mut ctx.comfort, &mut ctx.playfulness,
        &mut ctx.focus, &mut ctx.fulfillment, &mut ctx.cleanliness, &mut ctx.curiosity,
        &mut ctx.sociability, &mut ctx.intelligence, &mut ctx.maturity, &mut ctx.affection,
        &mut ctx.fitness, &mut ctx.serenity, &mut ctx.courage, &mut ctx.loyalty,
        &mut ctx.mischievousness,
    ]
}

fn opt<T>(v: Option<T>, f: fn(T) -> u8) -> u8 {
    v.map(f).unwrap_or(NONE)
}

fn encode_pet(ctx: &GameContext, w: &mut Writer) {
    w.field(pet::SEED, |w| w.u64(ctx.pet_seed));
    w.field(pet::NAME, |w| w.bytes(ctx.pet_name.as_bytes()));
    w.field(pet::STATS, |w| stats(ctx).iter().for_each(|&s| w.f32(s)));
    w.field(pet::SICKNESS, |w| w.f32(ctx.sickness));
    w.field(pet::COINS, |w| w.i32(ctx.coins));
    w.field(pet::IDENTITY, |w| {
        w.u8(opt(ctx.pet_gender, gender_id));
        w.u8(opt(ctx.star_sign, star_sign_id));
        w.u8(opt(ctx.fav_weather, fav_weather_id));
        w.u8(opt(ctx.fav_meal, food_id));
        w.u8(opt(ctx.least_fav_meal, food_id));
        w.u8(opt(ctx.fav_snack, food_id));
        w.u8(opt(ctx.least_fav_snack, food_id));
        w.u8(opt(ctx.fav_toy, toy_id));
        w.u8(opt(ctx.least_fav_toy, toy_id));
        w.u8(opt(ctx.fav_location, scene_id));
        w.u8(opt(ctx.least_fav_location, scene_id));
    });
    w.field(pet::RECENT_MEALS, |w| {
        for m in ctx.recent_meals.iter() {
            w.u8(match m {
                MealEntry::Item(item) => food_id(*item),
                MealEntry::CaughtSnack => CAUGHT_SNACK,
            });
        }
    });
    w.field(pet::MILESTONES, |w| {
        let bits = [
            ctx.milestone_fed,
            ctx.milestone_groomed,
            ctx.milestone_played,
            ctx.milestone_petted,
            ctx.milestone_store,
        ];
        w.u8(pack_bits(&bits));
    });
    w.field(pet::FOOD_STOCK, |w| {
        for item in FOOD_BY_ID {
            w.u8(ctx.food_stock[item as usize]);
        }
    });
    w.field(pet::TOYS, |w| {
        w.u8(ctx.toys.len() as u8);
        for t in ctx.toys.iter() {
            w.record(|w| {
                w.u8(toy_id(t.variant));
                w.u8(t.durability);
            });
        }
    });
    w.field(pet::POTS, |w| POTS_ORDER.iter().for_each(|&p| w.u8(ctx.pots[p as usize])));
    w.field(pet::SEEDS, |w| SEEDS_ORDER.iter().for_each(|&s| w.u8(ctx.seeds[s as usize])));
    w.field(pet::TOOLS, |w| {
        w.u8(pack_bits(&[
            ctx.tools[ToolKind::Spade as usize],
            ctx.tools[ToolKind::WateringCan as usize],
        ]))
    });
    w.field(pet::FERTILIZER, |w| w.u8(ctx.fertilizer));
    w.field(pet::MEDICINE, |w| w.u8(ctx.medicine));
    w.field(pet::MEDICINE_PENDING, |w| w.u8(ctx.medicine_pending as u8));
    w.field(pet::FRIENDS, |w| {
        w.u8(ctx.friends.len() as u8);
        for f in ctx.friends.iter() {
            w.record(|w| {
                w.bytes(&f.mac);
                w.f32(f.total_secs);
                w.u16(f.visits);
                w.short_str(&f.name);
            });
        }
    });
}

fn encode_world(ctx: &GameContext, w: &mut Writer) {
    w.field(world::CLOCK, |w| {
        w.u8(ctx.time_hours);
        w.u8(ctx.time_minutes);
        w.u32(ctx.day_number);
        w.u16(ctx.season_offset);
        w.u8(season_id(ctx.season));
        w.u8(ctx.moon_phase);
    });
    w.field(world::WEATHER, |w| {
        w.u8(weather_id(ctx.weather));
        w.f32(ctx.temperature);
        w.u32(ctx.weather_step);
        w.f32(ctx.weather_timer);
        w.f32(ctx.meteor_shower_timer);
    });
    w.field(world::TIME_SPEED, |w| w.f32(ctx.time_speed));
    w.field(world::PLANTS, |w| {
        w.u8(ctx.plants.len() as u8);
        for p in ctx.plants.iter() {
            w.record(|w| encode_plant(p, w));
        }
    });
    w.field(world::NEXT_PLANT_ID, |w| w.u32(ctx.next_plant_id));
    w.field(world::SCORES, |w| {
        w.i32(ctx.zoomies_high_score);
        w.i32(ctx.maze_best_time);
        w.i32(ctx.snake_high_score);
        w.i32(ctx.memory_best_score);
        w.i32(ctx.meowltiply_high_score);
    });
    w.field(world::SUDOKAT, |w| {
        w.u8(ctx.sudokat_difficulty);
        // No board: the difficulty alone.
        if ctx.sudokat_givens.iter().any(|&v| v != 0) {
            pack_cells(&ctx.sudokat_givens, w);
            pack_cells(&ctx.sudokat_entries, w);
        }
    });
    w.field(world::WIFI_FAMILIAR, |w| encode_wifi(&ctx.wifi_familiar, w));
    w.field(world::WIFI_RECENT, |w| encode_wifi(&ctx.wifi_recent, w));
    w.field(world::SCAN_STARTED, |w| w.u8(ctx.scan_started as u8));
}

fn encode_plant(p: &Plant, w: &mut Writer) {
    w.varint(p.id);
    w.u8(scene_id(p.scene));
    w.u8(stage_id(p.stage));
    let seed = p.seed.map(seed_id).unwrap_or(0x0F);
    w.u8((seed & 0x0F) | (layer_id(p.layer) << 4));
    w.u8((pot_id(p.pot) & 0x0F) | ((p.mirror as u8) << 4) | ((p.aged as u8) << 5));
    w.zigzag(p.x);
    w.zigzag(p.y_snap);
    w.varint(p.age_hours);
    w.varint(to_fixed(p.water_debt, WATER_SCALE));
    w.varint(to_fixed(p.fertilizer, FERT_SCALE));
    w.varint(p.planted_day.map(|d| d.saturating_add(1)).unwrap_or(0));
}

fn encode_wifi<const N: usize>(list: &Vec<WifiEntry, N>, w: &mut Writer) {
    w.u8(list.len() as u8);
    for e in list.iter() {
        w.record(|w| {
            w.bytes(&e.bssid);
            w.f32(e.count);
            w.short_str(&e.ssid);
        });
    }
}

fn to_fixed(v: f32, scale: f32) -> u32 {
    // Rounds to nearest. NaN and negatives become 0; the cast saturates.
    (v.max(0.0) * scale + 0.5) as u32
}

fn from_fixed(v: u32, scale: f32) -> f32 {
    v as f32 / scale
}

fn pack_bits(bits: &[bool]) -> u8 {
    bits.iter().enumerate().fold(0, |acc, (i, &b)| acc | ((b as u8) << i))
}

fn bit(v: u8, i: u8) -> bool {
    v & (1 << i) != 0
}

/// 81 cells, two per byte, low nibble first.
fn pack_cells(cells: &[u8; 81], w: &mut Writer) {
    for pair in cells.chunks(2) {
        let lo = pair[0].min(9);
        let hi = pair.get(1).copied().unwrap_or(0).min(9);
        w.u8(lo | (hi << 4));
    }
}

fn unpack_cells(b: &[u8]) -> Option<[u8; 81]> {
    if b.len() < 41 {
        return None;
    }
    let mut cells = [0u8; 81];
    for (i, cell) in cells.iter_mut().enumerate() {
        let byte = b[i / 2];
        let v = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
        if v > 9 {
            return None;
        }
        *cell = v;
    }
    Some(cells)
}

// ---------------------------------------------------------------------------
// Decode.
// ---------------------------------------------------------------------------

/// Find section `id` in a payload. `None` if it's absent or its CRC fails.
pub fn section(payload: &[u8], id: u8) -> Option<&[u8]> {
    let mut r = Reader::new(payload);
    loop {
        let head = r.pos;
        let sid = r.u8()?;
        let len = r.u16()? as usize;
        let crc = r.u16()?;
        let body = r.bytes(len)?;
        if sid == id {
            let want = crc16_update(crc16_update(0xFFFF, &payload[head..head + 3]), body);
            return (want == crc).then_some(body);
        }
    }
}

fn fields(body: &[u8], mut f: impl FnMut(u8, Reader)) {
    let mut r = Reader::new(body);
    while let Some((tag, value)) = r.field() {
        f(tag, Reader::new(value));
    }
}

/// Apply a PET section body to `ctx`. Fields absent from the body keep
/// their current values.
pub fn decode_pet(body: &[u8], ctx: &mut GameContext) {
    fields(body, |tag, mut r| match tag {
        pet::SEED => {
            if let Some(v) = r.u64() {
                ctx.pet_seed = v;
            }
        }
        pet::NAME => {
            let s = core::str::from_utf8(r.b).unwrap_or("");
            let mut name = String::<PET_NAME_MAX>::new();
            push_truncated(&mut name, s);
            ctx.pet_name = name;
        }
        pet::STATS => {
            for s in stats_mut(ctx) {
                match r.f32() {
                    Some(v) => *s = v,
                    None => break,
                }
            }
        }
        pet::SICKNESS => {
            if let Some(v) = r.f32() {
                ctx.sickness = v;
            }
        }
        pet::COINS => {
            if let Some(v) = r.i32() {
                ctx.coins = v;
            }
        }
        pet::IDENTITY => {
            // Order is frozen: see `encode_pet`. Stops at the end of what
            // was written, leaving later traits at their current values.
            macro_rules! take {
                ($field:ident, $from:ident) => {
                    match r.u8() {
                        Some(v) => ctx.$field = $from(v),
                        None => return,
                    }
                };
            }
            take!(pet_gender, gender_from_id);
            take!(star_sign, star_sign_from_id);
            take!(fav_weather, fav_weather_from_id);
            take!(fav_meal, food_from_id);
            take!(least_fav_meal, food_from_id);
            take!(fav_snack, food_from_id);
            take!(least_fav_snack, food_from_id);
            take!(fav_toy, toy_from_id);
            take!(least_fav_toy, toy_from_id);
            take!(fav_location, scene_from_id);
            take!(least_fav_location, scene_from_id);
        }
        pet::RECENT_MEALS => {
            ctx.recent_meals.clear();
            for &v in r.b {
                let entry = match v {
                    CAUGHT_SNACK => MealEntry::CaughtSnack,
                    _ => match food_from_id(v) {
                        Some(item) => MealEntry::Item(item),
                        None => continue,
                    },
                };
                let _ = ctx.recent_meals.push(entry);
            }
        }
        pet::MILESTONES => {
            if let Some(v) = r.u8() {
                ctx.milestone_fed = bit(v, 0);
                ctx.milestone_groomed = bit(v, 1);
                ctx.milestone_played = bit(v, 2);
                ctx.milestone_petted = bit(v, 3);
                ctx.milestone_store = bit(v, 4);
            }
        }
        pet::FOOD_STOCK => {
            for (i, &count) in r.b.iter().enumerate() {
                if let Some(&item) = FOOD_BY_ID.get(i) {
                    ctx.food_stock[item as usize] = count;
                }
            }
        }
        pet::TOYS => {
            ctx.toys.clear();
            r.records(|mut rec| {
                let (Some(v), Some(durability)) = (rec.u8(), rec.u8()) else {
                    return;
                };
                if let Some(variant) = toy_from_id(v) {
                    let _ = ctx.toys.push(ToyEntry { variant, durability });
                }
            });
        }
        pet::POTS => {
            for (&p, &count) in POTS_ORDER.iter().zip(r.b) {
                ctx.pots[p as usize] = count;
            }
        }
        pet::SEEDS => {
            for (&s, &count) in SEEDS_ORDER.iter().zip(r.b) {
                ctx.seeds[s as usize] = count;
            }
        }
        pet::TOOLS => {
            if let Some(v) = r.u8() {
                ctx.tools[ToolKind::Spade as usize] = bit(v, 0);
                ctx.tools[ToolKind::WateringCan as usize] = bit(v, 1);
            }
        }
        pet::FERTILIZER => {
            if let Some(v) = r.u8() {
                ctx.fertilizer = v;
            }
        }
        pet::MEDICINE => {
            if let Some(v) = r.u8() {
                ctx.medicine = v;
            }
        }
        pet::MEDICINE_PENDING => {
            if let Some(v) = r.u8() {
                ctx.medicine_pending = v != 0;
            }
        }
        pet::FRIENDS => {
            ctx.friends.clear();
            r.records(|mut rec| {
                let (Some(mac), Some(total_secs), Some(visits), Some(name)) =
                    (rec.array::<6>(), rec.f32(), rec.u16(), rec.short_str())
                else {
                    return;
                };
                let mut entry = FriendEntry {
                    mac,
                    name: String::new(),
                    total_secs,
                    visits,
                };
                push_truncated(&mut entry.name, name);
                let _ = ctx.friends.push(entry);
            });
        }
        _ => {}
    });
}

/// Apply a WORLD section body to `ctx`. Fields absent from the body keep
/// their current values.
pub fn decode_world(body: &[u8], ctx: &mut GameContext) {
    let mut next_id = None;
    fields(body, |tag, mut r| match tag {
        world::CLOCK => {
            // Each step stops at the end of what was written.
            let _ = (|| {
                ctx.time_hours = r.u8()?;
                ctx.time_minutes = r.u8()?;
                ctx.day_number = r.u32()?;
                ctx.season_offset = r.u16()?;
                if let Some(s) = season_from_id(r.u8()?) {
                    ctx.season = s;
                }
                ctx.moon_phase = r.u8()? % 8;
                Some(())
            })();
        }
        world::WEATHER => {
            let _ = (|| {
                if let Some(w) = weather_from_id(r.u8()?) {
                    ctx.weather = w;
                }
                ctx.temperature = r.f32()?;
                ctx.weather_step = r.u32()?;
                ctx.weather_timer = r.f32()?;
                ctx.meteor_shower_timer = r.f32()?;
                Some(())
            })();
        }
        world::TIME_SPEED => {
            if let Some(v) = r.f32() {
                ctx.time_speed = v;
            }
        }
        world::PLANTS => {
            ctx.plants.clear();
            r.records(|rec| {
                if let Some(p) = decode_plant(rec) {
                    let _ = ctx.plants.push(p);
                }
            });
        }
        world::NEXT_PLANT_ID => next_id = r.u32(),
        world::SCORES => {
            let _ = (|| {
                ctx.zoomies_high_score = r.i32()?;
                ctx.maze_best_time = r.i32()?;
                ctx.snake_high_score = r.i32()?;
                ctx.memory_best_score = r.i32()?;
                ctx.meowltiply_high_score = r.i32()?;
                Some(())
            })();
        }
        world::SUDOKAT => {
            let Some(difficulty) = r.u8() else {
                return;
            };
            ctx.sudokat_difficulty = difficulty.min(2);
            ctx.sudokat_givens = [0; 81];
            ctx.sudokat_entries = [0; 81];
            let rest = &r.b[1..];
            let (Some(givens), Some(entries)) =
                (unpack_cells(rest), rest.get(41..).and_then(unpack_cells))
            else {
                return;
            };
            // Same consistency rule as the v0 loader: an entry never sits on
            // a given. Drop a malformed board rather than load it.
            if givens.iter().zip(entries.iter()).any(|(&g, &e)| g != 0 && e != 0) {
                return;
            }
            ctx.sudokat_givens = givens;
            ctx.sudokat_entries = entries;
        }
        world::WIFI_FAMILIAR => decode_wifi(r, &mut ctx.wifi_familiar),
        world::WIFI_RECENT => decode_wifi(r, &mut ctx.wifi_recent),
        world::SCAN_STARTED => {
            if let Some(v) = r.u8() {
                ctx.scan_started = v != 0;
            }
        }
        _ => {}
    });
    ctx.next_plant_id = match next_id {
        Some(id) => id,
        None => ctx.plants.iter().map(|p| p.id + 1).max().unwrap_or(ctx.next_plant_id),
    };
}

fn decode_plant(mut r: Reader) -> Option<Plant> {
    let id = r.varint()?;
    let scene = scene_from_id(r.u8()?).unwrap_or(SceneId::Inside);
    let stage = stage_from_id(r.u8()?).unwrap_or(PlantStage::Young);
    let seed_layer = r.u8()?;
    let pot_flags = r.u8()?;
    let mut plant = Plant {
        id,
        seed: seed_from_id(seed_layer & 0x0F),
        scene,
        layer: layer_from_id(seed_layer >> 4).unwrap_or(PlantLayer::Midground),
        x: 0,
        y_snap: 0,
        pot: pot_from_id(pot_flags & 0x0F).unwrap_or(PotKind::Small),
        stage,
        age_hours: 0,
        water_debt: 0.0,
        fertilizer: 0.0,
        planted_day: None,
        mirror: bit(pot_flags, 4),
        aged: bit(pot_flags, 5),
    };
    // Everything after the fixed bytes is optional, oldest first.
    let _ = (|| {
        plant.x = r.zigzag()?;
        plant.y_snap = r.zigzag()?;
        plant.age_hours = r.varint()?;
        plant.water_debt = from_fixed(r.varint()?, WATER_SCALE);
        plant.fertilizer = from_fixed(r.varint()?, FERT_SCALE);
        plant.planted_day = r.varint()?.checked_sub(1);
        Some(())
    })();
    Some(plant)
}

fn decode_wifi<const N: usize>(r: Reader, dst: &mut Vec<WifiEntry, N>) {
    dst.clear();
    r.records(|mut rec| {
        let (Some(bssid), Some(count), Some(ssid)) = (rec.array::<6>(), rec.f32(), rec.short_str())
        else {
            return;
        };
        let mut entry = WifiEntry {
            bssid,
            ssid: String::new(),
            count,
        };
        push_truncated(&mut entry.ssid, ssid);
        let _ = dst.push(entry);
    });
}
