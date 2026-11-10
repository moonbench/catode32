//! Pet seed to personality / favorites derivation.

use crate::t;

use crate::{
    context::{FoodItem, ToyVariant},
    rand::xorshift32,
    scene::SceneId,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PetGender {
    Tom,
    Queen,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum StarSign {
    Aries = 0,
    Taurus,
    Gemini,
    Cancer,
    Leo,
    Virgo,
    Libra,
    Scorpio,
    Sagittarius,
    Capricorn,
    Aquarius,
    Pisces,
}

impl StarSign {
    pub fn from_index(idx: u32) -> Self {
        Self::ALL[(idx as usize) % Self::ALL.len()]
    }

    pub const ALL: [StarSign; 12] = [
        StarSign::Aries,
        StarSign::Taurus,
        StarSign::Gemini,
        StarSign::Cancer,
        StarSign::Leo,
        StarSign::Virgo,
        StarSign::Libra,
        StarSign::Scorpio,
        StarSign::Sagittarius,
        StarSign::Capricorn,
        StarSign::Aquarius,
        StarSign::Pisces,
    ];
}

crate::enum_str_method! {
    /// Stable English identifier used as the save-file serialization key.
    /// Do not translate — saves would break across language changes.
    StarSign::name;
    Aries       => "Aries",
    Taurus      => "Taurus",
    Gemini      => "Gemini",
    Cancer      => "Cancer",
    Leo         => "Leo",
    Virgo       => "Virgo",
    Libra       => "Libra",
    Scorpio     => "Scorpio",
    Sagittarius => "Sagittarius",
    Capricorn   => "Capricorn",
    Aquarius    => "Aquarius",
    Pisces      => "Pisces",
}

crate::enum_str_method! {
    /// Translated display label for UI rendering.
    StarSign::label;
    Aries       => t!("Aries"),
    Taurus      => t!("Taurus"),
    Gemini      => t!("Gemini"),
    Cancer      => t!("Cancer"),
    Leo         => t!("Leo"),
    Virgo       => t!("Virgo"),
    Libra       => t!("Libra"),
    Scorpio     => t!("Scorpio"),
    Sagittarius => t!("Sagittarius"),
    Capricorn   => t!("Capricorn"),
    Aquarius    => t!("Aquarius"),
    Pisces      => t!("Pisces"),
}

crate::enum_str_method! {
    /// Legacy ASCII lowercase used in a few places that still need a
    /// `&'static str`. Prefer `to_lower(s.label())` for new code so the
    /// embedded form respects the active translation.
    StarSign::lower_name;
    Aries       => "aries",
    Taurus      => "taurus",
    Gemini      => "gemini",
    Cancer      => "cancer",
    Leo         => "leo",
    Virgo       => "virgo",
    Libra       => "libra",
    Scorpio     => "scorpio",
    Sagittarius => "sagittarius",
    Capricorn   => "capricorn",
    Aquarius    => "aquarius",
    Pisces      => "pisces",
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Temperament {
    Bold = 0,
    Loyal,
    Mischievous,
    Curious,
    Sociable,
}

impl Temperament {
    pub fn from_dominant_index(idx: usize) -> Self {
        match idx {
            0 => Temperament::Bold,
            1 => Temperament::Loyal,
            2 => Temperament::Mischievous,
            3 => Temperament::Curious,
            _ => Temperament::Sociable,
        }
    }
}

crate::enum_str_method! {
    Temperament::label;
    Bold        => t!("Bold"),
    Loyal       => t!("Loyal"),
    Mischievous => t!("Mischievous"),
    Curious     => t!("Curious"),
    Sociable    => t!("Sociable"),
}

crate::enum_str_method! {
    Temperament::lower_label;
    Bold        => "bold",
    Loyal       => "loyal",
    Mischievous => "mischievous",
    Curious     => "curious",
    Sociable    => "sociable",
}

pub fn fav_weather_from_index(idx: u32) -> crate::context::FavWeather {
    use crate::context::FavWeather;
    match idx % 4 {
        0 => FavWeather::Sunny,
        1 => FavWeather::Rainy,
        2 => FavWeather::Snowy,
        _ => FavWeather::Overcast,
    }
}

const MEALS: [FoodItem; 13] = [
    FoodItem::Kibble,
    FoodItem::Cod,
    FoodItem::Haddock,
    FoodItem::Trout,
    FoodItem::Shrimp,
    FoodItem::Herring,
    FoodItem::Turkey,
    FoodItem::Tuna,
    FoodItem::Salmon,
    FoodItem::Chicken,
    FoodItem::Liver,
    FoodItem::Beef,
    FoodItem::Lamb,
];

const SNACKS: [FoodItem; 9] = [
    FoodItem::Carrots,
    FoodItem::Pumpkin,
    FoodItem::Treats,
    FoodItem::FishBite,
    FoodItem::Eggs,
    FoodItem::Nugget,
    FoodItem::Milk,
    FoodItem::ChewStick,
    FoodItem::Puree,
];

const TOY_VARIANTS: [ToyVariant; 5] = [
    ToyVariant::String_,
    ToyVariant::Feather,
    ToyVariant::Ball,
    ToyVariant::Laser,
    ToyVariant::Mouse,
];

const LOCATIONS: [SceneId; 4] = [
    SceneId::Outside,
    SceneId::Kitchen,
    SceneId::Treehouse,
    SceneId::Bedroom,
];

const TRAIT_MAGNITUDE: u32 = 10;
const PERSONALITY_TRAITS: usize = 5;

/// Fold `word` into `state`. Each step is invertible, so absorbing more data
/// never shrinks the entropy already held; the multiply keeps it non-linear.
pub fn absorb(state: &mut u64, word: u64) {
    let mut s = (*state ^ word).rotate_left(23);
    s = s.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    *state = s ^ (s >> 29);
}

/// Splitmix64 step: the next pet seed derived from a root `state`. The
/// constant increment means a zero state is not a fixed point.
pub fn next_expanded(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Returns five `(courage, loyalty, mischievousness, curiosity, sociability)`
/// offsets in roughly `[-TRAIT_MAGNITUDE, +TRAIT_MAGNITUDE]`, mean-centered.
pub fn derive_trait_offsets(seed: u64) -> [i32; PERSONALITY_TRAITS] {
    let mut state = ((seed ^ (seed >> 32)) & 0xFFFF_FFFF) as u32;
    if state == 0 {
        state = 1;
    }
    let span = 2 * TRAIT_MAGNITUDE + 1;
    let mut raw = [0u32; PERSONALITY_TRAITS];
    for slot in raw.iter_mut() {
        *slot = xorshift32(&mut state) % span;
    }
    let mean: u32 = raw.iter().sum::<u32>() / PERSONALITY_TRAITS as u32;
    let mut out = [0i32; PERSONALITY_TRAITS];
    for (i, &v) in raw.iter().enumerate() {
        out[i] = v as i32 - mean as i32;
    }
    out
}

#[derive(Clone, Copy)]
pub struct DerivedFavorites {
    pub pet_gender: PetGender,
    pub fav_weather: crate::context::FavWeather,
    pub star_sign: StarSign,
    pub fav_meal: FoodItem,
    pub least_fav_meal: FoodItem,
    pub fav_snack: FoodItem,
    pub least_fav_snack: FoodItem,
    pub fav_toy: ToyVariant,
    pub least_fav_toy: ToyVariant,
    pub fav_location: SceneId,
    pub least_fav_location: SceneId,
}

/// Uses the upper 32 bits of `seed` as the xorshift starting state, keeping
/// derivation independent from `derive_trait_offsets` (which uses the lower).
pub fn derive_favorites(seed: u64) -> DerivedFavorites {
    let mut state = ((seed >> 32) & 0xFFFF_FFFF) as u32;
    if state == 0 {
        state = 1;
    }
    let mut next = || xorshift32(&mut state);

    let gender = if next() % 2 == 0 {
        PetGender::Tom
    } else {
        PetGender::Queen
    };
    let fav_weather = fav_weather_from_index(next());

    fn pick_two<T: Copy, const N: usize>(items: &[T; N], next: &mut impl FnMut() -> u32) -> (T, T) {
        let n = N as u32;
        let fi = next() % n;
        let li = (fi + 1 + next() % (n - 1)) % n;
        (items[fi as usize], items[li as usize])
    }

    let star_sign = StarSign::from_index(next());
    let (fav_meal, least_fav_meal) = pick_two(&MEALS, &mut next);
    let (fav_snack, least_fav_snack) = pick_two(&SNACKS, &mut next);
    let (fav_toy, least_fav_toy) = pick_two(&TOY_VARIANTS, &mut next);
    let (fav_location, least_fav_location) = pick_two(&LOCATIONS, &mut next);

    DerivedFavorites {
        pet_gender: gender,
        fav_weather,
        star_sign,
        fav_meal,
        least_fav_meal,
        fav_snack,
        least_fav_snack,
        fav_toy,
        least_fav_toy,
        fav_location,
        least_fav_location,
    }
}
