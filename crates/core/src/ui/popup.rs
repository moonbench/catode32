use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};

use crate::{
    assets::icons,
    render::{Renderer, SpriteOpts},
};

const LINE_HEIGHT: i32 = 10;
const CHAR_WIDTH: i32 = 6;
const MAX_LINE_CHARS: usize = 32;
const MAX_LINES: usize = 16;

pub struct Popup {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub padding: i32,
    lines: Vec<String<MAX_LINE_CHARS>, MAX_LINES>,
    scroll_offset: usize,
    center: bool,
}

impl Popup {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            padding: 4,
            lines: Vec::new(),
            scroll_offset: 0,
            center: false,
        }
    }

    pub fn set_text(&mut self, text: &str, wrap: bool, center: bool) {
        self.center = center;
        self.scroll_offset = 0;
        self.lines.clear();

        if wrap {
            self.wrap_text(text);
        } else {
            for line in text.split('\n') {
                let mut s: String<MAX_LINE_CHARS> = String::new();
                let _ = s.push_str(&line[..line.len().min(MAX_LINE_CHARS)]);
                let _ = self.lines.push(s);
            }
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.scroll_offset < self.max_scroll() {
            self.scroll_offset += 1;
        }
    }

    pub fn visible_lines(&self) -> usize {
        let content_height = self.height as i32 - self.padding * 2;
        (content_height / LINE_HEIGHT).max(0) as usize
    }

    /// Resize the box to show every line of the current text (never shorter
    /// than `min_height`), centred on `center_y` and kept on screen.
    pub fn fit_height(&mut self, min_height: u32, center_y: i32) {
        let needed = self.padding * 2 + self.lines.len() as i32 * LINE_HEIGHT;
        let height = needed.max(min_height as i32);
        self.height = height as u32;
        self.y = (center_y - height / 2).max(0);
    }

    pub fn can_scroll(&self) -> bool {
        self.lines.len() > self.visible_lines()
    }

    pub fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.visible_lines())
    }

    pub fn draw(&self, renderer: &mut Renderer, show_scroll_indicators: bool) {
        renderer.fill_rect_off(Point::new(self.x, self.y), Size::new(self.width, self.height));
        renderer.draw_rect(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height),
            false,
        );

        let vis = self.visible_lines();
        let end = (self.scroll_offset + vis).min(self.lines.len());
        for (i, line) in self.lines[self.scroll_offset..end].iter().enumerate() {
            let line_x = if self.center {
                self.x + (self.width as i32 - line.len() as i32 * CHAR_WIDTH) / 2
            } else {
                self.x + self.padding
            };
            let line_y = self.y + self.padding + (i as i32) * LINE_HEIGHT;
            renderer.draw_text(line.as_str(), Point::new(line_x, line_y));
        }

        if show_scroll_indicators && self.can_scroll() {
            let icon_x = self.x + self.width as i32 - 12;
            if self.scroll_offset > 0 {
                renderer.draw_sprite_raw(
                    icons::UP_ARROW,
                    icons::ARROW_W,
                    icons::ARROW_H,
                    Point::new(icon_x, self.y + 2),
                    SpriteOpts::default(),
                );
            }
            if self.scroll_offset < self.max_scroll() {
                renderer.draw_sprite_raw(
                    icons::DOWN_ARROW,
                    icons::ARROW_W,
                    icons::ARROW_H,
                    Point::new(icon_x, self.y + self.height as i32 - 10),
                    SpriteOpts::default(),
                );
            }
        }
    }

    fn wrap_text(&mut self, text: &str) {
        let chars_per_line =
            ((self.width as i32 - self.padding * 2) / CHAR_WIDTH).max(1) as usize;
        let chars_per_line = chars_per_line.min(MAX_LINE_CHARS);

        for paragraph in text.split('\n') {
            let mut current: String<MAX_LINE_CHARS> = String::new();
            for word in paragraph.split(' ') {
                let needs_space = !current.is_empty();
                let extra = (if needs_space { 1 } else { 0 }) + word.len();
                if current.len() + extra <= chars_per_line {
                    if needs_space {
                        let _ = current.push(' ');
                    }
                    let _ = current.push_str(word);
                } else {
                    if !current.is_empty() {
                        let _ = self.lines.push(current.clone());
                        current.clear();
                    }
                    let _ = current.push_str(&word[..word.len().min(chars_per_line)]);
                }
            }
            let _ = self.lines.push(current);
            if self.lines.is_full() {
                return;
            }
        }
    }
}
