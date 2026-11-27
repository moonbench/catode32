//! Word wrapping for short text blocks, without allocating: lines are
//! slices of the original string.

use embedded_graphics::prelude::Point;
use heapless::Vec;

use crate::render::Renderer;

/// Advance of FONT_4X6, as drawn by `Renderer::draw_text_small`.
pub const SMALL_CHAR_W: i32 = 4;
/// Advance of the 6x10 font, as drawn by `Renderer::draw_text`.
pub const CHAR_W: i32 = 6;

/// Break `text` at spaces into lines of at most `max_chars` characters.
/// A single word longer than that gets a line to itself. Lines past `N`
/// are dropped.
pub fn wrap<const N: usize>(text: &str, max_chars: usize) -> Vec<&str, N> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut start = 0;
        let mut end = 0;
        let mut len = 0;
        for (i, word) in word_spans(paragraph) {
            let wlen = word.chars().count();
            if len == 0 {
                start = i;
                end = i + word.len();
                len = wlen;
            } else if len + 1 + wlen <= max_chars {
                end = i + word.len();
                len += 1 + wlen;
            } else {
                if lines.push(&paragraph[start..end]).is_err() {
                    return lines;
                }
                start = i;
                end = i + word.len();
                len = wlen;
            }
        }
        if lines.push(&paragraph[start..end]).is_err() {
            return lines;
        }
    }
    lines
}

/// Words with their byte offsets.
fn word_spans(s: &str) -> impl Iterator<Item = (usize, &str)> {
    s.split(' ')
        .scan(0usize, |pos, w| {
            let start = *pos;
            *pos += w.len() + 1;
            Some((start, w))
        })
        .filter(|(_, w)| !w.is_empty())
}

/// Draw wrapped lines centred on `cx` in the small font, starting at `y`.
pub fn draw_centered_small(r: &mut Renderer, lines: &[&str], cx: i32, y: i32, line_h: i32) {
    for (i, line) in lines.iter().enumerate() {
        let w = line.chars().count() as i32 * SMALL_CHAR_W;
        r.draw_text_small(line, Point::new(cx - w / 2, y + i as i32 * line_h));
    }
}

/// Draw wrapped lines centred on `cx` in the regular 6x10 font.
pub fn draw_centered(r: &mut Renderer, lines: &[&str], cx: i32, y: i32, line_h: i32) {
    for (i, line) in lines.iter().enumerate() {
        let w = line.chars().count() as i32 * CHAR_W;
        r.draw_text(line, Point::new(cx - w / 2, y + i as i32 * line_h));
    }
}
