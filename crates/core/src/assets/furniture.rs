use embedded_graphics::prelude::{Point, Size};

use crate::render::{Renderer, Sprite, SpriteOpts};

/// Bookshelf and dresser share a 69x48 cabinet: a capped frame with a
/// middle shelf, standing on a patterned baseboard. The frame is drawn in
/// code; only the books, baseboard and drawer handles are sprites.
pub const CABINET_W: i32 = 69;
pub const CABINET_H: i32 = 48;
/// Rows of the two shelf boards (each 2 thick), from the cabinet's top.
const CABINET_SHELF_YS: [i32; 2] = [20, 40];
const CABINET_BASE_Y: i32 = 43;

const CABINET_BASE_FRAMES: &[&[u8]] = &[b"\x3f\xff\xff\xff\xff\xff\xff\xff\xe0\x20\x20\x20\x20\x20\x20\x20\x20\x20\x77\x77\x77\x77\x77\x77\x77\x77\x70\xe2\x02\x02\x02\x02\x02\x02\x02\x38\xff\xff\xff\xff\xff\xff\xff\xff\xf8"];
pub static CABINET_BASE: Sprite = Sprite { width: 69, height: 5, frames: CABINET_BASE_FRAMES, fill_frames: None };

const BOOKSHELF_BOOKS1_FRAMES: &[&[u8]] = &[b"\x00\x00\x00\x00\x00\x03\xdb\x60\x00\x01\x98\x08\x00\x00\x18\x60\x5f\xf9\x80\x80\x0e\x03\xc3\x60\x5f\xf9\x98\x88\x0a\x03\x5b\x60\x00\x01\xd8\x48\x1f\x02\xdb\x60\x7f\xfc\xd8\x48\x00\x03\x5b\x60\x00\x00\xd8\x28\x3f\xfa\xdb\x60\xdf\xf6\xcc\x28\x3f\xfb\xdb\x60\xca\xa6\xcc\x7c\x00\x03\xdb\x60\xd5\x56\xcc\x00\xff\xfb\xdb\x60\xdf\xf6\xec\xfe\x00\x03\xdb\x60\x00\x00\x6c\xaa\xff\xfb\xc3\x60\x2f\xfd\x60\xaa\xff\xf8\x18\x60\x2f\xfd\x6c\xfe\xff\xfb\xdb\x60"];
pub static BOOKSHELF_BOOKS1: Sprite = Sprite { width: 59, height: 14, frames: BOOKSHELF_BOOKS1_FRAMES, fill_frames: None };

const BOOKSHELF_BOOKS2_FRAMES: &[&[u8]] = &[b"\x00\x00\x00\x00\x00\x00\x00\x60\x00\x00\x00\x00\x23\x00\x01\x00\x00\x00\x00\x01\xa8\x00\x01\x60\x0c\x00\xdf\xf1\xab\x00\x01\x60\x12\x00\xdf\xf1\xab\x00\x81\x60\x00\x00\x00\x01\xab\x01\xc9\x60\x3f\xfc\xff\xe9\xab\x01\x5d\x60\x00\x00\xff\xe9\x8b\x01\x55\x60\xfa\xf8\xff\xe9\x8b\x01\xdd\x60\x00\x00\x00\x01\xab\x00\x01\x60\xff\xf9\xff\xf9\xab\x7f\xfd\x60\xaa\xa8\x00\x01\xab\x7f\xfd\x60\xd5\x59\x7f\xe9\xab\x00\x01\x60\xaa\xa9\x7f\xe9\xa8\x3f\xfd\x00\xff\xf9\x7f\xe9\xab\x3f\xfd\x60"];
pub static BOOKSHELF_BOOKS2: Sprite = Sprite { width: 59, height: 15, frames: BOOKSHELF_BOOKS2_FRAMES, fill_frames: None };

const DRESSER_HANDLE_FRAMES: &[&[u8]] = &[b"\xc0\xc0\xc0\xc0\x7f\x80"];
pub static DRESSER_HANDLE: Sprite = Sprite { width: 10, height: 3, frames: DRESSER_HANDLE_FRAMES, fill_frames: None };

/// Black out the cabinet's footprint and draw the empty frame and base,
/// top-left corner at (x, y).
fn draw_cabinet(renderer: &mut Renderer, x: i32, y: i32) {
    let w = CABINET_W;
    let rect = |r: &mut Renderer, x0: i32, y0: i32, rw: i32, rh: i32| {
        r.draw_rect(Point::new(x + x0, y + y0), Size::new(rw as u32, rh as u32), true);
    };
    // Masked, so anything further back (the bedroom's basket) hides behind.
    renderer.fill_rect_off(Point::new(x + 1, y), Size::new(w as u32 - 2, CABINET_BASE_Y as u32));
    renderer.fill_rect_off(Point::new(x, y + CABINET_BASE_Y), Size::new(w as u32, 5));
    // Cap, overhanging the sides by a pixel with a bracket underneath.
    rect(renderer, 1, 0, w - 2, 2);
    rect(renderer, 1, 2, 3, 1);
    rect(renderer, w - 4, 2, 3, 1);
    // Sides and shelf boards.
    let side_h = CABINET_SHELF_YS[1] + 2 - 3;
    rect(renderer, 2, 3, 2, side_h);
    rect(renderer, w - 4, 3, 2, side_h);
    for sy in CABINET_SHELF_YS {
        rect(renderer, 2, sy, w - 4, 2);
    }
    renderer.draw_sprite(&CABINET_BASE, Point::new(x, y + CABINET_BASE_Y), SpriteOpts::default());
}

/// Bookshelf, top-left corner at (x, y). Its base ends at `y + CABINET_H`.
pub fn draw_bookshelf(renderer: &mut Renderer, x: i32, y: i32) {
    draw_cabinet(renderer, x, y);
    renderer.draw_sprite(&BOOKSHELF_BOOKS1, Point::new(x + 5, y + 5), SpriteOpts::default());
    renderer.draw_sprite(&BOOKSHELF_BOOKS2, Point::new(x + 5, y + 24), SpriteOpts::default());
}

/// Dresser: one wide drawer over two, top-left corner at (x, y).
pub fn draw_dresser(renderer: &mut Renderer, x: i32, y: i32) {
    draw_cabinet(renderer, x, y);
    // (left, top, width, height) of each drawer front.
    let drawers = [(5, 4, 59, 15), (5, 23, 29, 16), (35, 23, 29, 16)];
    for (dx, dy, dw, dh) in drawers {
        let (px, py) = (x + dx, y + dy);
        renderer.draw_rect(Point::new(px, py), Size::new(dw as u32, dh as u32), false);
        renderer.draw_rect(Point::new(px + 2, py + 2), Size::new(dw as u32 - 4, dh as u32 - 4), false);
        let hx = px + (dw - DRESSER_HANDLE.width as i32) / 2;
        let hy = py + (dh - DRESSER_HANDLE.height as i32) / 2;
        renderer.draw_sprite(&DRESSER_HANDLE, Point::new(hx, hy), SpriteOpts::default());
    }
}

const PILLOW_FRAMES: &[&[u8]] = &[
    b"\x0f\xff\xfc\x00\x30\x00\x03\x00\x40\x00\x00\x80\x80\x00\x00\x40\x80\x00\x00\x40\xa8\x00\x0a\xc0\x55\x00\x55\x80\x3f\xff\xff\x00",
];

pub static PILLOW: Sprite = Sprite {
    width: 26,
    height: 8,
    frames: PILLOW_FRAMES,
    fill_frames: None,
};

/// The bedroom bed, side on: footboard post, mattress on a side rail, and a
/// taller headboard post with the pillow against it.
pub const BED_W: i32 = 89;
pub const BED_H: i32 = 48;
/// Rows, from the top of the headboard, of the mattress top, the pillow top
/// and the bottom of the side rail (below it there's only the two posts).
pub const BED_MATTRESS_Y: i32 = 18;
pub const BED_PILLOW_Y: i32 = 10;
pub const BED_RAIL_BOTTOM_Y: i32 = 34;
pub const BED_PILLOW_X: i32 = 53;
const BED_FOOT_W: i32 = 6;
const BED_HEAD_X: i32 = 79;

/// Draw the bed with the top of its headboard at (x, y). Its posts end at
/// `y + BED_H`. `flip` turns it round so the headboard is on the left.
pub fn draw_bed(renderer: &mut Renderer, x: i32, y: i32, flip: bool) {
    // Left edge, within the bed, of a span `w` wide starting `dx` in.
    let fx = |dx: i32, w: i32| x + if flip { BED_W - dx - w } else { dx };
    let mask = |r: &mut Renderer, x0: i32, y0: i32, w: u32, h: u32| {
        r.fill_rect_off(Point::new(fx(x0, w as i32), y + y0), Size::new(w, h));
    };
    mask(renderer, 0, 14, 6, 34);
    mask(renderer, 5, 18, 76, 17);
    mask(renderer, BED_HEAD_X, 0, 10, 48);
    mask(renderer, BED_PILLOW_X, 10, 23, 8);
    mask(renderer, 76, 12, 2, 6);
    let p = |dx: i32, dy: i32, w: i32| Point::new(fx(dx, w), y + dy);
    // Footboard
    draw_rounded_outline(renderer, p(0, 14, BED_FOOT_W), Size::new(BED_FOOT_W as u32, 34));
    // Headboard: capped post
    draw_rounded_outline(renderer, p(BED_HEAD_X, 0, 10), Size::new(10, 4));
    renderer.draw_rect(p(BED_HEAD_X + 1, 3, 8), Size::new(8, 45), false);
    // Side rail between the posts
    renderer.draw_rect(p(5, 30, 76), Size::new(76, 5), false);
    // Mattress
    draw_rounded_outline(renderer, p(7, BED_MATTRESS_Y, 72), Size::new(72, 11));
    let pillow = p(BED_PILLOW_X, BED_PILLOW_Y, PILLOW.width as i32);
    renderer.draw_sprite(&PILLOW, pillow, SpriteOpts { mirror_h: flip, ..Default::default() });
}

/// Just the posts below the side rail, for drawing over anything that slid
/// under the bed.
pub fn draw_bed_legs(renderer: &mut Renderer, x: i32, y: i32, flip: bool) {
    let fx = |dx: i32, w: i32| x + if flip { BED_W - dx - w } else { dx };
    let top = y + BED_RAIL_BOTTOM_Y + 1;
    let bottom = y + BED_H - 1;
    let h = (bottom - top + 1) as u32;
    let foot = fx(0, BED_FOOT_W);
    renderer.fill_rect_off(Point::new(foot, top), Size::new(BED_FOOT_W as u32, h));
    renderer.draw_line(Point::new(foot, top), Point::new(foot, bottom - 1));
    renderer.draw_line(Point::new(foot + BED_FOOT_W - 1, top), Point::new(foot + BED_FOOT_W - 1, bottom - 1));
    renderer.draw_line(Point::new(foot + 1, bottom), Point::new(foot + BED_FOOT_W - 2, bottom));
    let hx = fx(BED_HEAD_X + 1, 8);
    renderer.fill_rect_off(Point::new(hx, top), Size::new(8, h));
    renderer.draw_line(Point::new(hx, top), Point::new(hx, bottom));
    renderer.draw_line(Point::new(hx + 7, top), Point::new(hx + 7, bottom));
    renderer.draw_line(Point::new(hx, bottom), Point::new(hx + 7, bottom));
}

/// Rectangle outline with the four corner pixels left off.
fn draw_rounded_outline(renderer: &mut Renderer, pos: Point, size: Size) {
    let (x0, y0) = (pos.x, pos.y);
    let x1 = x0 + size.width as i32 - 1;
    let y1 = y0 + size.height as i32 - 1;
    renderer.draw_line(Point::new(x0 + 1, y0), Point::new(x1 - 1, y0));
    renderer.draw_line(Point::new(x0 + 1, y1), Point::new(x1 - 1, y1));
    renderer.draw_line(Point::new(x0, y0 + 1), Point::new(x0, y1 - 1));
    renderer.draw_line(Point::new(x1, y0 + 1), Point::new(x1, y1 - 1));
}

/// Floor lamp: a tall stem under a cone shade, 25 wide, centred on `x`,
/// standing with its base on row `floor_y`.
pub fn draw_floor_lamp(renderer: &mut Renderer, x: i32, floor_y: i32) {
    let line = |r: &mut Renderer, x0: i32, y0: i32, x1: i32, y1: i32| {
        r.draw_line(Point::new(x + x0, floor_y + y0), Point::new(x + x1, floor_y + y1));
    };
    // Base
    line(renderer, -5, 0, 5, 0);
    line(renderer, -3, -1, 3, -1);
    // Stem
    renderer.draw_rect(Point::new(x - 1, floor_y - 43), Size::new(3, 42), true);
    // Shade
    line(renderer, -12, -43, 12, -43);
    line(renderer, -5, -55, 5, -55);
    line(renderer, -12, -43, -5, -55);
    line(renderer, 12, -43, 5, -55);
    // Cord
    line(renderer, 4, -43, 4, -35);
}

const CAT_BED_SIDE_FRAMES: &[&[u8]] = &[
    b"\x07\xc0\x00\x18\x3e\x00\x20\x01\xf8\x40\x00\x00\x80\x00\x00\xf0\x00\x00\x8f\x80\x00\x40\x7f\xf8\x40\x00\x00\x20\x00\x00\x20\x00\x00\x1f\xff\xf8",
];

const CAT_BED_SIDE_FILL_FRAMES: &[&[u8]] = &[
    b"\x07\xc0\x00\x1f\xfe\x00\x3f\xff\xf8\x7f\xff\xf8\xff\xff\xf8\xff\xff\xf8\xff\xff\xf8\x7f\xff\xf8\x7f\xff\xf8\x3f\xff\xf8\x3f\xff\xf8\x1f\xff\xf8",
];

pub static CAT_BED_SIDE: Sprite = Sprite {
    width: 21,
    height: 12,
    frames: CAT_BED_SIDE_FRAMES,
    fill_frames: Some(CAT_BED_SIDE_FILL_FRAMES),
};

const FAUCET_FRAMES: &[&[u8]] = &[
    b"\x1f\x80\x00\x20\x40\x00\x40\x20\x00\x8e\x20\x00\x91\x20\x00\x91\x20\x00\xf1\x20\x00\x01\x20\x00\x01\x20\x00\x01\x20\x00\x01\x20\x00\x01\x20\x00\xf1\x23\xc0\x91\x22\x40\x91\x22\x40\x91\x22\x40",
];

pub static FAUCET: Sprite = Sprite {
    width: 18,
    height: 16,
    frames: FAUCET_FRAMES,
    fill_frames: None,
};

const STREET_LAMP_FRAMES: &[&[u8]] = &[
    b"\x10\x7c\xfe\x44\x44\x44\x54\x44\x7c\x38\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x38\x10\x38\x38\x38\x38\x38\x38\x7c\xfe\xfe",
];

pub static STREET_LAMP: Sprite = Sprite {
    width: 7,
    height: 37,
    frames: STREET_LAMP_FRAMES,
    fill_frames: None,
};

const PARK_BENCH_FRAMES: &[&[u8]] = &[
    b"\x20\x00\x80\x7f\xff\xc0\xff\xff\xe0\x20\x00\x80\xff\xff\xe0\xff\xff\xe0\x20\x00\x80\x7f\xff\xf0\x10\x00\x40\x10\x00\x40\x38\x00\xe0",
];

pub static PARK_BENCH: Sprite = Sprite {
    width: 20,
    height: 11,
    frames: PARK_BENCH_FRAMES,
    fill_frames: None,
};

/// Draw the two CAT_BED_SIDE sprites plus the masked rim that joins them.
/// `bed_x` is the screen-space x of the left rim sprite (callers subtract the
/// foreground camera offset from their world x).
/// Total width drawn by `draw_cat_bed_rim`: two bed sides plus the 20px middle.
pub const CAT_BED_RIM_WIDTH: i32 = CAT_BED_SIDE.width as i32 * 2 + 20;

pub fn draw_cat_bed_rim(renderer: &mut Renderer, bed_x: i32) {
    draw_cat_bed_rim_at(renderer, bed_x, 52);
}

/// `draw_cat_bed_rim` with the top of the sides at `y` (it ends at `y + 11`).
pub fn draw_cat_bed_rim_at(renderer: &mut Renderer, bed_x: i32, y: i32) {
    let w = CAT_BED_SIDE.width as i32;
    renderer.draw_sprite(&CAT_BED_SIDE, Point::new(bed_x, y), SpriteOpts::default());
    renderer.fill_rect_off(Point::new(bed_x + w, y + 2), Size::new(20, 10));
    renderer.draw_line(Point::new(bed_x + w, y + 2), Point::new(bed_x + w + 20, y + 2));
    renderer.draw_line(Point::new(bed_x + w, y + 11), Point::new(bed_x + w + 20, y + 11));
    renderer.draw_line(Point::new(bed_x + w, y + 7), Point::new(bed_x + w + 20, y + 7));
    renderer.draw_sprite(
        &CAT_BED_SIDE,
        Point::new(bed_x + w + 20, y),
        SpriteOpts {
            mirror_h: true,
            ..Default::default()
        },
    );
}

// ---------------------------------------------------------------------------
// Fridge
// ---------------------------------------------------------------------------
//
// The body, doors and feet are drawn from primitives; only the handles and
// the things stuck on the doors are sprites. Where those things go comes from
// the pet's genome, so every pet's fridge is a little different but always
// the same for that pet.

pub const FRIDGE_W: i32 = 36;
pub const FRIDGE_H: i32 = 58;
/// Height of the body (outline included); the feet hang below it.
pub const FRIDGE_BODY_H: i32 = 55;
pub const FRIDGE_FEET_H: i32 = FRIDGE_H - FRIDGE_BODY_H;
/// Row of the line between the freezer door and the main door.
const FRIDGE_DIVIDER_Y: i32 = 18;
/// Row of the line along the bottom of the main door.
const FRIDGE_KICK_Y: i32 = 52;
const FRIDGE_HANDLE_X: i32 = 3;
const FRIDGE_HANDLE_YS: [i32; 2] = [6, 22];
/// Left edge of each foot.
const FRIDGE_FEET_X: [i32; 2] = [3, 27];
const FRIDGE_FOOT_W: u32 = 6;

const FRIDGE_HANDLE_FRAMES: &[&[u8]] = &[b"\xc0\xc0\x80\x80\x80\x80\x80\x80\x40"];
pub static FRIDGE_HANDLE: Sprite = Sprite { width: 2, height: 9, frames: FRIDGE_HANDLE_FRAMES, fill_frames: None };

const FRIDGE_CALENDAR_FRAMES: &[&[u8]] = &[
    b"\xff\xf8\xa7\x28\x80\x08\xff\xf8\x80\x08\x8a\xa8\x80\x08\xaa\xa8\x80\x08\xaa\x88\x80\x08\x7f\xf0",
];
pub static FRIDGE_CALENDAR: Sprite = Sprite { width: 13, height: 12, frames: FRIDGE_CALENDAR_FRAMES, fill_frames: None };

const FRIDGE_MAGNET_FRAMES: &[&[u8]] = &[b"\x40\xa0\x40"];
pub static FRIDGE_MAGNET: Sprite = Sprite { width: 3, height: 3, frames: FRIDGE_MAGNET_FRAMES, fill_frames: None };

const FRIDGE_NOTE_FRAMES: &[&[u8]] = &[
    b"\x07\x00\x07\x00\x18\x80\x60\x80\x86\x40\x90\x40\x42\x20\x4c\x20\x20\xc0\x23\x00\x1c\x00",
];
pub static FRIDGE_NOTE: Sprite = Sprite { width: 11, height: 11, frames: FRIDGE_NOTE_FRAMES, fill_frames: None };

const FRIDGE_PHOTO_FRAMES: &[&[u8]] = &[
    b"\x21\x00\x7f\x80\x80\x40\xa1\x40\xbf\x40\xad\x40\xad\x40\xbf\x40\x9e\x40\x80\x40\x7f\x80",
];
pub static FRIDGE_PHOTO: Sprite = Sprite { width: 10, height: 11, frames: FRIDGE_PHOTO_FRAMES, fill_frames: None };

const FRIDGE_TODOLIST_FRAMES: &[&[u8]] = &[b"\x10\xfe\x92\x82\xba\x82\x9a\x82\xb2\x82\xaa\x54"];
pub static FRIDGE_TODOLIST: Sprite = Sprite { width: 7, height: 12, frames: FRIDGE_TODOLIST_FRAMES, fill_frames: None };

/// Everything stuck on the fridge, biggest first so it gets placed first.
const FRIDGE_ITEMS: [&Sprite; 7] = [
    &FRIDGE_CALENDAR,
    &FRIDGE_NOTE,
    &FRIDGE_PHOTO,
    &FRIDGE_TODOLIST,
    &FRIDGE_MAGNET,
    &FRIDGE_MAGNET,
    &FRIDGE_MAGNET,
];

/// Where items may go on each door, as inclusive pixel bounds relative to
/// the fridge's top-left: clear of the outline, the handles and the line
/// between the doors.
const FRIDGE_DOORS: [(i32, i32, i32, i32); 2] = [
    (7, 2, 33, FRIDGE_DIVIDER_Y - 2),
    (7, FRIDGE_DIVIDER_Y + 2, 33, FRIDGE_KICK_Y - 2),
];

/// Used if random placement somehow keeps failing. Fits `FRIDGE_DOORS`
/// with nothing overlapping.
const FRIDGE_DEFAULT: [(i8, i8); 7] = [(10, 36), (19, 4), (21, 22), (9, 3), (31, 6), (12, 25), (28, 44)];

/// Positions of the items on a pet's fridge, matching `FRIDGE_ITEMS`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FridgeLayout {
    pub items: [(i8, i8); 7],
}

impl FridgeLayout {
    pub fn from_seed(pet_seed: u64) -> Self {
        let mut rng = ((pet_seed ^ (pet_seed >> 32)) as u32) ^ 0x0F21_D6E5;
        for _ in 0..16 {
            if let Some(items) = Self::try_place(&mut rng) {
                return Self { items };
            }
        }
        Self { items: FRIDGE_DEFAULT }
    }

    fn try_place(rng: &mut u32) -> Option<[(i8, i8); 7]> {
        use crate::rand::rand_range_u32;
        let mut items = [(0i8, 0i8); 7];
        for (i, sprite) in FRIDGE_ITEMS.iter().enumerate() {
            let (w, h) = (sprite.width as i32, sprite.height as i32);
            let mut placed = false;
            for _ in 0..40 {
                // The main door is twice the size, so it gets two thirds of
                // the picks.
                let door = FRIDGE_DOORS[if rand_range_u32(rng, 0, 2) == 0 { 0 } else { 1 }];
                let (x0, y0, x1, y1) = door;
                if x1 - x0 + 1 < w || y1 - y0 + 1 < h {
                    continue;
                }
                let x = rand_range_u32(rng, x0 as u32, (x1 - w + 1) as u32) as i32;
                let y = rand_range_u32(rng, y0 as u32, (y1 - h + 1) as u32) as i32;
                // Keep a pixel of fridge between items.
                let clear = items[..i].iter().zip(FRIDGE_ITEMS.iter()).all(|(&(ox, oy), o)| {
                    let (ox, oy, ow, oh) = (ox as i32, oy as i32, o.width as i32, o.height as i32);
                    x + w + 1 <= ox || ox + ow + 1 <= x || y + h + 1 <= oy || oy + oh + 1 <= y
                });
                if clear {
                    items[i] = (x as i8, y as i8);
                    placed = true;
                    break;
                }
            }
            if !placed {
                return None;
            }
        }
        Some(items)
    }
}

/// Draw a fridge with its top-left corner at (x, y). Its feet end at
/// `y + FRIDGE_H`.
pub fn draw_fridge(renderer: &mut Renderer, x: i32, y: i32, layout: &FridgeLayout) {
    let (w, bottom) = (FRIDGE_W, FRIDGE_BODY_H - 1);
    let line = |r: &mut Renderer, x0: i32, y0: i32, x1: i32, y1: i32| {
        r.draw_line(Point::new(x + x0, y + y0), Point::new(x + x1, y + y1));
    };
    // Outline with the corners rounded off.
    line(renderer, 1, 0, w - 2, 0);
    line(renderer, 1, bottom, w - 2, bottom);
    line(renderer, 0, 1, 0, bottom - 1);
    line(renderer, w - 1, 1, w - 1, bottom - 1);
    line(renderer, 0, FRIDGE_DIVIDER_Y, w - 1, FRIDGE_DIVIDER_Y);
    line(renderer, 0, FRIDGE_KICK_Y, w - 1, FRIDGE_KICK_Y);
    for fx in FRIDGE_FEET_X {
        renderer.draw_rect(
            Point::new(x + fx, y + FRIDGE_BODY_H),
            Size::new(FRIDGE_FOOT_W, FRIDGE_FEET_H as u32),
            true,
        );
    }
    for hy in FRIDGE_HANDLE_YS {
        renderer.draw_sprite(&FRIDGE_HANDLE, Point::new(x + FRIDGE_HANDLE_X, y + hy), SpriteOpts::default());
    }
    for (sprite, &(ix, iy)) in FRIDGE_ITEMS.iter().zip(layout.items.iter()) {
        renderer.draw_sprite(sprite, Point::new(x + ix as i32, y + iy as i32), SpriteOpts::default());
    }
}

/// Just the feet, for redrawing over anything that slid under the fridge.
pub fn draw_fridge_feet(renderer: &mut Renderer, x: i32, y: i32) {
    for fx in FRIDGE_FEET_X {
        renderer.draw_rect(
            Point::new(x + fx, y + FRIDGE_BODY_H),
            Size::new(FRIDGE_FOOT_W, FRIDGE_FEET_H as u32),
            true,
        );
    }
}

/// Couch, 84x34. The armrests are a sprite (the right one is the left
/// mirrored) and the long middle is drawn with lines, so the mostly empty
/// span costs no sprite bytes. Unmasked: it never stands in front of
/// anything.
pub const COUCH_W: i32 = 84;
pub const COUCH_H: i32 = 34;
/// Row of the seat cushions' top edge, from the top of the couch.
pub const COUCH_SEAT_Y: i32 = 16;
/// Row of the base's bottom edge. Below it there's only the feet.
pub const COUCH_BASE_Y: i32 = 29;
const COUCH_ARM_W: i32 = 8;

const COUCH_ARM_FRAMES: &[&[u8]] = &[b"\x00\x01\x02\x04\x04\x04\x04\x04\x04\x3e\x41\x81\x81\x81\x81\x81\x81\x82\x44\x44\x44\x43\x41\x41\x41\x41\x41\x41\x41\x41\x7f\x22\x22\x3e"];
pub static COUCH_ARM: Sprite = Sprite { width: 8, height: 34, frames: COUCH_ARM_FRAMES, fill_frames: None };

/// Draw the couch with its top-left corner at (x, y). Its feet end at
/// `y + COUCH_H`.
pub fn draw_couch(renderer: &mut Renderer, x: i32, y: i32) {
    let line = |r: &mut Renderer, x0: i32, y0: i32, x1: i32, y1: i32| {
        r.draw_line(Point::new(x + x0, y + y0), Point::new(x + x1, y + y1));
    };
    let (l, r) = (COUCH_ARM_W, COUCH_W - COUCH_ARM_W - 1);
    renderer.draw_sprite(&COUCH_ARM, Point::new(x, y), SpriteOpts::default());
    renderer.draw_sprite(
        &COUCH_ARM,
        Point::new(x + COUCH_W - COUCH_ARM_W, y),
        SpriteOpts { mirror_h: true, ..Default::default() },
    );
    // Three back cushions, with a notch where each pair meets.
    for (cx, dir) in [(l + 21, 1), (r - 21, -1)] {
        renderer.draw_pixel(Point::new(x + cx - dir, y + 1), true);
        renderer.draw_pixel(Point::new(x + cx + dir, y + 1), true);
        line(renderer, cx, 2, cx, COUCH_SEAT_Y - 1);
    }
    line(renderer, l, 0, l + 19, 0);
    line(renderer, l + 23, 0, r - 23, 0);
    line(renderer, r - 19, 0, r, 0);
    // Seat cushions, split just outboard of the back seams.
    line(renderer, l, COUCH_SEAT_Y, r, COUCH_SEAT_Y);
    line(renderer, l + 20, COUCH_SEAT_Y + 1, l + 20, COUCH_SEAT_Y + 4);
    line(renderer, r - 20, COUCH_SEAT_Y + 1, r - 20, COUCH_SEAT_Y + 4);
    line(renderer, l, COUCH_SEAT_Y + 5, r, COUCH_SEAT_Y + 5);
    line(renderer, l, COUCH_BASE_Y, r, COUCH_BASE_Y);
}

#[cfg(test)]
mod fridge_tests {
    use super::*;

    #[test]
    fn fridge_layouts_stay_on_the_doors() {
        let check = |items: &[(i8, i8); 7]| {
            for (i, (s, &(x, y))) in FRIDGE_ITEMS.iter().zip(items.iter()).enumerate() {
                let (x, y, w, h) = (x as i32, y as i32, s.width as i32, s.height as i32);
                assert!(
                    FRIDGE_DOORS.iter().any(|&(x0, y0, x1, y1)| x >= x0 && y >= y0 && x + w - 1 <= x1 && y + h - 1 <= y1),
                    "item {} at {:?} is off its door",
                    i,
                    (x, y)
                );
                for (o, &(ox, oy)) in FRIDGE_ITEMS.iter().zip(items.iter()).skip(i + 1) {
                    let (ox, oy) = (ox as i32, oy as i32);
                    assert!(
                        x + w <= ox || ox + o.width as i32 <= x || y + h <= oy || oy + o.height as i32 <= y,
                        "items overlap"
                    );
                }
            }
        };
        check(&FRIDGE_DEFAULT);
        for seed in 0..500u64 {
            check(&FridgeLayout::from_seed(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)).items);
        }
    }
}

// Kitchen bin, as an open body and a lid. Stack the lid so its bottom row
// lands on the body's top row (one line, not two): a lidded bin is
// TRASHCAN_LIDDED_H tall. Fill masks cover them edge to edge so they hide
// whatever is behind them.
const TRASHCAN_BODY_FRAMES: &[&[u8]] = &[
    b"\xff\xff\xf8\x6a\xaa\xb0\x40\x00\x10\x40\x00\x10\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x22\x22\x20\x20\x00\x20\x20\x00\x20\x38\x00\xe0\x1f\xff\xc0",
];
const TRASHCAN_BODY_FILL_FRAMES: &[&[u8]] = &[
    b"\xff\xff\xf8\x7f\xff\xf0\x7f\xff\xf0\x7f\xff\xf0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x3f\xff\xe0\x1f\xff\xc0",
];
pub static TRASHCAN_BODY: Sprite = Sprite {
    width: 21,
    height: 22,
    frames: TRASHCAN_BODY_FRAMES,
    fill_frames: Some(TRASHCAN_BODY_FILL_FRAMES),
};

const TRASHCAN_LID_FRAMES: &[&[u8]] = &[b"\x01\xfc\x00\x7f\x07\xf0\x80\x00\x08\x80\x00\x08\xff\xff\xf8"];
const TRASHCAN_LID_FILL_FRAMES: &[&[u8]] = &[b"\x01\xfc\x00\x7f\xff\xf0\xff\xff\xf8\xff\xff\xf8\xff\xff\xf8"];
pub static TRASHCAN_LID: Sprite = Sprite {
    width: 21,
    height: 5,
    frames: TRASHCAN_LID_FRAMES,
    fill_frames: Some(TRASHCAN_LID_FILL_FRAMES),
};

pub const TRASHCAN_LIDDED_H: i32 = TRASHCAN_BODY.height as i32 + TRASHCAN_LID.height as i32 - 1;

/// A lidded bin standing on `floor_y` (the row below its bottom pixel).
pub fn draw_trashcan(renderer: &mut Renderer, x: i32, floor_y: i32) {
    let body_y = floor_y - TRASHCAN_BODY.height as i32;
    renderer.draw_sprite(&TRASHCAN_BODY, Point::new(x, body_y), SpriteOpts::default());
    renderer.draw_sprite(&TRASHCAN_LID, Point::new(x, body_y - TRASHCAN_LID.height as i32 + 1), SpriteOpts::default());
}

// Paper bag, 42x22, lying on its side: the open mouth and its curled cuff
// on the left, the folded bottom on the right.
const PAPER_BAG_FRAMES: &[&[u8]] = &[
    b"\x07\xf0\x00\x0f\xfc\x00\x04\xcc\x00\xf0\x42\x00\x02\x33\x0f\x00\x22\x00\x04\x0f\xf0\x00\x22\x00\x04\x02\x00\x00\x22\x00\x02\x02\x00\x00\x21\x00\x1c\x02\x00\x00\x21\x00\x7c\x04\x00\x00\x11\x00\xe4\x00\x40\x00\x11\x00\x82\x04\x00\x00\x11\x00\x82\x04\x40\x00\x09\x00\x82\x04\x00\x00\x08\x80\x84\x00\x40\x00\x08\x80\x9c\x08\x40\x00\x08\x80\xfe\x08\x41\x00\x08\x80\x64\x00\x20\x00\x08\x80\x04\x11\x22\x08\x08\x80\x02\x01\x22\x08\x04\x80\x02\x00\xa4\x04\x04\x40\x04\x08\xf4\x04\x04\x40\x04\x07\x8c\x07\x04\x40\x07\xf8\x03\xf8\xff\xc0",
];
pub static PAPER_BAG: Sprite = Sprite {
    width: 42,
    height: 22,
    frames: PAPER_BAG_FRAMES,
    fill_frames: None,
};

// Laundry basket, 27x20: a rolled rim over a woven body.
const BASKET_FRAMES: &[&[u8]] = &[
    b"\x6b\xff\xfa\xc0\x94\x00\x05\x20\x94\x00\x05\x20\x6b\xff\xfa\xc0\x40\x40\x40\x40\x40\x40\x40\x40\x40\x3f\x80\x40\x51\x00\x11\x40\x40\x00\x00\x40\x44\x44\x44\x40\x40\x00\x00\x40\x51\x11\x11\x40\x40\x00\x00\x40\x44\x44\x44\x40\x40\x00\x00\x40\x51\x11\x11\x40\x40\x00\x00\x40\x24\x44\x44\x80\x10\x00\x01\x00\x1f\xff\xff\x00",
];
const BASKET_FILL_FRAMES: &[&[u8]] = &[
    b"\x7f\xff\xff\xc0\xff\xff\xff\xe0\xff\xff\xff\xe0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x7f\xff\xff\xc0\x3f\xff\xff\x80\x1f\xff\xff\x00\x1f\xff\xff\x00",
];
pub static BASKET: Sprite = Sprite {
    width: 27,
    height: 20,
    frames: BASKET_FRAMES,
    fill_frames: Some(BASKET_FILL_FRAMES),
};
