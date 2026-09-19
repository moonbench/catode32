//! Save / load for `GameContext`.
//!
//! Storage layer is `crate::storage` (round-robin sector rotation on the
//! `nvs` partition).

use core::str::FromStr;

use crate::platform::time::{Duration, Instant};
use crate::println;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

use crate::{
    assets::plants::{PlantStage, PotKind},
    context::{
        FavWeather, FoodItem, GameContext, PotSize, SeedKind, ToolKind, ToyEntry, ToyVariant,
        WifiEntry, FOOD_ITEM_COUNT, MAX_PLANTS, PET_NAME_MAX, RECENT_HISTORY, TOY_VARIANT_COUNT,
        WIFI_FAMILIAR_MAX, WIFI_RECENT_MAX, WIFI_SSID_MAX,
    },
    pet_seed::{PetGender, StarSign},
    plant_system::{Plant, PlantLayer},
    scene::SceneId,
    storage,
    time_system::{Season, Weather},
    wifi_tracker,
};

/// Major schema version. Bumped when the on-disk shape changes in a
/// backwards-incompatible way.
const SCHEMA_VERSION: u8 = 0;

/// Save interval used by `save_if_needed`.
const SAVE_INTERVAL: Duration = Duration::from_secs(59 * 60);

/// Max size of the JSON payload. Sized to `storage::MAX_PAYLOAD`, which
/// covers a maxed-out save (80 plants, full inventory) with room to spare.
const JSON_BUF_SIZE: usize = storage::MAX_PAYLOAD;

/// Working buffer for JSON encode/decode. `static mut` because the buffer is
/// large enough that a stack allocation would dwarf typical task stacks.
static mut JSON_BUF: [u8; JSON_BUF_SIZE] = [0u8; JSON_BUF_SIZE];

// ---------------------------------------------------------------------------
// Strings used in the JSON. Centralised here so the encoder and the tolerant
// decoder agree.
// ---------------------------------------------------------------------------

type SStr = String<24>;
type StageStr = String<20>;
type NameStr = String<PET_NAME_MAX>;

fn sstr(s: &str) -> SStr {
    let mut out = SStr::new();
    // `push_str` truncates by returning Err. We deliberately allow truncation
    // since every string we hand it is shorter than the buffer.
    let _ = out.push_str(s);
    out
}

// ---------------------------------------------------------------------------
// Per-enum string mapping. Snake_case keys.
// ---------------------------------------------------------------------------

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

fn scene_save_key(s: SceneId) -> &'static str {
    match s {
        SceneId::Inside => "inside",
        SceneId::Outside => "outside",
        SceneId::Bedroom => "bedroom",
        SceneId::Kitchen => "kitchen",
        SceneId::Treehouse => "treehouse",
        // Non-main scenes never appear on a saved plant, but keep the mapping
        // exhaustive so the compiler catches future SceneId additions.
        _ => "inside",
    }
}

fn scene_from_key(s: &str) -> SceneId {
    match s {
        "outside" => SceneId::Outside,
        "bedroom" => SceneId::Bedroom,
        "kitchen" => SceneId::Kitchen,
        "treehouse" => SceneId::Treehouse,
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

fn weather_save_key(w: Weather) -> &'static str {
    w.name()
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

fn season_save_key(s: Season) -> &'static str {
    s.name()
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

/// `moon_phase` is stored as a 0-7 index in memory but written to disk as a
/// human label ("1st Qtr", "Full", ...). Parse either form on load.
fn moon_phase_save_key(p: u8) -> &'static str {
    match p % 8 {
        0 => "New",
        1 => "Waxing Crescent",
        2 => "1st Qtr",
        3 => "Waxing Gibbous",
        4 => "Full",
        5 => "Waning Gibbous",
        6 => "Last Qtr",
        _ => "Waning Crescent",
    }
}

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
        // Fall back to numeric parse so a Rust-emitted save can also round-trip
        // even if a future revision drops the labels.
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

fn star_sign_save_key(s: StarSign) -> &'static str {
    s.name()
}

fn star_sign_from_key(s: &str) -> Option<StarSign> {
    StarSign::ALL.iter().copied().find(|x| x.name() == s)
}

fn fav_location_save_key(s: SceneId) -> &'static str {
    match s {
        SceneId::Outside => "outside",
        SceneId::Kitchen => "kitchen",
        SceneId::Treehouse => "treehouse",
        SceneId::Bedroom => "bedroom",
        _ => "inside",
    }
}

fn fav_location_from_key(s: &str) -> Option<SceneId> {
    Some(match s {
        "outside" => SceneId::Outside,
        "kitchen" => SceneId::Kitchen,
        "treehouse" => SceneId::Treehouse,
        "bedroom" => SceneId::Bedroom,
        "inside" => SceneId::Inside,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Wire-format structs. #[serde(default)] on every field keeps loading
// tolerant when keys are missing.
// ---------------------------------------------------------------------------

#[derive(Default, Serialize, Deserialize)]
struct EnvData {
    #[serde(default)] season_offset: u16,
    #[serde(default)] season: SStr,
    #[serde(default)] weather: SStr,
    #[serde(default)] weather_step: u32,
    #[serde(default)] weather_timer: f32,
    #[serde(default)] meteor_shower_timer: f32,
    #[serde(default)] moon_phase: SStr,
    #[serde(default)] temperature: f32,
    #[serde(default)] time_hours: u8,
    #[serde(default)] time_minutes: u8,
    #[serde(default)] day_number: u32,
}

#[derive(Default, Serialize, Deserialize)]
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

#[derive(Default, Serialize, Deserialize)]
struct PotsData {
    #[serde(default)] small: u8,
    #[serde(default)] medium: u8,
    #[serde(default)] large: u8,
    #[serde(default)] planter: u8,
}

#[derive(Default, Serialize, Deserialize)]
struct SeedsData {
    #[serde(default)] cat_grass: u8,
    #[serde(default)] sunflower: u8,
    #[serde(default)] rose: u8,
    #[serde(default)] freesia: u8,
}

#[derive(Default, Serialize, Deserialize)]
struct ToolsData {
    #[serde(default)] watering_can: bool,
    #[serde(default)] spade: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct ToyData {
    #[serde(default)] variant: SStr,
    #[serde(default)] durability: u8,
}

#[derive(Default, Serialize, Deserialize)]
struct PlantRecord {
    #[serde(default)] id: u32,
    #[serde(default, rename = "type")]
    seed_type: SStr,
    #[serde(default)] scene: SStr,
    #[serde(default)] layer: SStr,
    #[serde(default)] x: i32,
    #[serde(default)] y_snap: i32,
    #[serde(default)] pot: SStr,
    #[serde(default)] stage: StageStr,
    #[serde(default)] age_hours: u32,
    #[serde(default)] water_debt_hours: f32,
    #[serde(default)] fertilizer: f32,
    /// Wire format uses 0 as the "no planted_day recorded" sentinel; in
    /// memory we hold `Option<u32>` and map 0 to `Some(0)` on load. The
    /// distinction is purely cosmetic during gameplay.
    #[serde(default)] planted_day: u32,
    #[serde(default)] mirror: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct MilestonesData {
    #[serde(default)] fed: bool,
    #[serde(default)] groomed: bool,
    #[serde(default)] played: bool,
    #[serde(default)] petted: bool,
    #[serde(default)] store: bool,
}

/// Wire-format for a single wifi AP entry: `{'b': ..., 's': ..., 'n': ...}`.
#[derive(Default, Serialize, Deserialize)]
struct WifiEntryData {
    #[serde(default, rename = "b")]
    bssid: heapless::String<17>,
    #[serde(default, rename = "s")]
    ssid: heapless::String<WIFI_SSID_MAX>,
    #[serde(default, rename = "n")]
    count: f32,
}

/// Placeholder for the friends map (not yet implemented). Serialises as an
/// empty JSON object; deserialise is a no-op accept-anything.
#[derive(Default)]
struct StubMap;
impl Serialize for StubMap {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        s.serialize_map(Some(0))?.end()
    }
}
impl<'de> Deserialize<'de> for StubMap {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(d).map(|_| StubMap)
    }
}

#[derive(Serialize, Deserialize)]
struct SaveData {
    #[serde(default)] v: u8,
    #[serde(default)] env: EnvData,
    #[serde(default)] food_stock: FoodStockData,
    #[serde(default)] toys: Vec<ToyData, TOY_VARIANT_COUNT>,
    #[serde(default)] pots: PotsData,
    #[serde(default)] seeds: SeedsData,
    #[serde(default)] tools: ToolsData,
    #[serde(default)] fertilizer: u8,
    #[serde(default)] medicine: u8,
    #[serde(default)] sickness: f32,
    #[serde(default)] medicine_pending: bool,
    #[serde(default)] plants: Vec<PlantRecord, MAX_PLANTS>,
    #[serde(default)] next_plant_id: u32,
    #[serde(default)] pet_seed: u64,
    #[serde(default)] pet_gender: Option<SStr>,
    #[serde(default)] fav_weather: Option<SStr>,
    #[serde(default)] star_sign: Option<SStr>,
    #[serde(default)] fav_meal: Option<SStr>,
    #[serde(default)] least_fav_meal: Option<SStr>,
    #[serde(default)] fav_snack: Option<SStr>,
    #[serde(default)] least_fav_snack: Option<SStr>,
    #[serde(default)] fav_toy: Option<SStr>,
    #[serde(default)] least_fav_toy: Option<SStr>,
    #[serde(default)] fav_location: Option<SStr>,
    #[serde(default)] least_fav_location: Option<SStr>,
    #[serde(default)] wifi_familiar: Vec<WifiEntryData, WIFI_FAMILIAR_MAX>,
    #[serde(default)] wifi_recent: Vec<WifiEntryData, WIFI_RECENT_MAX>,
    #[serde(default)] pet_name: Option<NameStr>,
    #[serde(default)] friends: StubMap,
    #[serde(default)] recent_meals: Vec<SStr, RECENT_HISTORY>,
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
    #[serde(default)] time_speed: f32,
    #[serde(default)] coins: i32,
}

// ---------------------------------------------------------------------------
// Bridging GameContext ↔ SaveData.
// ---------------------------------------------------------------------------

fn build(ctx: &GameContext) -> SaveData {
    let mut toys: Vec<ToyData, TOY_VARIANT_COUNT> = Vec::new();
    for t in ctx.toys.iter() {
        let _ = toys.push(ToyData {
            variant: sstr(toy_save_key(t.variant)),
            durability: t.durability,
        });
    }

    let mut plants: Vec<PlantRecord, MAX_PLANTS> = Vec::new();
    for p in ctx.plants.iter() {
        let _ = plants.push(PlantRecord {
            id: p.id,
            seed_type: p
                .seed
                .map(|s| sstr(seed_save_key(s)))
                .unwrap_or_default(),
            scene: sstr(scene_save_key(p.scene)),
            layer: sstr(layer_save_key(p.layer)),
            x: p.x,
            y_snap: p.y_snap,
            pot: sstr(pot_save_key(p.pot)),
            stage: {
                let mut s = StageStr::new();
                let _ = s.push_str(stage_save_key(p.stage));
                s
            },
            age_hours: p.age_hours,
            water_debt_hours: p.water_debt,
            fertilizer: p.fertilizer,
            planted_day: p.planted_day.unwrap_or(0),
            mirror: p.mirror,
        });
    }

    let mut recent_meals: Vec<SStr, RECENT_HISTORY> = Vec::new();
    for m in ctx.recent_meals.iter() {
        use crate::context::MealEntry;
        let key = match m {
            MealEntry::Item(item) => food_save_key(*item),
            MealEntry::CaughtSnack => "caught_snack",
        };
        let _ = recent_meals.push(sstr(key));
    }

    SaveData {
        v: SCHEMA_VERSION,
        env: EnvData {
            season_offset: ctx.season_offset,
            season: sstr(season_save_key(ctx.season)),
            weather: sstr(weather_save_key(ctx.weather)),
            weather_step: ctx.weather_step,
            weather_timer: ctx.weather_timer,
            meteor_shower_timer: ctx.meteor_shower_timer,
            moon_phase: sstr(moon_phase_save_key(ctx.moon_phase)),
            temperature: ctx.temperature,
            time_hours: ctx.time_hours,
            time_minutes: ctx.time_minutes,
            day_number: ctx.day_number,
        },
        food_stock: FoodStockData {
            kibble: ctx.food_stock[FoodItem::Kibble as usize],
            cod: ctx.food_stock[FoodItem::Cod as usize],
            haddock: ctx.food_stock[FoodItem::Haddock as usize],
            trout: ctx.food_stock[FoodItem::Trout as usize],
            shrimp: ctx.food_stock[FoodItem::Shrimp as usize],
            herring: ctx.food_stock[FoodItem::Herring as usize],
            turkey: ctx.food_stock[FoodItem::Turkey as usize],
            tuna: ctx.food_stock[FoodItem::Tuna as usize],
            salmon: ctx.food_stock[FoodItem::Salmon as usize],
            chicken: ctx.food_stock[FoodItem::Chicken as usize],
            liver: ctx.food_stock[FoodItem::Liver as usize],
            beef: ctx.food_stock[FoodItem::Beef as usize],
            lamb: ctx.food_stock[FoodItem::Lamb as usize],
            mackerel: ctx.food_stock[FoodItem::Mackerel as usize],
            carrots: ctx.food_stock[FoodItem::Carrots as usize],
            pumpkin: ctx.food_stock[FoodItem::Pumpkin as usize],
            treats: ctx.food_stock[FoodItem::Treats as usize],
            fish_bite: ctx.food_stock[FoodItem::FishBite as usize],
            eggs: ctx.food_stock[FoodItem::Eggs as usize],
            nugget: ctx.food_stock[FoodItem::Nugget as usize],
            milk: ctx.food_stock[FoodItem::Milk as usize],
            chew_stick: ctx.food_stock[FoodItem::ChewStick as usize],
            puree: ctx.food_stock[FoodItem::Puree as usize],
        },
        toys,
        pots: PotsData {
            small: ctx.pots[PotSize::Small as usize],
            medium: ctx.pots[PotSize::Medium as usize],
            large: ctx.pots[PotSize::Large as usize],
            planter: ctx.pots[PotSize::Planter as usize],
        },
        seeds: SeedsData {
            cat_grass: ctx.seeds[SeedKind::CatGrass as usize],
            sunflower: ctx.seeds[SeedKind::Sunflower as usize],
            rose: ctx.seeds[SeedKind::Rose as usize],
            freesia: ctx.seeds[SeedKind::Freesia as usize],
        },
        tools: ToolsData {
            watering_can: ctx.tools[ToolKind::WateringCan as usize],
            spade: ctx.tools[ToolKind::Spade as usize],
        },
        fertilizer: ctx.fertilizer,
        medicine: ctx.medicine,
        sickness: ctx.sickness,
        medicine_pending: ctx.medicine_pending,
        plants,
        next_plant_id: ctx.next_plant_id,
        pet_seed: ctx.pet_seed,
        pet_gender: ctx.pet_gender.map(|g| sstr(gender_save_key(g))),
        fav_weather: ctx.fav_weather.map(|w| sstr(fav_weather_save_key(w))),
        star_sign: ctx.star_sign.map(|s| sstr(star_sign_save_key(s))),
        fav_meal: ctx.fav_meal.map(|f| sstr(food_save_key(f))),
        least_fav_meal: ctx.least_fav_meal.map(|f| sstr(food_save_key(f))),
        fav_snack: ctx.fav_snack.map(|f| sstr(food_save_key(f))),
        least_fav_snack: ctx.least_fav_snack.map(|f| sstr(food_save_key(f))),
        fav_toy: ctx.fav_toy.map(|t| sstr(toy_save_key(t))),
        least_fav_toy: ctx.least_fav_toy.map(|t| sstr(toy_save_key(t))),
        fav_location: ctx.fav_location.map(|s| sstr(fav_location_save_key(s))),
        least_fav_location: ctx.least_fav_location.map(|s| sstr(fav_location_save_key(s))),
        wifi_familiar: build_wifi_list(&ctx.wifi_familiar),
        wifi_recent: build_wifi_list(&ctx.wifi_recent),
        pet_name: if ctx.pet_name.is_empty() {
            None
        } else {
            let mut n = NameStr::new();
            let _ = n.push_str(ctx.pet_name.as_str());
            Some(n)
        },
        friends: StubMap,
        recent_meals,
        milestones: MilestonesData {
            fed: ctx.milestone_fed,
            groomed: ctx.milestone_groomed,
            played: ctx.milestone_played,
            petted: ctx.milestone_petted,
            store: ctx.milestone_store,
        },

        fullness: ctx.fullness,
        energy: ctx.energy,
        comfort: ctx.comfort,
        playfulness: ctx.playfulness,
        focus: ctx.focus,
        fulfillment: ctx.fulfillment,
        cleanliness: ctx.cleanliness,
        curiosity: ctx.curiosity,
        sociability: ctx.sociability,
        intelligence: ctx.intelligence,
        maturity: ctx.maturity,
        affection: ctx.affection,
        fitness: ctx.fitness,
        serenity: ctx.serenity,
        courage: ctx.courage,
        loyalty: ctx.loyalty,
        mischievousness: ctx.mischievousness,
        zoomies_high_score: ctx.zoomies_high_score,
        maze_best_time: ctx.maze_best_time,
        snake_high_score: ctx.snake_high_score,
        memory_best_score: ctx.memory_best_score,
        time_speed: ctx.time_speed,
        coins: ctx.coins,
    }
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
    ctx.time_speed = data.time_speed;
    ctx.coins = data.coins;

    // Identity
    ctx.pet_seed = data.pet_seed;
    ctx.pet_gender = data.pet_gender.as_deref().and_then(gender_from_key);
    ctx.fav_weather = data.fav_weather.as_deref().and_then(fav_weather_from_key);
    ctx.star_sign = data.star_sign.as_deref().and_then(star_sign_from_key);
    ctx.fav_meal = data.fav_meal.as_deref().and_then(food_from_key);
    ctx.least_fav_meal = data.least_fav_meal.as_deref().and_then(food_from_key);
    ctx.fav_snack = data.fav_snack.as_deref().and_then(food_from_key);
    ctx.least_fav_snack = data.least_fav_snack.as_deref().and_then(food_from_key);
    ctx.fav_toy = data.fav_toy.as_deref().and_then(toy_from_key);
    ctx.least_fav_toy = data.least_fav_toy.as_deref().and_then(toy_from_key);
    ctx.fav_location = data.fav_location.as_deref().and_then(fav_location_from_key);
    ctx.least_fav_location = data
        .least_fav_location
        .as_deref()
        .and_then(fav_location_from_key);
    ctx.pet_name.clear();
    if let Some(n) = data.pet_name.as_deref() {
        for c in n.chars() {
            if ctx.pet_name.push(c).is_err() {
                break;
            }
        }
    }

    // Env
    ctx.season_offset = data.env.season_offset;
    ctx.season = season_from_key(&data.env.season);
    ctx.weather = weather_from_key(&data.env.weather);
    ctx.weather_step = data.env.weather_step;
    ctx.weather_timer = data.env.weather_timer;
    ctx.meteor_shower_timer = data.env.meteor_shower_timer;
    ctx.moon_phase = moon_phase_from_key(&data.env.moon_phase);
    ctx.temperature = data.env.temperature;
    ctx.time_hours = data.env.time_hours;
    ctx.time_minutes = data.env.time_minutes;
    ctx.day_number = data.env.day_number;

    // Inventory
    ctx.food_stock = [0u8; FOOD_ITEM_COUNT];
    ctx.food_stock[FoodItem::Kibble as usize] = data.food_stock.kibble;
    ctx.food_stock[FoodItem::Cod as usize] = data.food_stock.cod;
    ctx.food_stock[FoodItem::Haddock as usize] = data.food_stock.haddock;
    ctx.food_stock[FoodItem::Trout as usize] = data.food_stock.trout;
    ctx.food_stock[FoodItem::Shrimp as usize] = data.food_stock.shrimp;
    ctx.food_stock[FoodItem::Herring as usize] = data.food_stock.herring;
    ctx.food_stock[FoodItem::Turkey as usize] = data.food_stock.turkey;
    ctx.food_stock[FoodItem::Tuna as usize] = data.food_stock.tuna;
    ctx.food_stock[FoodItem::Salmon as usize] = data.food_stock.salmon;
    ctx.food_stock[FoodItem::Chicken as usize] = data.food_stock.chicken;
    ctx.food_stock[FoodItem::Liver as usize] = data.food_stock.liver;
    ctx.food_stock[FoodItem::Beef as usize] = data.food_stock.beef;
    ctx.food_stock[FoodItem::Lamb as usize] = data.food_stock.lamb;
    ctx.food_stock[FoodItem::Mackerel as usize] = data.food_stock.mackerel;
    ctx.food_stock[FoodItem::Carrots as usize] = data.food_stock.carrots;
    ctx.food_stock[FoodItem::Pumpkin as usize] = data.food_stock.pumpkin;
    ctx.food_stock[FoodItem::Treats as usize] = data.food_stock.treats;
    ctx.food_stock[FoodItem::FishBite as usize] = data.food_stock.fish_bite;
    ctx.food_stock[FoodItem::Eggs as usize] = data.food_stock.eggs;
    ctx.food_stock[FoodItem::Nugget as usize] = data.food_stock.nugget;
    ctx.food_stock[FoodItem::Milk as usize] = data.food_stock.milk;
    ctx.food_stock[FoodItem::ChewStick as usize] = data.food_stock.chew_stick;
    ctx.food_stock[FoodItem::Puree as usize] = data.food_stock.puree;

    ctx.toys.clear();
    for t in data.toys.iter() {
        if let Some(v) = toy_from_key(&t.variant) {
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
                seed: seed_from_key(&r.seed_type),
                scene: scene_from_key(&r.scene),
                layer: layer_from_key(&r.layer),
                x: r.x,
                y_snap: r.y_snap,
                pot: pot_from_key(&r.pot).unwrap_or(PotKind::Small),
                stage: stage_from_key(&r.stage),
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
    for s in data.recent_meals.iter() {
        use crate::context::MealEntry;
        let entry = if s.as_str() == "caught_snack" {
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

    ctx.first_impressions = false;
    ctx.recompute_health();
}

fn build_wifi_list<const N: usize>(src: &heapless::Vec<WifiEntry, N>) -> Vec<WifiEntryData, N> {
    let mut out: Vec<WifiEntryData, N> = Vec::new();
    for e in src.iter() {
        let _ = out.push(WifiEntryData {
            bssid: wifi_tracker::format_bssid(&e.bssid),
            ssid: e.ssid.clone(),
            count: e.count,
        });
    }
    out
}

fn apply_wifi_list<const N: usize>(
    src: &Vec<WifiEntryData, N>,
    dst: &mut heapless::Vec<WifiEntry, N>,
) {
    dst.clear();
    for e in src.iter() {
        let Some(bssid) = wifi_tracker::parse_bssid(&e.bssid) else {
            continue;
        };
        let _ = dst.push(WifiEntry {
            bssid,
            ssid: e.ssid.clone(),
            count: e.count,
        });
    }
}

// ---------------------------------------------------------------------------
// Public API.
// ---------------------------------------------------------------------------

/// True iff the storage layer holds at least one syntactically valid save.
pub fn has_save() -> bool {
    storage::has_save()
}

/// Load the most recent save into `ctx`. Returns true on success.
pub fn load(ctx: &mut GameContext) -> bool {
    // SAFETY: single-threaded boot path; JSON_BUF is only touched here and in
    // `save` below, never concurrently.
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(JSON_BUF) };
    let Some(len) = storage::read_latest(buf) else {
        println!("[Save] storage::read_latest returned None, no save loaded");
        return false;
    };
    match serde_json_core::from_slice::<SaveData>(&buf[..len]) {
        Ok((data, consumed)) => {
            apply(&data, ctx);
            ctx.last_save_time = Some(Instant::now());
            println!("[Save] Loaded {} bytes ({} parsed)", len, consumed);
            true
        }
        Err(e) => {
            println!("[Save] Parse failed ({} bytes): {:?}", len, e);
            false
        }
    }
}

/// Encode `ctx` to JSON and persist it. Returns true on success.
pub fn save(ctx: &mut GameContext) -> bool {
    let data = build(ctx);
    // SAFETY: see `load`.
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(JSON_BUF) };
    let len = match serde_json_core::to_slice(&data, buf) {
        Ok(n) => n,
        Err(_) => {
            println!("[Save] Encode failed");
            return false;
        }
    };
    if !storage::write_next(&buf[..len]) {
        return false;
    }
    ctx.last_save_time = Some(Instant::now());
    true
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
