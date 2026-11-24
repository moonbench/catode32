use crate::render::Sprite;

const PAW_SMALL1_FRAMES: &[&[u8]] = &[
    b"\x28\xaa\x82\x38\x7c\x7c\x28",
];

pub static PAW_SMALL1: Sprite = Sprite {
    width: 7,
    height: 7,
    frames: PAW_SMALL1_FRAMES,
    fill_frames: None,
};

const PAW_MED_FRAMES: &[&[u8]] = &[
    b"\x0a\x00\x1b\x00\x5b\x40\xd1\x60\xc4\x60\x9f\x20\x3f\x80\x3f\x80\x7f\xc0\x7f\xc0\x7f\xc0\x3f\x80\x1b\x00",
];

pub static PAW_MED: Sprite = Sprite {
    width: 11,
    height: 13,
    frames: PAW_MED_FRAMES,
    fill_frames: None,
};

const PAW_LARGE1_FRAMES: &[&[u8]] = &[
    b"\x04\x40\x0c\x60\x0e\xe0\x1e\xf0\x5c\x74\x5c\x74\xe8\x2e\xe0\x0e\xe7\xce\x4f\xe4\x1f\xf0\x1f\xf0\x3f\xf8\x3f\xf8\x3f\xf8\x1f\xf0\x0c\x60",
];

pub static PAW_LARGE1: Sprite = Sprite {
    width: 15,
    height: 17,
    frames: PAW_LARGE1_FRAMES,
    fill_frames: None,
};

// Snake minigame head frames. The two frames have different dimensions,
// so they don't fit a single `Sprite` (fixed width/height) and are drawn
// via `draw_sprite_raw` with explicit dimensions per direction.
pub const CAT_THIN_DOWN: &[u8] = b"\x84\xcc\xfc\xb4\xb4\xfc\x78\x30"; // 6x8
pub const CAT_THIN_RIGHT: &[u8] = b"\xfc\x66\x3f\x3f\x66\xfc"; // 8x6

const SPOT_FRAMES: &[&[u8]] = &[b"\x60\xf0\xf0\x60"];

pub static SPOT: Sprite = Sprite {
    width: 4,
    height: 4,
    frames: SPOT_FRAMES,
    fill_frames: None,
};

// Pipes minigame sprites, all 9x9 (18 bytes each).
// Frame indices:
//   PIPE_STRAIGHT / PIPE_FAT: 0=LR empty, 1=UD empty, 2=LR filled, 3=UD filled
//   PIPE_CORNER (CW: RD -> LD -> LU -> RU): 0-3 empty, 4-7 filled
//   PIPE_OUTLET_LEFT / PIPE_OUTLET_RIGHT: 0=empty, 1=filled

const PIPE_STRAIGHT_FRAMES: &[&[u8]] = &[
    b"\x00\x00\x00\x00\x80\x80\xff\x80\x80\x80\xff\x80\x80\x80\x00\x00\x00\x00",
    b"\x3e\x00\x14\x00\x14\x00\x14\x00\x14\x00\x14\x00\x14\x00\x14\x00\x3e\x00",
    b"\x00\x00\x00\x00\x80\x80\xff\x80\xff\x80\xff\x80\x80\x80\x00\x00\x00\x00",
    b"\x3e\x00\x1c\x00\x1c\x00\x1c\x00\x1c\x00\x1c\x00\x1c\x00\x1c\x00\x3e\x00",
];

pub static PIPE_STRAIGHT: Sprite = Sprite {
    width: 9,
    height: 9,
    frames: PIPE_STRAIGHT_FRAMES,
    fill_frames: None,
};

const PIPE_CORNER_FRAMES: &[&[u8]] = &[
    b"\x00\x00\x00\x00\x00\x80\x07\x80\x08\x80\x13\x80\x14\x80\x14\x00\x3e\x00",
    b"\x00\x00\x00\x00\x80\x00\xf0\x00\x88\x00\xe4\x00\x94\x00\x14\x00\x3e\x00",
    b"\x3e\x00\x14\x00\x94\x00\xe4\x00\x88\x00\xf0\x00\x80\x00\x00\x00\x00\x00",
    b"\x3e\x00\x14\x00\x14\x80\x13\x80\x08\x80\x07\x80\x00\x80\x00\x00\x00\x00",
    b"\x00\x00\x00\x00\x00\x80\x07\x80\x0f\x80\x1f\x80\x1c\x80\x1c\x00\x3e\x00",
    b"\x00\x00\x00\x00\x80\x00\xf0\x00\xf8\x00\xfc\x00\x9c\x00\x1c\x00\x3e\x00",
    b"\x3e\x00\x1c\x00\x9c\x00\xfc\x00\xf8\x00\xf0\x00\x80\x00\x00\x00\x00\x00",
    b"\x3e\x00\x1c\x00\x1c\x80\x1f\x80\x0f\x80\x07\x80\x00\x80\x00\x00\x00\x00",
];

pub static PIPE_CORNER: Sprite = Sprite {
    width: 9,
    height: 9,
    frames: PIPE_CORNER_FRAMES,
    fill_frames: None,
};

const PIPE_FAT_FRAMES: &[&[u8]] = &[
    b"\x00\x00\x1c\x00\xa2\x80\xc1\x80\x80\x80\xc1\x80\xa2\x80\x1c\x00\x00\x00",
    b"\x3e\x00\x14\x00\x22\x00\x41\x00\x41\x00\x41\x00\x22\x00\x14\x00\x3e\x00",
    b"\x00\x00\x1c\x00\xbe\x80\xff\x80\xff\x80\xff\x80\xbe\x80\x1c\x00\x00\x00",
    b"\x3e\x00\x1c\x00\x3e\x00\x7f\x00\x7f\x00\x7f\x00\x3e\x00\x1c\x00\x3e\x00",
];

pub static PIPE_FAT: Sprite = Sprite {
    width: 9,
    height: 9,
    frames: PIPE_FAT_FRAMES,
    fill_frames: None,
};

const PIPE_OUTLET_LEFT_FRAMES: &[&[u8]] = &[
    b"\x21\x00\xa1\x00\xe1\x00\x81\x00\x81\x00\x81\x00\xe1\x00\xa1\x00\x21\x00",
    b"\x3f\x00\xbf\x00\xff\x00\xff\x00\xff\x00\xff\x00\xff\x00\xbf\x00\x3f\x00",
];

pub static PIPE_OUTLET_LEFT: Sprite = Sprite {
    width: 9,
    height: 9,
    frames: PIPE_OUTLET_LEFT_FRAMES,
    fill_frames: None,
};

const PIPE_OUTLET_RIGHT_FRAMES: &[&[u8]] = &[
    b"\x42\x00\x42\x80\x43\x80\x40\x80\x40\x80\x40\x80\x43\x80\x42\x80\x42\x00",
    b"\x7e\x00\x7e\x80\x7f\x80\x7f\x80\x7f\x80\x7f\x80\x7f\x80\x7e\x80\x7e\x00",
];

pub static PIPE_OUTLET_RIGHT: Sprite = Sprite {
    width: 9,
    height: 9,
    frames: PIPE_OUTLET_RIGHT_FRAMES,
    fill_frames: None,
};

const LOCK_FRAMES: &[&[u8]] = &[
    b"\x38\x44\x44\xfe\xfe\xee\xee\xfe\xfe",
];

pub static LOCK: Sprite = Sprite {
    width: 7,
    height: 9,
    frames: LOCK_FRAMES,
    fill_frames: None,
};

// Cat Tree pickups. Placeholder art.

// Spring sitting on a perch, coiled, and sprung after the cat lands on it.
// Different heights, so they're separate sprites; both sit on their bottom
// row.
pub static CATTREE_SPRING: Sprite = Sprite {
    width: 8,
    height: 4,
    frames: &[b"\x7e\x8d\x30\x0c"],
    fill_frames: None,
};

pub static CATTREE_SPRING_SPRUNG: Sprite = Sprite {
    width: 8,
    height: 11,
    frames: &[b"\xff\x0c\x30\x0c\x30\x0c\x30\x0c\x30\x0c\xff"],
    fill_frames: None,
};

// Propeller hat, two spin frames. The brim is the bottom row.
const CATTREE_PROPELLER_HAT_FRAMES: &[&[u8]] = &[
    b"\xff\x80\x08\x00\x1c\x00\x3e\x00\x55\x00",
    b"\x1c\x00\x08\x00\x1c\x00\x3e\x00\x55\x00",
];
const CATTREE_PROPELLER_HAT_FILL: &[&[u8]] = &[
    b"\xff\x80\x08\x00\x1c\x00\x3e\x00\x7f\x00",
    b"\x1c\x00\x08\x00\x1c\x00\x3e\x00\x7f\x00",
];

pub static CATTREE_PROPELLER_HAT: Sprite = Sprite {
    width: 9,
    height: 5,
    frames: CATTREE_PROPELLER_HAT_FRAMES,
    fill_frames: Some(CATTREE_PROPELLER_HAT_FILL),
};

// Jetpack: frame 0 unlit (the pickup), frames 1-2 flicker while worn. The
// tank is the top 7 rows, the flame the bottom 3.
const CATTREE_JETPACK_FRAMES: &[&[u8]] = &[
    b"\x70\x88\xa8\xa8\x88\xf8\x50\x00\x00\x00",
    b"\x70\x88\xa8\xa8\x88\xf8\x50\x50\xd8\x50",
    b"\x70\x88\xa8\xa8\x88\xf8\x50\x50\x50\x00",
];
const CATTREE_JETPACK_FILL: &[&[u8]] = &[
    b"\x70\xf8\xf8\xf8\xf8\xf8\x70\x00\x00\x00",
    b"\x70\xf8\xf8\xf8\xf8\xf8\x70\x50\xd8\x50",
    b"\x70\xf8\xf8\xf8\xf8\xf8\x70\x50\x50\x00",
];

pub static CATTREE_JETPACK: Sprite = Sprite {
    width: 5,
    height: 10,
    frames: CATTREE_JETPACK_FRAMES,
    fill_frames: Some(CATTREE_JETPACK_FILL),
};

// Treat Toss: a 2x2 treat with a 1px black outline (the fill mask) so it
// reads on any background.
const TREAT_FRAMES: &[&[u8]] = &[b"\x00\x60\x60\x00"];
const TREAT_FILL_FRAMES: &[&[u8]] = &[b"\xf0\xf0\xf0\xf0"];

pub static TREAT: Sprite = Sprite {
    width: 4,
    height: 4,
    frames: TREAT_FRAMES,
    fill_frames: Some(TREAT_FILL_FRAMES),
};
