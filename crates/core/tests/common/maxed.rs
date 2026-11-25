//! A `GameContext` with every saved list full and every field set to a
//! distinctive value. Used for save-size budgets and round-trip tests, and
//! (run against the v0 encoder) to produce `fixtures/v0_maxed.json`.

use catode32_core::{
    assets::plants::{PlantStage, PotKind},
    context::{
        FavWeather, FoodItem, FriendEntry, GameContext, MealEntry, ToyEntry, ToyVariant,
        WifiEntry, FOOD_ITEM_COUNT, FRIENDS_MAX, MAX_PLANTS, WIFI_FAMILIAR_MAX, WIFI_RECENT_MAX,
    },
    led::Led,
    pet_seed::{PetGender, StarSign},
    plant_system::{Plant, PlantLayer},
    scene::SceneId,
    context::SeedKind,
    time_system::{Season, Weather},
};

const ROOMS: [SceneId; 6] = [
    SceneId::Inside,
    SceneId::Outside,
    SceneId::Bedroom,
    SceneId::Kitchen,
    SceneId::Treehouse,
    SceneId::Garage,
];

const STAGES: [PlantStage; 18] = [
    PlantStage::EmptyPot,
    PlantStage::Seedling,
    PlantStage::Young,
    PlantStage::Growing,
    PlantStage::Mature,
    PlantStage::Thriving,
    PlantStage::SeedlingWilted,
    PlantStage::YoungWilted,
    PlantStage::GrowingWilted,
    PlantStage::MatureWilted,
    PlantStage::ThrivingWilted,
    PlantStage::SeedlingDead,
    PlantStage::YoungDead,
    PlantStage::GrowingDead,
    PlantStage::MatureDead,
    PlantStage::ThrivingDead,
    PlantStage::Dead,
    PlantStage::Dormant,
];

const SEEDS: [Option<SeedKind>; 5] = [
    Some(SeedKind::CatGrass),
    Some(SeedKind::Freesia),
    Some(SeedKind::Sunflower),
    Some(SeedKind::Rose),
    None,
];

const POTS: [PotKind; 5] = [
    PotKind::Small,
    PotKind::Medium,
    PotKind::Large,
    PotKind::Planter,
    PotKind::Ground,
];

const LAYERS: [PlantLayer; 3] = [PlantLayer::Background, PlantLayer::Midground, PlantLayer::Foreground];

pub fn maxed_context() -> GameContext {
    let mut ctx = GameContext::new(Led::new());

    ctx.pet_seed = 0x0123_4567_89AB_CDEF;
    ctx.pet_name.clear();
    ctx.pet_name.push_str("Señor Mochi").unwrap();
    ctx.pet_gender = Some(PetGender::Queen);
    ctx.star_sign = Some(StarSign::Capricorn);
    ctx.fav_weather = Some(FavWeather::Snowy);
    ctx.fav_meal = Some(FoodItem::Salmon);
    ctx.least_fav_meal = Some(FoodItem::Liver);
    ctx.fav_snack = Some(FoodItem::ChewStick);
    ctx.least_fav_snack = Some(FoodItem::Carrots);
    ctx.fav_toy = Some(ToyVariant::Laser);
    ctx.least_fav_toy = Some(ToyVariant::Bubbles);
    ctx.fav_location = Some(SceneId::Garage);
    ctx.least_fav_location = Some(SceneId::Treehouse);

    let stats = [
        &mut ctx.fullness, &mut ctx.energy, &mut ctx.comfort, &mut ctx.playfulness,
        &mut ctx.focus, &mut ctx.fulfillment, &mut ctx.cleanliness, &mut ctx.curiosity,
        &mut ctx.sociability, &mut ctx.intelligence, &mut ctx.maturity, &mut ctx.affection,
        &mut ctx.fitness, &mut ctx.serenity, &mut ctx.courage, &mut ctx.loyalty,
        &mut ctx.mischievousness,
    ];
    for (i, s) in stats.into_iter().enumerate() {
        *s = 12.345 + i as f32 * 4.917;
    }
    ctx.sickness = 7.3125;
    ctx.coins = 98_765;
    ctx.time_speed = 1.0;

    ctx.recent_meals.clear();
    for m in [
        MealEntry::Item(FoodItem::Tuna),
        MealEntry::CaughtSnack,
        MealEntry::Item(FoodItem::Kibble),
        MealEntry::Item(FoodItem::Puree),
        MealEntry::CaughtSnack,
    ] {
        assert!(ctx.recent_meals.push(m).is_ok());
    }
    ctx.milestone_fed = true;
    ctx.milestone_groomed = false;
    ctx.milestone_played = true;
    ctx.milestone_petted = true;
    ctx.milestone_store = false;

    for i in 0..FOOD_ITEM_COUNT {
        ctx.food_stock[i] = (i as u8 * 7 + 3) % 50;
    }
    ctx.toys.clear();
    for (i, v) in [
        ToyVariant::String_,
        ToyVariant::Feather,
        ToyVariant::Mouse,
        ToyVariant::Ball,
        ToyVariant::Bubbles,
        ToyVariant::Laser,
    ]
    .into_iter()
    .enumerate()
    {
        assert!(ctx.toys.push(ToyEntry { variant: v, durability: 10 + i as u8 * 9 }).is_ok());
    }
    ctx.pots = [1, 2, 3, 4];
    ctx.seeds = [5, 6, 7, 8];
    ctx.tools = [true, false];
    ctx.fertilizer = 9;
    ctx.medicine = 2;
    ctx.medicine_pending = true;

    ctx.friends.clear();
    for i in 0..FRIENDS_MAX {
        let mut name = heapless::String::new();
        name.push_str("Friend").unwrap();
        name.push_str(["Abby", "Bo", "Cleo", "Dot", "Echo", "Fig", "Gus", "Hal", "Ivy", "Jinx"][i]).unwrap();
        let pushed = ctx.friends
            .push(FriendEntry {
                mac: [0x10 + i as u8, 0x22, 0x33, 0x44, 0x55, 0x66],
                name,
                total_secs: 12.5 * i as f32 + 0.1,
                visits: (i * 7) as u16,
            })
            .is_ok();
        assert!(pushed);
    }

    ctx.time_hours = 23;
    ctx.time_minutes = 59;
    ctx.day_number = 1234;
    ctx.season_offset = 200;
    ctx.season = Season::Fall;
    ctx.moon_phase = 5;
    ctx.weather = Weather::Storm;
    ctx.temperature = -3.75;
    ctx.weather_step = 0xDEAD_BEEF;
    ctx.weather_timer = 33.5;
    ctx.meteor_shower_timer = 12.25;

    ctx.plants.clear();
    for i in 0..MAX_PLANTS {
        let pushed = ctx.plants
            .push(Plant {
                id: 1000 + i as u32,
                seed: SEEDS[i % SEEDS.len()],
                scene: ROOMS[i % ROOMS.len()],
                layer: LAYERS[i % LAYERS.len()],
                x: -20 + i as i32 * 3,
                y_snap: 20 + (i % 40) as i32,
                pot: POTS[i % POTS.len()],
                stage: STAGES[i % STAGES.len()],
                age_hours: 20_000 + i as u32 * 13,
                // Off-grid values, so rounding is exercised.
                water_debt: ((i * 37) % 15_400) as f32 / 40.0,
                fertilizer: ((i * 53) % 8_000) as f32 / 40.0,
                planted_day: Some(20_000 + i as u32),
                mirror: i % 2 == 1,
                aged: false,
            })
            .is_ok();
        assert!(pushed);
    }
    ctx.next_plant_id = 1000 + MAX_PLANTS as u32;

    ctx.zoomies_high_score = 54;
    ctx.maze_best_time = 1_234;
    ctx.snake_high_score = 77;
    ctx.memory_best_score = -1;
    ctx.meowltiply_high_score = 31;

    ctx.sudokat_difficulty = 2;
    for i in 0..81 {
        if i % 3 == 0 {
            ctx.sudokat_givens[i] = (i % 9 + 1) as u8;
        } else if i % 3 == 1 {
            ctx.sudokat_entries[i] = ((i + 4) % 9 + 1) as u8;
        }
    }

    ctx.wifi_familiar.clear();
    for i in 0..WIFI_FAMILIAR_MAX {
        let mut ssid = heapless::String::new();
        ssid.push_str("CafeNetwork_00").unwrap();
        ssid.push(char::from(b'A' + i as u8)).unwrap();
        ssid.push('!').unwrap();
        let pushed = ctx.wifi_familiar
            .push(WifiEntry { bssid: [0xA0, 1, 2, 3, 4, i as u8], ssid, count: 5.25 + i as f32 })
            .is_ok();
        assert!(pushed);
    }
    ctx.wifi_recent.clear();
    for i in 0..WIFI_RECENT_MAX {
        let mut ssid = heapless::String::new();
        ssid.push_str("NeighbourNet_0").unwrap();
        ssid.push(char::from(b'a' + i as u8)).unwrap();
        ssid.push('?').unwrap();
        let pushed = ctx.wifi_recent
            .push(WifiEntry { bssid: [0xB0, 9, 8, 7, 6, i as u8], ssid, count: 1.75 })
            .is_ok();
        assert!(pushed);
    }
    ctx.scan_started = true;
    ctx
}
