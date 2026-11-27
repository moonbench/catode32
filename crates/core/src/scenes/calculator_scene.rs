//! Calculator tool. Type an expression on a 5x4 keypad; precedence,
//! exponents and parentheses are handled by [`crate::calc`].
//!
//! Layout (128x64):
//!   y=0..9    expression, 6x10 font, scrolled to keep the end visible
//!   y=11..20  answer after `=` (2x digits) or a live preview, right-aligned
//!   y=22..61  keypad, five 25px columns by four 10px rows
//!
//! Keypad digits use the 6x10 font; the operators get their own 7x7
//! symbols so they read as clearly as the digits. D-pad moves, A presses,
//! B deletes a character.

use embedded_graphics::prelude::{Point, Size};
use heapless::String;

use crate::{
    calc::{Calculator, Key, MAX_EXPR},
    context::GameContext,
    input::{Button, Buttons},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::tiny_font,
};

const COLS: usize = 5;
const ROWS: usize = 4;
const KEY_W: i32 = 25;
const KEY_H: i32 = 10;
const PAD_X: i32 = 1;
const PAD_Y: i32 = 22;

const CHAR_W: i32 = 6;
const RIGHT_X: i32 = 126;
/// Characters of expression that fit in the 6x10 font.
const EXPR_VISIBLE: usize = 21;

const SYM: u16 = 7;
const PLUS: [u8; 7] = [0x10, 0x10, 0x10, 0xFE, 0x10, 0x10, 0x10];
const MINUS: [u8; 7] = [0x00, 0x00, 0x00, 0xFE, 0x00, 0x00, 0x00];
const TIMES: [u8; 7] = [0x82, 0x44, 0x28, 0x10, 0x28, 0x44, 0x82];
const DIVIDE: [u8; 7] = [0x10, 0x10, 0x00, 0xFE, 0x00, 0x10, 0x10];
const EQUALS: [u8; 7] = [0x00, 0x00, 0xFE, 0x00, 0xFE, 0x00, 0x00];
const CARET: [u8; 7] = [0x10, 0x28, 0x44, 0x82, 0x00, 0x00, 0x00];
const DOT: [u8; 7] = [0x00, 0x00, 0x00, 0x00, 0x38, 0x38, 0x38];

#[derive(Clone, Copy)]
enum Label {
    Text(&'static str),
    Symbol(&'static [u8; 7]),
}

use Label::{Symbol, Text};

const KEYS: [[(Label, Key); COLS]; ROWS] = [
    [(Text("7"), Key::Digit(7)), (Text("8"), Key::Digit(8)), (Text("9"), Key::Digit(9)), (Text("("), Key::Open), (Text(")"), Key::Close)],
    [(Text("4"), Key::Digit(4)), (Text("5"), Key::Digit(5)), (Text("6"), Key::Digit(6)), (Symbol(&TIMES), Key::Mul), (Symbol(&DIVIDE), Key::Div)],
    [(Text("1"), Key::Digit(1)), (Text("2"), Key::Digit(2)), (Text("3"), Key::Digit(3)), (Symbol(&PLUS), Key::Add), (Symbol(&MINUS), Key::Sub)],
    [(Text("0"), Key::Digit(0)), (Symbol(&DOT), Key::Dot), (Symbol(&CARET), Key::Pow), (Text("C"), Key::Clear), (Symbol(&EQUALS), Key::Equals)],
];

pub struct CalculatorScene {
    calc: Calculator,
    row: usize,
    col: usize,
}

impl CalculatorScene {
    pub fn new() -> Self {
        Self { calc: Calculator::new(), row: 3, col: 4 }
    }
}

impl Scene for CalculatorScene {
    fn update(&mut self, _ctx: &mut GameContext, buttons: &mut Buttons, _dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        if buttons.was_just_pressed(Button::Up) {
            self.row = (self.row + ROWS - 1) % ROWS;
        }
        if buttons.was_just_pressed(Button::Down) {
            self.row = (self.row + 1) % ROWS;
        }
        if buttons.was_just_pressed(Button::Left) {
            self.col = (self.col + COLS - 1) % COLS;
        }
        if buttons.was_just_pressed(Button::Right) {
            self.col = (self.col + 1) % COLS;
        }
        if buttons.was_just_pressed(Button::A) {
            self.calc.press(KEYS[self.row][self.col].1);
        }
        if buttons.was_just_pressed(Button::B) {
            self.calc.backspace();
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        // Expression with real times/divide signs, scrolled so the newest
        // characters stay visible.
        // × and ÷ are two bytes each in UTF-8.
        let mut expr: String<{ MAX_EXPR * 2 }> = String::new();
        for c in self.calc.expr().chars() {
            let _ = expr.push(match c {
                '*' => '×',
                '/' => '÷',
                c => c,
            });
        }
        let len = expr.chars().count();
        if len > EXPR_VISIBLE {
            let skip = len - (EXPR_VISIBLE - 1);
            let start = expr.char_indices().nth(skip).map_or(expr.len(), |(i, _)| i);
            r.draw_text("<", Point::new(0, 0));
            r.draw_text(&expr[start..], Point::new(CHAR_W, 0));
        } else {
            r.draw_text(&expr, Point::new(0, 0));
        }

        // The answer at double size once `=` is pressed, else a preview.
        let result = self.calc.result_text();
        if !result.is_empty() {
            if self.calc.is_evaluated() {
                let w = tiny_font::str_width(result, 2);
                tiny_font::draw_str_scaled(r, result, RIGHT_X - w, 11, 2, true);
            } else {
                let mut preview: String<16> = String::new();
                let _ = preview.push('=');
                let _ = preview.push_str(result);
                let w = preview.chars().count() as i32 * CHAR_W;
                r.draw_text(&preview, Point::new(RIGHT_X + 1 - w, 11));
            }
        }

        r.draw_line(Point::new(0, 21), Point::new(127, 21));

        for (ri, row) in KEYS.iter().enumerate() {
            for (ci, &(label, _)) in row.iter().enumerate() {
                let x = PAD_X + ci as i32 * KEY_W;
                let y = PAD_Y + ri as i32 * KEY_H;
                let selected = ri == self.row && ci == self.col;
                if selected {
                    r.draw_rect(Point::new(x + 2, y), Size::new(KEY_W as u32 - 4, KEY_H as u32 - 1), true);
                }
                match label {
                    Text(t) => {
                        let p = Point::new(x + (KEY_W - CHAR_W) / 2, y);
                        if selected {
                            r.draw_text_inverted(t, p);
                        } else {
                            r.draw_text(t, p);
                        }
                    }
                    Symbol(bits) => {
                        let opts = SpriteOpts {
                            transparent: true,
                            transparent_color: selected,
                            invert: selected,
                            ..SpriteOpts::default()
                        };
                        let p = Point::new(x + (KEY_W - SYM as i32) / 2, y + 1);
                        r.draw_sprite_raw(bits, SYM, SYM, p, opts);
                    }
                }
            }
        }
    }
}
