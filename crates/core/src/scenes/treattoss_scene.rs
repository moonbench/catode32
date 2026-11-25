//! Treat Toss. Throw a treat through the house and let the pet hunt it down.
//! The aim swings up and down on its own: A locks the angle, then a power
//! gauge swings and A again throws. B backs out of the power gauge.
//!
//! The treat flies, bounces and slides through a fixed run of rooms: living
//! room, kitchen, bedroom, then out the back door into the garden. Each room
//! has one fixed piece of furniture (couch, fridge, bed, rock pile) and a few
//! random extras. The living room also has a window with a sill to land on,
//! and the kitchen a long flat counter. Floors decide how far it skids:
//! kitchen tile is slippery, carpet settles it and rugs stop it dead. No
//! plain throw reaches the garden; getting there takes a good bounce off the
//! couch, bed or tunnel. The couch and bed are trampolines, the cat tunnel
//! shoots it out the far side, and rocks kick it off at odd angles. Some places swallow it for good: under the fridge, the
//! water bowl, the bin, a puddle, the robot vacuum, a passing bird.
//!
//! When it stops the pet runs in and hunts for it. A treat in the open is
//! found at once; one on a rug, up on the furniture or tucked in a basket
//! takes some sniffing, and harder finds score more. So the best throws are
//! long ones that land somewhere interesting without getting lost.
//!
//! Five treats a round, all at the same house, so each throw can learn from
//! the last. The round scores all five added up. The world is one strip about fifteen screens wide and one screen
//! tall; the camera follows the treat sideways, and a caret on the top edge
//! marks it when it flies above the screen.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{
        character::PoseId,
        furniture::{
            draw_bed, draw_bed_legs, draw_bookshelf, draw_cat_bed_rim_at, draw_couch, draw_dresser, draw_fridge,
            draw_fridge_feet, FridgeLayout, BED_H, BED_MATTRESS_Y, BED_PILLOW_X, BED_PILLOW_Y, BED_RAIL_BOTTOM_Y,
            BED_W, CABINET_H, CABINET_W, CAT_BED_RIM_WIDTH, COUCH_BASE_Y,
            COUCH_H, COUCH_SEAT_Y, COUCH_W, FRIDGE_FEET_H, FRIDGE_H, FRIDGE_W, PILLOW, PARK_BENCH,
            TRASHCAN_BODY, BASKET, PAPER_BAG, FAUCET,
        },
        items::{FOOD_BOWL, FOOD_BOWL_KIBBLE, HAND, YARN_BALL},
        minigame_assets::TREAT,
        minigame_character::SMALL_BIRD1,
        nature::{BUSH, CLOUD2, CLOUD3, ROCK_PILE},
        plants::POT_MEDIUM,
    },
    context::{GameContext, StatId},
    entities::character::Character,
    input::{Button, Buttons},
    rand::{rand_bool, rand_range_f32, rand_range_u32},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

const SCREEN_W: i32 = 128;

// World, in px. y grows downwards and the screen shows y 0..64.
const FLOOR: f32 = 60.0;
/// Indoor ceiling, out of sight above the screen.
const CEIL: f32 = -250.0;
/// Bare floor between the thrower and the living room's furniture.
const ENTRY: f32 = 150.0;
const LIVING_END: f32 = ENTRY + 520.0;
const KITCHEN_END: f32 = ENTRY + 900.0;
const BEDROOM_END: f32 = ENTRY + 1320.0;
const FENCE_X: f32 = ENTRY + 1760.0;
const FENCE_TOP: f32 = 26.0;
const WORLD_W: f32 = ENTRY + 1800.0;
const ROOMS: [(f32, f32); 4] = [
    (0.0, LIVING_END),
    (LIVING_END, KITCHEN_END),
    (KITCHEN_END, BEDROOM_END),
    (BEDROOM_END, FENCE_X),
];
/// Floor kept clear for the hand, the waiting pet and the entry.
const LAUNCH_ZONE: f32 = ENTRY + 106.0;
/// Gap kept between random props.
const PROP_GAP: f32 = 24.0;

// Treat physics, in px and seconds.
const LAUNCH_X: f32 = 20.0;
const LAUNCH_Y: f32 = 25.0;
const R: f32 = 1.5;
const EPS: f32 = 0.01;
const G: f32 = 85.0;
const V_MIN: f32 = 45.0;
const V_MAX: f32 = 285.0;
/// Longest substep, so thin walls and rims can't be skipped.
const MAX_STEP: f32 = 1.0;
const WALL_BOUNCE: f32 = 0.35;
const CEIL_BOUNCE: f32 = 0.4;
const UNDERSIDE_BOUNCE: f32 = 0.3;
/// Landing faster than this counts as an impact and costs some speed.
const IMPACT: f32 = 35.0;
/// Bounces slower than this die out and the treat slides.
const REST_VY: f32 = 12.0;
const TRAMP_MIN: f32 = 40.0;
const TRAMP_GAIN: f32 = 1.05;
/// Trampolines also throw the treat onwards.
const TRAMP_PUSH: f32 = 1.15;
/// A second bounce on the same trampoline in a row is damped instead.
const TRAMP_REPEAT: f32 = 0.45;
const TRAMP_MAX: f32 = 190.0;
const TUNNEL_BOOST: f32 = 1.5;
const TUNNEL_MIN: f32 = 70.0;
const BUMPER_BOUNCE: f32 = 0.9;
const WALL_TRICK_SPEED: f32 = 30.0;
/// A sliding treat hops over anything no taller than this instead of
/// bouncing back off it.
const RAMP_HEIGHT: f32 = 7.0;
/// Share of a sliding treat's speed turned upwards by a hop.
const RAMP_LIFT: f32 = 0.6;
const SOFT_DAMP: f32 = 9.0;
const STOP_SPEED: f32 = 3.0;
const STOP_TIME: f32 = 0.25;
/// Safety net for a treat that somehow never settles.
const MAX_FLIGHT: f32 = 25.0;

const TREATS: usize = 5;
/// px per "paw" of distance.
const PAW: f32 = 8.0;

// Aiming, in degrees above horizontal.
const ANGLE_MIN: f32 = 5.0;
const ANGLE_MAX: f32 = 80.0;
/// One full swing of the aim, bottom to top and back. A sine, so it
/// lingers at the extremes and rushes through the middle angles.
const AIM_PERIOD: f32 = 2.2;
const POWER_PERIOD: f32 = 1.4;
/// How quickly the camera's look-ahead swings round when the treat turns.
const CAM_LEAD_RATE: f32 = 2.5;
/// Path length of the dotted aim preview.
const PREVIEW_LEN: f32 = 26.0;
const PREVIEW_SPEED: f32 = 110.0;

// Robot vacuum.
const VAC_W: f32 = 24.0;
const VAC_H: f32 = 6.0;
const VAC_RANGE: f32 = 80.0;
const VAC_SPEED: f32 = 18.0;

/// Chance the pet shrugs off a lost treat instead of lying down to sulk.
const SHRUG_CHANCE: f32 = 0.5;

// Garden bird.
const BIRD_CHANCE: f32 = 0.2;
const BIRD_SWOOP: f32 = 1.0;
const BIRD_CARRY: f32 = 1.6;

// The pet.
const PET_HOME_X: f32 = 58.0;
/// Pet x sits this far behind a treat it is eating, on the side it faces.
const EAT_OFF: f32 = 17.0;
/// Half the pet's body width: up on furniture it keeps this far from the
/// ends so it isn't standing on thin air.
const PET_HALF: f32 = 16.0;
const NOTICE: f32 = 0.5;
const WALK_SPEED: f32 = 40.0;
const JUMP_TIME: f32 = 0.45;
const POUNCE_TIME: f32 = 0.35;
const EAT_TIME: f32 = 1.4;
const HAPPY_TIME: f32 = 1.3;
const LEAVE_TIME: f32 = 2.5;

const CALLOUT_TIME: f32 = 1.3;
const SPLASH_TIME: f32 = 0.6;

// Tricks, as bits. Each one adds 10% to a found treat's score.
const T_COUCH: u8 = 1;
const T_BED: u8 = 2;
const T_TUNNEL: u8 = 4;
const T_ROCK: u8 = 8;
const T_WALL: u8 = 16;
const T_YARN: u8 = 32;
const T_CATBED: u8 = 64;
const T_RESCUE: u8 = 128;
const TRICK_TEXT: [&str; 8] = [
    t!("Couch bounce!"),
    t!("Bed bounce!"),
    t!("Through the tunnel!"),
    t!("Rock hop!"),
    t!("Wall bank!"),
    t!("Yarn bonk!"),
    t!("Cosy landing!"),
    t!("Pawed it out!"),
];

// Where the treat ended up.
const L_OPEN: &str = t!("In plain sight");
const L_UNDER_FRIDGE: &str = t!("Under the fridge");
const L_ON_FRIDGE: &str = t!("On the fridge");
const L_WATER: &str = t!("In the water bowl");
const L_FOOD: &str = t!("In the food bowl");
const L_BAG: &str = t!("In the paper bag");
const L_BIN: &str = t!("In the bin");
const L_MAT: &str = t!("On the mat");
const L_UNDER_COUCH: &str = t!("Under the couch");
const L_ON_COUCH: &str = t!("On the couch");
const L_TUNNEL: &str = t!("In the tunnel");
const L_ON_TUNNEL: &str = t!("On the tunnel");
const L_UNDER_TABLE: &str = t!("Under the table");
const L_ON_TABLE: &str = t!("On the table");
const L_RUG: &str = t!("On the rug");
const L_BASKET: &str = t!("In the basket");
const L_BOOKSHELF: &str = t!("On the bookshelf");
const L_DRESSER: &str = t!("On the dresser");
const L_SILL: &str = t!("On the windowsill");
const L_COUNTER: &str = t!("On the counter");
const L_VACUUM: &str = t!("Vacuumed up!");
const L_UNDER_BED: &str = t!("Under the bed");
const L_ON_BED: &str = t!("On the bed");
const L_PILLOW: &str = t!("On the pillow");
const L_CATBED: &str = t!("In their bed");
const L_LAUNDRY: &str = t!("In the laundry");
const L_BLANKET: &str = t!("Under the blanket");
const L_BUSH: &str = t!("In the bush");
const L_POT: &str = t!("In a flower pot");
const L_PUDDLE: &str = t!("In a puddle");
const L_UNDER_BENCH: &str = t!("Under the bench");
const L_ON_BENCH: &str = t!("On the bench");
const L_ROCKS: &str = t!("On the rocks");
const L_FENCE: &str = t!("Over the fence");
const L_ON_FENCE: &str = t!("On the fence");
const L_BIRD: &str = t!("A bird took it!");
const L_UP_HIGH: &str = t!("Up high");

const MAX_PROPS: usize = 20;
const MAX_SOLIDS: usize = 64;
const MAX_ZONES: usize = 40;
const MAX_STEPS: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TreatTossAction {
    NewRound,
}

const OPTIONS_ITEMS: &[MenuItem<TreatTossAction>] = &[
    MenuItem { label: t!("New Round"), icon: None, submenu: None, action: Some(TreatTossAction::NewRound), confirm: None, confirm_on_vacation: None },
];

// ---------------------------------------------------------------------------
// Surfaces and hiding spots
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Surface {
    Tile,
    Carpet,
    Rug,
    Grass,
    Wood,
    Soft,
    /// Couch seat and mattress: a trampoline on a hard landing.
    Bouncy,
    /// Kicks the treat off at an odd angle.
    Rock,
    Fabric,
}

struct Feel {
    /// Vertical restitution.
    bounce: f32,
    /// Horizontal speed kept through an impact.
    keep: f32,
    /// Sliding deceleration, px/s².
    drag: f32,
}

fn feel(s: Surface) -> Feel {
    let (bounce, keep, drag) = match s {
        Surface::Tile => (0.6, 0.55, 20.0),
        Surface::Carpet => (0.6, 0.5, 80.0),
        Surface::Rug => (0.2, 0.4, 260.0),
        Surface::Grass => (0.4, 0.45, 130.0),
        Surface::Wood => (0.6, 0.6, 45.0),
        Surface::Soft => (0.1, 0.4, 300.0),
        Surface::Bouncy => (0.55, 0.9, 120.0),
        Surface::Rock => (0.75, 0.85, 60.0),
        Surface::Fabric => (0.55, 0.55, 90.0),
    };
    Feel { bounce, keep, drag }
}

/// How hard a resting treat is for the pet to find.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Find {
    Open,
    Tucked,
    Hidden,
    Lost,
}

impl Find {
    fn mult(self) -> f32 {
        match self {
            Find::Open => 1.0,
            Find::Tucked => 1.5,
            Find::Hidden => 2.0,
            Find::Lost => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ZoneKind {
    /// Floor covering. Tucked.
    Rug,
    /// A pile the treat sinks into. Hidden.
    Soft,
    /// Inside a basket or bag. Hidden.
    Inside,
    /// Inside the cat tunnel. Hidden, and it boosts a treat passing through.
    Tunnel,
    /// Under a table or bench. Tucked.
    Tucked,
    /// Under the couch or bed: tucked within `edge` of either side, lost
    /// further in. A curious pet can sometimes paw a lost one back out.
    Under { edge: f32 },
    /// The food bowl. Found at once.
    Bowl,
    /// The pet's own bed. Found at once, with a bonus.
    CatBed,
    /// Gone the moment it gets in: under the fridge, water, the bin, over
    /// the fence.
    Gone { splash: bool },
}

#[derive(Clone, Copy, Debug)]
struct Zone {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    kind: ZoneKind,
    label: &'static str,
    /// Floor surface inside the zone, if it differs from the room's.
    floor: Option<Surface>,
}

impl Zone {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y <= self.y1
    }
}

#[derive(Clone, Copy, Debug)]
struct Solid {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    top: Surface,
    /// How hidden a treat resting on top is.
    find: Find,
    label: &'static str,
    /// Trick awarded for bouncing on top.
    trick: u8,
}

impl Solid {
    fn hit(&self, x: f32, y: f32) -> bool {
        x > self.x0 - R && x < self.x1 + R && y > self.y0 - R && y < self.y1 + R
    }

    fn under(&self, x: f32) -> bool {
        x > self.x0 - R && x < self.x1 + R
    }
}

/// Where a resting treat is and how hard it will be to find.
#[derive(Clone, Copy, Debug)]
struct Spot {
    find: Find,
    label: &'static str,
    /// Lost, but in reach of a determined paw.
    rescue: bool,
    trick: u8,
}

// ---------------------------------------------------------------------------
// The house
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Fridge,
    WaterBowl,
    FoodBowl,
    PaperBag,
    Bin,
    Mat,
    Couch,
    Tunnel,
    Table,
    Rug,
    Basket,
    Bookshelf,
    Dresser,
    Vacuum,
    Yarn,
    Bed,
    Pillow,
    CatBed,
    Laundry,
    Blanket,
    Bush,
    Pot,
    Puddle,
    Bench,
    Rocks,
    /// Living room window. Only the sill is solid.
    Window,
    /// Kitchen counter, drawn with a faucet. Solid as one flat block.
    Counter,
}

impl Kind {
    /// Floor space the prop takes up.
    fn width(self) -> f32 {
        match self {
            Kind::Fridge => FRIDGE_W as f32,
            Kind::WaterBowl | Kind::FoodBowl => 22.0,
            Kind::PaperBag => PAPER_BAG.width as f32,
            Kind::Bin => TRASHCAN_BODY.width as f32,
            Kind::Mat => 30.0,
            Kind::Couch => COUCH_W as f32,
            Kind::Tunnel => 44.0,
            Kind::Table => 36.0,
            Kind::Rug => 40.0,
            Kind::Basket => BASKET.width as f32,
            Kind::Bookshelf | Kind::Dresser => CABINET_W as f32,
            Kind::Vacuum => VAC_RANGE + VAC_W,
            Kind::Yarn => 13.0,
            Kind::Bed => BED_W as f32,
            Kind::Pillow => 26.0,
            Kind::CatBed => CAT_BED_RIM_WIDTH as f32,
            Kind::Laundry => 24.0,
            Kind::Blanket => 30.0,
            Kind::Bush => 16.0,
            Kind::Pot => 17.0,
            Kind::Puddle => 24.0,
            Kind::Bench => 20.0,
            Kind::Rocks => 47.0,
            Kind::Window => SILL_W,
            Kind::Counter => COUNTER_W,
        }
    }
}

/// The window's sill, its top 30px above the floor like the real living
/// room's.
const SILL_W: f32 = 68.0;
const SILL_Y: f32 = 30.0;
/// The counter, as tall as the real kitchen's, and where the faucet stands
/// on it.
const COUNTER_W: f32 = 66.0;
const COUNTER_H: f32 = 36.0;
const FAUCET_X: f32 = 30.0;

/// Each room's fixed piece and the range of x it can stand at.
const ANCHORS: [(Kind, f32, f32); 4] = [
    (Kind::Couch, LAUNCH_ZONE + 40.0, LIVING_END - 150.0),
    (Kind::Fridge, LIVING_END + 60.0, KITCHEN_END - 120.0),
    (Kind::Bed, KITCHEN_END + 60.0, BEDROOM_END - 160.0),
    (Kind::Rocks, BEDROOM_END + 80.0, FENCE_X - 140.0),
];

/// Pieces always in a room besides its anchor, each somewhere in a range of
/// x (left edge to right edge).
const FIXTURES: [(Kind, f32, f32); 2] = [
    (Kind::Window, LAUNCH_ZONE, LIVING_END - 24.0),
    (Kind::Counter, LIVING_END + 24.0, KITCHEN_END - 24.0),
];

/// Each room's pool of random extras and how many it gets.
const EXTRAS: [(&[Kind], u32, u32); 4] = [
    (&[Kind::Tunnel, Kind::Table, Kind::Rug, Kind::Basket, Kind::Bookshelf, Kind::Vacuum, Kind::Yarn], 3, 3),
    (&[Kind::WaterBowl, Kind::FoodBowl, Kind::PaperBag, Kind::Bin, Kind::Mat], 1, 2),
    (&[Kind::Pillow, Kind::CatBed, Kind::Laundry, Kind::Blanket, Kind::Rug, Kind::Dresser], 2, 3),
    (&[Kind::Bush, Kind::Pot, Kind::Puddle, Kind::Bench], 2, 3),
];

#[derive(Clone, Copy, Debug)]
struct Prop {
    kind: Kind,
    x: f32,
}

#[derive(Clone, Copy, Debug)]
struct Vacuum {
    x: f32,
    lo: f32,
    hi: f32,
    dir: f32,
}

impl Vacuum {
    fn solid(&self) -> Solid {
        Solid {
            x0: self.x,
            y0: FLOOR - VAC_H,
            x1: self.x + VAC_W,
            y1: FLOOR,
            top: Surface::Wood,
            find: Find::Open,
            label: L_UP_HIGH,
            trick: 0,
        }
    }

    fn update(&mut self, dt: f32) {
        self.x += self.dir * VAC_SPEED * dt;
        if self.x <= self.lo {
            self.x = self.lo;
            self.dir = 1.0;
        } else if self.x >= self.hi {
            self.x = self.hi;
            self.dir = -1.0;
        }
    }
}

/// Laundry pile and blanket lump heights, column by column.
const LAUNDRY_PROFILE: [u8; 24] = [3, 5, 7, 8, 9, 10, 10, 11, 11, 12, 12, 11, 10, 10, 11, 12, 12, 11, 10, 9, 7, 6, 4, 2];
const BLANKET_PROFILE: [u8; 30] = [2, 3, 4, 5, 6, 6, 7, 7, 7, 8, 8, 8, 7, 7, 6, 6, 7, 7, 8, 8, 8, 7, 7, 6, 6, 5, 4, 4, 3, 2];

struct House {
    props: Vec<Prop, MAX_PROPS>,
    solids: Vec<Solid, MAX_SOLIDS>,
    zones: Vec<Zone, MAX_ZONES>,
    /// Yarn ball bumper: centre and radius.
    yarn: Option<(f32, f32, f32)>,
    vacuum: Option<Vacuum>,
}

impl House {
    /// Where the pet stands on the furniture to eat a treat resting at
    /// (`tx`, `top`): as close as it can reach while its body stays on the
    /// piece (centred if the piece is narrower than the pet), on the
    /// surface under that spot, facing the treat. Returns (x, y, faces
    /// right).
    fn perch_for(&self, tx: f32, top: f32) -> (f32, f32, bool) {
        let Some(p) = self.props.iter().find(|p| tx >= p.x && tx <= p.x + p.kind.width()) else {
            return (tx - EAT_OFF, top, true);
        };
        let (x0, x1) = (p.x, p.x + p.kind.width());
        let right = tx >= (x0 + x1) / 2.0;
        let want = if right { tx - EAT_OFF } else { tx + EAT_OFF };
        let (lo, hi) = (x0 + PET_HALF, x1 - PET_HALF);
        let x = if lo <= hi { want.clamp(lo, hi) } else { (x0 + x1) / 2.0 };
        let y = self
            .solids
            .iter()
            .filter(|s| x >= s.x0 && x <= s.x1 && s.y0 < FLOOR - 1.0)
            .map(|s| s.y0)
            .fold(f32::MAX, f32::min);
        (x, if y == f32::MAX { top } else { y }, right)
    }

    const fn empty() -> Self {
        Self {
            props: Vec::new(),
            solids: Vec::new(),
            zones: Vec::new(),
            yarn: None,
            vacuum: None,
        }
    }

    fn generate(rng: &mut u32) -> Self {
        let mut h = House::empty();
        let mut spans: Vec<(f32, f32), MAX_PROPS> = Vec::new();
        for &(kind, lo, hi) in ANCHORS.iter() {
            let x = rand_range_f32(rng, lo, hi).round();
            h.add(kind, x, &mut spans);
        }
        for &(kind, lo, hi) in FIXTURES.iter() {
            let w = kind.width();
            for _ in 0..50 {
                let x = rand_range_f32(rng, lo, hi - w).round();
                if !spans.iter().any(|&(a, b)| x < b + PROP_GAP && x + w > a - PROP_GAP) {
                    h.add(kind, x, &mut spans);
                    break;
                }
            }
        }
        for (room, &(pool, nmin, nmax)) in ROOMS.iter().zip(EXTRAS.iter()) {
            let want = rand_range_u32(rng, nmin, nmax);
            let mut used = 0u32;
            let mut placed = 0;
            for _ in 0..200 {
                if placed >= want {
                    break;
                }
                let i = rand_range_u32(rng, 0, pool.len() as u32 - 1) as usize;
                if used & (1 << i) != 0 {
                    continue;
                }
                let kind = pool[i];
                let w = kind.width();
                let lo = (room.0 + 24.0).max(LAUNCH_ZONE);
                let hi = room.1 - 24.0 - w;
                if hi <= lo {
                    continue;
                }
                let x = rand_range_f32(rng, lo, hi).round();
                if spans.iter().any(|&(a, b)| x < b + PROP_GAP && x + w > a - PROP_GAP) {
                    continue;
                }
                h.add(kind, x, &mut spans);
                used |= 1 << i;
                placed += 1;
            }
        }
        h.add_structure();
        h
    }

    fn add(&mut self, kind: Kind, x: f32, spans: &mut Vec<(f32, f32), MAX_PROPS>) {
        let _ = spans.push((x, x + kind.width()));
        let _ = self.props.push(Prop { kind, x });
        self.build(kind, x);
    }

    #[allow(clippy::too_many_arguments)]
    fn solid(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, top: Surface, find: Find, label: &'static str, trick: u8) {
        let _ = self.solids.push(Solid { x0, y0, x1, y1, top, find, label, trick });
    }

    fn zone(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, kind: ZoneKind, label: &'static str, floor: Option<Surface>) {
        let _ = self.zones.push(Zone { x0, y0, x1, y1, kind, label, floor });
    }

    /// Collision geometry for one prop with its left edge at `x`.
    fn build(&mut self, kind: Kind, x: f32) {
        let f = FLOOR;
        let w = kind.width();
        match kind {
            Kind::Fridge => {
                // A treat can just squeeze into the gap between the feet.
                let under = f - FRIDGE_FEET_H as f32 - 0.5;
                self.solid(x, f - FRIDGE_H as f32, x + w, under, Surface::Wood, Find::Hidden, L_ON_FRIDGE, 0);
                self.zone(x, under, x + w, f, ZoneKind::Gone { splash: false }, L_UNDER_FRIDGE, None);
            }
            Kind::WaterBowl | Kind::FoodBowl => {
                self.solid(x, f - 6.0, x + 3.0, f, Surface::Wood, Find::Open, L_UP_HIGH, 0);
                self.solid(x + w - 3.0, f - 6.0, x + w, f, Surface::Wood, Find::Open, L_UP_HIGH, 0);
                if kind == Kind::WaterBowl {
                    self.zone(x + 3.0, f - 5.0, x + w - 3.0, f, ZoneKind::Gone { splash: true }, L_WATER, None);
                } else {
                    self.zone(x + 3.0, f - 6.0, x + w - 3.0, f, ZoneKind::Bowl, L_FOOD, Some(Surface::Soft));
                }
            }
            Kind::PaperBag => {
                // Lying on its side, open to the left. The top counts as
                // one flat side; the folded bottom closes the right end.
                let top = f - PAPER_BAG.height as f32;
                self.solid(x + 5.0, top, x + 40.0, top + 2.0, Surface::Fabric, Find::Tucked, L_UP_HIGH, 0);
                self.solid(x + 36.0, top + 2.0, x + w, f, Surface::Fabric, Find::Tucked, L_UP_HIGH, 0);
                self.zone(x + 5.0, top + 2.0, x + 36.0, f, ZoneKind::Inside, L_BAG, Some(Surface::Rug));
            }
            Kind::Bin => {
                // Open-topped, walls three pixels thick at the rim. Anything
                // that drops in is gone.
                let h = TRASHCAN_BODY.height as f32;
                self.solid(x, f - h, x + 3.0, f, Surface::Wood, Find::Hidden, L_UP_HIGH, 0);
                self.solid(x + w - 3.0, f - h, x + w, f, Surface::Wood, Find::Hidden, L_UP_HIGH, 0);
                self.zone(x + 3.0, f - h + 1.0, x + w - 3.0, f, ZoneKind::Gone { splash: false }, L_BIN, None);
            }
            Kind::Mat => {
                self.zone(x, f - 4.0, x + w, f + 1.0, ZoneKind::Rug, L_MAT, Some(Surface::Rug));
            }
            Kind::Rug => {
                self.zone(x, f - 4.0, x + w, f + 1.0, ZoneKind::Rug, L_RUG, Some(Surface::Rug));
            }
            Kind::Couch => {
                // Armrests and seat are solid down to the base; the treat
                // can only slip into the low gap between the feet.
                let top = f - COUCH_H as f32;
                let under = top + COUCH_BASE_Y as f32 + 0.5;
                let arm = 8.0;
                self.solid(x, top + 9.0, x + arm, under, Surface::Soft, Find::Hidden, L_ON_COUCH, 0);
                self.solid(x + arm, top + COUCH_SEAT_Y as f32, x + w - arm, under, Surface::Bouncy, Find::Hidden, L_ON_COUCH, T_COUCH);
                self.solid(x + w - arm, top + 9.0, x + w, under, Surface::Soft, Find::Hidden, L_ON_COUCH, 0);
                self.zone(x, under, x + w, f, ZoneKind::Under { edge: 8.0 }, L_UNDER_COUCH, Some(Surface::Rug));
            }
            Kind::Tunnel => {
                self.solid(x, f - 14.0, x + w, f - 12.0, Surface::Fabric, Find::Hidden, L_ON_TUNNEL, 0);
                self.zone(x, f - 12.0, x + w, f, ZoneKind::Tunnel, L_TUNNEL, Some(Surface::Tile));
            }
            Kind::Table => {
                self.solid(x, f - 20.0, x + w, f - 17.0, Surface::Wood, Find::Hidden, L_ON_TABLE, 0);
                self.zone(x, f - 17.0, x + w, f, ZoneKind::Tucked, L_UNDER_TABLE, None);
            }
            Kind::Basket => {
                // Open-topped: the rolled rim is the top of each wall.
                let h = BASKET.height as f32;
                self.solid(x, f - h, x + 2.0, f, Surface::Wood, Find::Hidden, L_UP_HIGH, 0);
                self.solid(x + w - 2.0, f - h, x + w, f, Surface::Wood, Find::Hidden, L_UP_HIGH, 0);
                self.zone(x + 2.0, f - h + 1.0, x + w - 2.0, f, ZoneKind::Inside, L_BASKET, Some(Surface::Soft));
            }
            Kind::Bookshelf => {
                self.solid(x, f - CABINET_H as f32, x + w, f, Surface::Wood, Find::Hidden, L_BOOKSHELF, 0);
            }
            Kind::Dresser => {
                self.solid(x, f - CABINET_H as f32, x + w, f, Surface::Wood, Find::Hidden, L_DRESSER, 0);
            }
            Kind::Vacuum => {
                self.vacuum = Some(Vacuum { x, lo: x, hi: x + VAC_RANGE, dir: 1.0 });
            }
            Kind::Yarn => {
                self.yarn = Some((x + 6.5, f - 6.5, 6.0));
            }
            Kind::Bed => {
                // Turned round from the bedroom's, headboard nearest the
                // thrower, so a bounce off the mattress carries on over the
                // low footboard instead of into the headboard. The posts
                // only block down to the side rail: below it a treat can
                // slip under, behind them.
                let top = f - BED_H as f32;
                let under = top + BED_RAIL_BOTTOM_Y as f32 + 1.0;
                let mattress = top + BED_MATTRESS_Y as f32;
                let (head, foot) = (10.0, w - 6.0);
                let pillow = w - BED_PILLOW_X as f32;
                self.solid(x, top, x + head, under, Surface::Wood, Find::Hidden, L_ON_BED, 0);
                self.solid(x + head, top + BED_PILLOW_Y as f32, x + pillow, mattress, Surface::Soft, Find::Hidden, L_ON_BED, 0);
                self.solid(x + head, mattress, x + foot, under, Surface::Bouncy, Find::Hidden, L_ON_BED, T_BED);
                self.solid(x + foot, top + 14.0, x + w, under, Surface::Wood, Find::Hidden, L_ON_BED, 0);
                self.zone(x, under, x + w, f, ZoneKind::Under { edge: 12.0 }, L_UNDER_BED, Some(Surface::Rug));
            }
            Kind::Pillow => {
                self.solid(x + 1.0, f - 7.0, x + w - 1.0, f, Surface::Soft, Find::Tucked, L_PILLOW, 0);
            }
            Kind::CatBed => {
                self.solid(x, f - 10.0, x + 8.0, f, Surface::Soft, Find::Tucked, L_CATBED, 0);
                self.solid(x + w - 8.0, f - 10.0, x + w, f, Surface::Soft, Find::Tucked, L_CATBED, 0);
                self.zone(x + 8.0, f - 9.0, x + w - 8.0, f, ZoneKind::CatBed, L_CATBED, Some(Surface::Soft));
            }
            Kind::Laundry => {
                self.zone(x, f - 11.0, x + w, f, ZoneKind::Soft, L_LAUNDRY, Some(Surface::Soft));
            }
            Kind::Blanket => {
                self.zone(x, f - 7.0, x + w, f, ZoneKind::Soft, L_BLANKET, Some(Surface::Soft));
            }
            Kind::Bush => {
                self.zone(x + 1.0, f - 11.0, x + w - 1.0, f, ZoneKind::Soft, L_BUSH, Some(Surface::Soft));
            }
            Kind::Pot => {
                self.solid(x + 2.0, f - 11.0, x + w - 2.0, f, Surface::Soft, Find::Hidden, L_POT, 0);
            }
            Kind::Puddle => {
                self.zone(x + 1.0, f - 2.0, x + w - 1.0, f, ZoneKind::Gone { splash: true }, L_PUDDLE, None);
            }
            Kind::Bench => {
                self.solid(x, f - 8.0, x + w, f - 6.0, Surface::Wood, Find::Hidden, L_ON_BENCH, 0);
                self.zone(x, f - 6.0, x + w, f, ZoneKind::Tucked, L_UNDER_BENCH, None);
            }
            Kind::Window => {
                self.solid(x, SILL_Y, x + w, SILL_Y + 3.0, Surface::Wood, Find::Hidden, L_SILL, 0);
            }
            Kind::Counter => {
                // Worktop with a lip over the cupboards. The faucet is only
                // drawn: treats pass in front of it.
                let top = f - COUNTER_H;
                self.solid(x, top, x + w, top + 4.0, Surface::Tile, Find::Hidden, L_COUNTER, 0);
                self.solid(x + 3.0, top + 4.0, x + w - 3.0, f, Surface::Wood, Find::Hidden, L_COUNTER, 0);
            }
            Kind::Rocks => {
                self.solid(x + 3.0, f - 10.0, x + 44.0, f, Surface::Rock, Find::Tucked, L_ROCKS, T_ROCK);
                self.solid(x + 10.0, f - 17.0, x + 37.0, f - 10.0, Surface::Rock, Find::Tucked, L_ROCKS, T_ROCK);
                self.solid(x + 17.0, f - 22.0, x + 30.0, f - 17.0, Surface::Rock, Find::Tucked, L_ROCKS, T_ROCK);
            }
        }
    }

    /// The garden fence. Doorways are open all the way to the ceiling.
    fn add_structure(&mut self) {
        self.solid(FENCE_X, FENCE_TOP, FENCE_X + 4.0, FLOOR, Surface::Wood, Find::Hidden, L_ON_FENCE, 0);
        self.zone(FENCE_X + 4.0, -1000.0, WORLD_W + 100.0, FLOOR, ZoneKind::Gone { splash: false }, L_FENCE, None);
    }

    fn room_floor(x: f32) -> Surface {
        if x < LIVING_END {
            Surface::Carpet
        } else if x < KITCHEN_END {
            Surface::Tile
        } else if x < BEDROOM_END {
            Surface::Carpet
        } else {
            Surface::Grass
        }
    }

    fn floor_surface(&self, x: f32) -> Surface {
        self.zones
            .iter()
            .find(|z| z.contains(x, FLOOR - R))
            .and_then(|z| z.floor)
            .unwrap_or(House::room_floor(x))
    }

    fn zone_at(&self, x: f32, y: f32) -> Option<(usize, &Zone)> {
        self.zones.iter().enumerate().find(|(_, z)| z.contains(x, y))
    }

    /// The first solid the treat would overlap at (x, y), the vacuum included.
    fn hit(&self, x: f32, y: f32) -> Option<(u8, Solid)> {
        for (i, s) in self.solids.iter().enumerate() {
            if s.hit(x, y) {
                return Some((i as u8, *s));
            }
        }
        if let Some(v) = &self.vacuum {
            let s = v.solid();
            if s.hit(x, y) {
                return Some((u8::MAX, s));
            }
        }
        None
    }

    /// The solid a treat is resting on top of, if any (not the floor).
    fn resting_on(&self, t: &Treat) -> Option<Solid> {
        let foot = t.y + R;
        self.solids
            .iter()
            .copied()
            .chain(self.vacuum.as_ref().map(|v| v.solid()))
            .find(|s| (s.y0 - foot).abs() < 0.1 && s.under(t.x))
    }

    /// Surface under a treat touching the floor or a solid.
    fn support(&self, t: &Treat) -> Option<Surface> {
        if t.y + R >= FLOOR - 0.05 {
            return Some(self.floor_surface(t.x));
        }
        self.resting_on(t).map(|s| s.top)
    }

    fn spot(&self, t: &Treat) -> Spot {
        let open = |find, label| Spot { find, label, rescue: false, trick: 0 };
        if let Some((_, z)) = self.zone_at(t.x, t.y) {
            return match z.kind {
                ZoneKind::Rug | ZoneKind::Tucked => open(Find::Tucked, z.label),
                ZoneKind::Soft | ZoneKind::Inside | ZoneKind::Tunnel => open(Find::Hidden, z.label),
                ZoneKind::Under { edge } => {
                    let d = (t.x - z.x0).min(z.x1 - t.x);
                    if d < edge {
                        open(Find::Tucked, z.label)
                    } else {
                        Spot { find: Find::Lost, label: z.label, rescue: true, trick: 0 }
                    }
                }
                ZoneKind::Bowl => open(Find::Open, z.label),
                ZoneKind::CatBed => Spot { find: Find::Open, label: z.label, rescue: false, trick: T_CATBED },
                ZoneKind::Gone { .. } => open(Find::Lost, z.label),
            };
        }
        if t.y + R < FLOOR - 0.05 {
            if let Some(s) = self.resting_on(t) {
                return open(s.find, s.label);
            }
        }
        open(Find::Open, L_OPEN)
    }
}

// ---------------------------------------------------------------------------
// Treat physics
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Treat {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Flight {
    still: f32,
    time: f32,
    /// Trampoline last bounced on, so a second bounce in a row is damped.
    last_tramp: Option<u8>,
    /// Tunnel zone the treat is in, and whether it came in from the left.
    tunnel: Option<(usize, bool)>,
    tricks: u8,
    /// Tricks not yet called out.
    fresh: u8,
    /// Whether the treat went somewhere it's gone for good.
    splash: bool,
}

impl Flight {
    fn trick(&mut self, bit: u8) {
        if bit != 0 && self.tricks & bit == 0 {
            self.tricks |= bit;
            self.fresh |= bit;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Fly {
    Moving,
    Stopped,
    Gone(&'static str),
}

fn launch(angle_deg: f32, power: f32) -> Treat {
    let a = angle_deg * core::f32::consts::PI / 180.0;
    let v = V_MIN + power.clamp(0.0, 1.0) * (V_MAX - V_MIN);
    Treat { x: LAUNCH_X, y: LAUNCH_Y, vx: a.cos() * v, vy: -a.sin() * v }
}

fn indoor(x: f32) -> bool {
    x < BEDROOM_END
}

fn sign(v: f32) -> f32 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

/// A landing on top of `surf`.
fn land(t: &mut Treat, f: &mut Flight, surf: Surface, solid: Option<u8>, trick: u8, rng: &mut u32) {
    let vy = t.vy;
    if surf == Surface::Bouncy && vy > TRAMP_MIN {
        if f.last_tramp != solid {
            t.vy = -(vy * TRAMP_GAIN).min(TRAMP_MAX);
            t.vx = (t.vx * TRAMP_PUSH).clamp(-V_MAX, V_MAX);
            f.trick(trick);
        } else {
            t.vy = -vy * TRAMP_REPEAT;
        }
        f.last_tramp = solid;
        return;
    }
    f.last_tramp = None;
    let p = feel(surf);
    if vy > IMPACT {
        t.vx *= p.keep;
        if surf == Surface::Rock {
            t.vx += rand_range_f32(rng, -30.0, 45.0);
            f.trick(trick);
        }
    }
    t.vy = -vy * p.bounce;
    if t.vy.abs() < REST_VY {
        t.vy = 0.0;
    }
}

/// Advance the treat by one short substep. Returns the label of the place
/// that swallowed it, if it went somewhere it's gone for good.
fn sub_step(h: &House, t: &mut Treat, f: &mut Flight, rng: &mut u32, dt: f32) -> Option<&'static str> {
    let mut grav = G;
    let zone = h.zone_at(t.x, t.y);
    if let Some((_, z)) = zone {
        match z.kind {
            ZoneKind::Gone { splash } => {
                f.splash = splash;
                return Some(z.label);
            }
            ZoneKind::Soft => {
                let k = (1.0 - SOFT_DAMP * dt).max(0.0);
                t.vx *= k;
                t.vy *= k;
                grav *= 0.4;
            }
            _ => {}
        }
    }
    t.vy += grav * dt;

    // Sideways: walls.
    let mut nx = t.x + t.vx * dt;
    if let Some((_, s)) = h.hit(nx, t.y) {
        nx = if t.vx > 0.0 { s.x0 - R - EPS } else { s.x1 + R + EPS };
        if s.y0 > t.y + R - RAMP_HEIGHT && t.vy > -REST_VY {
            // A low rim: clip it and hop over.
            t.vy = -t.vx.abs() * RAMP_LIFT;
            t.vx *= 0.75;
        } else {
            if t.vx.abs() > WALL_TRICK_SPEED {
                f.trick(T_WALL);
            }
            t.vx = -t.vx * WALL_BOUNCE;
        }
    }
    if nx < R {
        nx = R;
        t.vx = -t.vx * WALL_BOUNCE;
    }
    t.x = nx;

    // Yarn ball bumper.
    if let Some((cx, cy, r)) = h.yarn {
        let (dx, dy) = (t.x - cx, t.y - cy);
        let d2 = dx * dx + dy * dy;
        let rr = r + R;
        if d2 < rr * rr && d2 > 1e-4 {
            let d = d2.sqrt();
            let (nx, ny) = (dx / d, dy / d);
            t.x = cx + nx * (rr + EPS);
            t.y = cy + ny * (rr + EPS);
            let vn = t.vx * nx + t.vy * ny;
            if vn < 0.0 {
                t.vx -= (1.0 + BUMPER_BOUNCE) * vn * nx;
                t.vy -= (1.0 + BUMPER_BOUNCE) * vn * ny;
                if -vn > 20.0 {
                    f.trick(T_YARN);
                }
            }
            // Never balance on top of the ball.
            if t.vx.abs() < 5.0 && ny < -0.8 {
                t.vx += 12.0 * sign(dx);
            }
        }
    }

    // Up and down: floor, ceiling, tops and undersides.
    let mut ny = t.y + t.vy * dt;
    if ny + R > FLOOR {
        ny = FLOOR - R - EPS;
        land(t, f, h.floor_surface(t.x), None, 0, rng);
    } else if indoor(t.x) && ny - R < CEIL {
        ny = CEIL + R + EPS;
        t.vy = t.vy.abs() * CEIL_BOUNCE;
    } else if let Some((i, s)) = h.hit(t.x, ny) {
        if t.vy > 0.0 {
            ny = s.y0 - R - EPS;
            land(t, f, s.top, Some(i), s.trick, rng);
            // The vacuum won't carry a treat; it slides off.
            if i == u8::MAX && t.vx.abs() < 20.0 {
                t.vx = 30.0 * sign(t.x - (s.x0 + s.x1) / 2.0);
            }
        } else {
            ny = s.y1 + R + EPS;
            t.vy = -t.vy * UNDERSIDE_BOUNCE;
        }
    }
    t.y = ny;

    // Sliding friction. Gravity tugs vy off zero between substeps even while
    // the treat sits on something, so "slow and touching" counts as sliding.
    if t.vy.abs() < REST_VY {
        if let Some(surf) = h.support(t) {
            let d = feel(surf).drag * dt;
            if t.vx.abs() <= d {
                t.vx = 0.0;
            } else {
                t.vx -= d * sign(t.vx);
            }
        }
    }

    // Tunnel: shoot out of the far end faster than it went in.
    let tz = h.zone_at(t.x, t.y).filter(|(_, z)| z.kind == ZoneKind::Tunnel);
    match (f.tunnel, tz) {
        (None, Some((i, z))) => f.tunnel = Some((i, t.x < (z.x0 + z.x1) / 2.0)),
        (Some((i, from_left)), None) => {
            let z = &h.zones[i];
            let far = if from_left { t.x >= z.x1 } else { t.x < z.x0 };
            if far {
                let dir = if from_left { 1.0 } else { -1.0 };
                t.vx = dir * (t.vx.abs() * TUNNEL_BOOST).clamp(TUNNEL_MIN, V_MAX);
                f.trick(T_TUNNEL);
            }
            f.tunnel = None;
        }
        _ => {}
    }
    None
}

fn fly(h: &House, t: &mut Treat, f: &mut Flight, rng: &mut u32, dt: f32) -> Fly {
    f.time += dt;
    let speed = t.vx.abs().max(t.vy.abs());
    let n = ((speed * dt / MAX_STEP).ceil() as usize).clamp(1, 64);
    let step = dt / n as f32;
    for _ in 0..n {
        if let Some(label) = sub_step(h, t, f, rng, step) {
            return Fly::Gone(label);
        }
    }
    let soft = matches!(h.zone_at(t.x, t.y), Some((_, z)) if z.kind == ZoneKind::Soft);
    let settled = if soft {
        t.vx.abs() < STOP_SPEED * 2.0 && t.vy.abs() < STOP_SPEED * 4.0
    } else {
        h.support(t).is_some() && t.vx.abs() < STOP_SPEED && t.vy.abs() < REST_VY
    };
    if settled {
        f.still += dt;
        if f.still >= STOP_TIME {
            t.vx = 0.0;
            t.vy = 0.0;
            return Fly::Stopped;
        }
    } else {
        f.still = 0.0;
    }
    if f.time > MAX_FLIGHT {
        return Fly::Stopped;
    }
    Fly::Moving
}

/// Gauge reading, 0..1, swinging smoothly up and down.
fn gauge(phase: f32) -> f32 {
    (1.0 - phase.cos()) / 2.0
}

fn paws(x: f32) -> u32 {
    ((x - LAUNCH_X) / PAW).max(0.0) as u32
}

fn score(paws: u32, find: Find, tricks: u8) -> u32 {
    let bonus = 1.0 + 0.1 * tricks.count_ones() as f32;
    (paws as f32 * find.mult() * bonus + 0.5) as u32
}

// ---------------------------------------------------------------------------
// The pet's hunt
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Go { x: f32, run: bool },
    Hold { pose: PoseId, secs: f32, right: bool },
    Jump { x: f32, y: f32 },
    /// Paw a lost treat back out from under the furniture.
    Reveal,
    Pounce,
    Eat,
    Happy,
    Leave,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bird {
    Swoop,
    Carry,
    Flee,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Intro,
    Aim,
    Power,
    Flight,
    Hunt,
    Result,
    RoundOver,
}

pub struct TreatTossScene {
    house: House,
    /// This pet's fridge door decorations.
    fridge: FridgeLayout,
    state: State,
    treat: Treat,
    flight: Flight,
    treat_visible: bool,
    /// Drawn in front of everything: pawed out from under the furniture.
    treat_revealed: bool,
    spot: Spot,

    angle: f32,
    aim_phase: f32,
    phase: f32,
    cam_x: f32,
    /// How far ahead of the flying treat the camera looks, eased so a bounce
    /// back off a wall swings the view round instead of snapping it.
    cam_lead: f32,

    character: Character,
    pet_x: f32,
    pet_y: f32,
    steps: Vec<Step, MAX_STEPS>,
    step_i: usize,
    step_t: f32,
    jump_from: (f32, f32),
    /// Which way the pet faces to pounce and eat.
    eat_right: bool,
    notice: f32,
    pet_run: f32,
    pet_crazy: bool,
    sniff_k: f32,
    rescue_chance: f32,
    /// Shrugs off a lost treat this time instead of sulking.
    shrug: bool,

    bird: Option<(Bird, f32, f32, f32)>,
    splash: Option<(f32, f32, f32)>,

    callout: Option<&'static str>,
    callout_t: f32,

    throw_no: usize,
    scores: [Option<u32>; TREATS],
    /// Where the round's best throw landed, flagged on the floor.
    best_x: Option<f32>,
    best_throw: u32,
    /// Every throw this round added up.
    round_score: u32,

    session_throws: u32,
    session_found: u32,
    session_score: u32,

    popup: Popup,
    options_menu: Menu<TreatTossAction>,
    menu_active: bool,
}

impl TreatTossScene {
    pub fn new() -> Self {
        Self {
            house: House::empty(),
            fridge: FridgeLayout::from_seed(0),
            state: State::Intro,
            treat: Treat { x: LAUNCH_X, y: LAUNCH_Y, vx: 0.0, vy: 0.0 },
            flight: Flight::default(),
            treat_visible: true,
            treat_revealed: false,
            spot: Spot { find: Find::Open, label: L_OPEN, rescue: false, trick: 0 },

            angle: ANGLE_MIN,
            aim_phase: 0.0,
            phase: 0.0,
            cam_x: 0.0,
            cam_lead: 0.0,

            character: Character::new(Point::new(PET_HOME_X as i32, FLOOR as i32)),
            pet_x: PET_HOME_X,
            pet_y: FLOOR,
            steps: Vec::new(),
            step_i: 0,
            step_t: 0.0,
            jump_from: (0.0, 0.0),
            eat_right: true,
            notice: 0.0,
            pet_run: 120.0,
            pet_crazy: false,
            sniff_k: 1.0,
            rescue_chance: 0.0,
            shrug: false,

            bird: None,
            splash: None,

            callout: None,
            callout_t: 0.0,

            throw_no: 0,
            scores: [None; TREATS],
            best_x: None,
            best_throw: 0,
            round_score: 0,

            session_throws: 0,
            session_found: 0,
            session_score: 0,

            popup: Popup::new(8, 8, 112, 48),
            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn start_round(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.house = House::generate(&mut rng);
        ctx.rng = rng;
        self.throw_no = 0;
        self.scores = [None; TREATS];
        self.best_x = None;
        self.best_throw = 0;
        self.round_score = 0;
        self.ready_throw();
    }

    /// Put a fresh treat in the hand and the pet back at its spot.
    fn ready_throw(&mut self) {
        self.treat = Treat { x: LAUNCH_X, y: LAUNCH_Y, vx: 0.0, vy: 0.0 };
        self.flight = Flight::default();
        self.treat_visible = true;
        self.treat_revealed = false;
        self.bird = None;
        self.splash = None;
        self.callout = None;
        self.pet_x = PET_HOME_X;
        self.pet_y = FLOOR;
        self.character.mirror_h = true;
        self.character.set_pose(PoseId::LeaningForwardSidePounce);
        self.steps.clear();
        self.aim_phase = 0.0;
        self.angle = ANGLE_MIN;
        if self.state != State::Intro {
            self.state = State::Aim;
        }
    }

    fn handle_menu(&mut self, ctx: &mut GameContext, buttons: &mut Buttons) {
        match self.options_menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => self.menu_active = false,
            MenuResult::Action(TreatTossAction::NewRound) => {
                self.menu_active = false;
                self.state = State::Aim;
                self.start_round(ctx);
            }
        }
    }

    fn update_aim(&mut self, dt: f32) {
        self.aim_phase = (self.aim_phase + dt * 2.0 * core::f32::consts::PI / AIM_PERIOD) % (2.0 * core::f32::consts::PI);
        self.angle = ANGLE_MIN + gauge(self.aim_phase) * (ANGLE_MAX - ANGLE_MIN);
    }

    fn throw(&mut self, ctx: &GameContext) {
        self.treat = launch(self.angle, gauge(self.phase));
        self.flight = Flight::default();
        self.state = State::Flight;
        self.cam_lead = 0.0;
        self.pet_run = 110.0 + 0.4 * ctx.fitness + 0.2 * ctx.energy;
        self.pet_crazy = ctx.playfulness > 70.0;
        self.sniff_k = 1.35 - 0.007 * (ctx.curiosity + ctx.focus) / 2.0;
        self.rescue_chance = 0.25 * ctx.curiosity / 100.0;
    }

    fn ease_cam(&mut self, target: f32, rate: f32, dt: f32) {
        let target = target.clamp(0.0, WORLD_W - SCREEN_W as f32);
        let k = (rate * dt).min(1.0);
        self.cam_x += (target - self.cam_x) * k;
    }

    fn update_flight(&mut self, ctx: &mut GameContext, dt: f32) {
        if let Some(v) = self.house.vacuum.as_mut() {
            v.update(dt);
        }
        let mut rng = ctx.rng;
        let result = fly(&self.house, &mut self.treat, &mut self.flight, &mut rng, dt);
        ctx.rng = rng;
        self.show_tricks();
        let lead = (self.treat.vx * 0.3).clamp(-30.0, 30.0);
        self.cam_lead += (lead - self.cam_lead) * (CAM_LEAD_RATE * dt).min(1.0);
        self.ease_cam(self.treat.x - 56.0 + self.cam_lead, 8.0, dt);

        if result == Fly::Moving && self.vacuum_eats() {
            self.settle(ctx, Some(L_VACUUM));
            return;
        }
        match result {
            Fly::Moving => {}
            Fly::Stopped => self.settle(ctx, None),
            Fly::Gone(label) => {
                if self.flight.splash {
                    self.splash = Some((self.treat.x, FLOOR - 3.0, 0.0));
                }
                self.settle(ctx, Some(label));
            }
        }
    }

    /// Whether the robot vacuum has run into a treat lying on the floor.
    fn vacuum_eats(&self) -> bool {
        match &self.house.vacuum {
            Some(v) if self.treat_visible => {
                let s = v.solid();
                self.treat.y + R >= FLOOR - 1.0
                    && self.treat.vx.abs() < 40.0
                    && self.treat.x > s.x0 - R
                    && self.treat.x < s.x1 + R
            }
            _ => false,
        }
    }

    fn show_tricks(&mut self) {
        let fresh = self.flight.fresh;
        if fresh != 0 {
            let bit = 7 - fresh.leading_zeros() as usize;
            self.callout = Some(TRICK_TEXT[bit]);
            self.callout_t = CALLOUT_TIME;
            self.flight.fresh = 0;
        }
    }

    /// The treat has stopped, or gone somewhere it won't come back from.
    fn settle(&mut self, ctx: &mut GameContext, gone: Option<&'static str>) {
        self.spot = match gone {
            Some(label) => Spot { find: Find::Lost, label, rescue: false, trick: 0 },
            None => self.house.spot(&self.treat),
        };
        if gone.is_some() {
            self.treat_visible = false;
            self.treat.x = self.treat.x.min(FENCE_X - 4.0);
        }
        self.flight.trick(self.spot.trick);
        self.show_tricks();

        let mut rng = ctx.rng;
        if gone.is_none()
            && self.spot.find == Find::Open
            && self.treat.x > BEDROOM_END
            && rand_bool(&mut rng, BIRD_CHANCE)
        {
            self.bird = Some((Bird::Swoop, 0.0, self.treat.x + 90.0, -20.0));
        }
        let rescued = self.spot.rescue && rand_bool(&mut rng, self.rescue_chance);
        self.plan_hunt(&mut rng, rescued);
        ctx.rng = rng;

        // The pet runs in from just off the left of the screen, unless the
        // treat is close enough to run from where it was waiting.
        let cam = (self.treat.x - 76.0).clamp(0.0, WORLD_W - SCREEN_W as f32);
        if self.treat.x - EAT_OFF > PET_HOME_X + 90.0 {
            self.pet_x = (cam - 30.0).max(PET_HOME_X);
        }
        self.pet_y = FLOOR;
        self.character.set_pose(PoseId::SittingSideNeutral);
        self.notice = NOTICE;
        self.state = State::Hunt;
    }

    fn plan_hunt(&mut self, rng: &mut u32, rescued: bool) {
        let tx = self.treat.x.clamp(30.0, FENCE_X - 8.0);
        let top = self.treat.y + R;
        let eat_x = (tx - EAT_OFF).max(10.0);
        let elevated = top < FLOOR - 4.0 && self.treat_visible;
        let k = self.sniff_k;
        let sniff = |rng: &mut u32, lo: f32, hi: f32| rand_range_f32(rng, lo, hi) * k;
        let mut s: Vec<Step, MAX_STEPS> = Vec::new();
        let sniff_side = PoseId::StandingSideSniffing;
        let sniff_low = PoseId::LeaningForwardSideSniffing;
        self.shrug = rand_bool(rng, SHRUG_CHANCE);

        match self.spot.find {
            Find::Open => {
                let _ = s.push(Step::Go { x: eat_x, run: true });
            }
            Find::Tucked => {
                let _ = s.push(Step::Go { x: eat_x - 4.0, run: true });
                let _ = s.push(Step::Hold { pose: sniff_low, secs: sniff(rng, 0.9, 1.6), right: true });
            }
            Find::Hidden => {
                let _ = s.push(Step::Go { x: tx - 36.0, run: true });
                let _ = s.push(Step::Hold { pose: sniff_side, secs: sniff(rng, 0.8, 1.3), right: true });
                let _ = s.push(Step::Go { x: tx + 20.0, run: false });
                let _ = s.push(Step::Hold { pose: sniff_side, secs: sniff(rng, 0.8, 1.3), right: false });
                let _ = s.push(Step::Go { x: eat_x, run: false });
                let _ = s.push(Step::Hold { pose: sniff_low, secs: sniff(rng, 0.8, 1.4), right: true });
            }
            Find::Lost => {
                let _ = s.push(Step::Go { x: tx - 30.0, run: true });
                let _ = s.push(Step::Hold { pose: sniff_side, secs: sniff(rng, 0.9, 1.3), right: true });
                let _ = s.push(Step::Go { x: tx + 14.0, run: false });
                let _ = s.push(Step::Hold { pose: sniff_low, secs: sniff(rng, 0.9, 1.3), right: false });
                let _ = s.push(Step::Go { x: eat_x, run: false });
                if self.spot.rescue {
                    let _ = s.push(Step::Hold { pose: PoseId::PawingSideNeutral, secs: 1.6, right: true });
                }
                if rescued {
                    let _ = s.push(Step::Reveal);
                } else {
                    push_grump(&mut s, self.shrug);
                    self.steps = s;
                    self.step_i = 0;
                    self.step_t = 0.0;
                    return;
                }
            }
        }
        self.eat_right = true;
        if elevated {
            let (x, y, right) = self.house.perch_for(tx, top);
            self.eat_right = right;
            let _ = s.push(Step::Jump { x, y });
        }
        let _ = s.push(Step::Pounce);
        let _ = s.push(Step::Eat);
        let _ = s.push(Step::Happy);
        self.steps = s;
        self.step_i = 0;
        self.step_t = 0.0;
    }

    /// Something took the treat while the pet was still looking.
    fn interrupt(&mut self, label: &'static str) {
        self.spot = Spot { find: Find::Lost, label, rescue: false, trick: 0 };
        self.treat_visible = false;
        self.pet_y = FLOOR;
        let mut s: Vec<Step, MAX_STEPS> = Vec::new();
        push_grump(&mut s, self.shrug);
        self.steps = s;
        self.step_i = 0;
        self.step_t = 0.0;
    }

    fn update_hunt(&mut self, ctx: &mut GameContext, dt: f32) {
        if let Some(v) = self.house.vacuum.as_mut() {
            v.update(dt);
        }
        let eating = matches!(self.steps.get(self.step_i), Some(Step::Eat | Step::Happy));
        if !eating && self.spot.find != Find::Lost && self.vacuum_eats() {
            self.interrupt(L_VACUUM);
        }
        self.update_bird(dt);

        let focus = if self.treat_visible || self.spot.find == Find::Lost { self.treat.x } else { self.pet_x + EAT_OFF };
        self.ease_cam(focus - 76.0, 4.0, dt);

        if self.notice > 0.0 {
            self.notice -= dt;
            return;
        }
        let Some(&step) = self.steps.get(self.step_i) else {
            self.finish_throw();
            return;
        };
        self.step_t += dt;
        let done = match step {
            Step::Go { x, run } => {
                let speed = if run { self.pet_run } else { WALK_SPEED };
                let d = x - self.pet_x;
                self.character.mirror_h = d > 0.0;
                self.character.set_pose(if !run {
                    PoseId::WalkingSideNeutral
                } else if self.pet_crazy {
                    PoseId::RunningSideCrazy
                } else {
                    PoseId::RunningSideNeutral
                });
                let mv = speed * dt;
                if d.abs() <= mv {
                    self.pet_x = x;
                    true
                } else {
                    self.pet_x += mv * sign(d);
                    false
                }
            }
            Step::Hold { pose, secs, right } => {
                self.character.mirror_h = right;
                self.character.set_pose(pose);
                self.step_t >= secs
            }
            Step::Jump { x, y } => {
                if self.step_t <= dt {
                    self.jump_from = (self.pet_x, self.pet_y);
                }
                let (x0, y0) = self.jump_from;
                self.character.mirror_h = if (x - x0).abs() > 1.0 { x > x0 } else { self.eat_right };
                self.character.set_pose(PoseId::LeaningForwardSidePounce);
                let s = (self.step_t / JUMP_TIME).min(1.0);
                let arc = 12.0 + (y0 - y).max(0.0) * 0.3;
                self.pet_x = x0 + (x - x0) * s;
                self.pet_y = y0 + (y - y0) * s - 4.0 * arc * s * (1.0 - s);
                s >= 1.0
            }
            Step::Reveal => {
                self.treat.x = self.pet_x + EAT_OFF;
                self.treat.y = FLOOR - R - EPS;
                self.treat_visible = true;
                self.treat_revealed = true;
                self.spot = Spot { find: Find::Hidden, label: self.spot.label, rescue: false, trick: 0 };
                self.flight.trick(T_RESCUE);
                self.show_tricks();
                true
            }
            Step::Pounce => {
                self.character.mirror_h = self.eat_right;
                self.character.set_pose(PoseId::LeaningForwardSidePounce);
                self.step_t >= POUNCE_TIME
            }
            Step::Eat => {
                self.character.mirror_h = self.eat_right;
                self.character.set_pose(PoseId::LeaningForwardSideEating);
                if self.step_t >= EAT_TIME / 2.0 {
                    self.treat_visible = false;
                }
                self.step_t >= EAT_TIME
            }
            Step::Happy => {
                if self.step_t <= dt {
                    let mut rng = ctx.rng;
                    self.character.play_bursts(&mut rng, 3);
                    ctx.rng = rng;
                }
                self.character.set_pose(PoseId::SittingSideHappy);
                self.step_t >= HAPPY_TIME
            }
            Step::Leave => {
                self.character.mirror_h = false;
                self.character.set_pose(PoseId::WalkingSideNeutral);
                self.pet_x -= WALK_SPEED * dt;
                self.step_t >= LEAVE_TIME || self.pet_x < self.cam_x - 30.0
            }
        };
        if done {
            self.step_i += 1;
            self.step_t = 0.0;
        }
    }

    fn update_bird(&mut self, dt: f32) {
        let Some((mode, t, x, y)) = self.bird else { return };
        let t = t + dt;
        let (tx, ty) = (self.treat.x, self.treat.y - 8.0);
        let gone = self.spot.find == Find::Lost || !self.treat_visible;
        self.bird = match mode {
            Bird::Swoop if gone => Some((Bird::Flee, 0.0, x, y)),
            Bird::Swoop => {
                let left = (BIRD_SWOOP - t).max(dt);
                let k = (dt / left).min(1.0);
                let (nx, ny) = (x + (tx - x) * k, y + (ty - y) * k);
                if t >= BIRD_SWOOP {
                    if matches!(self.steps.get(self.step_i), Some(Step::Eat | Step::Happy)) {
                        Some((Bird::Flee, 0.0, nx, ny))
                    } else {
                        self.interrupt(L_BIRD);
                        Some((Bird::Carry, 0.0, nx, ny))
                    }
                } else {
                    Some((mode, t, nx, ny))
                }
            }
            Bird::Carry => {
                let (nx, ny) = (x + 70.0 * dt, y - 45.0 * dt);
                if t > BIRD_CARRY { None } else { Some((mode, t, nx, ny)) }
            }
            Bird::Flee => {
                let (nx, ny) = (x - 50.0 * dt, y - 60.0 * dt);
                if t > BIRD_CARRY { None } else { Some((mode, t, nx, ny)) }
            }
        };
    }

    fn finish_throw(&mut self) {
        let found = self.spot.find != Find::Lost;
        let p = paws(self.treat.x);
        let s = if found { score(p, self.spot.find, self.flight.tricks) } else { 0 };
        self.scores[self.throw_no] = Some(s);
        self.session_throws += 1;
        if found {
            self.session_found += 1;
            self.session_score += s;
            self.round_score += s;
            if s >= self.best_throw {
                self.best_throw = s;
                self.best_x = Some(self.treat.x);
            }
        }

        let mut text: String<96> = String::new();
        let _ = text.push_str(self.spot.label);
        let _ = text.push('\n');
        if found {
            let mut n: String<8> = String::new();
            let _ = write!(n, "{}", p);
            crate::i18n::substitute(&mut text, t!("Distance: {n}"), &[("n", n.as_str())]);
            let _ = text.push('\n');
            n.clear();
            let _ = write!(n, "{}", s);
            crate::i18n::substitute(&mut text, t!("Score: {n}"), &[("n", n.as_str())]);
        } else {
            let _ = text.push_str(t!("Lost it!"));
        }
        self.popup = Popup::new(2, 14, 124, 36);
        self.popup.set_text(&text, true, true);
        self.popup.fit_height(24, 32);
        self.state = State::Result;
    }

    fn next_throw(&mut self) {
        self.throw_no += 1;
        if self.throw_no >= TREATS {
            self.state = State::RoundOver;
        } else {
            self.state = State::Aim;
            self.ready_throw();
        }
    }

    // -----------------------------------------------------------------------
    // Drawing
    // -----------------------------------------------------------------------

    fn sx(&self, x: f32) -> i32 {
        (x - self.cam_x).round() as i32
    }

    fn visible(&self, x0: f32, x1: f32) -> bool {
        x1 >= self.cam_x - 4.0 && x0 <= self.cam_x + SCREEN_W as f32 + 4.0
    }

    fn draw_background(&self, r: &mut Renderer) {
        let floor = FLOOR as i32;
        r.draw_line(Point::new(0, floor), Point::new(SCREEN_W - 1, floor));
        let cam = self.cam_x.round() as i32;
        for sx in 0..SCREEN_W {
            let wx = sx + cam;
            let x = wx as f32;
            let carpet = x < LIVING_END || (x >= KITCHEN_END && x < BEDROOM_END);
            if x >= LIVING_END && x < KITCHEN_END {
                if wx % 10 == 0 {
                    r.draw_line(Point::new(sx, floor + 1), Point::new(sx, floor + 3));
                }
            } else if carpet {
                if wx % 4 == 0 {
                    r.draw_pixel(Point::new(sx, floor + 2), true);
                } else if wx % 4 == 2 {
                    r.draw_pixel(Point::new(sx, floor + 3), true);
                }
            } else if x < FENCE_X {
                if wx % 7 == 0 {
                    r.draw_pixel(Point::new(sx, floor - 1), true);
                    r.draw_pixel(Point::new(sx + 1, floor - 2), true);
                } else if wx % 7 == 3 {
                    r.draw_pixel(Point::new(sx, floor - 1), true);
                }
                if wx % 5 == 1 {
                    r.draw_pixel(Point::new(sx, floor + 2 + (wx / 5) % 2), true);
                }
            }
        }

        // Sky and clouds out in the garden.
        for &(wx, y, big) in &[(BEDROOM_END + 40.0, 2, false), (BEDROOM_END + 160.0, 8, true), (BEDROOM_END + 290.0, 0, false), (BEDROOM_END + 400.0, 6, true)] {
            let sprite = if big { &CLOUD3 } else { &CLOUD2 };
            if self.visible(wx, wx + sprite.width as f32) {
                r.draw_sprite(sprite, Point::new(self.sx(wx), y), SpriteOpts::default());
            }
        }

        // Garden fence, with the neighbour's pickets beyond.
        if self.visible(FENCE_X, WORLD_W) {
            let s = self.sx(FENCE_X);
            let top = FENCE_TOP as i32;
            r.draw_rect(Point::new(s, top), Size::new(4, (floor - top) as u32), false);
            let end = self.sx(WORLD_W).min(SCREEN_W);
            r.draw_line(Point::new(s + 4, top + 6), Point::new(end, top + 6));
            r.draw_line(Point::new(s + 4, floor - 8), Point::new(end, floor - 8));
            let mut px = s + 9;
            while px < end {
                r.draw_line(Point::new(px, top + 2), Point::new(px, floor));
                r.draw_pixel(Point::new(px, top + 1), true);
                px += 6;
            }
        }
    }

    fn draw_prop_back(&self, r: &mut Renderer, p: &Prop) {
        let s = self.sx(p.x);
        let f = FLOOR as i32;
        let w = p.kind.width() as i32;
        let line = |r: &mut Renderer, x0: i32, y0: i32, x1: i32, y1: i32| r.draw_line(Point::new(x0, y0), Point::new(x1, y1));
        match p.kind {
            Kind::Fridge => {
                draw_fridge(r, s, f - FRIDGE_H, &self.fridge);
            }
            Kind::WaterBowl => {
                // The empty bowl, the last frame of the eating animation.
                let empty = FOOD_BOWL.frames.len() - 1;
                r.draw_sprite(&FOOD_BOWL, Point::new(s, f - 8), SpriteOpts { frame: empty, ..Default::default() });
                for i in (4..w - 4).step_by(3) {
                    r.draw_pixel(Point::new(s + i, f - 6), true);
                }
            }
            Kind::FoodBowl => {
                r.draw_sprite(&FOOD_BOWL_KIBBLE, Point::new(s, f - 8), SpriteOpts::default());
            }
            Kind::Mat | Kind::Rug => {
                for i in (0..w).step_by(2) {
                    r.draw_pixel(Point::new(s + i, f - 1), true);
                }
                if p.kind == Kind::Rug {
                    for i in (1..w).step_by(4) {
                        r.draw_pixel(Point::new(s + i, f - 2), true);
                    }
                }
                for y in [f - 1, f + 1] {
                    r.draw_pixel(Point::new(s - 2, y), true);
                    r.draw_pixel(Point::new(s + w + 1, y), true);
                }
            }
            Kind::Couch => {
                draw_couch(r, s, f - COUCH_H);
            }
            Kind::Table => {
                r.draw_rect(Point::new(s, f - 20), Size::new(w as u32, 3), true);
                line(r, s + 2, f - 17, s + 2, f - 1);
                line(r, s + w - 3, f - 17, s + w - 3, f - 1);
                line(r, s + 3, f - 15, s + w - 4, f - 15);
            }
            Kind::Bookshelf => {
                draw_bookshelf(r, s, f - CABINET_H);
            }
            Kind::Dresser => {
                draw_dresser(r, s, f - CABINET_H);
            }
            Kind::Yarn => {
                r.draw_sprite(&YARN_BALL, Point::new(s, f - 13), SpriteOpts::default());
            }
            Kind::Bed => {
                draw_bed(r, s, f - BED_H, true);
            }
            Kind::Pillow => {
                r.draw_sprite(&PILLOW, Point::new(s, f - 8), SpriteOpts::default());
            }
            Kind::CatBed => {
                draw_cat_bed_rim_at(r, s, f - 12);
            }
            Kind::Pot => {
                r.draw_sprite(&POT_MEDIUM, Point::new(s, f - 12), SpriteOpts::default());
            }
            Kind::Puddle => {
                for i in (3..w - 3).step_by(2) {
                    r.draw_pixel(Point::new(s + i, f - 1), true);
                }
                line(r, s, f + 1, s + w - 1, f + 1);
                line(r, s + 3, f + 2, s + w - 4, f + 2);
            }
            Kind::Bench => {
                r.draw_sprite(&PARK_BENCH, Point::new(s, f - 11), SpriteOpts::default());
            }
            Kind::Window => {
                // The living room window, frame and sill, with nothing
                // drawn outside.
                r.draw_rect(Point::new(s + 6, -10), Size::new(56, 36), false);
                r.draw_rect(Point::new(s + 2, -14), Size::new(64, 44), false);
                r.draw_rect(Point::new(s, SILL_Y as i32), Size::new(w as u32, 3), true);
            }
            Kind::Counter => {
                // Like the kitchen's: worktop, cupboard fronts, faucet.
                let top = f - COUNTER_H as i32;
                r.draw_rect(Point::new(s, top), Size::new(w as u32, 4), true);
                r.draw_rect(Point::new(s + 3, top + 4), Size::new(w as u32 - 6, 30), false);
                for dx in [18, 48] {
                    line(r, s + dx, top + 4, s + dx, top + 33);
                }
                r.draw_sprite(&FAUCET, Point::new(s + FAUCET_X as i32, top - FAUCET.height as i32), SpriteOpts::default());
            }
            Kind::Rocks => {
                r.draw_sprite(&ROCK_PILE, Point::new(s, f - 23), SpriteOpts::default());
            }
            // Drawn in front of the treat, or by themselves.
            Kind::PaperBag | Kind::Bin | Kind::Tunnel | Kind::Basket | Kind::Vacuum
            | Kind::Laundry | Kind::Blanket | Kind::Bush => {}
        }
    }

    /// Parts drawn in front of the treat, so it can hide behind or inside.
    fn draw_prop_front(&self, r: &mut Renderer, p: &Prop) {
        let s = self.sx(p.x);
        let f = FLOOR as i32;
        let w = p.kind.width() as i32;
        let line = |r: &mut Renderer, x0: i32, y0: i32, x1: i32, y1: i32| r.draw_line(Point::new(x0, y0), Point::new(x1, y1));
        match p.kind {
            Kind::Fridge => {
                // Hide anything that slid underneath, then put the feet and
                // floor back.
                r.fill_rect_off(Point::new(s, f - FRIDGE_FEET_H), Size::new(w as u32, FRIDGE_FEET_H as u32 + 1));
                line(r, s, f, s + w - 1, f);
                draw_fridge_feet(r, s, f - FRIDGE_H);
            }
            Kind::PaperBag => {
                // Hide a treat inside, then draw the bag over it.
                let top = f - PAPER_BAG.height as i32;
                r.fill_rect_off(Point::new(s + 6, top + 3), Size::new(30, PAPER_BAG.height as u32 - 3));
                r.draw_sprite(&PAPER_BAG, Point::new(s, top), SpriteOpts::default());
            }
            Kind::Bin => {
                r.draw_sprite(&TRASHCAN_BODY, Point::new(s, f - TRASHCAN_BODY.height as i32), SpriteOpts::default());
            }
            Kind::Couch => {
                // Hide a treat that's deep under, between the feet.
                let gap = COUCH_H - COUCH_BASE_Y - 1;
                r.fill_rect_off(Point::new(s + 8, f - gap), Size::new(w as u32 - 16, gap as u32));
            }
            Kind::Tunnel => {
                r.fill_rect_off(Point::new(s, f - 14), Size::new(w as u32, 14));
                r.draw_rect(Point::new(s, f - 14), Size::new(w as u32, 14), false);
                for i in (6..w - 3).step_by(6) {
                    for y in (f - 13..f - 1).step_by(2) {
                        r.draw_pixel(Point::new(s + i, y), true);
                    }
                }
                line(r, s + 1, f - 12, s + 1, f - 3);
                line(r, s + w - 2, f - 12, s + w - 2, f - 3);
            }
            Kind::Basket => {
                r.draw_sprite(&BASKET, Point::new(s, f - BASKET.height as i32), SpriteOpts::default());
            }
            Kind::Bed => {
                // Hide a treat that's deep under, then put the posts back in
                // front of one near either end.
                let gap = BED_H - BED_RAIL_BOTTOM_Y - 1;
                r.fill_rect_off(Point::new(s + 12, f - gap), Size::new(w as u32 - 24, gap as u32));
                draw_bed_legs(r, s, f - BED_H, true);
            }
            Kind::Laundry | Kind::Blanket => {
                let prof: &[u8] = if p.kind == Kind::Laundry { &LAUNDRY_PROFILE } else { &BLANKET_PROFILE };
                for (i, &hgt) in prof.iter().enumerate() {
                    let x = s + i as i32;
                    let top = f - hgt as i32;
                    r.fill_rect_off(Point::new(x, top), Size::new(1, hgt as u32));
                    r.draw_pixel(Point::new(x, top), true);
                }
                if p.kind == Kind::Laundry {
                    line(r, s + 5, f - 5, s + 9, f - 8);
                    line(r, s + 14, f - 9, s + 18, f - 4);
                } else {
                    line(r, s + 8, f - 4, s + 12, f - 6);
                    line(r, s + 18, f - 6, s + 22, f - 3);
                }
            }
            Kind::Bush => {
                r.draw_sprite(&BUSH, Point::new(s, f - 12), SpriteOpts::default());
            }
            _ => {}
        }
    }

    fn draw_vacuum(&self, r: &mut Renderer, blink: bool) {
        let Some(v) = &self.house.vacuum else { return };
        if !self.visible(v.x, v.x + VAC_W) {
            return;
        }
        let s = self.sx(v.x);
        let f = FLOOR as i32;
        r.fill_rect_off(Point::new(s, f - 6), Size::new(VAC_W as u32, 6));
        r.draw_line(Point::new(s + 2, f - 6), Point::new(s + 13, f - 6));
        r.draw_line(Point::new(s, f - 4), Point::new(s, f - 1));
        r.draw_line(Point::new(s + 15, f - 4), Point::new(s + 15, f - 1));
        r.draw_pixel(Point::new(s + 1, f - 5), true);
        r.draw_pixel(Point::new(s + 14, f - 5), true);
        r.draw_line(Point::new(s + 1, f - 1), Point::new(s + 14, f - 1));
        if blink {
            r.draw_pixel(Point::new(s + 7, f - 4), true);
            r.draw_pixel(Point::new(s + 8, f - 4), true);
        }
    }

    fn draw_treat_at(&self, r: &mut Renderer, x: f32, y: f32) {
        let p = Point::new((x - self.cam_x - 2.0).round() as i32, (y - 2.0).round() as i32);
        r.draw_sprite(&TREAT, p, SpriteOpts::default());
    }

    fn draw_hand(&self, r: &mut Renderer) {
        if !self.visible(0.0, 30.0) {
            return;
        }
        r.draw_sprite(&HAND, Point::new(self.sx(LAUNCH_X - 17.0), LAUNCH_Y as i32 - 4), SpriteOpts { mirror_h: true, ..Default::default() });
    }

    fn draw_aim(&self, r: &mut Renderer) {
        let a = self.angle * core::f32::consts::PI / 180.0;
        let (mut x, mut y) = (LAUNCH_X, LAUNCH_Y);
        let (vx, mut vy) = (a.cos() * PREVIEW_SPEED, -a.sin() * PREVIEW_SPEED);
        let mut travelled = 0.0;
        let mut next_dot = 4.0;
        let step = 0.01;
        while travelled < PREVIEW_LEN {
            let (ox, oy) = (x, y);
            vy += G * step;
            x += vx * step;
            y += vy * step;
            travelled += ((x - ox) * (x - ox) + (y - oy) * (y - oy)).sqrt();
            if travelled >= next_dot {
                r.draw_pixel(Point::new(self.sx(x), y.round() as i32), true);
                next_dot += 3.0;
            }
        }
    }

    fn draw_power(&self, r: &mut Renderer) {
        const W: i32 = 6;
        const H: i32 = 48;
        let (x, y) = (SCREEN_W - W - 2, 8);
        r.fill_rect_off(Point::new(x - 1, y - 1), Size::new(W as u32 + 2, H as u32 + 2));
        r.draw_rect(Point::new(x, y), Size::new(W as u32, H as u32), false);
        let fill = (gauge(self.phase) * (H - 2) as f32 + 0.5) as i32;
        if fill > 0 {
            r.draw_rect(Point::new(x + 1, y + H - 1 - fill), Size::new(W as u32 - 2, fill as u32), true);
        }
    }

    fn draw_label(&self, r: &mut Renderer, text: &str, right: bool) {
        let w = text.chars().count() as i32 * 4 + 1;
        let x = if right { SCREEN_W - w } else { (SCREEN_W - w) / 2 };
        let y = if right { 0 } else { 14 };
        r.draw_rect(Point::new(x, y), Size::new(w as u32, 7), true);
        r.draw_text_small_inverted(text, Point::new(x + 1, y + 1));
    }

    fn draw_hud(&self, r: &mut Renderer) {
        match self.state {
            State::Aim | State::Power => {
                let left = TREATS - self.throw_no;
                for i in 0..left {
                    r.draw_sprite(&TREAT, Point::new(1 + i as i32 * 5, 1), SpriteOpts::default());
                }
                if self.round_score > 0 && self.state == State::Aim {
                    let mut text: String<24> = String::new();
                    let mut n: String<8> = String::new();
                    let _ = write!(n, "{}", self.round_score);
                    crate::i18n::substitute(&mut text, t!("Score: {n}"), &[("n", n.as_str())]);
                    self.draw_label(r, &text, true);
                }
            }
            State::Flight | State::Hunt => {
                let mut text: String<8> = String::new();
                let _ = write!(text, "{}", paws(self.treat.x));
                self.draw_label(r, &text, true);
            }
            _ => {}
        }
        if let Some(text) = self.callout {
            if self.callout_t > 0.0 {
                self.draw_label(r, text, false);
            }
        }
        // Caret on the top edge while the treat is above the screen.
        if self.treat_visible && self.treat.y < -1.0 && matches!(self.state, State::Flight) {
            let x = self.sx(self.treat.x);
            r.draw_pixel(Point::new(x, 0), true);
            r.draw_line(Point::new(x - 1, 1), Point::new(x + 1, 1));
            r.draw_line(Point::new(x - 2, 2), Point::new(x + 2, 2));
        }
    }

    fn draw_round_over(&self, r: &mut Renderer) {
        r.draw_text(t!("Round over"), Point::new(2, 1));
        let mut text: String<24> = String::new();
        let mut n: String<8> = String::new();
        let _ = write!(n, "{}", self.round_score);
        crate::i18n::substitute(&mut text, t!("Score: {n}"), &[("n", n.as_str())]);
        r.draw_text(&text, Point::new(2, 14));
        for (i, s) in self.scores.iter().enumerate() {
            let y = 25 + i as i32 * 7;
            r.draw_sprite(&TREAT, Point::new(4, y + 1), SpriteOpts::default());
            let mut line: String<16> = String::new();
            match s {
                Some(0) | None => {
                    let _ = line.push_str("-");
                }
                Some(v) => {
                    let _ = write!(line, "{}", v);
                }
            }
            r.draw_text_small(&line, Point::new(12, y));
        }
        r.draw_text_small(t!("A: Next"), Point::new(SCREEN_W - 4 * t!("A: Next").chars().count() as i32 - 2, 56));
    }
}

/// The pet gives up on a treat. Sometimes it takes it hard and lies down to
/// sulk; sometimes it has a last sniff around and wanders off.
fn push_grump(s: &mut Vec<Step, MAX_STEPS>, shrug: bool) {
    if shrug {
        let _ = s.push(Step::Hold { pose: PoseId::LeaningForwardSideSniffing, secs: 0.9, right: true });
        let _ = s.push(Step::Hold { pose: PoseId::StandingSideSniffing, secs: 0.7, right: false });
        let _ = s.push(Step::Hold { pose: PoseId::SittingSideNeutral, secs: 0.6, right: false });
    } else {
        let _ = s.push(Step::Hold { pose: PoseId::SittingSideAnnoyed, secs: 1.3, right: true });
        let _ = s.push(Step::Hold { pose: PoseId::LayingSideSulking, secs: 1.6, right: true });
    }
    let _ = s.push(Step::Leave);
}

impl Scene for TreatTossScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_throws = 0;
        self.session_found = 0;
        self.session_score = 0;
        self.menu_active = false;
        self.state = State::Intro;
        self.cam_x = 0.0;
        self.popup = Popup::new(2, 8, 124, 48);
        self.popup.set_text(t!("A: set angle\nA again: throw"), true, true);
        self.popup.fit_height(24, 30);
        self.character.reseed_anim();
        self.fridge = FridgeLayout::from_seed(ctx.pet_seed);
        self.start_round(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.session_throws == 0 {
            return;
        }
        let play = (self.session_throws as f32 / TREATS as f32).sqrt().min(2.0);
        let hunt = (self.session_score as f32 / 100.0).sqrt().min(2.0);
        ctx.apply_stat_changes(&[
            (StatId::Fitness,     4.0 * play),
            (StatId::Playfulness, 4.0 * play),
            (StatId::Curiosity,   3.0 * hunt),
            (StatId::Focus,       1.5 * hunt),
            (StatId::Fulfillment, 1.0 * hunt),
            (StatId::Fullness,    (0.4 * self.session_found as f32).min(3.0)),
        ]);
        let coins = (3.0 * hunt) as i32 + (self.session_found / 3) as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        if self.menu_active {
            self.handle_menu(ctx, buttons);
            return None;
        }
        if buttons.was_just_pressed(Button::Menu2) {
            self.menu_active = true;
            self.options_menu.reset_to(OPTIONS_ITEMS);
            return None;
        }

        self.character.animate(dt);
        if self.callout_t > 0.0 {
            self.callout_t -= dt;
        }
        if let Some((x, y, t)) = self.splash {
            self.splash = if t + dt < SPLASH_TIME { Some((x, y, t + dt)) } else { None };
        }

        match self.state {
            State::Intro => {
                if buttons.was_just_pressed(Button::A) {
                    self.state = State::Aim;
                }
            }
            State::Aim => {
                self.update_aim(dt);
                self.ease_cam(0.0, 10.0, dt);
                if let Some(v) = self.house.vacuum.as_mut() {
                    v.update(dt);
                }
                if buttons.was_just_pressed(Button::A) {
                    self.phase = 0.0;
                    self.state = State::Power;
                }
            }
            State::Power => {
                self.phase = (self.phase + dt * 2.0 * core::f32::consts::PI / POWER_PERIOD) % (2.0 * core::f32::consts::PI);
                self.ease_cam(0.0, 10.0, dt);
                if let Some(v) = self.house.vacuum.as_mut() {
                    v.update(dt);
                }
                if buttons.was_just_pressed(Button::A) {
                    self.throw(ctx);
                } else if buttons.was_just_pressed(Button::B) {
                    self.state = State::Aim;
                }
            }
            State::Flight => self.update_flight(ctx, dt),
            State::Hunt => self.update_hunt(ctx, dt),
            State::Result => {
                if buttons.was_just_pressed(Button::A) {
                    self.next_throw();
                }
            }
            State::RoundOver => {
                if buttons.was_just_pressed(Button::A) {
                    self.state = State::Aim;
                    self.start_round(ctx);
                }
            }
        }
        self.character.pos = Point::new(self.pet_x.round() as i32, self.pet_y.round() as i32);
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, dt_ms: u64) {
        let _ = dt_ms;
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }
        if self.state == State::RoundOver {
            self.draw_round_over(renderer);
            return;
        }
        let r = renderer;
        self.draw_background(r);
        for p in self.house.props.iter() {
            if self.visible(p.x - 2.0, p.x + p.kind.width() + 2.0) {
                self.draw_prop_back(r, p);
            }
        }
        if let Some(bx) = self.best_x {
            if self.visible(bx - 2.0, bx + 6.0) {
                let s = self.sx(bx);
                let f = FLOOR as i32;
                r.draw_line(Point::new(s, f - 12), Point::new(s, f - 1));
                r.draw_rect(Point::new(s + 1, f - 12), Size::new(4, 3), true);
            }
        }
        let in_hand = matches!(self.state, State::Intro | State::Aim | State::Power);
        if self.treat_visible && !self.treat_revealed && !in_hand {
            self.draw_treat_at(r, self.treat.x, self.treat.y);
        }
        for p in self.house.props.iter() {
            if self.visible(p.x - 2.0, p.x + p.kind.width() + 2.0) {
                self.draw_prop_front(r, p);
            }
        }
        let blink = (self.flight.time * 4.0) as i32 % 2 == 0;
        self.draw_vacuum(r, blink);
        if self.treat_visible && self.treat_revealed {
            self.draw_treat_at(r, self.treat.x, self.treat.y);
        }

        self.character.draw(r, self.cam_x.round() as i32);
        let pet_screen = Point::new(self.pet_x.round() as i32 - self.cam_x.round() as i32, self.pet_y.round() as i32);
        self.character.draw_bursts(r, pet_screen);

        self.draw_hand(r);
        if in_hand {
            self.draw_treat_at(r, LAUNCH_X, LAUNCH_Y);
        }
        if matches!(self.state, State::Aim | State::Power) {
            self.draw_aim(r);
        }

        if let Some((mode, t, x, y)) = self.bird {
            let frame = (t * 8.0) as usize % 2 + if mode == Bird::Swoop { 2 } else { 0 };
            let s = self.sx(x);
            r.draw_sprite(&SMALL_BIRD1, Point::new(s - 8, y.round() as i32 - 7), SpriteOpts { frame, mirror_h: mode != Bird::Flee, ..Default::default() });
            if mode == Bird::Carry {
                self.draw_treat_at(r, x, y + 8.0);
            }
        }

        if let Some((x, y, t)) = self.splash {
            let s = self.sx(x);
            let rise = (t / SPLASH_TIME * 6.0) as i32;
            for (dx, k) in [(-4, 1), (-2, 2), (2, 2), (4, 1)] {
                r.draw_pixel(Point::new(s + dx * (1 + rise / 3), y.round() as i32 - rise * k / 2 + rise * rise / 12), true);
            }
        }

        if self.state == State::Power {
            self.draw_power(r);
        }
        self.draw_hud(r);

        match self.state {
            State::Intro | State::Result => self.popup.draw(r, false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn throw_until_rest(h: &House, angle: f32, power: f32, rng: &mut u32) -> (Treat, Flight, Fly) {
        let mut t = launch(angle, power);
        let mut f = Flight::default();
        let mut res = Fly::Moving;
        for _ in 0..(40 * 30) {
            res = fly(h, &mut t, &mut f, rng, 1.0 / 30.0);
            if res != Fly::Moving {
                break;
            }
        }
        (t, f, res)
    }

    #[test]
    fn houses_are_valid() {
        for seed in 1..400u32 {
            let mut rng = seed.wrapping_mul(2654435761);
            let h = House::generate(&mut rng);
            // Every room has its anchor, plus at least two extras (one in
            // the smaller kitchen).
            for (room, &(kind, _, _)) in ROOMS.iter().zip(ANCHORS.iter()) {
                assert!(h.props.iter().any(|p| p.kind == kind && p.x >= room.0 && p.x < room.1), "seed {} missing {:?}", seed, kind);
                let n = h.props.iter().filter(|p| p.x >= room.0 && p.x < room.1).count();
                let want = if room.0 == LIVING_END { 2 } else { 3 };
                assert!(n >= want, "seed {} room {:?} has {}", seed, room, n);
            }
            for &(kind, lo, hi) in FIXTURES.iter() {
                assert!(h.props.iter().any(|p| p.kind == kind && p.x >= lo && p.x + p.kind.width() <= hi), "seed {} missing {:?}", seed, kind);
            }
            // Nothing overlaps, crosses a doorway or crowds the thrower.
            for (i, a) in h.props.iter().enumerate() {
                let (a0, a1) = (a.x, a.x + a.kind.width());
                assert!(a0 >= LAUNCH_ZONE, "seed {} {:?} at {}", seed, a.kind, a0);
                for d in [KITCHEN_END, LIVING_END, BEDROOM_END, FENCE_X] {
                    assert!(a1 < d - 4.0 || a0 > d + 4.0, "seed {} {:?} crosses {}", seed, a.kind, d);
                }
                for b in h.props.iter().skip(i + 1) {
                    let (b0, b1) = (b.x, b.x + b.kind.width());
                    assert!(a1 <= b0 || b1 <= a0, "seed {} {:?} overlaps {:?}", seed, a.kind, b.kind);
                }
            }
            assert!(h.solids.len() < MAX_SOLIDS && h.zones.len() < MAX_ZONES);
        }
    }

    #[test]
    fn resting_spots_are_classified() {
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Couch, 300.0, &mut spans);
        h.add(Kind::Rug, 400.0, &mut spans);
        h.add(Kind::Fridge, 100.0, &mut spans);
        h.add(Kind::Basket, 460.0, &mut spans);
        h.add_structure();
        let at = |x: f32, y: f32| Treat { x, y, vx: 0.0, vy: 0.0 };
        let floor = FLOOR - R - EPS;
        assert_eq!(h.spot(&at(200.0, floor)).find, Find::Open);
        assert_eq!(h.spot(&at(420.0, floor)).find, Find::Tucked);
        assert_eq!(h.spot(&at(303.0, floor)).find, Find::Tucked);
        let deep = h.spot(&at(329.0, floor));
        assert_eq!(deep.find, Find::Lost);
        assert!(deep.rescue);
        assert_eq!(h.spot(&at(329.0, FLOOR - 18.0 - R - EPS)).find, Find::Hidden);
        assert_eq!(h.spot(&at(470.0, floor)).find, Find::Hidden);
        assert_eq!(h.spot(&at(110.0, floor)).find, Find::Lost);
        assert_eq!(score(20, Find::Lost, T_COUCH), 0);
        assert_eq!(score(20, Find::Hidden, T_COUCH), 44);
    }

    #[test]
    fn sliding_under_the_fridge_is_lost() {
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Fridge, LIVING_END + 100.0, &mut spans);
        h.add_structure();
        let mut rng = 7u32;
        let mut t = Treat { x: LIVING_END + 40.0, y: FLOOR - R - EPS, vx: 120.0, vy: 0.0 };
        let mut f = Flight::default();
        let mut res = Fly::Moving;
        for _ in 0..300 {
            res = fly(&h, &mut t, &mut f, &mut rng, 1.0 / 30.0);
            if res != Fly::Moving {
                break;
            }
        }
        assert_eq!(res, Fly::Gone(L_UNDER_FRIDGE));
    }

    #[test]
    fn the_paper_bag_opens_on_the_left() {
        let bag_x = LIVING_END + 100.0;
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::PaperBag, bag_x, &mut spans);
        h.add_structure();
        let run = |t: Treat| {
            let mut t = t;
            let (mut f, mut rng) = (Flight::default(), 7u32);
            for _ in 0..300 {
                if fly(&h, &mut t, &mut f, &mut rng, 1.0 / 30.0) != Fly::Moving {
                    break;
                }
            }
            t
        };
        // Slid in through the mouth: stopped inside, short of the far end.
        let t = run(Treat { x: bag_x - 30.0, y: FLOOR - R - EPS, vx: 90.0, vy: 0.0 });
        assert_eq!(h.spot(&t).label, L_BAG, "{:?}", t);
        assert!(t.x < bag_x + 36.0, "{:?}", t);
        // Dropped from above: sits on the flat top.
        let t = run(Treat { x: bag_x + 20.0, y: 0.0, vx: 0.0, vy: 0.0 });
        assert!((t.y - (FLOOR - PAPER_BAG.height as f32 - R)).abs() < 1.0, "{:?}", t);
    }

    #[test]
    fn the_sill_and_counter_catch_treats() {
        let (sill_x, counter_x) = (LAUNCH_ZONE + 20.0, LIVING_END + 60.0);
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Window, sill_x, &mut spans);
        h.add(Kind::Counter, counter_x, &mut spans);
        h.add_structure();
        let drop = |x: f32| {
            let mut t = Treat { x, y: -20.0, vx: 0.0, vy: 0.0 };
            let (mut f, mut rng) = (Flight::default(), 7u32);
            for _ in 0..300 {
                if fly(&h, &mut t, &mut f, &mut rng, 1.0 / 30.0) != Fly::Moving {
                    break;
                }
            }
            t
        };
        let t = drop(sill_x + 30.0);
        assert_eq!(h.spot(&t).label, L_SILL, "{:?}", t);
        assert!((t.y - (SILL_Y - R)).abs() < 1.0, "{:?}", t);
        // The faucet doesn't get in the way: one dropped on it lands on the
        // worktop.
        let t = drop(counter_x + FAUCET_X + 9.0);
        assert_eq!(h.spot(&t).label, L_COUNTER, "{:?}", t);
        assert!((t.y - (FLOOR - COUNTER_H - R)).abs() < 1.0, "{:?}", t);
    }

    #[test]
    fn treats_always_come_to_rest_in_the_world() {
        for seed in 1..40u32 {
            let mut rng = seed.wrapping_mul(2654435761);
            let h = House::generate(&mut rng);
            for a in (5..=80).step_by(15) {
                for p in [0.2, 0.5, 0.8, 1.0] {
                    let (t, f, res) = throw_until_rest(&h, a as f32, p, &mut rng);
                    assert!(res != Fly::Moving, "seed {} a {} p {} still moving at {:?}", seed, a, p, t);
                    assert!(f.time < MAX_FLIGHT, "seed {} a {} p {} timed out at {:?}", seed, a, p, t);
                    assert!(t.x >= 0.0 && t.x < WORLD_W + 100.0 && t.y <= FLOOR, "seed {} at {:?}", seed, t);
                    if res == Fly::Stopped {
                        assert!(h.hit(t.x, t.y).is_none(), "seed {} a {} p {} inside a solid at {:?}", seed, a, p, t);
                    }
                }
            }
        }
    }

    fn hunt_scene(house: House, treat: Treat) -> (TreatTossScene, GameContext) {
        let mut ctx = GameContext::new(crate::led::Led::new());
        ctx.rng = 99;
        let mut sc = TreatTossScene::new();
        sc.house = house;
        sc.state = State::Flight;
        sc.treat = treat;
        sc.throw(&ctx);
        sc.treat = treat;
        (sc, ctx)
    }

    fn run_hunt(sc: &mut TreatTossScene, ctx: &mut GameContext) {
        for _ in 0..3000 {
            if sc.state != State::Hunt {
                break;
            }
            sc.update_hunt(ctx, 1.0 / 30.0);
        }
        assert_eq!(sc.state, State::Result);
    }

    #[test]
    fn the_camera_turns_smoothly_off_a_wall() {
        // A fast treat banks off the fridge and heads back the way it came.
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Fridge, LIVING_END + 120.0, &mut spans);
        h.add_structure();
        let treat = Treat { x: LIVING_END, y: FLOOR - 30.0, vx: 220.0, vy: -40.0 };
        let (mut sc, mut ctx) = hunt_scene(h, treat);
        sc.cam_x = treat.x - 56.0 + 30.0;
        sc.cam_lead = 30.0;
        let mut cams = std::vec::Vec::new();
        for _ in 0..45 {
            if sc.state != State::Flight {
                break;
            }
            sc.update_flight(&mut ctx, 1.0 / 30.0);
            cams.push(sc.cam_x);
        }
        assert!(sc.flight.tricks & T_WALL != 0, "never hit the fridge");
        // Change in camera speed from one frame to the next.
        let jerk = cams.windows(3).map(|w| ((w[2] - w[1]) - (w[1] - w[0])).abs()).fold(0.0, f32::max);
        // It used to jump by 16 when the look-ahead flipped sides at once.
        assert!(jerk < 5.0, "camera lurches {} px/frame", jerk);
    }

    #[test]
    fn the_pet_finds_a_treat_in_the_open() {
        let mut h = House::empty();
        h.add_structure();
        let (mut sc, mut ctx) = hunt_scene(h, Treat { x: 200.0, y: FLOOR - R - EPS, vx: 0.0, vy: 0.0 });
        sc.settle(&mut ctx, None);
        run_hunt(&mut sc, &mut ctx);
        assert_eq!(sc.scores[0], Some(paws(200.0)));
        assert_eq!(sc.round_score, paws(200.0));
        assert!(!sc.treat_visible, "eaten");
        assert_eq!(sc.session_found, 1);
    }

    #[test]
    fn the_vacuum_beats_a_slow_pet() {
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Vacuum, 380.0, &mut spans);
        h.add_structure();
        // Just ahead of the vacuum, which gets there while the pet is still
        // noticing where the treat went.
        let (mut sc, mut ctx) = hunt_scene(h, Treat { x: 400.0, y: FLOOR - R - EPS, vx: 0.0, vy: 0.0 });
        sc.settle(&mut ctx, None);
        run_hunt(&mut sc, &mut ctx);
        assert_eq!(sc.spot.label, L_VACUUM);
        assert_eq!(sc.scores[0], Some(0));
    }

    #[test]
    fn a_bird_can_steal_a_garden_treat() {
        let mut h = House::empty();
        h.add_structure();
        let (mut sc, mut ctx) = hunt_scene(h, Treat { x: BEDROOM_END + 120.0, y: FLOOR - R - EPS, vx: 0.0, vy: 0.0 });
        sc.settle(&mut ctx, None);
        sc.bird = Some((Bird::Swoop, 0.0, BEDROOM_END + 210.0, -20.0));
        sc.notice = 5.0;
        run_hunt(&mut sc, &mut ctx);
        assert_eq!(sc.spot.label, L_BIRD);
        assert_eq!(sc.scores[0], Some(0));
    }

    #[test]
    fn a_curious_pet_can_paw_a_treat_out_from_under_the_couch() {
        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Couch, 300.0, &mut spans);
        h.add_structure();
        let (mut sc, mut ctx) = hunt_scene(h, Treat { x: 329.0, y: FLOOR - R - EPS, vx: 0.0, vy: 0.0 });
        sc.rescue_chance = 1.0;
        sc.settle(&mut ctx, None);
        run_hunt(&mut sc, &mut ctx);
        assert!(sc.flight.tricks & T_RESCUE != 0);
        assert!(sc.scores[0].unwrap() > 0);

        let mut h = House::empty();
        let mut spans = Vec::new();
        h.add(Kind::Couch, 300.0, &mut spans);
        h.add_structure();
        let (mut sc, mut ctx) = hunt_scene(h, Treat { x: 329.0, y: FLOOR - R - EPS, vx: 0.0, vy: 0.0 });
        sc.rescue_chance = 0.0;
        sc.settle(&mut ctx, None);
        run_hunt(&mut sc, &mut ctx);
        assert_eq!(sc.scores[0], Some(0));
    }
}
