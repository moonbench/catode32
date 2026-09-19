use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
use crate::t;

use crate::{
    assets::store as art,
    context::{FoodItem, GameContext, PotSize, SeedKind, StatId, ToolKind, ToyVariant},
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

#[derive(Clone, Copy)]
pub enum ServiceKind {
    Groom,
    Train,
}

#[derive(Clone, Copy)]
pub enum StoreAction {
    BuyFood(FoodItem, u8),
    BuyToy(ToyVariant, u8),
    BuyPot(PotSize, u8),
    BuySeeds(SeedKind, u8),
    BuyTool(ToolKind, u8),
    BuyFertilizer(u8),
    BuyMedicine(u8),
    BuyService(ServiceKind, u8),
    BuyTrip(SceneId, u8),
    Leave,
}

const FOOD_USES: u8 = 5;
const SEEDS_PER_PACK: u8 = 3;
const FERTILIZER_COST: u8 = 25;
const MEDICINE_COST: u8 = 50;
const GROOM_COST: u8 = 50;
const TRAIN_COST: u8 = 100;

const MENU_WIDTH: i32 = 60;
const ART_PANEL_X: i32 = 64;

const COIN_X: i32 = 84;
const COIN_Y: i32 = 29;

const FOOD: &[MenuItem<StoreAction>] = &[
    food_item(t!("Kibble"),  FoodItem::Kibble,  5,  "Kibble(5): 5c"),
    food_item(t!("Cod"),     FoodItem::Cod,     6,  "Cod(5): 6c"),
    food_item(t!("Haddock"), FoodItem::Haddock, 7,  "Haddock(5): 7c"),
    food_item(t!("Trout"),   FoodItem::Trout,   8,  "Trout(5): 8c"),
    food_item(t!("Shrimp"),  FoodItem::Shrimp,  9,  "Shrimp(5): 9c"),
    food_item(t!("Herring"), FoodItem::Herring, 10, "Herring(5): 10c"),
    food_item(t!("Turkey"),  FoodItem::Turkey,  10, "Turkey(5): 10c"),
    food_item(t!("Tuna"),    FoodItem::Tuna,    12, "Tuna(5): 12c"),
    food_item(t!("Salmon"),  FoodItem::Salmon,  12, "Salmon(5): 12c"),
    food_item(t!("Chicken"), FoodItem::Chicken, 13, "Chicken(5): 13c"),
    food_item(t!("Liver"),   FoodItem::Liver,   14, "Liver(5): 14c"),
    food_item(t!("Beef"),    FoodItem::Beef,    14, "Beef(5): 14c"),
    food_item(t!("Lamb"),    FoodItem::Lamb,    15, "Lamb(5): 15c"),
];

const SNACKS: &[MenuItem<StoreAction>] = &[
    food_item(t!("Carrots"), FoodItem::Carrots,  2, "Carrots(5): 2c"),
    food_item(t!("Pumpkin"), FoodItem::Pumpkin,  2, "Pumpkin(5): 2c"),
    food_item(t!("Treats"),  FoodItem::Treats,   3, "Treats(5): 3c"),
    food_item(t!("Bytes"),   FoodItem::FishBite, 4, "Bytes(5): 4c"),
    food_item(t!("Eggs"),    FoodItem::Eggs,     5, "Eggs(5): 5c"),
    food_item(t!("Nuggets"), FoodItem::Nugget,   5, "Nuggets(5): 5c"),
    food_item(t!("Milk"),    FoodItem::Milk,     6, "Milk(5): 6c"),
    food_item(t!("Sticks"),  FoodItem::ChewStick, 6, "Sticks(5): 6c"),
    food_item(t!("Puree"),   FoodItem::Puree,    8, "Puree(5): 8c"),
];

const TOYS: &[MenuItem<StoreAction>] = &[
    toy_item(t!("String"),  ToyVariant::String_, 20, "String: 20c"),
    toy_item(t!("Feather"), ToyVariant::Feather, 35, "Feather: 35c"),
    toy_item(t!("Mouse"),   ToyVariant::Mouse,   40, "Mouse Toy: 40c"),
    toy_item(t!("Yarn"),    ToyVariant::Ball,    50, "Yarn Ball: 50c"),
    toy_item(t!("Bubbles"), ToyVariant::Bubbles, 45, "Bubbles: 45c"),
    toy_item(t!("Laser"),   ToyVariant::Laser,   75, "Laser Pointer: 75c"),
];

const POTS: &[MenuItem<StoreAction>] = &[
    pot_item(t!("Small"),   PotSize::Small,   15, "Small pot: 15c"),
    pot_item(t!("Medium"),  PotSize::Medium,  25, "Medium pot: 25c"),
    pot_item(t!("Large"),   PotSize::Large,   40, "Large pot: 40c"),
    pot_item(t!("Planter"), PotSize::Planter, 55, "Planter box: 55c"),
];

const SEEDS: &[MenuItem<StoreAction>] = &[
    seed_item(t!("Grass"),   SeedKind::CatGrass,  4,  "Cat Grass x3: 4c"),
    seed_item(t!("Freesia"), SeedKind::Freesia,   10, "Freesia x3: 10c"),
    seed_item(t!("Sun"),     SeedKind::Sunflower, 12, "Sunflower x3: 12c"),
    seed_item(t!("Rose"),    SeedKind::Rose,      15, "Rose x3: 15c"),
];

const TOOLS: &[MenuItem<StoreAction>] = &[
    tool_item(t!("Spade"),  ToolKind::Spade,       40, "Spade: 40c"),
    tool_item(t!("W. Can"), ToolKind::WateringCan, 50, "Watering Can: 50c"),
];

const GARDEN: &[MenuItem<StoreAction>] = &[
    MenuItem { label: t!("Pots"),       icon: None, submenu: Some(POTS),  action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Seeds"),      icon: None, submenu: Some(SEEDS), action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Tools"),      icon: None, submenu: Some(TOOLS), action: None, confirm: None, confirm_on_vacation: None },
    MenuItem {
        label: t!("Fertilizer"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyFertilizer(FERTILIZER_COST)),
        confirm: None,
        confirm_on_vacation: None,
    },
];

const SERVICE: &[MenuItem<StoreAction>] = &[
    MenuItem {
        label: t!("Groom"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyService(ServiceKind::Groom, GROOM_COST)),
        confirm: None,
        confirm_on_vacation: None,
    },
    MenuItem {
        label: t!("Train"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyService(ServiceKind::Train, TRAIN_COST)),
        confirm: None,
        confirm_on_vacation: None,
    },
];

const TRIPS: &[MenuItem<StoreAction>] = &[
    MenuItem {
        label: t!("Park"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyTrip(SceneId::VacationPark, 15)),
        confirm: None,
        confirm_on_vacation: None,
    },
    MenuItem {
        label: t!("Forest"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyTrip(SceneId::VacationForest, 25)),
        confirm: None,
        confirm_on_vacation: None,
    },
    MenuItem {
        label: t!("Aqua."),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyTrip(SceneId::VacationAquarium, 50)),
        confirm: None,
        confirm_on_vacation: None,
    },
    MenuItem {
        label: t!("Beach"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyTrip(SceneId::VacationBeach, 100)),
        confirm: None,
        confirm_on_vacation: None,
    },
];

const ROOT: &[MenuItem<StoreAction>] = &[
    MenuItem { label: t!("Food"),    icon: None, submenu: Some(FOOD),    action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Snacks"),  icon: None, submenu: Some(SNACKS),  action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Toys"),    icon: None, submenu: Some(TOYS),    action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Garden"),  icon: None, submenu: Some(GARDEN),  action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Service"), icon: None, submenu: Some(SERVICE), action: None, confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Trips"),   icon: None, submenu: Some(TRIPS),   action: None, confirm: None, confirm_on_vacation: None },
    MenuItem {
        label: t!("Meds."),
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyMedicine(MEDICINE_COST)),
        confirm: None,
        confirm_on_vacation: None,
    },
    MenuItem {
        label: t!("Exit"),
        icon: None,
        submenu: None,
        action: Some(StoreAction::Leave),
        confirm: None,
        confirm_on_vacation: None,
    },
];

fn pot_full_label(p: PotSize) -> &'static str {
    match p {
        PotSize::Small => t!("Small pot"),
        PotSize::Medium => t!("Medium pot"),
        PotSize::Large => t!("Large pot"),
        PotSize::Planter => t!("Planter box"),
    }
}

fn seed_full_label(s: SeedKind) -> &'static str {
    match s {
        SeedKind::CatGrass => t!("Cat Grass"),
        SeedKind::Freesia => t!("Freesia"),
        SeedKind::Sunflower => t!("Sunflower"),
        SeedKind::Rose => t!("Rose"),
    }
}

fn tool_label(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Spade => t!("Spade"),
        ToolKind::WateringCan => t!("Watering Can"),
    }
}

// Per-item confirm strings are built dynamically at action time so they can
// pull from translation keys like "{item}({uses}): {cost}c". The `_confirm`
// parameter on these helpers is unused but kept to keep the call sites
// readable as "<label>, ..., <visible price>" rows.
const fn food_item(label: &'static str, item: FoodItem, cost: u8, _confirm: &'static str) -> MenuItem<StoreAction> {
    MenuItem {
        label,
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyFood(item, cost)),
        confirm: None,
        confirm_on_vacation: None,
    }
}

const fn toy_item(label: &'static str, variant: ToyVariant, cost: u8, _confirm: &'static str) -> MenuItem<StoreAction> {
    MenuItem {
        label,
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyToy(variant, cost)),
        confirm: None,
        confirm_on_vacation: None,
    }
}

const fn pot_item(label: &'static str, pot: PotSize, cost: u8, _confirm: &'static str) -> MenuItem<StoreAction> {
    MenuItem {
        label,
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyPot(pot, cost)),
        confirm: None,
        confirm_on_vacation: None,
    }
}

const fn seed_item(label: &'static str, seed: SeedKind, cost: u8, _confirm: &'static str) -> MenuItem<StoreAction> {
    MenuItem {
        label,
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuySeeds(seed, cost)),
        confirm: None,
        confirm_on_vacation: None,
    }
}

const fn tool_item(label: &'static str, tool: ToolKind, cost: u8, _confirm: &'static str) -> MenuItem<StoreAction> {
    MenuItem {
        label,
        icon: None,
        submenu: None,
        action: Some(StoreAction::BuyTool(tool, cost)),
        confirm: None,
        confirm_on_vacation: None,
    }
}

pub struct StoreScene {
    menu: Menu<StoreAction>,
    popup: Popup,
    popup_active: bool,
    pending_scene: Option<SceneId>,
    confirm: crate::ui::confirm::Confirm,
    pending_action: Option<StoreAction>,
}

impl StoreScene {
    pub fn new() -> Self {
        Self {
            menu: Menu::with_width(ROOT, MENU_WIDTH, MENU_WIDTH),
            popup: Popup::new(14, 20, 100, 24),
            popup_active: false,
            pending_scene: None,
            confirm: crate::ui::confirm::Confirm::new(),
            pending_action: None,
        }
    }

    /// Build the localized confirm prompt for a store action. Returns `None`
    /// for actions that fire immediately without confirmation (e.g. `Leave`).
    fn confirm_text(action: StoreAction) -> Option<heapless::String<48>> {
        use crate::i18n::substitute;
        let mut buf: heapless::String<48> = heapless::String::new();
        match action {
            StoreAction::Leave => return None,
            StoreAction::BuyFood(item, cost) => {
                let mut uses_b: heapless::String<8> = heapless::String::new();
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(uses_b, "{}", FOOD_USES);
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("{item}({uses}): {cost}c"),
                    &[("item", item.label()), ("uses", uses_b.as_str()), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyToy(variant, cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("{item}: {cost}c"),
                    &[("item", variant.label()), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyPot(pot, cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("{item}: {cost}c"),
                    &[("item", pot_full_label(pot)), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuySeeds(seed, cost) => {
                let mut n_b: heapless::String<8> = heapless::String::new();
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(n_b, "{}", SEEDS_PER_PACK);
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("{item}x{n}: {cost}c"),
                    &[("item", seed_full_label(seed)), ("n", n_b.as_str()), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyTool(tool, cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("{item}: {cost}c"),
                    &[("item", tool_label(tool)), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyFertilizer(cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("Fertilizer: {cost}c"),
                    &[("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyMedicine(cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                substitute(
                    &mut buf,
                    t!("{item}: {cost}c"),
                    &[("item", t!("Medicine")), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyService(kind, cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                let label = match kind {
                    ServiceKind::Groom => t!("Groom"),
                    ServiceKind::Train => t!("Train"),
                };
                substitute(
                    &mut buf,
                    t!("{item}: {cost}c"),
                    &[("item", label), ("cost", cost_b.as_str())],
                );
            }
            StoreAction::BuyTrip(dest, cost) => {
                let mut cost_b: heapless::String<8> = heapless::String::new();
                let _ = write!(cost_b, "{}", cost);
                let dest_label = match dest {
                    SceneId::VacationPark => t!("Park"),
                    SceneId::VacationForest => t!("Forest"),
                    SceneId::VacationAquarium => t!("Aquarium"),
                    SceneId::VacationBeach => t!("Beach"),
                    _ => "?",
                };
                substitute(
                    &mut buf,
                    t!("A trip to the {dest}: {cost}c"),
                    &[("dest", dest_label), ("cost", cost_b.as_str())],
                );
            }
        }
        Some(buf)
    }

    fn set_popup(&mut self, text: &str) {
        self.popup.set_text(text, true, true);
        self.popup_active = true;
    }

    /// Build `template` with `item` substituted in, then show it as a popup.
    /// Template is expected to contain a `{item}` placeholder.
    fn set_popup_with_item(&mut self, template: &str, item: &str) {
        let mut buf: heapless::String<48> = heapless::String::new();
        crate::i18n::substitute(&mut buf, template, &[("item", item)]);
        self.popup.set_text(buf.as_str(), true, true);
        self.popup_active = true;
    }

    fn try_spend(ctx: &mut GameContext, cost: u8) -> bool {
        if ctx.coins >= cost as i32 {
            ctx.coins -= cost as i32;
            true
        } else {
            false
        }
    }

    fn handle_store_action(&mut self, ctx: &mut GameContext, action: StoreAction) -> Option<SceneId> {
        match action {
            StoreAction::Leave => return Some(ctx.last_main_scene),

            StoreAction::BuyFood(item, cost) => {
                if Self::try_spend(ctx, cost) {
                    ctx.add_food_stock(item, FOOD_USES);
                    self.set_popup_with_item(t!("{item} purchased!"), item.label());
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuyToy(variant, cost) => {
                let existing = ctx.find_toy(variant);
                if let Some(idx) = existing {
                    if ctx.toys[idx].durability > 0 {
                        self.set_popup(t!("Already owned!"));
                    } else if Self::try_spend(ctx, cost) {
                        ctx.refresh_toy(variant);
                        self.set_popup_with_item(t!("{item} replaced!"), variant.label());
                    } else {
                        self.set_popup(t!("Can't afford!"));
                    }
                } else if Self::try_spend(ctx, cost) {
                    ctx.add_toy(variant);
                    self.set_popup_with_item(t!("{item} purchased!"), variant.label());
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuyPot(pot, cost) => {
                if Self::try_spend(ctx, cost) {
                    ctx.add_pot(pot);
                    self.set_popup_with_item(t!("{item} bought!"), pot_full_label(pot));
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuySeeds(seed, cost) => {
                if Self::try_spend(ctx, cost) {
                    ctx.add_seeds(seed, SEEDS_PER_PACK);
                    self.set_popup_with_item(t!("{item} bought!"), seed_full_label(seed));
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuyTool(tool, cost) => {
                if ctx.owns_tool(tool) {
                    self.set_popup(t!("Already owned!"));
                } else if Self::try_spend(ctx, cost) {
                    ctx.set_tool(tool, true);
                    self.set_popup_with_item(t!("{item} bought!"), tool_label(tool));
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuyFertilizer(cost) => {
                if Self::try_spend(ctx, cost) {
                    ctx.fertilizer = ctx.fertilizer.saturating_add(1);
                    self.set_popup(t!("Fertilizer bought!"));
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuyMedicine(cost) => {
                if Self::try_spend(ctx, cost) {
                    ctx.medicine = ctx.medicine.saturating_add(1);
                    self.set_popup_with_item(t!("{item} bought!"), t!("Medicine"));
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }

            StoreAction::BuyService(kind, cost) => {
                if !Self::try_spend(ctx, cost) {
                    self.set_popup(t!("Can't afford!"));
                } else {
                    match kind {
                        ServiceKind::Groom => {
                            ctx.apply_stat_changes(&[
                                (StatId::Cleanliness, 40.0),
                                (StatId::Sociability, 8.0),
                                (StatId::Courage, 6.0),
                            ]);
                            self.set_popup(t!("A luxurious spa day!"));
                        }
                        ServiceKind::Train => {
                            ctx.apply_stat_changes(&[
                                (StatId::Maturity, 5.0),
                                (StatId::Sociability, 5.0),
                                (StatId::Intelligence, 8.0),
                                (StatId::Fitness, 6.0),
                                (StatId::Mischievousness, -8.0),
                            ]);
                            self.set_popup(t!("Professional training done!"));
                        }
                    }
                }
            }

            StoreAction::BuyTrip(dest, cost) => {
                if Self::try_spend(ctx, cost) {
                    self.pending_scene = Some(dest);
                    self.set_popup("Enjoy the trip!");
                } else {
                    self.set_popup(t!("Can't afford!"));
                }
            }
        }
        None
    }

    fn draw_store_art(&self, renderer: &mut Renderer) {
        use crate::render::SpriteOpts;

        let inverted = SpriteOpts {
            invert: true,
            transparent: true,
            transparent_color: true,
            ..Default::default()
        };

        // Roof shingles row 1
        for i in 0..7 {
            renderer.draw_sprite_raw(
                art::SHINGLE1,
                art::SHINGLE1_W,
                art::SHINGLE1_H,
                Point::new(128 - 62 + (i * 9), 0),
                SpriteOpts::default(),
            );
        }

        // Sign frame
        renderer.draw_line(Point::new(128, 5), Point::new(66, 5));
        renderer.draw_line(Point::new(128, 17), Point::new(66, 17));
        renderer.draw_pixel(Point::new(66, 6), true);
        renderer.draw_pixel(Point::new(66, 16), true);
        renderer.draw_line(Point::new(65, 6), Point::new(65, 16));
        renderer.draw_rect(Point::new(68, 7), Size::new(64, 9), true);
        renderer.draw_line(Point::new(67, 8), Point::new(67, 14));

        renderer.draw_sprite_raw(art::PAW_LOGO, art::PAW_LOGO_W, art::PAW_LOGO_H, Point::new(73, 8), inverted);
        renderer.draw_sprite_raw(art::STORE_TEXT, art::STORE_TEXT_W, art::STORE_TEXT_H, Point::new(86, 9), inverted);
        renderer.draw_sprite_raw(art::PAW_LOGO, art::PAW_LOGO_W, art::PAW_LOGO_H, Point::new(113, 8), inverted);

        // Roof shingles row 2
        for i in 0..9 {
            renderer.draw_sprite_raw(
                art::SHINGLE2,
                art::SHINGLE2_W,
                art::SHINGLE2_H,
                Point::new(128 - 68 + (i * 8), 19),
                SpriteOpts::default(),
            );
        }

        // Pillar
        renderer.draw_rect(Point::new(65, 29), Size::new(2, 15), false);
        renderer.draw_sprite_raw(art::SHADOW1, art::SHADOW1_W, art::SHADOW1_H, Point::new(68, 31), SpriteOpts::default());
        renderer.draw_rect(Point::new(68, 35), Size::new(3, 9), true);

        // Counter
        renderer.draw_rect(Point::new(59, 45), Size::new(69, 2), false);
        renderer.draw_pixel(Point::new(58, 45), true);

        for i in 0..4 {
            renderer.draw_sprite_raw(
                art::COUNTER_STRUT,
                art::COUNTER_STRUT_W,
                art::COUNTER_STRUT_H,
                Point::new(128 - 66 + (i * 18), 48),
                SpriteOpts::default(),
            );
        }

        // Items on counter
        renderer.draw_sprite_raw(art::ITEMS1, art::ITEMS1_W, art::ITEMS1_H, Point::new(79, 37), SpriteOpts::default());
        renderer.draw_sprite_raw(art::ITEMS2, art::ITEMS2_W, art::ITEMS2_H, Point::new(110, 39), SpriteOpts::default());
        renderer.draw_sprite_raw(art::ITEMS3, art::ITEMS3_W, art::ITEMS3_H, Point::new(73, 27), SpriteOpts::default());

        // Under-counter slats
        renderer.draw_line(Point::new(63, 53), Point::new(63, 63));
        renderer.draw_line(Point::new(63, 63), Point::new(128, 63));
        for i in 0..13 {
            renderer.draw_rect(Point::new(65 + i * 5, 53), Size::new(4, 9), true);
        }
    }

    fn draw_coins(&self, ctx: &GameContext, renderer: &mut Renderer) {
        let mut buf: String<8> = String::new();
        let coins = ctx.coins.clamp(0, 9999);
        let _ = write_u16(&mut buf, coins as u16);
        let _ = buf.push('c');
        renderer.draw_text(buf.as_str(), Point::new(COIN_X, COIN_Y));
    }
}

fn write_u16<const N: usize>(buf: &mut String<N>, mut v: u16) -> Result<(), ()> {
    if v == 0 {
        return buf.push('0').map_err(|_| ());
    }
    let mut digits = [0u8; 5];
    let mut n = 0;
    while v > 0 {
        digits[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    for i in (0..n).rev() {
        buf.push(digits[i] as char).map_err(|_| ())?;
    }
    Ok(())
}

impl Scene for StoreScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.menu.reset_to(ROOT);
        self.popup_active = false;
        self.pending_scene = None;
        self.confirm.close();
        self.pending_action = None;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        if self.popup_active {
            if buttons.was_just_pressed(Button::A) || buttons.was_just_pressed(Button::B) {
                self.popup_active = false;
                if let Some(dest) = self.pending_scene.take() {
                    return Some(dest);
                }
            }
            return None;
        }

        if self.confirm.is_open() {
            use crate::ui::confirm::ConfirmResult;
            match self.confirm.handle_input(buttons) {
                ConfirmResult::Pending => return None,
                ConfirmResult::Confirmed => {
                    if let Some(action) = self.pending_action.take() {
                        return self.handle_store_action(ctx, action);
                    }
                    return None;
                }
                ConfirmResult::Cancelled => {
                    self.pending_action = None;
                    return None;
                }
            }
        }

        match self.menu.handle_input(buttons, false) {
            MenuResult::Continue => None,
            MenuResult::Closed => Some(ctx.last_main_scene),
            MenuResult::Action(action) => {
                if let Some(text) = Self::confirm_text(action) {
                    self.pending_action = Some(action);
                    self.confirm.open(text.as_str());
                    None
                } else {
                    self.handle_store_action(ctx, action)
                }
            }
        }
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.draw_store_art(renderer);
        self.draw_coins(ctx, renderer);
        self.menu.draw(renderer);
        if self.popup_active {
            self.popup.draw(renderer, false);
        }
        if self.confirm.is_open() {
            self.confirm.draw(renderer);
        }
        // Silence unused-import lint when ART_PANEL_X is not actively used as a
        // const expression at runtime. Kept as documentation of the layout.
        let _ = ART_PANEL_X;
    }
}
