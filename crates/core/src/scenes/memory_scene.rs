//! Memory minigame. Find matching pairs. Board grows over rounds:
//! round 1: 9x6, round 2: 10x7, round 3+: 11x8.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::character::PoseId,
    context::{GameContext, StatId},
    entities::character::Character,
    input::{Button, Buttons},
    rand::xorshift32,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::popup::Popup,
};

const MAX_COLS: usize = 11;
const MAX_ROWS: usize = 8;
const CELL: i32 = 8;
const MAX_TOTAL: usize = MAX_COLS * MAX_ROWS; // 88
const MAX_PAIRS: usize = MAX_TOTAL / 2;       // 44

const BOARD_SIZES: [(usize, usize); 3] = [(9, 6), (10, 7), (11, 8)];

// Cell states.
const HIDDEN: u8 = 0;
const FLIPPED: u8 = 1;
const SOLVED_SHOWING: u8 = 2;
const SOLVED_BLANK: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Mismatch,
    Win,
}

const MISMATCH_DELAY: f32 = 1.0;
const MATCH_SHOW_DELAY: f32 = 0.75;
const WIN_RESET_DELAY: f32 = 3.0;

// 44 glyphs, 4 bytes each. High nibble of each byte encodes one row of 4 pixels.
const GLYPHS: [u8; MAX_PAIRS * 4] = [
    0x90, 0x90, 0x00, 0x90, // memory_1
    0x90, 0xd0, 0xb0, 0x90, // memory_2
    0xf0, 0x90, 0xf0, 0x90, // memory_3
    0xc0, 0xe0, 0xb0, 0x90, // memory_4
    0xc0, 0x40, 0xf0, 0x90, // memory_5
    0xd0, 0xd0, 0xd0, 0xd0, // memory_6
    0xb0, 0x80, 0x10, 0xd0, // memory_7
    0xc0, 0xc0, 0x30, 0x30, // memory_8
    0xd0, 0x40, 0x20, 0xb0, // memory_9
    0x40, 0xf0, 0x10, 0x90, // memory_10
    0xf0, 0x00, 0xf0, 0x60, // memory_11
    0x50, 0xa0, 0x50, 0xa0, // memory_12
    0x80, 0xf0, 0x80, 0xf0, // memory_13
    0xe0, 0xb0, 0x10, 0xf0, // memory_14
    0xf0, 0xf0, 0xf0, 0x60, // memory_15
    0x60, 0xf0, 0xf0, 0x90, // memory_16
    0xd0, 0x50, 0x70, 0xc0, // memory_17
    0x90, 0x90, 0x90, 0xf0, // memory_18
    0xf0, 0x90, 0x90, 0x90, // memory_19
    0xf0, 0xc0, 0xc0, 0xf0, // memory_20
    0xf0, 0x70, 0x70, 0xf0, // memory_21
    0x70, 0x50, 0x50, 0xf0, // memory_22
    0xe0, 0xb0, 0xa0, 0xf0, // memory_23
    0xb0, 0x90, 0x90, 0xb0, // memory_24
    0xd0, 0x90, 0x90, 0xf0, // memory_25
    0xd0, 0xd0, 0x10, 0xf0, // memory_26
    0xf0, 0xb0, 0x80, 0xf0, // memory_27
    0xd0, 0x90, 0x90, 0xb0, // memory_28
    0x40, 0xf0, 0x40, 0x40, // memory_29
    0x10, 0x30, 0x70, 0xf0, // memory_30
    0x90, 0xf0, 0x10, 0x10, // memory_31
    0x60, 0xf0, 0x60, 0xf0, // memory_32
    0xb0, 0xf0, 0x20, 0x30, // memory_33
    0xf0, 0x60, 0x60, 0x60, // memory_34
    0x90, 0x80, 0x80, 0x70, // memory_35
    0x90, 0xf0, 0x10, 0xf0, // memory_36
    0xa0, 0xa0, 0xf0, 0x20, // memory_37
    0x90, 0x60, 0x60, 0x90, // memory_38
    0x80, 0xf0, 0xe0, 0xe0, // memory_39
    0x10, 0xf0, 0x70, 0x70, // memory_40
    0x90, 0x50, 0x20, 0xd0, // memory_41
    0xf0, 0x30, 0x50, 0x90, // memory_42
    0x70, 0xc0, 0xb0, 0xb0, // memory_43
    0x90, 0xf0, 0xf0, 0x60, // memory_44
];

/// Expand a 4-byte glyph (high nibble per row) into an 8x8 raster using 2x2 blocks.
fn make_icon(glyph: &[u8; 4]) -> [u8; 8] {
    let mut data = [0u8; 8];
    for row in 0..4 {
        let bits = glyph[row] >> 4;
        let mut b: u8 = 0;
        for col in 0..4 {
            if bits & (1 << (3 - col)) != 0 {
                b |= 0x3 << (6 - col * 2);
            }
        }
        data[row * 2] = b;
        data[row * 2 + 1] = b;
    }
    data
}

pub struct MemoryScene {
    character: Character,
    win_popup: Popup,

    icons: [[u8; 8]; MAX_PAIRS],
    cell_icons: [u8; MAX_TOTAL],
    cell_state: [u8; MAX_TOTAL],

    cols: usize,
    rows: usize,
    total: usize,
    pairs: usize,
    panel_x: i32,

    cursor: usize,
    first_flipped: i16,
    second_flipped: i16,
    score: i32,
    solved_count: usize,
    total_solved: usize,
    game_count: usize,

    state: State,
    mismatch_timer: f32,
    match_show_timer: f32,
    recently_solved: [i16; 2],
    win_timer: f32,

    /// Best score snapshot read at scene entry / win, used when formatting the
    /// win popup (which doesn't have access to `&GameContext`).
    pending_best: i32,
}

impl MemoryScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(102, 63)),
            win_popup: Popup::new(14, 12, 100, 40),

            icons: [[0u8; 8]; MAX_PAIRS],
            cell_icons: [0u8; MAX_TOTAL],
            cell_state: [HIDDEN; MAX_TOTAL],

            cols: MAX_COLS,
            rows: MAX_ROWS,
            total: MAX_TOTAL,
            pairs: MAX_PAIRS,
            panel_x: MAX_COLS as i32 * CELL,

            cursor: 0,
            first_flipped: -1,
            second_flipped: -1,
            score: 0,
            solved_count: 0,
            total_solved: 0,
            game_count: 0,

            state: State::Playing,
            mismatch_timer: 0.0,
            match_show_timer: 0.0,
            recently_solved: [-1, -1],
            win_timer: 0.0,

            pending_best: -1,
        }
    }

    fn generate_icons(&mut self, rng: &mut u32) {
        let n = MAX_PAIRS;
        let mut indices: [u8; MAX_PAIRS] = [0; MAX_PAIRS];
        for (i, slot) in indices.iter_mut().enumerate() {
            *slot = i as u8;
        }
        // Fisher-Yates over the glyph index list.
        for i in (1..n).rev() {
            let j = (xorshift32(rng) % (i as u32 + 1)) as usize;
            indices.swap(i, j);
        }
        for slot in 0..self.pairs {
            let gi = indices[slot] as usize;
            let glyph: &[u8; 4] = (&GLYPHS[gi * 4..gi * 4 + 4]).try_into().unwrap();
            self.icons[slot] = make_icon(glyph);
        }
    }

    fn init_game(&mut self, rng: &mut u32) {
        self.total_solved += self.solved_count;

        let (cols, rows) = BOARD_SIZES[self.game_count.min(BOARD_SIZES.len() - 1)];
        self.cols = cols;
        self.rows = rows;
        self.total = cols * rows;
        self.pairs = self.total / 2;
        self.panel_x = cols as i32 * CELL;
        self.game_count += 1;

        self.generate_icons(rng);

        // Two copies of each icon index, shuffled.
        for i in 0..self.pairs {
            self.cell_icons[i] = i as u8;
            self.cell_icons[i + self.pairs] = i as u8;
        }
        for i in (1..self.total).rev() {
            let j = (xorshift32(rng) % (i as u32 + 1)) as usize;
            self.cell_icons.swap(i, j);
        }

        for i in 0..MAX_TOTAL {
            self.cell_state[i] = HIDDEN;
        }
        self.cursor = 0;
        self.first_flipped = -1;
        self.second_flipped = -1;
        self.score = 0;
        self.solved_count = 0;
        self.state = State::Playing;
        self.mismatch_timer = 0.0;
        self.match_show_timer = 0.0;
        self.recently_solved = [-1, -1];
        self.win_timer = 0.0;

        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn try_flip(&mut self, idx: usize) {
        let st = self.cell_state[idx];
        if st == SOLVED_SHOWING || st == SOLVED_BLANK || st == FLIPPED {
            return;
        }

        self.cell_state[idx] = FLIPPED;

        if self.first_flipped == -1 {
            self.first_flipped = idx as i16;
            return;
        }

        let first = self.first_flipped as usize;
        if self.cell_icons[idx] == self.cell_icons[first] {
            // Match: clear any previously-showing pair first.
            for &i in &self.recently_solved {
                if i >= 0 {
                    self.cell_state[i as usize] = SOLVED_BLANK;
                }
            }
            self.cell_state[idx] = SOLVED_SHOWING;
            self.cell_state[first] = SOLVED_SHOWING;
            self.recently_solved = [first as i16, idx as i16];
            self.match_show_timer = MATCH_SHOW_DELAY;
            self.solved_count += 2;
            self.first_flipped = -1;
            self.character.set_pose(PoseId::SittingSideHappy);

            if self.solved_count == self.total {
                self.enter_win_state();
            }
        } else {
            // Mismatch.
            self.second_flipped = idx as i16;
            self.score += 1;
            self.state = State::Mismatch;
            self.mismatch_timer = 0.0;
            self.character.set_pose(PoseId::SittingSideAnnoyed);
        }
    }

    fn enter_win_state(&mut self) {
        // Best is stored on `GameContext` and snapshotted into `pending_best`
        // by `enter()`. Update it locally so the popup reflects the new best;
        // `update()` writes the real value back to context on the win frame.
        let prev_best = self.pending_best;
        let new_best = if prev_best < 0 || self.score < prev_best {
            self.score
        } else {
            prev_best
        };
        self.pending_best = new_best;

        let rating = if self.score < 50 {
            t!("Incredible!")
        } else if self.score < 100 {
            t!("Amazing!")
        } else if self.score < 150 {
            t!("Impressive!")
        } else if self.score < 250 {
            t!("Well done!")
        } else if self.score < 500 {
            t!("Not bad!")
        } else {
            t!("Phwew!")
        };

        let mut text: String<96> = String::new();
        let _ = text.push_str(rating);
        let mut score_buf: String<8> = String::new();
        let _ = write!(score_buf, "{}", self.score);
        crate::i18n::substitute(&mut text, t!("\n\nScore: {n}"), &[("n", score_buf.as_str())]);
        let _ = text.push('\n');
        let mut best_buf: String<8> = String::new();
        let _ = write!(best_buf, "{}", new_best);
        crate::i18n::substitute(&mut text, t!("Best: {n}"), &[("n", best_buf.as_str())]);
        self.win_popup.set_text(text.as_str(), false, true);

        self.state = State::Win;
        self.win_timer = 0.0;
    }
}

impl Scene for MemoryScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.total_solved = 0;
        self.solved_count = 0;
        self.game_count = 0;
        self.pending_best = ctx.memory_best_score;
        let mut rng = ctx.rng;
        self.init_game(&mut rng);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let total = self.total_solved + self.solved_count;
        if total == 0 {
            return;
        }
        let progress = ((total as f32 / MAX_TOTAL as f32).min(1.0)).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 5.0 * progress),
            (StatId::Focus,        4.0 * progress),
            (StatId::Sociability,  3.0 * progress + 0.5),
            (StatId::Loyalty,      1.0 * progress),
        ]);
        let coins = (5.0 * progress) as i32;
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

        self.character.animate(dt);

        // Handle timers first so input runs against the latest state.
        if self.match_show_timer > 0.0 && self.state != State::Win {
            self.match_show_timer -= dt;
            if self.match_show_timer <= 0.0 {
                for &i in &self.recently_solved {
                    if i >= 0 {
                        self.cell_state[i as usize] = SOLVED_BLANK;
                    }
                }
                self.recently_solved = [-1, -1];
                if self.state != State::Mismatch {
                    self.character.set_pose(PoseId::SittingSideNeutral);
                }
            }
        }

        match self.state {
            State::Mismatch => {
                self.mismatch_timer += dt;
                if self.mismatch_timer >= MISMATCH_DELAY {
                    if self.first_flipped >= 0 {
                        self.cell_state[self.first_flipped as usize] = HIDDEN;
                    }
                    if self.second_flipped >= 0 {
                        self.cell_state[self.second_flipped as usize] = HIDDEN;
                    }
                    self.first_flipped = -1;
                    self.second_flipped = -1;
                    self.state = State::Playing;
                    self.character.set_pose(PoseId::SittingSideNeutral);
                }
                return None;
            }
            State::Win => {
                // On win, persist the best score once and then auto-reset.
                if self.score >= 0
                    && (ctx.memory_best_score < 0 || self.score < ctx.memory_best_score)
                {
                    ctx.memory_best_score = self.score;
                }
                self.pending_best = ctx.memory_best_score;

                if buttons.was_just_pressed(Button::A) {
                    let mut rng = ctx.rng;
                    self.init_game(&mut rng);
                    ctx.rng = rng;
                    return None;
                }
                self.win_timer += dt;
                if self.win_timer >= WIN_RESET_DELAY {
                    let mut rng = ctx.rng;
                    self.init_game(&mut rng);
                    ctx.rng = rng;
                }
                return None;
            }
            State::Playing => {}
        }

        // Cursor movement.
        let col = self.cursor % self.cols;
        let row = self.cursor / self.cols;
        if buttons.was_just_pressed(Button::Left) && col > 0 {
            self.cursor -= 1;
        } else if buttons.was_just_pressed(Button::Right) && col < self.cols - 1 {
            self.cursor += 1;
        } else if buttons.was_just_pressed(Button::Up) && row > 0 {
            self.cursor -= self.cols;
        } else if buttons.was_just_pressed(Button::Down) && row < self.rows - 1 {
            self.cursor += self.cols;
        }

        if buttons.was_just_pressed(Button::A) {
            self.try_flip(self.cursor);
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        for i in 0..self.total {
            let col = i % self.cols;
            let row = i / self.cols;
            let cx = col as i32 * CELL;
            let cy = row as i32 * CELL;
            let st = self.cell_state[i];
            let is_cursor = i == self.cursor;

            match st {
                SOLVED_BLANK => {
                    if is_cursor {
                        renderer.draw_rect(
                            Point::new(cx + 2, cy + 2),
                            Size::new(4, 4),
                            false,
                        );
                    }
                }
                HIDDEN => {
                    renderer.draw_rect(
                        Point::new(cx + 1, cy + 1),
                        Size::new(6, 6),
                        is_cursor,
                    );
                }
                _ => {
                    // FLIPPED or SOLVED_SHOWING. Draw the icon.
                    let icon_idx = self.cell_icons[i] as usize;
                    renderer.draw_sprite_raw(
                        &self.icons[icon_idx],
                        8,
                        8,
                        Point::new(cx, cy),
                        SpriteOpts {
                            transparent: true,
                            transparent_color: false,
                            ..Default::default()
                        },
                    );
                    if is_cursor {
                        renderer.draw_rect(
                            Point::new(cx - 2, cy - 2),
                            Size::new(12, 12),
                            false,
                        );
                    }
                }
            }
        }

        // Re-draw current-attempt cells on top so the black background and
        // outline always overlay neighbours.
        let attempt = [
            self.first_flipped,
            if self.state == State::Mismatch { self.second_flipped } else { -1 },
        ];
        for &i in &attempt {
            if i < 0 {
                continue;
            }
            let i = i as usize;
            let cx = (i % self.cols) as i32 * CELL;
            let cy = (i / self.cols) as i32 * CELL;
            renderer.fill_rect_off(Point::new(cx - 2, cy - 2), Size::new(12, 12));
            let icon_idx = self.cell_icons[i] as usize;
            renderer.draw_sprite_raw(
                &self.icons[icon_idx],
                8,
                8,
                Point::new(cx, cy),
                SpriteOpts {
                    transparent: true,
                    transparent_color: false,
                    ..Default::default()
                },
            );
            renderer.draw_rect(
                Point::new(cx - 2, cy - 2),
                Size::new(12, 12),
                false,
            );
        }

        // Right panel: current score.
        let mut s: String<8> = String::new();
        let _ = write!(s, "{}", self.score);
        renderer.draw_text(s.as_str(), Point::new(self.panel_x + 4, 2));

        self.character.draw(renderer, 0);

        if self.state == State::Win {
            self.win_popup.draw(renderer, false);
        }
    }
}
