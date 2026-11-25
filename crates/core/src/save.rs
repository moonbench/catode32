//! Save / load for `GameContext`.
//!
//! Saves are v1 snapshots (`crate::save_v1` format) in the snapshot ring
//! (`crate::ring`) on the `nvs` partition (`crate::storage`).
//!
//! Loading:
//! * The newest snapshot supplies each section. If a section's CRC fails,
//!   that section comes from the newest older snapshot where it's intact,
//!   so damage to the world (plants, wifi) never costs the pet.
//! * A v0 JSON save that is newer than every snapshot is loaded and then
//!   migrated: a v1 snapshot is written and verified alongside it. The v0
//!   record is never erased while it's still the newest record, so a power
//!   loss mid-migration just means the migration runs again next boot.
//! * If a save exists but nothing in it can be read, storage is locked: no
//!   save is written until the player explicitly erases it, so a firmware
//!   update that can read it can still recover the cat.

use core::str::FromStr;

use alloc::vec::Vec as AVec;
use heapless::{String, Vec};
use serde::Deserialize;

use crate::platform::time::{Duration, Instant};
use crate::{
    assets::plants::{PlantStage, PotKind},
    context::{
        FavWeather, FoodItem, FriendEntry, GameContext, PotSize, SeedKind, ToolKind, ToyEntry,
        ToyVariant, WifiEntry, FOOD_ITEM_COUNT, FRIENDS_MAX, WIFI_SSID_MAX,
    },
    pet_seed::{PetGender, StarSign},
    plant_system::{Plant, PlantLayer},
    ring,
    save_v1::{self, SECTION_PET, SECTION_WORLD},
    scene::SceneId,
    storage::{self, LegacyRef, Partition},
    time_system::{Season, Weather},
    wifi_tracker,
};

/// Save interval used by `save_if_needed`. Deliberately longer than
/// `wifi_tracker::SCAN_INTERVAL`: each scan is preceded by a save, so while
/// scans keep running this fallback never fires and the device writes about
/// once per scan interval rather than twice.
const SAVE_INTERVAL: Duration = Duration::from_secs(75 * 60);

/// Working buffer for one snapshot payload, used for both encode and decode.
/// `static mut` so it lives in `.bss` rather than on the task stack.
static mut SNAP_BUF: [u8; ring::MAX_PAYLOAD] = [0u8; ring::MAX_PAYLOAD];

fn snap_buf() -> &'static mut [u8; ring::MAX_PAYLOAD] {
    // SAFETY: single-threaded game loop. Only `load` and `save` use the
    // buffer and neither re-enters the other.
    unsafe { &mut *core::ptr::addr_of_mut!(SNAP_BUF) }
}

// ---------------------------------------------------------------------------
// Public API.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoadOutcome {
    /// The pet was restored.
    Loaded,
    /// No save exists: a fresh adoption.
    NoSave,
    /// A save exists but couldn't be read. Storage is now locked so nothing
    /// overwrites it.
    Unreadable,
}

/// True when any save exists, readable or not.
pub fn has_save() -> bool {
    storage::has_save()
}

/// Load the most recent save into `ctx`.
pub fn load(ctx: &mut GameContext) -> LoadOutcome {
    let Some(mut flash) = storage::open_partition() else {
        println!("[Save] No save partition");
        return LoadOutcome::NoSave;
    };
    let scan = storage::scan(&mut flash);
    let legacy = storage::legacy_save(&scan);
    let newest_v1 = scan.snaps.first().map(|s| s.seq);
    if newest_v1.is_none() && legacy.is_none() {
        return LoadOutcome::NoSave;
    }
    let legacy_is_newest = match (legacy, newest_v1) {
        (Some(l), Some(v)) => l.seq > v,
        (Some(_), None) => true,
        _ => false,
    };

    if !legacy_is_newest && load_v1(&mut flash, &scan, ctx) {
        finish_load(ctx);
        return LoadOutcome::Loaded;
    }
    if let Some(l) = legacy {
        if load_v0(&mut flash, &scan, l, ctx) {
            finish_load(ctx);
            if legacy_is_newest {
                migrate(&mut flash, &scan, ctx);
            }
            return LoadOutcome::Loaded;
        }
    }
    if legacy_is_newest && load_v1(&mut flash, &scan, ctx) {
        finish_load(ctx);
        return LoadOutcome::Loaded;
    }

    println!("[Save] A save exists but couldn't be read. Locking storage to keep it.");
    storage::lock();
    LoadOutcome::Unreadable
}

/// Encode `ctx` and append it to the ring. Returns true on success.
pub fn save(ctx: &mut GameContext) -> bool {
    if storage::is_locked() {
        println!("[Save] Storage locked (an unreadable save is being kept); not saving");
        return false;
    }
    let Some(mut flash) = storage::open_partition() else {
        println!("[Save] No save partition");
        return false;
    };
    let scan = storage::scan(&mut flash);
    if write_snapshot(&mut flash, &scan, ctx) {
        ctx.last_save_time = Some(Instant::now());
        true
    } else {
        false
    }
}

/// If `SAVE_INTERVAL` has elapsed since the last save, save now.
pub fn save_if_needed(ctx: &mut GameContext) {
    let due = match ctx.last_save_time {
        None => true,
        Some(t) => Instant::now().duration_since_epoch() - t.duration_since_epoch() > SAVE_INTERVAL,
    };
    if due {
        save(ctx);
    }
}

// ---------------------------------------------------------------------------
// v1.
// ---------------------------------------------------------------------------

fn write_snapshot(flash: &mut Partition, scan: &ring::Scan, ctx: &GameContext) -> bool {
    let buf = snap_buf();
    let Some(len) = save_v1::encode(ctx, buf) else {
        println!("[Save] Encode failed: snapshot larger than {} bytes", ring::MAX_PAYLOAD);
        return false;
    };
    match ring::write(flash, scan, &buf[..len], 0) {
        Ok(s) => {
            println!(
                "[Save] Saved {} bytes at slot {} (seq {})",
                len, s.slot, s.seq
            );
            true
        }
        Err(e) => {
            println!("[Save] Write failed: {:?}", e);
            false
        }
    }
}

/// Load the PET section from the newest snapshot where it's intact, then
/// WORLD the same way. False (with `ctx` untouched) if no PET is readable.
fn load_v1(flash: &mut Partition, scan: &ring::Scan, ctx: &mut GameContext) -> bool {
    let buf = snap_buf();
    let mut pet_seq = None;
    for s in scan.snaps.iter() {
        let pet = ring::read_snapshot(flash, scan, s, buf).and_then(|p| save_v1::section(p, SECTION_PET));
        match pet {
            Some(body) => {
                save_v1::decode_pet(body, ctx);
                pet_seq = Some(s.seq);
                break;
            }
            None => println!("[Save] Snapshot seq {}: pet section unreadable", s.seq),
        }
    }
    let Some(pet_seq) = pet_seq else {
        return false;
    };

    let mut world_seq = None;
    for s in scan.snaps.iter() {
        let world = ring::read_snapshot(flash, scan, s, buf).and_then(|p| save_v1::section(p, SECTION_WORLD));
        if let Some(body) = world {
            save_v1::decode_world(body, ctx);
            world_seq = Some(s.seq);
            break;
        }
    }
    match world_seq {
        Some(w) if w == pet_seq => println!("[Save] Loaded snapshot seq {}", pet_seq),
        Some(w) => println!("[Save] Loaded pet from seq {}, world from older seq {}", pet_seq, w),
        None => println!("[Save] Loaded pet from seq {}; no readable world, using defaults", pet_seq),
    }
    true
}

fn finish_load(ctx: &mut GameContext) {
    ctx.first_impressions = false;
    ctx.recompute_health();
    ctx.last_save_time = Some(Instant::now());
}

/// Write the just-loaded v0 cat as a v1 snapshot. The v0 record is the
/// newest record, so the ring won't erase it; it stays as a backup until
/// the ring wraps around to it.
fn migrate(flash: &mut Partition, scan: &ring::Scan, ctx: &GameContext) {
    if write_snapshot(flash, scan, ctx) {
        println!("[Save] Migrated v0 save to v1");
    } else {
        println!("[Save] Migration write failed; v0 save kept, will retry");
    }
}

// ---------------------------------------------------------------------------
// v0 (legacy JSON) reader, kept for migration. Never written any more.
//
// Strings borrow from the JSON buffer and lists live on the heap, so the
// parse needs about 0.5 KB of stack instead of the ~16 KB the old by-value
// structs needed (the cause of the ESP32-C3 boot loop, issue #23). The JSON
// buffer itself is a heap allocation that only exists during the parse;
// nothing else is on the heap yet at boot.
// ---------------------------------------------------------------------------

fn load_v0(flash: &mut Partition, scan: &ring::Scan, l: LegacyRef, ctx: &mut GameContext) -> bool {
    let mut json: AVec<u8> = AVec::new();
    if json.try_reserve_exact(l.len).is_err() {
        println!("[Save] Not enough memory to read the v0 save ({} bytes)", l.len);
        return false;
    }
    json.resize(l.len, 0);
    if !storage::read_legacy(flash, scan, &mut json) {
        println!("[Save] v0 save read failed");
        return false;
    }
    match serde_json_core::from_slice::<SaveData>(&json) {
        Ok((data, consumed)) => {
            apply(&data, ctx);
            println!("[Save] Loaded v0 save ({} bytes, {} parsed)", l.len, consumed);
            true
        }
        Err(e) => {
            println!("[Save] v0 parse failed ({} bytes): {:?}", l.len, e);
            false
        }
    }
}

crate::enum_key_pair_option! {
    food_save_key, food_from_key, FoodItem;
    Kibble    => "kibble",
    Cod       => "cod",
    Haddock   => "haddock",
    Trout     => "trout",
    Shrimp    => "shrimp",
    Herring   => "herring",
    Turkey    => "turkey",
    Tuna      => "tuna",
    Salmon    => "salmon",
    Chicken   => "chicken",
    Liver     => "liver",
    Beef      => "beef",
    Lamb      => "lamb",
    Mackerel  => "mackerel",
    Carrots   => "carrots",
    Pumpkin   => "pumpkin",
    Treats    => "treats",
    FishBite  => "fish_bite",
    Eggs      => "eggs",
    Nugget    => "nugget",
    Milk      => "milk",
    ChewStick => "chew_stick",
    Puree     => "puree",
}

crate::enum_key_pair_option! {
    toy_save_key, toy_from_key, ToyVariant;
    String_ => "string",
    Feather => "feather",
    Mouse   => "mouse",
    Ball    => "ball",
    Bubbles => "bubbles",
    Laser   => "laser",
}

crate::enum_key_pair_option! {
    pot_save_key, pot_from_key, PotKind;
    Small   => "small",
    Medium  => "medium",
    Large   => "large",
    Planter => "planter",
    Ground  => "ground",
}

crate::enum_key_pair_option! {
    seed_save_key, seed_from_key, SeedKind;
    CatGrass  => "cat_grass",
    Freesia   => "freesia",
    Sunflower => "sunflower",
    Rose      => "rose",
}

// Accept the legacy `fg/mg/bg` abbreviations so old saves still load. Unknown
// keys silently snap to Midground (the dominant layer in legacy saves).
crate::enum_key_pair_default! {
    layer_save_key, layer_from_key, PlantLayer;
    default: PlantLayer::Midground;
    Background => "background" | "bg",
    Midground  => "midground" | "mg",
    Foreground => "foreground" | "fg",
}

fn scene_from_key(s: &str) -> SceneId {
    match s {
        "outside" => SceneId::Outside,
        "bedroom" => SceneId::Bedroom,
        "kitchen" => SceneId::Kitchen,
        "treehouse" => SceneId::Treehouse,
        "garage" => SceneId::Garage,
        _ => SceneId::Inside,
    }
}

// "withering" is the legacy alias for YoungWilted from an earlier save
// format. Unknown keys fall back to Young (a safe non-dead stage).
crate::enum_key_pair_default! {
    stage_save_key, stage_from_key, PlantStage;
    default: PlantStage::Young;
    EmptyPot        => "empty_pot",
    Seedling        => "seedling",
    Young           => "young",
    Growing         => "growing",
    Mature          => "mature",
    Thriving        => "thriving",
    SeedlingWilted  => "seedling_wilted",
    YoungWilted     => "young_wilted" | "withering",
    GrowingWilted   => "growing_wilted",
    MatureWilted    => "mature_wilted",
    ThrivingWilted  => "thriving_wilted",
    SeedlingDead    => "seedling_dead",
    YoungDead       => "young_dead",
    GrowingDead     => "growing_dead",
    MatureDead      => "mature_dead",
    ThrivingDead    => "thriving_dead",
    Dead            => "dead",
    Dormant         => "dormant",
}

fn weather_from_key(s: &str) -> Weather {
    match s {
        "Clear" => Weather::Clear,
        "Cloudy" => Weather::Cloudy,
        "Overcast" => Weather::Overcast,
        "Windy" => Weather::Windy,
        "Rain" => Weather::Rain,
        "Storm" => Weather::Storm,
        "Snow" => Weather::Snow,
        _ => Weather::Clear,
    }
}

fn season_from_key(s: &str) -> Season {
    match s {
        "Winter" => Season::Winter,
        "Spring" => Season::Spring,
        "Summer" => Season::Summer,
        "Fall" => Season::Fall,
        _ => Season::Spring,
    }
}

/// v0 wrote `moon_phase` as a human label ("1st Qtr", "Full", ...). Parse
/// either form.
fn moon_phase_from_key(s: &str) -> u8 {
    match s {
        "New" => 0,
        "Waxing Crescent" => 1,
        "1st Qtr" | "First Qtr" => 2,
        "Waxing Gibbous" => 3,
        "Full" => 4,
        "Waning Gibbous" => 5,
        "Last Qtr" | "3rd Qtr" => 6,
        "Waning Crescent" => 7,
        _ => u8::from_str(s).unwrap_or(0),
    }
}

crate::enum_key_pair_option! {
    gender_save_key, gender_from_key, PetGender;
    Tom   => "tom",
    Queen => "queen",
}

crate::enum_key_pair_option! {
    fav_weather_save_key, fav_weather_from_key, FavWeather;
    Sunny    => "sunny",
    Rainy    => "rainy",
    Snowy    => "snowy",
    Overcast => "overcast",
}

fn star_sign_from_key(s: &str) -> Option<StarSign> {
    StarSign::ALL.iter().copied().find(|x| x.name() == s)
}

fn fav_location_from_key(s: &str) -> Option<SceneId> {
    Some(match s {
        "outside" => SceneId::Outside,
        "kitchen" => SceneId::Kitchen,
        "treehouse" => SceneId::Treehouse,
        "bedroom" => SceneId::Bedroom,
        "garage" => SceneId::Garage,
        "inside" => SceneId::Inside,
        _ => return None,
    })
}

// Wire-format structs. `#[serde(default)]` on every field keeps loading
// tolerant when keys are missing. `&str` fields borrow from the JSON buffer
// (serde-json-core never unescapes, so this matches the old owned-string
// behaviour, without the 28-byte copies).

#[derive(Default, Deserialize)]
struct EnvData<'a> {
    #[serde(default)] season_offset: u16,
    #[serde(default)] season: &'a str,
    #[serde(default)] weather: &'a str,
    #[serde(default)] weather_step: u32,
    #[serde(default)] weather_timer: f32,
    #[serde(default)] meteor_shower_timer: f32,
    #[serde(default)] moon_phase: &'a str,
    #[serde(default)] temperature: f32,
    #[serde(default)] time_hours: u8,
    #[serde(default)] time_minutes: u8,
    #[serde(default)] day_number: u32,
}

#[derive(Default, Deserialize)]
struct FoodStockData {
    #[serde(default)] kibble: u8,
    #[serde(default)] cod: u8,
    #[serde(default)] haddock: u8,
    #[serde(default)] trout: u8,
    #[serde(default)] shrimp: u8,
    #[serde(default)] herring: u8,
    #[serde(default)] turkey: u8,
    #[serde(default)] tuna: u8,
    #[serde(default)] salmon: u8,
    #[serde(default)] chicken: u8,
    #[serde(default)] liver: u8,
    #[serde(default)] beef: u8,
    #[serde(default)] lamb: u8,
    #[serde(default)] mackerel: u8,
    #[serde(default)] carrots: u8,
    #[serde(default)] pumpkin: u8,
    #[serde(default)] treats: u8,
    #[serde(default)] fish_bite: u8,
    #[serde(default)] eggs: u8,
    #[serde(default)] nugget: u8,
    #[serde(default)] milk: u8,
    #[serde(default)] chew_stick: u8,
    #[serde(default)] puree: u8,
}

#[derive(Default, Deserialize)]
struct PotsData {
    #[serde(default)] small: u8,
    #[serde(default)] medium: u8,
    #[serde(default)] large: u8,
    #[serde(default)] planter: u8,
}

#[derive(Default, Deserialize)]
struct SeedsData {
    #[serde(default)] cat_grass: u8,
    #[serde(default)] sunflower: u8,
    #[serde(default)] rose: u8,
    #[serde(default)] freesia: u8,
}

#[derive(Default, Deserialize)]
struct ToolsData {
    #[serde(default)] watering_can: bool,
    #[serde(default)] spade: bool,
}

#[derive(Default, Deserialize)]
struct ToyData<'a> {
    #[serde(default)] variant: &'a str,
    #[serde(default)] durability: u8,
}

#[derive(Default, Deserialize)]
struct PlantRecord<'a> {
    #[serde(default)] id: u32,
    #[serde(default, rename = "type")]
    seed_type: &'a str,
    #[serde(default)] scene: &'a str,
    #[serde(default)] layer: &'a str,
    #[serde(default)] x: i32,
    #[serde(default)] y_snap: i32,
    #[serde(default)] pot: &'a str,
    #[serde(default)] stage: &'a str,
    #[serde(default)] age_hours: u32,
    #[serde(default)] water_debt_hours: f32,
    #[serde(default)] fertilizer: f32,
    /// v0 used 0 as the "no planted_day recorded" sentinel and loaded it
    /// as `Some(0)`; kept as-is so migrated plants are unchanged.
    #[serde(default)] planted_day: u32,
    #[serde(default)] mirror: bool,
}

#[derive(Default, Deserialize)]
struct MilestonesData {
    #[serde(default)] fed: bool,
    #[serde(default)] groomed: bool,
    #[serde(default)] played: bool,
    #[serde(default)] petted: bool,
    #[serde(default)] store: bool,
}

/// Wire-format for a single wifi AP entry: `{'b': ..., 's': ..., 'n': ...}`.
#[derive(Default, Deserialize)]
struct WifiEntryData<'a> {
    #[serde(default, rename = "b")]
    bssid: &'a str,
    #[serde(default, rename = "s")]
    ssid: &'a str,
    #[serde(default, rename = "n")]
    count: f32,
}

#[derive(Default, Deserialize)]
struct FriendData<'a> {
    #[serde(default, rename = "n")]
    name: &'a str,
    #[serde(default, rename = "t")]
    total_secs: f32,
    #[serde(default, rename = "c")]
    visits: u16,
}

/// Playdate friends, keyed by peer MAC:
/// `{"aa:bb:cc:dd:ee:ff": {"n": .., "t": .., "c": ..}}`, matching the
/// MicroPython save format. Entries with unparseable keys, or past
/// `FRIENDS_MAX`, are dropped on load.
#[derive(Default)]
struct FriendsMap<'a>(AVec<([u8; 6], FriendData<'a>)>);

impl<'de: 'a, 'a> Deserialize<'de> for FriendsMap<'a> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<'a>(core::marker::PhantomData<&'a ()>);
        impl<'de: 'a, 'a> serde::de::Visitor<'de> for V<'a> {
            type Value = FriendsMap<'a>;
            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("a map of MAC -> friend")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<FriendsMap<'a>, A::Error> {
                let mut out = FriendsMap::default();
                while let Some(key) = access.next_key::<&'de str>()? {
                    match wifi_tracker::parse_bssid(key) {
                        Some(mac) if out.0.len() < FRIENDS_MAX => {
                            let data = access.next_value::<FriendData<'a>>()?;
                            out.0.push((mac, data));
                        }
                        _ => {
                            access.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(out)
            }
        }
        d.deserialize_map(V(core::marker::PhantomData))
    }
}

#[derive(Deserialize)]
struct SaveData<'a> {
    #[serde(default, borrow)] env: EnvData<'a>,
    #[serde(default)] food_stock: FoodStockData,
    #[serde(default, borrow)] toys: AVec<ToyData<'a>>,
    #[serde(default)] pots: PotsData,
    #[serde(default)] seeds: SeedsData,
    #[serde(default)] tools: ToolsData,
    #[serde(default)] fertilizer: u8,
    #[serde(default)] medicine: u8,
    #[serde(default)] sickness: f32,
    #[serde(default)] medicine_pending: bool,
    #[serde(default, borrow)] plants: AVec<PlantRecord<'a>>,
    #[serde(default)] next_plant_id: u32,
    #[serde(default)] pet_seed: u64,
    #[serde(default, borrow)] pet_gender: Option<&'a str>,
    #[serde(default, borrow)] fav_weather: Option<&'a str>,
    #[serde(default, borrow)] star_sign: Option<&'a str>,
    #[serde(default, borrow)] fav_meal: Option<&'a str>,
    #[serde(default, borrow)] least_fav_meal: Option<&'a str>,
    #[serde(default, borrow)] fav_snack: Option<&'a str>,
    #[serde(default, borrow)] least_fav_snack: Option<&'a str>,
    #[serde(default, borrow)] fav_toy: Option<&'a str>,
    #[serde(default, borrow)] least_fav_toy: Option<&'a str>,
    #[serde(default, borrow)] fav_location: Option<&'a str>,
    #[serde(default, borrow)] least_fav_location: Option<&'a str>,
    #[serde(default, borrow)] wifi_familiar: AVec<WifiEntryData<'a>>,
    #[serde(default, borrow)] wifi_recent: AVec<WifiEntryData<'a>>,
    /// True when this save was written immediately before a wifi scan.
    #[serde(default)] scan_started: bool,
    #[serde(default, borrow)] pet_name: Option<&'a str>,
    #[serde(default, borrow)] friends: FriendsMap<'a>,
    #[serde(default, borrow)] recent_meals: AVec<&'a str>,
    #[serde(default)] milestones: MilestonesData,

    // Flat stat fields at the top level.
    #[serde(default)] fullness: f32,
    #[serde(default)] energy: f32,
    #[serde(default)] comfort: f32,
    #[serde(default)] playfulness: f32,
    #[serde(default)] focus: f32,
    #[serde(default)] fulfillment: f32,
    #[serde(default)] cleanliness: f32,
    #[serde(default)] curiosity: f32,
    #[serde(default)] sociability: f32,
    #[serde(default)] intelligence: f32,
    #[serde(default)] maturity: f32,
    #[serde(default)] affection: f32,
    #[serde(default)] fitness: f32,
    #[serde(default)] serenity: f32,
    #[serde(default)] courage: f32,
    #[serde(default)] loyalty: f32,
    #[serde(default)] mischievousness: f32,
    #[serde(default)] zoomies_high_score: i32,
    #[serde(default)] maze_best_time: i32,
    #[serde(default)] snake_high_score: i32,
    #[serde(default)] memory_best_score: i32,
    #[serde(default)] meowltiply_high_score: i32,
    #[serde(default, borrow)] sudokat_givens: &'a str,
    #[serde(default, borrow)] sudokat_entries: &'a str,
    #[serde(default)] sudokat_difficulty: u8,
    #[serde(default)] time_speed: f32,
    #[serde(default)] coins: i32,
}

fn apply(data: &SaveData, ctx: &mut GameContext) {
    // Stats
    ctx.fullness = data.fullness;
    ctx.energy = data.energy;
    ctx.comfort = data.comfort;
    ctx.playfulness = data.playfulness;
    ctx.focus = data.focus;
    ctx.fulfillment = data.fulfillment;
    ctx.cleanliness = data.cleanliness;
    ctx.curiosity = data.curiosity;
    ctx.sociability = data.sociability;
    ctx.intelligence = data.intelligence;
    ctx.maturity = data.maturity;
    ctx.affection = data.affection;
    ctx.fitness = data.fitness;
    ctx.serenity = data.serenity;
    ctx.courage = data.courage;
    ctx.loyalty = data.loyalty;
    ctx.mischievousness = data.mischievousness;
    ctx.zoomies_high_score = data.zoomies_high_score;
    ctx.maze_best_time = data.maze_best_time;
    ctx.snake_high_score = data.snake_high_score;
    ctx.memory_best_score = data.memory_best_score;
    ctx.meowltiply_high_score = data.meowltiply_high_score;
    apply_sudokat(data, ctx);
    ctx.time_speed = data.time_speed;
    ctx.coins = data.coins;

    // Identity
    ctx.pet_seed = data.pet_seed;
    ctx.pet_gender = data.pet_gender.and_then(gender_from_key);
    ctx.fav_weather = data.fav_weather.and_then(fav_weather_from_key);
    ctx.star_sign = data.star_sign.and_then(star_sign_from_key);
    ctx.fav_meal = data.fav_meal.and_then(food_from_key);
    ctx.least_fav_meal = data.least_fav_meal.and_then(food_from_key);
    ctx.fav_snack = data.fav_snack.and_then(food_from_key);
    ctx.least_fav_snack = data.least_fav_snack.and_then(food_from_key);
    ctx.fav_toy = data.fav_toy.and_then(toy_from_key);
    ctx.least_fav_toy = data.least_fav_toy.and_then(toy_from_key);
    ctx.fav_location = data.fav_location.and_then(fav_location_from_key);
    ctx.least_fav_location = data.least_fav_location.and_then(fav_location_from_key);
    ctx.pet_name.clear();
    if let Some(n) = data.pet_name {
        for c in n.chars() {
            if ctx.pet_name.push(c).is_err() {
                break;
            }
        }
    }

    // Env
    ctx.season_offset = data.env.season_offset;
    ctx.season = season_from_key(data.env.season);
    ctx.weather = weather_from_key(data.env.weather);
    ctx.weather_step = data.env.weather_step;
    ctx.weather_timer = data.env.weather_timer;
    ctx.meteor_shower_timer = data.env.meteor_shower_timer;
    ctx.moon_phase = moon_phase_from_key(data.env.moon_phase);
    ctx.temperature = data.env.temperature;
    ctx.time_hours = data.env.time_hours;
    ctx.time_minutes = data.env.time_minutes;
    ctx.day_number = data.env.day_number;

    // Inventory
    ctx.food_stock = [0u8; FOOD_ITEM_COUNT];
    let f = &data.food_stock;
    for (item, count) in [
        (FoodItem::Kibble, f.kibble),
        (FoodItem::Cod, f.cod),
        (FoodItem::Haddock, f.haddock),
        (FoodItem::Trout, f.trout),
        (FoodItem::Shrimp, f.shrimp),
        (FoodItem::Herring, f.herring),
        (FoodItem::Turkey, f.turkey),
        (FoodItem::Tuna, f.tuna),
        (FoodItem::Salmon, f.salmon),
        (FoodItem::Chicken, f.chicken),
        (FoodItem::Liver, f.liver),
        (FoodItem::Beef, f.beef),
        (FoodItem::Lamb, f.lamb),
        (FoodItem::Mackerel, f.mackerel),
        (FoodItem::Carrots, f.carrots),
        (FoodItem::Pumpkin, f.pumpkin),
        (FoodItem::Treats, f.treats),
        (FoodItem::FishBite, f.fish_bite),
        (FoodItem::Eggs, f.eggs),
        (FoodItem::Nugget, f.nugget),
        (FoodItem::Milk, f.milk),
        (FoodItem::ChewStick, f.chew_stick),
        (FoodItem::Puree, f.puree),
    ] {
        ctx.food_stock[item as usize] = count;
    }

    ctx.toys.clear();
    for t in data.toys.iter() {
        if let Some(v) = toy_from_key(t.variant) {
            let _ = ctx.toys.push(ToyEntry {
                variant: v,
                durability: t.durability,
            });
        }
    }

    ctx.pots[PotSize::Small as usize] = data.pots.small;
    ctx.pots[PotSize::Medium as usize] = data.pots.medium;
    ctx.pots[PotSize::Large as usize] = data.pots.large;
    ctx.pots[PotSize::Planter as usize] = data.pots.planter;

    ctx.seeds[SeedKind::CatGrass as usize] = data.seeds.cat_grass;
    ctx.seeds[SeedKind::Sunflower as usize] = data.seeds.sunflower;
    ctx.seeds[SeedKind::Rose as usize] = data.seeds.rose;
    ctx.seeds[SeedKind::Freesia as usize] = data.seeds.freesia;

    ctx.tools[ToolKind::WateringCan as usize] = data.tools.watering_can;
    ctx.tools[ToolKind::Spade as usize] = data.tools.spade;

    ctx.fertilizer = data.fertilizer;
    ctx.medicine = data.medicine;
    ctx.sickness = data.sickness;
    ctx.medicine_pending = data.medicine_pending;

    // Plants: only overwrite when the save actually contains plant entries.
    // The starter set stays otherwise.
    if !data.plants.is_empty() {
        ctx.plants.clear();
        for r in data.plants.iter() {
            let plant = Plant {
                id: r.id,
                seed: seed_from_key(r.seed_type),
                scene: scene_from_key(r.scene),
                layer: layer_from_key(r.layer),
                x: r.x,
                y_snap: r.y_snap,
                pot: pot_from_key(r.pot).unwrap_or(PotKind::Small),
                stage: stage_from_key(r.stage),
                age_hours: r.age_hours,
                water_debt: r.water_debt_hours,
                fertilizer: r.fertilizer,
                planted_day: Some(r.planted_day),
                mirror: r.mirror,
                aged: false,
            };
            if ctx.plants.push(plant).is_err() {
                break;
            }
        }
    }
    if data.next_plant_id > 0 {
        ctx.next_plant_id = data.next_plant_id;
    } else {
        ctx.next_plant_id = ctx.plants.len() as u32;
    }

    // Recent meals.
    ctx.recent_meals.clear();
    for &s in data.recent_meals.iter() {
        use crate::context::MealEntry;
        let entry = if s == "caught_snack" {
            MealEntry::CaughtSnack
        } else if let Some(item) = food_from_key(s) {
            MealEntry::Item(item)
        } else {
            continue;
        };
        let _ = ctx.recent_meals.push(entry);
    }

    // Milestones.
    ctx.milestone_fed = data.milestones.fed;
    ctx.milestone_groomed = data.milestones.groomed;
    ctx.milestone_played = data.milestones.played;
    ctx.milestone_petted = data.milestones.petted;
    ctx.milestone_store = data.milestones.store;

    // WiFi tracker state. Defaults to empty lists if the save omits them
    // (e.g. saves from before wifi was wired up).
    apply_wifi_list(&data.wifi_familiar, &mut ctx.wifi_familiar);
    apply_wifi_list(&data.wifi_recent, &mut ctx.wifi_recent);
    ctx.friends.clear();
    for (mac, d) in data.friends.0.iter() {
        let mut name = String::new();
        for c in d.name.chars() {
            if name.push(c).is_err() {
                break;
            }
        }
        let _ = ctx.friends.push(FriendEntry {
            mac: *mac,
            name,
            total_secs: d.total_secs,
            visits: d.visits,
        });
    }
    ctx.scan_started = data.scan_started;
}

fn apply_wifi_list<const N: usize>(src: &[WifiEntryData], dst: &mut Vec<WifiEntry, N>) {
    dst.clear();
    for e in src.iter() {
        let Some(bssid) = wifi_tracker::parse_bssid(e.bssid) else {
            continue;
        };
        let mut ssid = String::<WIFI_SSID_MAX>::new();
        for c in e.ssid.chars() {
            if ssid.push(c).is_err() {
                break;
            }
        }
        if dst.push(WifiEntry { bssid, ssid, count: e.count }).is_err() {
            break;
        }
    }
}

fn parse_sudokat_cells(s: &str) -> Option<[u8; 81]> {
    if s.len() != 81 {
        return None;
    }
    let mut cells = [0u8; 81];
    for (cell, b) in cells.iter_mut().zip(s.bytes()) {
        if !b.is_ascii_digit() {
            return None;
        }
        *cell = b - b'0';
    }
    Some(cells)
}

/// Restore the Sudokat board, dropping it if anything is malformed.
fn apply_sudokat(data: &SaveData, ctx: &mut GameContext) {
    ctx.sudokat_difficulty = data.sudokat_difficulty.min(2);
    ctx.sudokat_givens = [0; 81];
    ctx.sudokat_entries = [0; 81];
    let Some(givens) = parse_sudokat_cells(data.sudokat_givens) else {
        return;
    };
    let entries = if data.sudokat_entries.is_empty() {
        [0; 81]
    } else {
        match parse_sudokat_cells(data.sudokat_entries) {
            Some(e) => e,
            None => return,
        }
    };
    if givens.iter().zip(entries.iter()).any(|(&g, &e)| g != 0 && e != 0) {
        return;
    }
    ctx.sudokat_givens = givens;
    ctx.sudokat_entries = entries;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #23: the old by-value `SaveData` was ~16 KB and serde keeps a
    /// few copies of it on the stack, which overflowed the ESP32-C3's stack
    /// during boot. Keep the v0 parse struct small (sizes here are for the
    /// 64-bit host; the 32-bit firmware's are smaller still).
    #[test]
    fn v0_parse_struct_stays_small() {
        let size = core::mem::size_of::<SaveData>();
        assert!(size < 1024, "v0 SaveData grew to {size} bytes");
    }
}
