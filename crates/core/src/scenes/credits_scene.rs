//! Credits scene, a scrollable acknowledgements page.
//!
//! Up/Down scroll, B or Menu2 returns to the last main scene. Paragraphs are
//! wrapped on-device at 20 chars per line; pre-formatted elements (title,
//! dividers, the cat ASCII) are passed through verbatim as `Raw` blocks.

use core::fmt::Write;

use embedded_graphics::prelude::Point;
use heapless::{String, Vec};

use crate::{
    assets::icons,
    context::GameContext,
    input::{Button, Buttons},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
};

const LINE_H: i32 = 10;
/// 64 / LINE_H = 6 lines on screen at a time.
const VISIBLE: usize = 6;
/// 20 chars x 6 px = 120 px, leaving 8 px for scroll arrows on the right.
const CPL: usize = 20;
const LINE_CAP: usize = 24;
const LINES_CAP: usize = 128;

const YEAR: &str = "2026";

enum Block {
    /// Word-wrapped paragraph. Newlines inside are paragraph breaks (each
    /// wrapped independently).
    Paragraph(&'static str),
    /// Pre-formatted line(s), passed through verbatim. Split on '\n'.
    Raw(&'static str),
    /// One blank spacer line.
    Blank,
}

const BLOCKS: &[Block] = &[
    Block::Raw("     Catode 32"),
    Block::Raw("===================="),
    Block::Blank,
    Block::Paragraph("Thank you for playing with this virtual pet!"),
    Block::Raw("____________________"),
    Block::Blank,
    Block::Paragraph("Code & Design:"),
    Block::Paragraph("Moonbench"),
    Block::Raw("____________________"),
    Block::Blank,
    Block::Paragraph(
        "This pet was inspired by my two wonderful cats. My sweet and very intelligent tortoiseshell girl, Bean, and her cute, rambunctious, black sister, Juno.",
    ),
    Block::Blank,
    Block::Paragraph(
        "I started to create the art for this project shortly after adopting Bean, and was finally inspired to make the toy into reality after adopting Juno.",
    ),
    Block::Blank,
    Block::Paragraph(
        "Their sweet personalities inspired the pet in this virtual toy. It wouldn't exist without them and their love.",
    ),
    Block::Blank,
    Block::Raw("____________________"),
    Block::Blank,
    Block::Paragraph(
        "I want to give thanks to my friends and family who helped test this and who provided their encouragment and support.",
    ),
    Block::Raw("____________________"),
    Block::Blank,
    Block::Paragraph(
        "I also want to give thanks to the open-source community for the projects that this was built upon.",
    ),
    Block::Blank,
    Block::Paragraph(
        "In that spirit, the source code for this pet is available. Just search for \"catode32\".",
    ),
    Block::Raw("____________________"),
    Block::Blank,
    Block::Paragraph(
        "If you have the time and space, then I hope you will consider adopting a real pet from your local animal shelter, and give them lots of love too!",
    ),
    Block::Raw("____________________"),
    Block::Blank,
    Block::Blank,
    Block::Raw("    |\\          /|"),
    Block::Raw("    | \\________/ |"),
    Block::Raw("    |            |"),
    Block::Raw("    |  /\\    /\\  |"),
    Block::Raw("    ==          =="),
    Block::Raw("     \\   ,__,   /"),
    Block::Raw("      \\________/"),
    Block::Blank,
    Block::Raw("===================="),
    Block::Blank,
];

pub struct CreditsScene {
    lines: Vec<String<LINE_CAP>, LINES_CAP>,
    scroll: usize,
    max_scroll: usize,
}

impl CreditsScene {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            scroll: 0,
            max_scroll: 0,
        }
    }

    fn build(&mut self) {
        self.lines.clear();
        for block in BLOCKS {
            match block {
                Block::Paragraph(text) => wrap_paragraph(&mut self.lines, text, CPL),
                Block::Raw(text) => {
                    for raw in text.split('\n') {
                        push_truncated(&mut self.lines, raw);
                    }
                }
                Block::Blank => {
                    let _ = self.lines.push(String::new());
                }
            }
            if self.lines.is_full() {
                break;
            }
        }

        // Footer: "Catode32 - YEAR" + "vX.Y.Z" from Cargo.
        let mut footer: String<LINE_CAP> = String::new();
        let _ = write!(&mut footer, "Catode32 - {YEAR}");
        let _ = self.lines.push(footer);
        let mut version: String<LINE_CAP> = String::new();
        let _ = write!(&mut version, "v{}", env!("CARGO_PKG_VERSION"));
        let _ = self.lines.push(version);

        self.max_scroll = self.lines.len().saturating_sub(VISIBLE);
    }
}

impl Scene for CreditsScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.scroll = 0;
        self.build();
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::B) || buttons.was_just_pressed(Button::Menu2) {
            return Some(ctx.last_main_scene);
        }
        if buttons.was_just_pressed(Button::Up) && self.scroll > 0 {
            self.scroll -= 1;
        } else if buttons.was_just_pressed(Button::Down) && self.scroll < self.max_scroll {
            self.scroll += 1;
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        let total = self.lines.len();
        for i in 0..VISIBLE {
            let idx = self.scroll + i;
            if idx >= total {
                break;
            }
            renderer.draw_text(self.lines[idx].as_str(), Point::new(0, (i as i32) * LINE_H));
        }

        if self.scroll > 0 {
            renderer.draw_sprite_raw(
                icons::UP_ARROW,
                icons::ARROW_W,
                icons::ARROW_H,
                Point::new(119, 0),
                SpriteOpts::default(),
            );
        }
        if self.scroll < self.max_scroll {
            renderer.draw_sprite_raw(
                icons::DOWN_ARROW,
                icons::ARROW_W,
                icons::ARROW_H,
                Point::new(119, 56),
                SpriteOpts::default(),
            );
        }
    }
}

fn push_truncated(out: &mut Vec<String<LINE_CAP>, LINES_CAP>, text: &str) {
    let mut s: String<LINE_CAP> = String::new();
    for c in text.chars().take(LINE_CAP) {
        if s.push(c).is_err() {
            break;
        }
    }
    let _ = out.push(s);
}

/// Greedy word-wrap to `cpl` chars per line. Words longer than the line are
/// hyphenated by emitting `cpl - 1` chars followed by '-' and continuing.
fn wrap_paragraph(out: &mut Vec<String<LINE_CAP>, LINES_CAP>, text: &str, cpl: usize) {
    let cpl = cpl.min(LINE_CAP);
    let mut current: String<LINE_CAP> = String::new();
    for raw_word in text.split(' ') {
        let mut word = raw_word;
        while word.len() > cpl && cpl > 1 {
            if !current.is_empty() {
                let _ = out.push(current.clone());
                current.clear();
            }
            let mut frag: String<LINE_CAP> = String::new();
            for c in word.chars().take(cpl - 1) {
                let _ = frag.push(c);
            }
            let _ = frag.push('-');
            let _ = out.push(frag);
            word = &word[cpl - 1..];
        }
        let needs_space = !current.is_empty();
        let extra = if needs_space { 1 } else { 0 } + word.len();
        if current.len() + extra <= cpl {
            if needs_space {
                let _ = current.push(' ');
            }
            let _ = current.push_str(word);
        } else {
            if !current.is_empty() {
                let _ = out.push(current.clone());
                current.clear();
            }
            let _ = current.push_str(word);
        }
    }
    if !current.is_empty() {
        let _ = out.push(current);
    }
}
