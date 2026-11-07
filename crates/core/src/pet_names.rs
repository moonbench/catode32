//! Cat name pools. Ports the three lists in `scenes/adoption.py`.

use crate::t;

pub const TOM_NAMES: &[&str] = &[
    "Jasper", "Orion", "Bennie", "Winston", "Reginald", "Odie", "Beasley",
    "Yoshi", "Zeus", "Zeke", "Leo", "Ajax", "Java", "Rio", "Gizmo", "Loki",
    "Smokey", "Rebel", "Milo", "Simba", "Rocky", "Jet", "Mozart", "Spunky",
    "Yogi", "Ollie", "Otto", "Skipper", "Rex", "Ace", "Casper", "Domino",
    "Knox", "Rocky",
];

pub const QUEEN_NAMES: &[&str] = &[
    "Bean", "Lyra", "Tressym", "Angel", "Callie", "Honey", "Piper", "Roxie",
    "Daisy", "Jasmine", "Lizzy", "Daphnie", "Paprika", "Mocha", "Cocoa",
    "Luna", "Peaches", "Kiki", "Suki", "Cleo", "Violet", "Lilith", "Buffie",
    "Piper", "Star", "Maya", "Hidey", t!("Bubbles"), t!("Rose"), "Fiona",
    "Zali",
];

pub const EITHER_NAMES: &[&str] = &[
    "Juno", "Jessie", "Remy", "Jiji", "Turtle", "Bandit", "Fuzzy", "June",
    "Koko", "Noodle", "Pixel", "Scratches", "Scraps", "Silver", "Sushi",
    "Tiger", "Tux", "Umi", "Whiskers", "Ziggy", "Patch", "Midnight", "Gato",
    t!("Hunter"), "Pepper", "Bengie", "Kitty", "Snowball", "Star", "Artemis",
    "Tang", "Titch", "Rainbow", "Speedy", "Lemony", "Milkshake", "Jingles",
    "Muffin", "Taco", "Turbo", "Speedy", "Ash", "Copper", "Cloud", "Dusk",
    "Echo", "Hero", "Karma", "Lynx", "Marble", "Mittens", "Mocha", "Mint",
    "Nutmeg", "Patches", "Saturn", "Scout", "Toast", "Xylo", "Yoko", "Zero",
    "Zephyr", "Nimbus",
];

/// Pick a name from the gendered pool concatenated with the unisex pool:
/// `pool = (TOM_NAMES if tom else QUEEN_NAMES) + EITHER_NAMES`
/// then `pool[(seed >> 20) % len(pool)]`.
pub fn pick_name(seed: u64, gendered: &'static [&'static str]) -> &'static str {
    let total = gendered.len() + EITHER_NAMES.len();
    let idx = ((seed >> 20) as usize) % total;
    if idx < gendered.len() {
        gendered[idx]
    } else {
        EITHER_NAMES[idx - gendered.len()]
    }
}
