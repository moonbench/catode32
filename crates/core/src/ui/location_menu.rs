//! Pet interaction menu opened with Menu2 on any location scene. Tree:
//! Affection / Train / Feed / Play / Gardening. Items, counts and inventory
//! are recomputed every time the player navigates a page so labels like
//! "Tuna (3)" and "Small pot (2)" stay accurate and out-of-stock entries
//! disappear.

use core::fmt::Write as _;

use heapless::{String, Vec};
use crate::t;

use crate::{
    assets::{
        icons,
        plants::{PlantStage, PotKind},
    },
    behavior::{AffectionVariant, AttentionVariant, PlayVariant, TrainingKind},
    context::{FoodItem, GameContext, PotSize, SeedKind},
    input::{Button, Buttons},
    plant_system::{self, Plant},
    render::Renderer,
    scene::SceneId,
    ui::{
        confirm::{Confirm, ConfirmResult},
        list_nav::ListNav,
        menu::{
            draw_menu_row, DEFAULT_CONTENT_WIDTH, DEFAULT_SCROLLBAR_X, ROW_HEIGHT, VISIBLE_ITEMS,
        },
        scrollbar::Scrollbar,
    },
};

const LABEL_LEN: usize = 20;
const MAX_PAGE_ITEMS: usize = 16;
const MAX_DEPTH: usize = 4;
const TRACK_HEIGHT: u32 = 64;
const MIN_THUMB_HEIGHT: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Root,
    Affection,
    Train,
    Feed,
    FeedMeals,
    FeedSnacks,
    Play,
    Gardening,
    GardeningPlacePot,
    GardeningPlantSeed,
    GardeningPlantSeedInPot,
    GardeningPlantSeedInGround,
    /// Per-plant submenu shown after the player selects a plant via the
    /// PlantSelectionMode cursor. Uses `self.tend_plant_id`.
    GardeningTend,
    GardeningRepot,
    GardeningMove,
    GardeningInspect,
}

#[derive(Clone, Copy)]
pub enum GardeningAction {
    PlacePot(PotKind),
    PlantSeedInPot(SeedKind),
    PlantSeedInGround(SeedKind),
    /// Open the tend cursor. LocationScene starts `PlantSelectionMode` and
    /// re-opens this menu on a dynamic Tend page once the player picks one.
    StartTend,
    Water(u32),
    Fertilize(u32),
    Pluck(u32),
    /// Pick up an empty pot and return it to inventory.
    StowPot(u32),
    Repot(u32, PotKind),
    MoveHere(u32),
    MoveTo(u32, SceneId),
    /// No-op: closing the inspect submenu line just dismisses the menu.
    InspectDismiss,
}

#[derive(Clone, Copy)]
pub enum LocationAction {
    Affection(AffectionVariant),
    Attention(AttentionVariant),
    Eat(FoodItem),
    Groom,
    Train(TrainingKind),
    Play(PlayVariant),
    Medicine,
    GoToStore,
    Gardening(GardeningAction),
    /// Vacation scenes inject this as the top menu item; selecting it (with
    /// the "Ready to go home?" confirm) returns the player to the inside scene.
    GoHome,
}

pub enum LocationMenuResult {
    Continue,
    Closed,
    Action(LocationAction),
}

#[derive(Clone)]
struct Item {
    label: String<LABEL_LEN>,
    icon: Option<&'static [u8]>,
    action: Option<LocationAction>,
    submenu: Option<Page>,
    confirm: Option<&'static str>,
}

struct StackFrame {
    page: Page,
    nav: ListNav,
}

pub struct LocationMenu {
    page: Page,
    items: Vec<Item, MAX_PAGE_ITEMS>,
    nav: ListNav,
    stack: Vec<StackFrame, MAX_DEPTH>,
    confirm: Confirm,
    /// Action that fires once the open `Confirm` returns `Confirmed`.
    pending_action: Option<LocationAction>,
    /// Set by the host scene before opening a tend page so dynamic per-plant
    /// builders know which plant to introspect.
    tend_plant_id: Option<u32>,
    /// Scene that supports plant placement (current scene). Used to filter
    /// the Move submenu so the player can't move to the scene they're in.
    current_scene: Option<SceneId>,
    /// True when the current scene has any PLANT_SURFACES defined; controls
    /// whether the Gardening root entry is shown.
    has_plant_surfaces: bool,
    /// True on vacation scenes; injects the "Go home" item at the top of Root.
    is_vacation: bool,
}

impl LocationMenu {
    pub fn new() -> Self {
        Self {
            page: Page::Root,
            items: Vec::new(),
            nav: ListNav::new(),
            stack: Vec::new(),
            confirm: Confirm::new(),
            pending_action: None,
            tend_plant_id: None,
            current_scene: None,
            has_plant_surfaces: false,
            is_vacation: false,
        }
    }

    pub fn open(
        &mut self,
        ctx: &GameContext,
        scene: SceneId,
        has_surfaces: bool,
        is_vacation: bool,
    ) {
        self.stack.clear();
        self.confirm.close();
        self.pending_action = None;
        self.tend_plant_id = None;
        self.current_scene = Some(scene);
        self.has_plant_surfaces = has_surfaces;
        self.is_vacation = is_vacation;
        self.set_page(Page::Root, ctx);
    }

    /// Open directly on the Tend page for a specific plant. Used by
    /// LocationScene after the PlantSelectionMode confirms a selection.
    pub fn open_tend(&mut self, ctx: &GameContext, scene: SceneId, plant_id: u32) {
        self.stack.clear();
        self.confirm.close();
        self.pending_action = None;
        self.tend_plant_id = Some(plant_id);
        self.current_scene = Some(scene);
        self.has_plant_surfaces = true;
        self.is_vacation = false;
        self.set_page(Page::GardeningTend, ctx);
    }

    pub fn handle_input(
        &mut self,
        ctx: &GameContext,
        buttons: &mut Buttons,
    ) -> LocationMenuResult {
        if self.confirm.is_open() {
            return match self.confirm.handle_input(buttons) {
                ConfirmResult::Pending => LocationMenuResult::Continue,
                ConfirmResult::Confirmed => match self.pending_action.take() {
                    Some(action) => LocationMenuResult::Action(action),
                    None => LocationMenuResult::Continue,
                },
                ConfirmResult::Cancelled => {
                    self.pending_action = None;
                    LocationMenuResult::Continue
                }
            };
        }

        if buttons.was_just_pressed(Button::Menu1) || buttons.was_just_pressed(Button::Menu2) {
            return LocationMenuResult::Closed;
        }

        if buttons.was_just_pressed(Button::Up) {
            self.nav.up(VISIBLE_ITEMS);
        }
        if buttons.was_just_pressed(Button::Down) {
            self.nav.down(self.items.len(), VISIBLE_ITEMS);
        }

        if buttons.was_just_pressed(Button::B) {
            if self.stack.is_empty() {
                return LocationMenuResult::Closed;
            }
            self.pop_page(ctx);
            return LocationMenuResult::Continue;
        }
        if buttons.was_just_pressed(Button::Left) && !self.stack.is_empty() {
            self.pop_page(ctx);
            return LocationMenuResult::Continue;
        }

        if buttons.was_just_pressed(Button::Right) {
            if let Some(item) = self.items.get(self.nav.selected) {
                if let Some(sub) = item.submenu {
                    self.push_page(sub, ctx);
                    return LocationMenuResult::Continue;
                }
            }
        }

        if buttons.was_just_pressed(Button::A) {
            if let Some(item) = self.items.get(self.nav.selected) {
                if let Some(sub) = item.submenu {
                    self.push_page(sub, ctx);
                    return LocationMenuResult::Continue;
                }
                if let Some(action) = item.action {
                    if let Some(text) = item.confirm {
                        self.pending_action = Some(action);
                        self.confirm.open(text);
                    } else {
                        return LocationMenuResult::Action(action);
                    }
                }
            }
        }

        LocationMenuResult::Continue
    }

    pub fn draw(&self, renderer: &mut Renderer) {
        for (i, idx) in self.nav.visible_range(self.items.len(), VISIBLE_ITEMS).enumerate() {
            let item = &self.items[idx];
            let y = (i as i32) * ROW_HEIGHT;
            let selected = idx == self.nav.selected;
            draw_menu_row(
                renderer,
                item.label.as_str(),
                item.icon,
                item.submenu.is_some(),
                y,
                selected,
                DEFAULT_CONTENT_WIDTH,
            );
        }
        let bar = Scrollbar::new(DEFAULT_SCROLLBAR_X, 0, TRACK_HEIGHT, MIN_THUMB_HEIGHT);
        bar.draw(renderer, self.items.len(), VISIBLE_ITEMS, self.nav.scroll);

        self.confirm.draw(renderer);
    }

    fn set_page(&mut self, page: Page, ctx: &GameContext) {
        self.page = page;
        self.nav.reset();
        self.items.clear();
        build_page(
            page,
            ctx,
            self.current_scene,
            self.has_plant_surfaces,
            self.is_vacation,
            self.tend_plant_id,
            &mut self.items,
        );
    }

    fn push_page(&mut self, page: Page, ctx: &GameContext) {
        let frame = StackFrame {
            page: self.page,
            nav: self.nav,
        };
        let _ = self.stack.push(frame);
        self.set_page(page, ctx);
    }

    fn pop_page(&mut self, ctx: &GameContext) {
        if let Some(frame) = self.stack.pop() {
            self.set_page(frame.page, ctx);
            self.nav.selected = frame.nav.selected.min(self.items.len().saturating_sub(1));
            self.nav.scroll = frame.nav.scroll;
        }
    }
}

// --- page builders -------------------------------------------------------

fn push_item(
    items: &mut Vec<Item, MAX_PAGE_ITEMS>,
    label: &str,
    icon: Option<&'static [u8]>,
    action: Option<LocationAction>,
    submenu: Option<Page>,
    confirm: Option<&'static str>,
) {
    if items.is_full() {
        return;
    }
    let mut s: String<LABEL_LEN> = String::new();
    let _ = s.push_str(&label[..label.len().min(LABEL_LEN)]);
    let _ = items.push(Item {
        label: s,
        icon,
        action,
        submenu,
        confirm,
    });
}

fn push_count(
    items: &mut Vec<Item, MAX_PAGE_ITEMS>,
    name: &str,
    count: u8,
    icon: Option<&'static [u8]>,
    action: LocationAction,
) {
    if items.is_full() {
        return;
    }
    let mut s: String<LABEL_LEN> = String::new();
    let _ = s.push_str(&name[..name.len().min(LABEL_LEN - 4)]);
    let _ = write!(&mut s, " ({})", count);
    let _ = items.push(Item {
        label: s,
        icon,
        action: Some(action),
        submenu: None,
        confirm: None,
    });
}

fn pot_label(p: PotSize) -> &'static str {
    match p {
        PotSize::Small => t!("Small pot"),
        PotSize::Medium => t!("Medium pot"),
        PotSize::Large => t!("Large pot"),
        PotSize::Planter => t!("Planter box"),
    }
}

fn seed_label(s: SeedKind) -> &'static str {
    match s {
        SeedKind::CatGrass => t!("Cat Grass"),
        SeedKind::Freesia => t!("Freesia"),
        SeedKind::Rose => t!("Rose"),
        SeedKind::Sunflower => t!("Sunflower"),
    }
}

fn scene_label(s: SceneId) -> &'static str {
    match s {
        SceneId::Inside => t!("To Inside"),
        SceneId::Outside => t!("To Outside"),
        SceneId::Bedroom => t!("To Bedroom"),
        SceneId::Kitchen => t!("To Kitchen"),
        SceneId::Treehouse => t!("To Treehouse"),
        _ => "To ???",
    }
}

const ALL_POTS: &[PotSize] = &[PotSize::Small, PotSize::Medium, PotSize::Large, PotSize::Planter];
const ALL_SEEDS: &[SeedKind] = &[
    SeedKind::CatGrass,
    SeedKind::Sunflower,
    SeedKind::Rose,
    SeedKind::Freesia,
];
const PLANTABLE_SCENES: &[SceneId] = &[
    SceneId::Inside,
    SceneId::Outside,
    SceneId::Bedroom,
    SceneId::Kitchen,
    SceneId::Treehouse,
];

fn build_page(
    page: Page,
    ctx: &GameContext,
    current_scene: Option<SceneId>,
    has_surfaces: bool,
    is_vacation: bool,
    tend_plant_id: Option<u32>,
    items: &mut Vec<Item, MAX_PAGE_ITEMS>,
) {
    match page {
        Page::Root => {
            if is_vacation {
                push_item(
                    items,
                    t!("Go home"),
                    Some(icons::HOUSE),
                    Some(LocationAction::GoHome),
                    None,
                    Some(t!("Ready to go home?")),
                );
            }
            push_item(items, t!("Affection"), Some(icons::HEART), None, Some(Page::Affection), None);
            push_item(items, t!("Train"), Some(icons::HAND), None, Some(Page::Train), None);
            push_item(items, t!("Feed"), Some(icons::MEAL), None, Some(Page::Feed), None);
            push_item(items, t!("Play"), Some(icons::TOYS), None, Some(Page::Play), None);
            if has_surfaces {
                push_item(items, t!("Gardening"), Some(icons::TREES), None, Some(Page::Gardening), None);
            }
        }
        Page::Affection => {
            push_item(items, t!("Pets"), Some(icons::HAND), Some(LocationAction::Affection(AffectionVariant::Pets)), None, None);
            push_item(items, t!("Scratch"), Some(icons::HAND), Some(LocationAction::Affection(AffectionVariant::Scratching)), None, None);
            push_item(items, t!("Kiss"), Some(icons::HEART), Some(LocationAction::Affection(AffectionVariant::Kiss)), None, None);
            push_item(items, t!("Psst psst"), Some(icons::HEART_BUBBLE), Some(LocationAction::Attention(AttentionVariant::Psst)), None, None);
            push_item(items, t!("Groom"), Some(icons::HAND), Some(LocationAction::Groom), None, None);
        }
        Page::Train => {
            push_item(items, t!("Intelligence"), Some(icons::HAND), Some(LocationAction::Train(TrainingKind::Intelligence)), None, None);
            push_item(items, t!("Behavior"), Some(icons::HAND), Some(LocationAction::Train(TrainingKind::Behavior)), None, None);
            push_item(items, t!("Fitness"), Some(icons::HAND), Some(LocationAction::Train(TrainingKind::Fitness)), None, None);
            push_item(items, t!("Sociability"), Some(icons::HAND), Some(LocationAction::Train(TrainingKind::Sociability)), None, None);
        }
        Page::Feed => {
            let has_meals = (0..FoodItem::Mackerel as usize + 1)
                .any(|i| ctx.food_stock[i] > 0);
            let has_snacks = (FoodItem::Carrots as usize..FoodItem::Puree as usize + 1)
                .any(|i| ctx.food_stock[i] > 0);
            if has_meals {
                push_item(items, t!("Meals"), Some(icons::MEAL), None, Some(Page::FeedMeals), None);
            }
            if has_snacks {
                push_item(items, t!("Snacks"), Some(icons::KIBBLE), None, Some(Page::FeedSnacks), None);
            }
            if ctx.medicine > 0 {
                let mut label: String<LABEL_LEN> = String::new();
                let _ = write!(&mut label, "{} ({})", t!("Medicine"), ctx.medicine);
                let _ = items.push(Item {
                    label,
                    icon: Some(icons::PILL),
                    action: Some(LocationAction::Medicine),
                    submenu: None,
                    confirm: Some(t!("Give medicine?")),
                });
            }
            push_item(items, t!("Store..."), None, Some(LocationAction::GoToStore), None, None);
        }
        Page::FeedMeals => {
            push_food_items(items, ctx, true);
        }
        Page::FeedSnacks => {
            push_food_items(items, ctx, false);
        }
        Page::Play => {
            push_item(
                items,
                t!("Hand"),
                Some(icons::HAND),
                Some(LocationAction::Play(PlayVariant::Hand)),
                None,
                None,
            );
            for toy in ctx.toys.iter() {
                let icon = toy.variant.icon();
                let label = toy.variant.label();
                let action = LocationAction::Play(toy.variant.to_play_variant());
                push_item(items, label, Some(icon), Some(action), None, None);
            }
            push_item(items, t!("Store..."), None, Some(LocationAction::GoToStore), None, None);
        }
        Page::Gardening => {
            let has_any_plants = current_scene
                .map(|s| ctx.plants.iter().any(|p| p.scene == s))
                .unwrap_or(false);
            let has_any_pot = ctx.pots.iter().any(|n| *n > 0);
            let has_any_seed = ctx.seeds.iter().any(|n| *n > 0);
            if has_any_plants {
                push_item(items, t!("Tend"), Some(icons::TREES),
                          Some(LocationAction::Gardening(GardeningAction::StartTend)), None, None);
            }
            if has_any_pot {
                push_item(items, t!("Place Pot"), Some(icons::TREES), None,
                          Some(Page::GardeningPlacePot), None);
            }
            if has_any_seed {
                push_item(items, t!("Plant Seed"), Some(icons::TREES), None,
                          Some(Page::GardeningPlantSeed), None);
            }
            push_item(items, t!("Store..."), None, Some(LocationAction::GoToStore), None, None);
        }
        Page::GardeningPlacePot => {
            for &pot in ALL_POTS {
                let count = ctx.pots[pot as usize];
                if count == 0 {
                    continue;
                }
                push_count(
                    items,
                    pot_label(pot),
                    count,
                    Some(icons::TREES),
                    LocationAction::Gardening(GardeningAction::PlacePot(PotKind::from_pot_size(pot))),
                );
            }
        }
        Page::GardeningPlantSeed => {
            let outside = matches!(current_scene, Some(SceneId::Outside));
            let has_any_seed = ctx.seeds.iter().any(|n| *n > 0);
            if has_any_seed {
                let has_empty_pot = current_scene
                    .map(|s| {
                        ctx.plants
                            .iter()
                            .any(|p| p.scene == s && p.stage == PlantStage::EmptyPot)
                    })
                    .unwrap_or(false);
                if has_empty_pot {
                    push_item(items, t!("In Pot"), Some(icons::TREES), None,
                              Some(Page::GardeningPlantSeedInPot), None);
                }
                if outside {
                    push_item(items, t!("In Ground"), Some(icons::TREES), None,
                              Some(Page::GardeningPlantSeedInGround), None);
                }
            }
        }
        Page::GardeningPlantSeedInPot => {
            for &seed in ALL_SEEDS {
                let count = ctx.seeds[seed as usize];
                if count == 0 {
                    continue;
                }
                push_count(
                    items,
                    seed_label(seed),
                    count,
                    Some(icons::TREES),
                    LocationAction::Gardening(GardeningAction::PlantSeedInPot(seed)),
                );
            }
        }
        Page::GardeningPlantSeedInGround => {
            for &seed in ALL_SEEDS {
                let count = ctx.seeds[seed as usize];
                if count == 0 {
                    continue;
                }
                push_count(
                    items,
                    seed_label(seed),
                    count,
                    Some(icons::TREES),
                    LocationAction::Gardening(GardeningAction::PlantSeedInGround(seed)),
                );
            }
        }
        Page::GardeningTend => {
            let plant = tend_plant_id.and_then(|id| ctx.plants.iter().find(|p| p.id == id));
            let plant = match plant {
                Some(p) => p,
                None => return,
            };
            build_tend_items(items, ctx, plant);
        }
        Page::GardeningInspect => {
            let plant = tend_plant_id.and_then(|id| ctx.plants.iter().find(|p| p.id == id));
            let plant = match plant {
                Some(p) => p,
                None => return,
            };
            for line in plant_system::inspect_lines(plant).iter() {
                push_item(
                    items,
                    line.as_str(),
                    None,
                    Some(LocationAction::Gardening(GardeningAction::InspectDismiss)),
                    None,
                    None,
                );
            }
        }
        Page::GardeningRepot => {
            let plant = tend_plant_id.and_then(|id| ctx.plants.iter().find(|p| p.id == id));
            let plant = match plant {
                Some(p) => p,
                None => return,
            };
            build_repot_items(items, ctx, plant);
        }
        Page::GardeningMove => {
            let plant = tend_plant_id.and_then(|id| ctx.plants.iter().find(|p| p.id == id));
            let (plant, cur_scene) = match (plant, current_scene) {
                (Some(p), Some(s)) => (p, s),
                _ => return,
            };
            push_item(
                items,
                t!("Around Here"),
                None,
                Some(LocationAction::Gardening(GardeningAction::MoveHere(plant.id))),
                None,
                None,
            );
            for &dest in PLANTABLE_SCENES {
                if dest == cur_scene {
                    continue;
                }
                push_item(
                    items,
                    scene_label(dest),
                    None,
                    Some(LocationAction::Gardening(GardeningAction::MoveTo(plant.id, dest))),
                    None,
                    None,
                );
            }
        }
    }
}

fn build_tend_items(items: &mut Vec<Item, MAX_PAGE_ITEMS>, ctx: &GameContext, plant: &Plant) {
    push_item(items, t!("Inspect"), None, None, Some(Page::GardeningInspect), None);

    let alive = plant.stage != PlantStage::EmptyPot && !plant.stage.is_dead();
    if alive {
        push_item(
            items,
            t!("Water"),
            None,
            Some(LocationAction::Gardening(GardeningAction::Water(plant.id))),
            None,
            None,
        );
        if ctx.fertilizer > 0 {
            push_item(
                items,
                t!("Fertilize"),
                None,
                Some(LocationAction::Gardening(GardeningAction::Fertilize(plant.id))),
                None,
                None,
            );
        }
    }

    if plant.pot != PotKind::Ground {
        push_item(items, t!("Move"), None, None, Some(Page::GardeningMove), None);
    }

    // Repot is available when at least one larger pot is in inventory (or, for
    // small/young stages, any other pot).
    if plant.pot != PotKind::Ground && repot_has_options(ctx, plant) {
        push_item(items, t!("Repot"), None, None, Some(Page::GardeningRepot), None);
    }

    if plant.stage == PlantStage::EmptyPot {
        push_item(
            items,
            t!("Move to inventory"),
            None,
            Some(LocationAction::Gardening(GardeningAction::StowPot(plant.id))),
            None,
            None,
        );
        return;
    }

    let needs_confirm = !plant.stage.is_dead();
    push_item(
        items,
        t!("Pluck"),
        None,
        Some(LocationAction::Gardening(GardeningAction::Pluck(plant.id))),
        None,
        if needs_confirm { Some(t!("Remove plant?")) } else { None },
    );
}

fn pot_rank(p: PotKind) -> i32 {
    match p {
        PotKind::Small => 0,
        PotKind::Medium => 1,
        PotKind::Large => 2,
        PotKind::Planter => 3,
        PotKind::Ground => 4,
    }
}

fn repot_has_options(ctx: &GameContext, plant: &Plant) -> bool {
    let is_large = matches!(plant.stage, PlantStage::Mature | PlantStage::Thriving);
    let cur_rank = pot_rank(plant.pot);
    for &pot in ALL_POTS {
        let kind = PotKind::from_pot_size(pot);
        if kind == plant.pot {
            continue;
        }
        let target_rank = pot_rank(kind);
        if target_rank < cur_rank && is_large {
            continue;
        }
        if ctx.pots[pot as usize] > 0 {
            return true;
        }
    }
    false
}

fn build_repot_items(items: &mut Vec<Item, MAX_PAGE_ITEMS>, ctx: &GameContext, plant: &Plant) {
    let is_large = matches!(plant.stage, PlantStage::Mature | PlantStage::Thriving);
    let cur_rank = pot_rank(plant.pot);
    for &pot in ALL_POTS {
        let kind = PotKind::from_pot_size(pot);
        if kind == plant.pot {
            continue;
        }
        let target_rank = pot_rank(kind);
        if target_rank < cur_rank && is_large {
            continue;
        }
        if ctx.pots[pot as usize] == 0 {
            continue;
        }
        push_item(
            items,
            pot_label(pot),
            None,
            Some(LocationAction::Gardening(GardeningAction::Repot(plant.id, kind))),
            None,
            None,
        );
    }
}

const MEAL_ORDER: &[FoodItem] = &[
    FoodItem::Chicken,
    FoodItem::Salmon,
    FoodItem::Tuna,
    FoodItem::Shrimp,
    FoodItem::Trout,
    FoodItem::Herring,
    FoodItem::Haddock,
    FoodItem::Cod,
    FoodItem::Turkey,
    FoodItem::Beef,
    FoodItem::Lamb,
    FoodItem::Liver,
    FoodItem::Mackerel,
    FoodItem::Kibble,
];

const SNACK_ORDER: &[FoodItem] = &[
    FoodItem::Treats,
    FoodItem::ChewStick,
    FoodItem::Nugget,
    FoodItem::Puree,
    FoodItem::Milk,
    FoodItem::FishBite,
    FoodItem::Eggs,
    FoodItem::Pumpkin,
    FoodItem::Carrots,
];

fn push_food_items(items: &mut Vec<Item, MAX_PAGE_ITEMS>, ctx: &GameContext, meals: bool) {
    let order = if meals { MEAL_ORDER } else { SNACK_ORDER };
    for &item in order {
        let count = ctx.food_stock[item as usize];
        if count == 0 {
            continue;
        }
        let icon = food_icon(item);
        push_count(items, item.label(), count, Some(icon), LocationAction::Eat(item));
    }
}

fn food_icon(item: FoodItem) -> &'static [u8] {
    match item {
        FoodItem::Chicken | FoodItem::Turkey => icons::CHICKEN,
        FoodItem::Cod
        | FoodItem::Haddock
        | FoodItem::Trout
        | FoodItem::Shrimp
        | FoodItem::Herring
        | FoodItem::Tuna
        | FoodItem::Salmon
        | FoodItem::Mackerel
        | FoodItem::FishBite => icons::FISH,
        FoodItem::Beef | FoodItem::Lamb | FoodItem::Liver => icons::MEAL,
        FoodItem::Kibble => icons::KIBBLE,
        _ => icons::KIBBLE,
    }
}
