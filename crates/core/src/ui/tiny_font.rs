//! 4x5 pixel font for digits and arithmetic symbols. Smaller and crisper
//! than FONT_4X6 for number-dense layouts (puzzle grids, the calculator,
//! dice faces). Each glyph is five rows, one byte per row, high nibble,
//! MSB = leftmost pixel.

use embedded_graphics::prelude::{Point, Size};

use crate::render::{Renderer, SpriteOpts};

pub const W: u16 = 4;
pub const H: u16 = 5;
/// Horizontal step between characters at 1x: glyph plus one blank column.
pub const ADVANCE: i32 = W as i32 + 1;

const DIGITS: [[u8; 5]; 10] = [
    [0x60, 0x90, 0x90, 0x90, 0x60], // 0
    [0x20, 0x60, 0x20, 0x20, 0x70], // 1
    [0xE0, 0x10, 0x60, 0x80, 0xF0], // 2
    [0xE0, 0x10, 0x60, 0x10, 0xE0], // 3
    [0x90, 0x90, 0xF0, 0x10, 0x10], // 4
    [0xF0, 0x80, 0xE0, 0x10, 0xE0], // 5
    [0x60, 0x80, 0xE0, 0x90, 0x60], // 6
    [0xF0, 0x10, 0x20, 0x40, 0x40], // 7
    [0x60, 0x90, 0x60, 0x90, 0x60], // 8
    [0x60, 0x90, 0x70, 0x10, 0x60], // 9
];

const PLUS: [u8; 5] = [0x00, 0x40, 0xE0, 0x40, 0x00];
const MINUS: [u8; 5] = [0x00, 0x00, 0xE0, 0x00, 0x00];
const TIMES: [u8; 5] = [0x00, 0xA0, 0x40, 0xA0, 0x00];
const DIVIDE: [u8; 5] = [0x40, 0x00, 0xE0, 0x00, 0x40];
const EQUALS: [u8; 5] = [0x00, 0xE0, 0x00, 0xE0, 0x00];
const DOT: [u8; 5] = [0x00, 0x00, 0x00, 0x00, 0x40];
const LPAREN: [u8; 5] = [0x20, 0x40, 0x40, 0x40, 0x20];
const RPAREN: [u8; 5] = [0x40, 0x20, 0x20, 0x20, 0x40];
const CARET: [u8; 5] = [0x40, 0xA0, 0x00, 0x00, 0x00];
const LESS: [u8; 5] = [0x20, 0x40, 0x80, 0x40, 0x20];
const LOWER_E: [u8; 5] = [0x00, 0x60, 0xF0, 0x80, 0x70];
const UPPER_C: [u8; 5] = [0x70, 0x80, 0x80, 0x80, 0x70];
const UPPER_E: [u8; 5] = [0xF0, 0x80, 0xE0, 0x80, 0xF0];
const LOWER_R: [u8; 5] = [0x00, 0xB0, 0xC0, 0x80, 0x80];
const LOWER_D: [u8; 5] = [0x10, 0x10, 0x70, 0x90, 0x70];
const COLON: [u8; 5] = [0x00, 0x40, 0x00, 0x40, 0x00];

/// Bitmap for `ch`, or `None` if the font has no such glyph. `*` and `/`
/// are accepted as aliases for the multiply and divide signs.
pub fn glyph(ch: char) -> Option<&'static [u8; 5]> {
    Some(match ch {
        '0'..='9' => &DIGITS[ch as usize - '0' as usize],
        '+' => &PLUS,
        '-' => &MINUS,
        '×' | '*' => &TIMES,
        '÷' | '/' => &DIVIDE,
        '=' => &EQUALS,
        '.' => &DOT,
        '(' => &LPAREN,
        ')' => &RPAREN,
        '^' => &CARET,
        '<' => &LESS,
        'e' => &LOWER_E,
        'C' => &UPPER_C,
        'E' => &UPPER_E,
        'r' => &LOWER_R,
        'd' => &LOWER_D,
        ':' => &COLON,
        _ => return None,
    })
}

/// Draw one glyph at 1x. `inverted` draws it dark-on-light, for a cell
/// whose background has already been filled.
pub fn draw_char(r: &mut Renderer, ch: char, pos: Point, inverted: bool) {
    let Some(bits) = glyph(ch) else { return };
    let opts = SpriteOpts {
        transparent: true,
        transparent_color: inverted,
        invert: inverted,
        ..SpriteOpts::default()
    };
    r.draw_sprite_raw(bits, W, H, pos, opts);
}

/// Draw digit 0-9 at 1x.
pub fn draw_digit(r: &mut Renderer, digit: u8, pos: Point, inverted: bool) {
    draw_char(r, (b'0' + digit.min(9)) as char, pos, inverted);
}

/// Draw one glyph with each pixel blown up to `scale`x`scale`. `on` picks
/// the ink colour.
pub fn draw_char_scaled(r: &mut Renderer, ch: char, x: i32, y: i32, scale: u32, on: bool) {
    let Some(bits) = glyph(ch) else { return };
    let s = scale as i32;
    for (gy, row) in bits.iter().enumerate() {
        for gx in 0..W as i32 {
            if row & (0x80 >> gx) != 0 {
                let p = Point::new(x + gx * s, y + gy as i32 * s);
                if on {
                    r.draw_rect(p, Size::new(scale, scale), true);
                } else {
                    r.fill_rect_off(p, Size::new(scale, scale));
                }
            }
        }
    }
}

/// Width in pixels of `s` at `scale`, without the trailing gap.
pub fn str_width(s: &str, scale: u32) -> i32 {
    let n = s.chars().count() as i32;
    if n == 0 {
        0
    } else {
        (n * ADVANCE - 1) * scale as i32
    }
}

/// Draw a string at 1x, left-aligned at `pos`. Unknown characters leave a
/// blank cell.
pub fn draw_str(r: &mut Renderer, s: &str, pos: Point) {
    draw_str_scaled(r, s, pos.x, pos.y, 1, true);
}

/// Draw a string at `scale`, left-aligned at (`x`, `y`).
pub fn draw_str_scaled(r: &mut Renderer, s: &str, x: i32, y: i32, scale: u32, on: bool) {
    let step = ADVANCE * scale as i32;
    for (i, ch) in s.chars().enumerate() {
        if scale == 1 && on {
            draw_char(r, ch, Point::new(x + i as i32 * step, y), false);
        } else {
            draw_char_scaled(r, ch, x + i as i32 * step, y, scale, on);
        }
    }
}
