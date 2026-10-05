use heapless::Vec;
use crate::t;

use crate::platform::{
    radio::{WifiController, WIFI},
    rng::Rng,
    time::Instant,
};

use crate::{
    assets::plants::{PlantStage, PotKind},
    behavior::BehaviorId,
    espnow_manager::EspNowManager,
    led::Led,
    pet_seed::{PetGender, StarSign},
    plant_system::{Plant, PlantLayer},
    scene::SceneId,
    time_system::{Season, Weather},
};

pub const PET_NAME_MAX: usize = 12;

/// Caps for the wifi tracker's two AP lists.
pub const WIFI_FAMILIAR_MAX: usize = 16;
pub const WIFI_RECENT_MAX: usize = 8;

/// Max SSID length we persist with each tracked AP. Truncated to 16 at scan
/// time so saves round-trip cleanly.
pub const WIFI_SSID_MAX: usize = 16;

/// One tracked access point. `b` is the BSSID (6 raw bytes; rendered as
/// `aa:bb:cc:dd:ee:ff` in JSON), `s` is the SSID, `n` is the running
/// "times seen" count subject to per-scan decay.
#[derive(Clone, Debug)]
pub struct WifiEntry {
    pub bssid: [u8; 6],
    pub ssid: heapless::String<WIFI_SSID_MAX>,
    pub count: f32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatId {
    Fullness,
    Energy,
    Comfort,
    Playfulness,
    Focus,
    Fulfillment,
    Cleanliness,
    Intelligence,
    Maturity,
    Affection,
    Fitness,
    Serenity,
    Courage,
    Loyalty,
    Mischievousness,
    Curiosity,
    Sociability,
}

impl StatId {
    pub fn name(self) -> &'static str {
        match self {
            StatId::Fullness => "fullness",
            StatId::Energy => "energy",
            StatId::Comfort => "comfort",
            StatId::Playfulness => "playfulness",
            StatId::Focus => "focus",
            StatId::Fulfillment => "fulfillment",
            StatId::Cleanliness => "cleanliness",
            StatId::Intelligence => "intelligence",
            StatId::Maturity => "maturity",
            StatId::Affection => "affection",
            StatId::Fitness => "fitness",
            StatId::Serenity => "serenity",
            StatId::Courage => "courage",
            StatId::Loyalty => "loyalty",
            StatId::Mischievousness => "mischievousness",
            StatId::Curiosity => "curiosity",
            StatId::Sociability => "sociability",
        }
    }

    fn affected_by_sickness(self) -> bool {
        matches!(
            self,
            StatId::Serenity
                | StatId::Fulfillment
                | StatId::Playfulness
                | StatId::Comfort
                | StatId::Energy
                | StatId::Fitness
                | StatId::Focus
        )
    }
}

/// Which role this device is playing in an active visit. Determines
/// which side runs greeting/proximity-sniff/environment-broadcast logic
/// in the visit manager (Phase 5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisitRole {
    Inviter,
    Invitee,
}

/// Active playdate session. Set by the social scene the moment the
/// vreq/vok handshake completes; cleared when the visit ends (`vbye`,
/// timeout, or local exit). While `Some`, the radio refcount is held
/// across scene transitions so the visit runtime keeps receiving.
#[derive(Clone)]
pub struct VisitState {
    pub peer_mac: [u8; 6],
    pub peer_name: heapless::String<PET_NAME_MAX>,
    pub role: VisitRole,
    /// Wall-clock seconds the visit has been running. Bumped each
    /// frame by the visit manager once Phase 5 lands.
    pub play_time: f32,
    /// Becomes true after the greeting ritual has fired on local-cat
    /// scene entry. Phase-5 hook.
    pub greeted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerAction {
    Reboot,
    LightSleep,
    DeepSleep,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FavWeather {
    Sunny,
    Rainy,
    Snowy,
    Overcast,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FoodKind {
    Kibble,
    WetFood,
    Treat,
    Fish,
    CaughtSnack,
}

/// Specific food/snack items the player can purchase and keep stock of.
/// Distinct from `FoodKind`, which is the coarse category used for meal variety.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum FoodItem {
    // Meals
    Kibble = 0,
    Cod,
    Haddock,
    Trout,
    Shrimp,
    Herring,
    Turkey,
    Tuna,
    Salmon,
    Chicken,
    Liver,
    Beef,
    Lamb,
    Mackerel,
    // Snacks
    Carrots,
    Pumpkin,
    Treats,
    FishBite,
    Eggs,
    Nugget,
    Milk,
    ChewStick,
    Puree,
}

pub const FOOD_ITEM_COUNT: usize = 23;

impl FoodItem {
    /// Map a specific food item to the coarse `FoodKind` used by the eating
    /// behavior for its bonus table.
    pub fn kind(self) -> FoodKind {
        match self {
            FoodItem::Kibble => FoodKind::Kibble,
            FoodItem::Cod
            | FoodItem::Haddock
            | FoodItem::Trout
            | FoodItem::Shrimp
            | FoodItem::Herring
            | FoodItem::Tuna
            | FoodItem::Salmon
            | FoodItem::Mackerel
            | FoodItem::FishBite => FoodKind::Fish,
            FoodItem::Turkey
            | FoodItem::Chicken
            | FoodItem::Liver
            | FoodItem::Beef
            | FoodItem::Lamb => FoodKind::WetFood,
            FoodItem::Treats
            | FoodItem::ChewStick
            | FoodItem::Nugget
            | FoodItem::Eggs => FoodKind::Treat,
            FoodItem::Puree
            | FoodItem::Milk
            | FoodItem::Pumpkin
            | FoodItem::Carrots => FoodKind::CaughtSnack,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FoodItem::Kibble => t!("Kibble"),
            FoodItem::Cod => t!("Cod"),
            FoodItem::Haddock => t!("Haddock"),
            FoodItem::Trout => t!("Trout"),
            FoodItem::Shrimp => t!("Shrimp"),
            FoodItem::Herring => t!("Herring"),
            FoodItem::Turkey => t!("Turkey"),
            FoodItem::Tuna => t!("Tuna"),
            FoodItem::Salmon => t!("Salmon"),
            FoodItem::Chicken => t!("Chicken"),
            FoodItem::Liver => t!("Liver"),
            FoodItem::Beef => t!("Beef"),
            FoodItem::Lamb => t!("Lamb"),
            FoodItem::Mackerel => t!("Mackerel"),
            FoodItem::Carrots => t!("Carrots"),
            FoodItem::Pumpkin => t!("Pumpkin"),
            FoodItem::Treats => t!("Treats"),
            FoodItem::FishBite => t!("Fish Bite"),
            FoodItem::Eggs => t!("Eggs"),
            FoodItem::Nugget => t!("Nugget"),
            FoodItem::Milk => t!("Milk"),
            FoodItem::ChewStick => t!("Chew Stick"),
            FoodItem::Puree => t!("Puree"),
        }
    }

    pub fn is_snack(self) -> bool {
        matches!(
            self,
            FoodItem::Carrots
                | FoodItem::Pumpkin
                | FoodItem::Treats
                | FoodItem::FishBite
                | FoodItem::Eggs
                | FoodItem::Nugget
                | FoodItem::Milk
                | FoodItem::ChewStick
                | FoodItem::Puree
        )
    }
}

/// Per-frame snapshot of player input. LocationScene refreshes this from the
/// current `Buttons` view before invoking the behavior layer so behaviors
/// (currently only Playing) can read held directions and one-shot presses.
#[derive(Clone, Copy, Default)]
pub struct InputSnapshot {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
    pub a_just_pressed: bool,
    pub b_just_pressed: bool,
}

/// What the eating behavior records in `recent_meals`. Typed so the variety
/// penalty can compare entries cleanly.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MealEntry {
    Item(FoodItem),
    CaughtSnack,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum ToyVariant {
    String_ = 0,
    Feather,
    Mouse,
    Ball,
    Bubbles,
    Laser,
}

pub const TOY_VARIANT_COUNT: usize = 6;

impl ToyVariant {
    pub const fn max_durability(self) -> u8 {
        match self {
            ToyVariant::String_ => 28,
            ToyVariant::Feather => 28,
            ToyVariant::Mouse => 42,
            ToyVariant::Ball => 42,
            ToyVariant::Bubbles => 35,
            ToyVariant::Laser => 100,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            ToyVariant::String_ => t!("String"),
            ToyVariant::Feather => t!("Feather"),
            ToyVariant::Mouse => t!("Mouse"),
            ToyVariant::Ball => t!("Yarn Ball"),
            ToyVariant::Bubbles => t!("Bubbles"),
            ToyVariant::Laser => t!("Laser"),
        }
    }

    pub fn icon(self) -> &'static [u8] {
        use crate::assets::icons;
        match self {
            ToyVariant::String_ => icons::STRING_ICON,
            ToyVariant::Feather => icons::FEATHER,
            ToyVariant::Mouse => icons::MOUSE,
            ToyVariant::Ball => icons::TOYS,
            ToyVariant::Bubbles => icons::BUBBLES,
            ToyVariant::Laser => icons::LASER,
        }
    }

    pub fn to_play_variant(self) -> crate::behavior::PlayVariant {
        use crate::behavior::PlayVariant;
        match self {
            ToyVariant::String_ => PlayVariant::String,
            ToyVariant::Feather => PlayVariant::Feather,
            ToyVariant::Mouse => PlayVariant::Mouse,
            ToyVariant::Ball => PlayVariant::Ball,
            ToyVariant::Bubbles => PlayVariant::Bubbles,
            ToyVariant::Laser => PlayVariant::Laser,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ToyEntry {
    pub variant: ToyVariant,
    pub durability: u8,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum PotSize {
    Small = 0,
    Medium,
    Large,
    Planter,
}

pub const POT_SIZE_COUNT: usize = 4;

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum SeedKind {
    CatGrass = 0,
    Freesia,
    Sunflower,
    Rose,
}

pub const SEED_KIND_COUNT: usize = 4;

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum ToolKind {
    Spade = 0,
    WateringCan,
}

pub const TOOL_COUNT: usize = 2;

pub const RECENT_HISTORY: usize = 5;

/// Global cap on live plants across every scene. Sized for "16 pots per
/// scene" across 5 plantable scenes.
pub const MAX_PLANTS: usize = 80;

/// Pending cross-scene move request. Set when the player picks "Move to <scene>"
/// in the Tend menu; consumed when the destination scene's enter() fires.
#[derive(Clone, Copy)]
pub struct PendingGardeningMove {
    pub plant_id: u32,
    pub dest_scene: SceneId,
}

/// Number of plants populated by `starter_plants()`. Drives the initial value
/// of `next_plant_id` so the first player-placed plant uses a fresh id.
pub const STARTER_PLANT_COUNT: usize = 8;

/// Developer-seeded plants placed in each room on first boot, so the world
/// feels inhabited without immediately demanding player attention.
pub fn starter_plants() -> Vec<Plant, MAX_PLANTS> {
    use crate::plant_system::Plant as P;
    let mut v: Vec<Plant, MAX_PLANTS> = Vec::new();
    let mut id: u32 = 0;
    let mut push = |seed: Option<crate::context::SeedKind>,
                    scene: SceneId,
                    layer: PlantLayer,
                    x: i32,
                    y_snap: i32,
                    pot: PotKind,
                    stage: PlantStage,
                    age_hours: u32,
                    mirror: bool|
     -> Plant {
        let plant = P {
            id,
            seed,
            scene,
            layer,
            x,
            y_snap,
            pot,
            stage,
            age_hours,
            water_debt: 0.0,
            fertilizer: 0.0,
            planted_day: Some(0),
            mirror,
            aged: false,
        };
        id += 1;
        plant
    };
    let _ = v.push(push(
        Some(SeedKind::Rose), SceneId::Inside, PlantLayer::Midground,
        110, 29, PotKind::Small, PlantStage::Growing, 200, false,
    ));
    let _ = v.push(push(
        Some(SeedKind::CatGrass), SceneId::Kitchen, PlantLayer::Midground,
        130, 24, PotKind::Medium, PlantStage::Mature, 160, false,
    ));
    let _ = v.push(push(
        Some(SeedKind::CatGrass), SceneId::Outside, PlantLayer::Foreground,
        10, 63, PotKind::Small, PlantStage::Growing, 160, false,
    ));
    let _ = v.push(push(
        Some(SeedKind::Sunflower), SceneId::Outside, PlantLayer::Midground,
        130, 61, PotKind::Ground, PlantStage::Mature, 360, true,
    ));
    let _ = v.push(push(
        Some(SeedKind::CatGrass), SceneId::Outside, PlantLayer::Midground,
        40, 61, PotKind::Ground, PlantStage::Thriving, 150, false,
    ));
    let _ = v.push(push(
        Some(SeedKind::CatGrass), SceneId::Outside, PlantLayer::Background,
        110, 56, PotKind::Ground, PlantStage::Thriving, 320, false,
    ));
    let _ = v.push(push(
        Some(SeedKind::Rose), SceneId::Treehouse, PlantLayer::Foreground,
        15, 63, PotKind::Small, PlantStage::Young, 72, false,
    ));
    let _ = v.push(push(
        Some(SeedKind::Freesia), SceneId::Treehouse, PlantLayer::Midground,
        120, 59, PotKind::Medium, PlantStage::Thriving, 88, true,
    ));
    v
}

#[allow(dead_code)]
pub struct GameContext {
    pub health: f32,
    pub fullness: f32,
    pub energy: f32,
    pub comfort: f32,
    pub playfulness: f32,
    pub focus: f32,

    pub fulfillment: f32,
    pub cleanliness: f32,
    pub intelligence: f32,
    pub maturity: f32,
    pub affection: f32,

    pub fitness: f32,
    pub serenity: f32,

    pub courage: f32,
    pub loyalty: f32,
    pub mischievousness: f32,
    pub curiosity: f32,
    pub sociability: f32,

    pub sickness: f32,

    pub time_speed: f32,

    pub coins: i32,

    // Inventory, separate per-category storage. Counts are indexed by enum
    // discriminant (use `as usize`). Toys are a sparse list with per-instance
    // durability since each variant has at most one entry in the player's bag.
    pub food_stock: [u8; FOOD_ITEM_COUNT],
    pub toys: Vec<ToyEntry, TOY_VARIANT_COUNT>,
    pub pots: [u8; POT_SIZE_COUNT],
    pub seeds: [u8; SEED_KIND_COUNT],
    pub tools: [bool; TOOL_COUNT],
    pub fertilizer: u8,
    pub medicine: u8,
    /// Set when the player administers a dose; cleared by a sleep/nap cycle.
    /// Drives the healing tick during the next rest.
    pub medicine_pending: bool,

    pub zoomies_high_score: i32,
    pub maze_best_time: i32,
    pub snake_high_score: i32,
    pub memory_best_score: i32,

    // World/environment state, advanced by TimeSystem each frame.
    pub time_hours: u8,
    pub time_minutes: u8,
    pub day_number: u32,
    pub season_offset: u16,
    pub season: Season,
    pub moon_phase: u8,
    pub weather: Weather,
    pub temperature: f32,
    pub weather_step: u32,
    pub weather_timer: f32,
    pub meteor_shower_timer: f32,

    pub last_main_scene: SceneId,

    // Behavior-system state.
    pub recent_behaviors: Vec<BehaviorId, RECENT_HISTORY>,
    pub current_behavior_name: Option<&'static str>,
    pub pending_wake_greeting: bool,

    // Scene-supplied bounds and props (set by scenes during enter()).
    pub scene_x_min: i32,
    pub scene_x_max: i32,
    pub cat_bed_x: Option<i32>,
    pub in_cat_bed: bool,
    /// Foreground-layer camera offset, refreshed by LocationScene each frame
    /// so behaviors can convert character world-x into screen-x without
    /// plumbing the environment through every call.
    pub scene_camera_x: i32,

    /// Per-frame input view set by the active scene before behavior updates
    /// so behaviors (currently only Playing) can react to held directions
    /// and one-shot B/A presses.
    pub input: InputSnapshot,

    // RNG seed used by the behavior layer.
    pub rng: u32,
    // Hardware RNG handle for code paths that need fresh entropy (e.g. adoption
    // seed generation). Carries no state; `Rng` is a zero-sized peripheral
    // marker; it's stashed on the context so scenes can reach it without
    // plumbing a separate parameter.
    pub hw_rng: Rng,

    /// On-board WS2812 RGB LED handle (GPIO8). Stored on the context so the
    /// LED debug scene can drive it without a separate plumbing path.
    pub led: Led,

    // Cross-scene signals.
    pub pending_scene: Option<SceneId>,
    pub pending_popup_icon: Option<&'static str>,
    pub pending_power: Option<PowerAction>,
    /// Set by the Vocalizing behavior when it transitions from
    /// WindingUp into the actual Vocalizing phase. `LocationScene`
    /// reads-and-clears this each frame and, if the cat is currently
    /// in a scene that has the ESP-NOW radio acquired
    /// (outside/treehouse), broadcasts a `voc ` frame so nearby cats
    /// can hear the vocalization.
    pub pending_vocalize_broadcast: Option<&'static str>,
    /// Target x for the next Greeting trigger. Set by `LocationScene`
    /// when entering a scene during an active visit or when a
    /// `vgrt` / `vprx` packet arrives; read-and-cleared by
    /// `GreetingBehavior::enter`. `None` means "sniff in place".
    pub pending_greeting_target_x: Option<i32>,

    /// Aggregate plant-health score for the current scene. Recomputed every
    /// frame by LocationScene from `ctx.plants` (thriving +2, healthy +1,
    /// wilted/dormant -1, dead -2). Behaviors read it during completion
    /// bonuses instead of walking `ctx.plants` themselves.
    pub scene_plant_health: i8,
    pub in_familiar_location: bool,

    /// Long-lived AP list. Survives reboots via the save file. Entries here
    /// are what `in_familiar_location` checks against.
    pub wifi_familiar: Vec<WifiEntry, WIFI_FAMILIAR_MAX>,
    /// Short-lived AP list. Entries here are candidates that haven't been
    /// seen enough times yet to count as "familiar".
    pub wifi_recent: Vec<WifiEntry, WIFI_RECENT_MAX>,
    /// Set by the debug scene's "Scan" action; Game::update consumes it
    /// next frame and drives a synchronous scan. Out-of-band from the
    /// hourly midpoint scan so the debug UI can force a fresh result.
    pub wifi_scan_requested: bool,
    /// True only while writing the pre-scan save, so that save records
    /// "a scan started at this save's moment". Loaded back on boot to defer
    /// the first scan, since no clock survives a reset.
    pub scan_started: bool,
    /// Hardware reset reason for this boot, for the debug wifi screen. Read
    /// from a chip register at boot; never saved.
    pub last_reset_reason: heapless::String<24>,

    /// ESP-NOW transport. The manager struct itself is always present;
    /// its inner driver handle is bound only while
    /// [`crate::radio::acquire`] has the radio up.
    pub espnow: Option<EspNowManager>,

    /// WiFi radio controller. `Some` only while the radio is acquired;
    /// `None` at rest. Managed by [`crate::radio`].
    pub wifi: Option<WifiController<'static>>,

    /// Stash for the WiFi peripheral while the radio is off. `wifi::new`
    /// consumes this on acquire; teardown steals it back via
    /// `WIFI::steal()` so the next acquire can re-init.
    pub wifi_peripheral: Option<WIFI<'static>>,

    /// Refcount of how many subsystems currently want the radio up.
    /// Bumped by [`crate::radio::acquire`], decremented by
    /// [`crate::radio::release`]; the actual init/teardown happens on
    /// the 0/1 transitions.
    pub radio_users: u8,

    /// Active playdate, if any. Owns one slot in `radio_users` for as
    /// long as it is `Some`. The social scene takes that ref on
    /// handshake success and the visit manager (Phase 5) releases it
    /// on visit end.
    pub visit: Option<VisitState>,

    /// True while the player is on a vacation scene. Suppresses the
    /// behavior-layer's auto-pick scene exit so the pet stays put.
    pub on_vacation: bool,
    /// Set by the active vacation scene once the enjoyment cap is reached;
    /// drives the vocalizing behavior to surface a "home" bubble icon.
    pub wants_to_go_home: bool,

    // --- Plant / gardening state ---
    pub plants: Vec<Plant, MAX_PLANTS>,
    pub next_plant_id: u32,
    /// Absolute in-game hour index (day_number * 24 + time_hours) at the last
    /// plant tick. None on cold boot, first tick anchors instead of catching
    /// up so saved water_debt remains authoritative.
    pub last_plant_tick_hour: Option<u32>,
    pub pending_gardening_move: Option<PendingGardeningMove>,

    // Pet identity, set during the adoption scene from a 64-bit seed.
    pub pet_seed: u64,
    pub pet_name: heapless::String<PET_NAME_MAX>,
    pub pet_gender: Option<PetGender>,
    pub star_sign: Option<StarSign>,

    // Personality-derived favorites. All set during adoption from `pet_seed`.
    pub fav_weather: Option<FavWeather>,
    pub fav_meal: Option<FoodItem>,
    pub least_fav_meal: Option<FoodItem>,
    pub fav_snack: Option<FoodItem>,
    pub least_fav_snack: Option<FoodItem>,
    pub fav_toy: Option<ToyVariant>,
    pub least_fav_toy: Option<ToyVariant>,
    pub fav_location: Option<SceneId>,
    pub least_fav_location: Option<SceneId>,

    /// Last few meals (per-item granularity). Drives the eating-behavior
    /// variety penalty and the snack-streak sickness ramp.
    pub recent_meals: Vec<MealEntry, RECENT_HISTORY>,

    // First-run tutorial state.
    pub first_impressions: bool,
    // Milestones tracked by interaction behaviors for first-time bonuses.
    pub milestone_fed: bool,
    pub milestone_petted: bool,
    pub milestone_played: bool,
    pub milestone_groomed: bool,
    pub milestone_store: bool,

    /// Instant of the most recent successful save. `None` until the first
    /// save (or load, which also stamps this). Drives `save::save_if_needed`.
    pub last_save_time: Option<Instant>,
}

impl GameContext {
    pub fn new(led: Led) -> Self {
        Self {
            health: 50.0,
            fullness: 50.0,
            energy: 50.0,
            comfort: 50.0,
            playfulness: 50.0,
            focus: 50.0,
            fulfillment: 50.0,
            cleanliness: 50.0,
            intelligence: 50.0,
            maturity: 50.0,
            affection: 50.0,
            fitness: 50.0,
            serenity: 50.0,
            courage: 50.0,
            loyalty: 50.0,
            mischievousness: 50.0,
            curiosity: 50.0,
            sociability: 50.0,
            sickness: 0.0,
            time_speed: 1.0,
            coins: 150,
            food_stock: {
                let mut s = [0u8; FOOD_ITEM_COUNT];
                s[FoodItem::Kibble as usize] = 5;
                s[FoodItem::Nugget as usize] = 3;
                s
            },
            toys: Vec::new(),
            pots: [0u8; POT_SIZE_COUNT],
            seeds: [0u8; SEED_KIND_COUNT],
            tools: [false; TOOL_COUNT],
            fertilizer: 0,
            medicine: 0,
            medicine_pending: false,
            zoomies_high_score: 0,
            maze_best_time: 0,
            snake_high_score: 0,
            memory_best_score: -1,

            time_hours: 12,
            time_minutes: 0,
            day_number: 0,
            // All pets start in late spring.
            season_offset: 120,
            season: Season::Spring,
            moon_phase: 2,
            weather: Weather::Clear,
            temperature: 20.0,
            weather_step: 0,
            weather_timer: 0.0,
            meteor_shower_timer: 0.0,

            last_main_scene: SceneId::Inside,

            recent_behaviors: Vec::new(),
            current_behavior_name: None,
            pending_wake_greeting: false,

            scene_x_min: 10,
            scene_x_max: 118,
            cat_bed_x: None,
            in_cat_bed: false,
            scene_camera_x: 0,
            input: InputSnapshot::default(),

            rng: 0xC0FFEEu32,
            hw_rng: Rng::new(),
            led,

            pending_scene: None,
            pending_popup_icon: None,
            pending_vocalize_broadcast: None,
            pending_greeting_target_x: None,
            pending_power: None,

            scene_plant_health: 0,
            in_familiar_location: true,
            wifi_familiar: Vec::new(),
            wifi_recent: Vec::new(),
            wifi_scan_requested: false,
            scan_started: false,
            last_reset_reason: heapless::String::new(),
            espnow: None,
            wifi: None,
            wifi_peripheral: None,
            radio_users: 0,
            visit: None,

            on_vacation: false,
            wants_to_go_home: false,

            plants: starter_plants(),
            next_plant_id: STARTER_PLANT_COUNT as u32,
            last_plant_tick_hour: None,
            pending_gardening_move: None,

            pet_seed: 0,
            pet_name: heapless::String::new(),
            pet_gender: None,
            star_sign: None,

            fav_weather: None,
            fav_meal: None,
            least_fav_meal: None,
            fav_snack: None,
            least_fav_snack: None,
            fav_toy: None,
            least_fav_toy: None,
            fav_location: None,
            least_fav_location: None,

            recent_meals: Vec::new(),

            first_impressions: false,
            milestone_fed: false,
            milestone_petted: false,
            milestone_played: false,
            milestone_groomed: false,
            milestone_store: false,

            last_save_time: None,
        }
    }

    pub fn meteor_shower_happening(&self) -> bool {
        self.meteor_shower_timer > 0.0
    }

    /// Daylight predicate. True when the in-game hour is between 6 (sunrise)
    /// and 20 (sunset). Drives critter spawn pools and any other day/night
    /// gameplay gate.
    pub fn is_daytime(&self) -> bool {
        (6..20).contains(&self.time_hours)
    }

    /// Reset every numeric stat to its default baseline. Personality traits
    /// fall back to `50 + seed-derived offset` so pet identity survives;
    /// nothing else (inventory, plants, favorites, name, scores) is touched.
    /// Surfaced by the debug-context scene.
    pub fn reset_stats_to_defaults(&mut self) {
        self.fullness = 50.0;
        self.energy = 50.0;
        self.comfort = 50.0;
        self.playfulness = 50.0;
        self.focus = 50.0;
        self.fulfillment = 50.0;
        self.cleanliness = 50.0;
        self.intelligence = 50.0;
        self.maturity = 50.0;
        self.affection = 50.0;
        self.fitness = 50.0;
        self.serenity = 50.0;
        let offsets = crate::pet_seed::derive_trait_offsets(self.pet_seed);
        self.courage = 50.0 + offsets[0] as f32;
        self.loyalty = 50.0 + offsets[1] as f32;
        self.mischievousness = 50.0 + offsets[2] as f32;
        self.curiosity = 50.0 + offsets[3] as f32;
        self.sociability = 50.0 + offsets[4] as f32;
        self.sickness = 0.0;
        self.medicine_pending = false;
        self.recompute_health();
    }

    /// Apply a new pet seed and re-derive every field that depends on it
    /// (gender, star sign, favorites, personality offsets). Stats, inventory,
    /// plants, and name are untouched.
    pub fn reseed(&mut self, new_seed: u64) {
        self.pet_seed = new_seed;
        let favs = crate::pet_seed::derive_favorites(new_seed);
        self.pet_gender = Some(favs.pet_gender);
        self.star_sign = Some(favs.star_sign);
        self.fav_weather = Some(favs.fav_weather);
        self.fav_meal = Some(favs.fav_meal);
        self.least_fav_meal = Some(favs.least_fav_meal);
        self.fav_snack = Some(favs.fav_snack);
        self.least_fav_snack = Some(favs.least_fav_snack);
        self.fav_toy = Some(favs.fav_toy);
        self.least_fav_toy = Some(favs.least_fav_toy);
        self.fav_location = Some(favs.fav_location);
        self.least_fav_location = Some(favs.least_fav_location);
        let offsets = crate::pet_seed::derive_trait_offsets(new_seed);
        self.courage = 50.0 + offsets[0] as f32;
        self.loyalty = 50.0 + offsets[1] as f32;
        self.mischievousness = 50.0 + offsets[2] as f32;
        self.curiosity = 50.0 + offsets[3] as f32;
        self.sociability = 50.0 + offsets[4] as f32;
    }

    /// Replace the player's plants with the developer starter set. Surfaced
    /// by the debug-context scene's "Reset Plants" action.
    pub fn reset_plants_to_starter(&mut self) {
        self.plants = starter_plants();
        self.next_plant_id = STARTER_PLANT_COUNT as u32;
        self.last_plant_tick_hour = None;
    }

    pub fn recompute_health(&mut self) {
        let raw = 0.25 * self.fullness
            + 0.20 * self.fitness
            + 0.20 * self.energy
            + 0.15 * self.cleanliness
            + 0.05 * self.comfort
            + 0.05 * self.affection
            + 0.025 * self.fulfillment
            + 0.025 * self.focus
            + 0.025 * self.intelligence
            + 0.025 * self.playfulness;
        self.health = raw.clamp(0.0, 100.0);
    }

    fn stat_mut(&mut self, id: StatId) -> &mut f32 {
        match id {
            StatId::Fullness => &mut self.fullness,
            StatId::Energy => &mut self.energy,
            StatId::Comfort => &mut self.comfort,
            StatId::Playfulness => &mut self.playfulness,
            StatId::Focus => &mut self.focus,
            StatId::Fulfillment => &mut self.fulfillment,
            StatId::Cleanliness => &mut self.cleanliness,
            StatId::Intelligence => &mut self.intelligence,
            StatId::Maturity => &mut self.maturity,
            StatId::Affection => &mut self.affection,
            StatId::Fitness => &mut self.fitness,
            StatId::Serenity => &mut self.serenity,
            StatId::Courage => &mut self.courage,
            StatId::Loyalty => &mut self.loyalty,
            StatId::Mischievousness => &mut self.mischievousness,
            StatId::Curiosity => &mut self.curiosity,
            StatId::Sociability => &mut self.sociability,
        }
    }

    /// Apply a batch of stat changes with asymptotic damping near 0 and 100.
    /// Stats near their ceiling resist further increases; stats near the floor
    /// resist further decreases.
    pub fn apply_stat_changes(&mut self, changes: &[(StatId, f32)]) {
        use crate::println;
        use micromath::F32Ext;
        const EXP: f32 = 0.7;
        let sickness = self.sickness;
        for &(stat, delta) in changes {
            if delta == 0.0 {
                continue;
            }
            let cur = *self.stat_mut(stat);
            let mut d = delta;
            if d > 0.0 {
                let room = ((100.0 - cur) / 100.0).max(0.0);
                d *= room.powf(EXP);
                if stat.affected_by_sickness() {
                    if sickness >= 8.0 {
                        d *= 0.4;
                    } else if sickness >= 5.0 {
                        d *= 0.6;
                    } else if sickness >= 2.0 {
                        d *= 0.8;
                    }
                }
            } else {
                let room = (cur / 100.0).max(0.0);
                d *= room.powf(EXP);
            }
            let new_val = (cur + d).clamp(0.0, 100.0);
            let color = if d >= 0.0 { "\x1b[32m" } else { "\x1b[31m" };
            println!(
                "[\x1b[36mStat\x1b[0m] {}: {:.1} {}{:+.2}\x1b[0m -> {:.1}",
                stat.name(),
                cur,
                color,
                d,
                new_val
            );
            *self.stat_mut(stat) = new_val;
        }
        self.recompute_health();
    }

    /// Push a behavior id onto the recent-history ring; index 0 = most recent.
    pub fn record_behavior(&mut self, id: BehaviorId) {
        if self.recent_behaviors.is_full() {
            self.recent_behaviors.pop();
        }
        let _ = self.recent_behaviors.insert(0, id);
    }

    pub fn record_meal(&mut self, entry: MealEntry) {
        if self.recent_meals.is_full() {
            self.recent_meals.pop();
        }
        let _ = self.recent_meals.insert(0, entry);
    }

    /// Recency index of `id` in the recent-behaviors ring, or None.
    pub fn recent_index(&self, id: BehaviorId) -> Option<usize> {
        self.recent_behaviors.iter().position(|b| *b == id)
    }

    pub fn add_food_stock(&mut self, item: FoodItem, uses: u8) {
        let i = item as usize;
        self.food_stock[i] = self.food_stock[i].saturating_add(uses);
    }

    pub fn add_pot(&mut self, pot: PotSize) {
        let i = pot as usize;
        self.pots[i] = self.pots[i].saturating_add(1);
    }

    pub fn add_seeds(&mut self, seed: SeedKind, n: u8) {
        let i = seed as usize;
        self.seeds[i] = self.seeds[i].saturating_add(n);
    }

    pub fn owns_tool(&self, tool: ToolKind) -> bool {
        self.tools[tool as usize]
    }

    pub fn set_tool(&mut self, tool: ToolKind, owned: bool) {
        self.tools[tool as usize] = owned;
    }

    pub fn find_toy(&self, variant: ToyVariant) -> Option<usize> {
        self.toys.iter().position(|t| t.variant == variant)
    }

    pub fn add_toy(&mut self, variant: ToyVariant) -> bool {
        if self.find_toy(variant).is_some() {
            return false;
        }
        self.toys
            .push(ToyEntry {
                variant,
                durability: variant.max_durability(),
            })
            .is_ok()
    }

    pub fn refresh_toy(&mut self, variant: ToyVariant) -> bool {
        if let Some(idx) = self.find_toy(variant) {
            self.toys[idx].durability = variant.max_durability();
            true
        } else {
            false
        }
    }
}
