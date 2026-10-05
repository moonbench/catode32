//! Hanjie (Nonogram) minigame. Fill the grid using row/column clues.
//!
//! Layout (128x64):
//!   x=0..23    row clues (right-aligned, single digits at 6px pitch)
//!   x=24..71+  6..9 column grid (8px cells), grows by round
//!   x=100..127 character (shifted right to 108 when grid is 9 wide)
//!   y=0..15    column clues (up to 2 groups, each 8px row)
//!   y=16..63   6-row grid

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
    render::Renderer,
    scene::{Scene, SceneId},
    ui::popup::Popup,
};

const ROWS: usize = 6;
const MAX_COLS: usize = 9;
const BASE_COLS: usize = 6;
const CELL: i32 = 8;

const GRID_X: i32 = 24;
const GRID_Y: i32 = 16;
const ROW_CLUE_PITCH: i32 = 7;

// Cell states.
const UNKNOWN: u8 = 0;
const FILLED: u8 = 1;
const CROSSED: u8 = 2;

const WIN_RESET_DELAY: f32 = 2.5;

// Display constraints.
const MAX_ROW_GROUPS: usize = 3;
const MAX_COL_GROUPS: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Win,
}

/// Run-length groups for a single row/column.
///
/// Stores up to MAX_ROW_GROUPS entries (rows can have more groups than
/// columns). An empty run is represented as `[0]` (len=1).
#[derive(Clone, Copy)]
struct Clue {
    groups: [u8; MAX_ROW_GROUPS],
    len: u8,
}

impl Clue {
    const fn empty() -> Self {
        Self { groups: [0; MAX_ROW_GROUPS], len: 1 }
    }

    fn as_slice(&self) -> &[u8] {
        &self.groups[..self.len as usize]
    }
}

pub struct HanjieScene {
    character: Character,
    win_popup: Popup,

    solution: [u8; MAX_COLS * ROWS],
    board: [u8; MAX_COLS * ROWS],
    row_clues: [Clue; ROWS],
    col_clues: [Clue; MAX_COLS],

    cols: usize,
    cursor: usize,
    state: State,
    elapsed: f32,
    win_timer: f32,

    session_completions: u32,
}

impl HanjieScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(100, 63)),
            win_popup: Popup::new(10, 14, 108, 36),

            solution: [0; MAX_COLS * ROWS],
            board: [0; MAX_COLS * ROWS],
            row_clues: [Clue::empty(); ROWS],
            col_clues: [Clue::empty(); MAX_COLS],

            cols: BASE_COLS,
            cursor: 0,
            state: State::Playing,
            elapsed: 0.0,
            win_timer: 0.0,
            session_completions: 0,
        }
    }

    fn init_game(&mut self, rng: &mut u32) {
        if self.state == State::Win {
            self.session_completions += 1;
        }

        self.cols = if self.session_completions >= 8 {
            9
        } else if self.session_completions >= 5 {
            8
        } else if self.session_completions >= 2 {
            7
        } else {
            BASE_COLS
        };

        self.generate_puzzle(rng);

        for i in 0..self.cols * ROWS {
            self.board[i] = UNKNOWN;
        }
        self.cursor = 0;
        self.state = State::Playing;
        self.elapsed = 0.0;
        self.win_timer = 0.0;

        self.character.pos.x = if self.cols >= 9 { 108 } else { 100 };
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    /// Generate a random solution and clues. Uses a rejection loop
    /// (up to 3 row groups, up to 2 col groups, 5+ filled cells) with a
    /// checkerboard fallback after 200 attempts.
    fn generate_puzzle(&mut self, rng: &mut u32) {
        let cols = self.cols;
        let total = cols * ROWS;

        for _ in 0..200 {
            let mut filled = 0u32;
            for i in 0..total {
                let bit = (xorshift32(rng) & 1) as u8;
                self.solution[i] = bit;
                filled += bit as u32;
            }

            if filled < 5 || filled as usize > total - 3 {
                continue;
            }

            self.compute_clues();

            let rows_ok = self.row_clues.iter().all(|c| c.len as usize <= MAX_ROW_GROUPS);
            let cols_ok = self.col_clues[..cols]
                .iter()
                .all(|c| c.len as usize <= MAX_COL_GROUPS);
            if rows_ok && cols_ok {
                return;
            }
        }

        // Fallback: checkerboard.
        for r in 0..ROWS {
            for c in 0..cols {
                self.solution[r * cols + c] = if (r + c) % 2 == 0 { 1 } else { 0 };
            }
        }
        self.compute_clues();
    }

    fn compute_clues(&mut self) {
        let cols = self.cols;
        for r in 0..ROWS {
            self.row_clues[r] = row_runs(&self.solution, r, cols);
        }
        for c in 0..cols {
            self.col_clues[c] = col_runs(&self.solution, c, cols);
        }
    }

    fn check_win_state(&mut self) -> bool {
        let cols = self.cols;
        for r in 0..ROWS {
            let groups = row_runs_state(&self.board, r, cols, FILLED);
            if !clue_eq(&groups, &self.row_clues[r]) {
                return false;
            }
        }
        for c in 0..cols {
            let groups = col_runs_state(&self.board, c, cols, FILLED);
            if !clue_eq(&groups, &self.col_clues[c]) {
                return false;
            }
        }
        true
    }

    fn enter_win_state(&mut self) {
        self.state = State::Win;
        self.win_timer = 0.0;

        let mut text: String<48> = String::new();
        let _ = text.push_str(t!("Well done!"));
        let _ = text.push('\n');
        let mut time_buf: String<48> = String::new();
        format_time(&mut time_buf, self.elapsed);
        crate::i18n::substitute(&mut text, t!("Time: {v}"), &[("v", time_buf.as_str())]);
        self.win_popup.set_text(text.as_str(), false, true);

        self.character.set_pose(PoseId::SittingSideHappy);
    }

    fn draw_col_clues(&self, r: &mut Renderer) {
        let mut buf: String<4> = String::new();
        for c in 0..self.cols {
            let cx = GRID_X + c as i32 * CELL;
            let clue = self.col_clues[c].as_slice();
            if clue.len() == 2 {
                buf.clear();
                let _ = write!(buf, "{}", clue[0]);
                r.draw_text(buf.as_str(), Point::new(cx, 0));
                buf.clear();
                let _ = write!(buf, "{}", clue[1]);
                r.draw_text(buf.as_str(), Point::new(cx, 8));
            } else {
                // Single group: bottom-align so it sits just above the grid.
                buf.clear();
                let _ = write!(buf, "{}", clue[0]);
                r.draw_text(buf.as_str(), Point::new(cx, 8));
            }
        }
    }

    fn draw_row_clues(&self, r: &mut Renderer) {
        let mut buf: String<4> = String::new();
        for row in 0..ROWS {
            let ry = GRID_Y + row as i32 * CELL;
            let clue = self.row_clues[row].as_slice();
            let n = clue.len() as i32;
            let x0 = GRID_X - n * ROW_CLUE_PITCH;
            for (j, &g) in clue.iter().enumerate() {
                buf.clear();
                let _ = write!(buf, "{}", g);
                r.draw_text(buf.as_str(), Point::new(x0 + j as i32 * ROW_CLUE_PITCH, ry));
            }
        }
    }

    fn draw_grid(&self, r: &mut Renderer) {
        let cols = self.cols;
        for i in 0..cols * ROWS {
            let col = i % cols;
            let row = i / cols;
            let cx = GRID_X + col as i32 * CELL;
            let cy = GRID_Y + row as i32 * CELL;
            let state = self.board[i];
            let is_cursor = i == self.cursor;

            match state {
                FILLED => {
                    r.draw_rect(
                        Point::new(cx + 1, cy + 1),
                        Size::new((CELL - 2) as u32, (CELL - 2) as u32),
                        true,
                    );
                    if is_cursor {
                        // 2x2 black dot in centre to mark cursor on filled cell.
                        r.fill_rect_off(Point::new(cx + 3, cy + 3), Size::new(2, 2));
                    }
                }
                CROSSED => {
                    r.draw_rect(
                        Point::new(cx + 1, cy + 1),
                        Size::new((CELL - 2) as u32, (CELL - 2) as u32),
                        false,
                    );
                    r.draw_line(
                        Point::new(cx + 2, cy + 2),
                        Point::new(cx + CELL - 3, cy + CELL - 3),
                    );
                    if is_cursor {
                        r.draw_rect(
                            Point::new(cx, cy),
                            Size::new(CELL as u32, CELL as u32),
                            false,
                        );
                    }
                }
                _ => {
                    // UNKNOWN.
                    r.draw_rect(
                        Point::new(cx + 1, cy + 1),
                        Size::new((CELL - 2) as u32, (CELL - 2) as u32),
                        is_cursor,
                    );
                }
            }
        }
    }
}

fn row_runs(sol: &[u8], r: usize, cols: usize) -> Clue {
    let mut clue = Clue { groups: [0; MAX_ROW_GROUPS], len: 0 };
    let mut count = 0u8;
    for c in 0..cols {
        if sol[r * cols + c] != 0 {
            count += 1;
        } else if count > 0 {
            push_group(&mut clue, count);
            count = 0;
        }
    }
    if count > 0 {
        push_group(&mut clue, count);
    }
    if clue.len == 0 {
        clue.groups[0] = 0;
        clue.len = 1;
    }
    clue
}

fn col_runs(sol: &[u8], c: usize, cols: usize) -> Clue {
    let mut clue = Clue { groups: [0; MAX_ROW_GROUPS], len: 0 };
    let mut count = 0u8;
    for r in 0..ROWS {
        if sol[r * cols + c] != 0 {
            count += 1;
        } else if count > 0 {
            push_group(&mut clue, count);
            count = 0;
        }
    }
    if count > 0 {
        push_group(&mut clue, count);
    }
    if clue.len == 0 {
        clue.groups[0] = 0;
        clue.len = 1;
    }
    clue
}

/// Row run-lengths over the player's board, counting only cells equal to `target`.
fn row_runs_state(board: &[u8], r: usize, cols: usize, target: u8) -> Clue {
    let mut clue = Clue { groups: [0; MAX_ROW_GROUPS], len: 0 };
    let mut count = 0u8;
    let mut overflow = false;
    for c in 0..cols {
        if board[r * cols + c] == target {
            count += 1;
        } else if count > 0 {
            if !push_group_checked(&mut clue, count) {
                overflow = true;
            }
            count = 0;
        }
    }
    if count > 0 && !push_group_checked(&mut clue, count) {
        overflow = true;
    }
    if clue.len == 0 {
        clue.groups[0] = 0;
        clue.len = 1;
    }
    // Overflow means the board has more groups than any valid clue. Flag it so
    // `clue_eq` reports inequality.
    if overflow {
        clue.groups[0] = u8::MAX;
    }
    clue
}

fn col_runs_state(board: &[u8], c: usize, cols: usize, target: u8) -> Clue {
    let mut clue = Clue { groups: [0; MAX_ROW_GROUPS], len: 0 };
    let mut count = 0u8;
    let mut overflow = false;
    for r in 0..ROWS {
        if board[r * cols + c] == target {
            count += 1;
        } else if count > 0 {
            if !push_group_checked(&mut clue, count) {
                overflow = true;
            }
            count = 0;
        }
    }
    if count > 0 && !push_group_checked(&mut clue, count) {
        overflow = true;
    }
    if clue.len == 0 {
        clue.groups[0] = 0;
        clue.len = 1;
    }
    if overflow {
        clue.groups[0] = u8::MAX;
    }
    clue
}

fn push_group(clue: &mut Clue, n: u8) {
    if (clue.len as usize) < MAX_ROW_GROUPS {
        clue.groups[clue.len as usize] = n;
        clue.len += 1;
    }
}

fn push_group_checked(clue: &mut Clue, n: u8) -> bool {
    if (clue.len as usize) < MAX_ROW_GROUPS {
        clue.groups[clue.len as usize] = n;
        clue.len += 1;
        true
    } else {
        false
    }
}

fn clue_eq(a: &Clue, b: &Clue) -> bool {
    a.len == b.len && a.as_slice() == b.as_slice()
}

fn format_time(out: &mut String<48>, secs: f32) {
    let s = secs as u32;
    let m = s / 60;
    let s = s % 60;
    if m > 0 {
        if s < 10 {
            let _ = write!(out, "{}:0{}", m, s);
        } else {
            let _ = write!(out, "{}:{}", m, s);
        }
    } else {
        let _ = write!(out, "{}s", s);
    }
}

impl Scene for HanjieScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_completions = 0;
        self.state = State::Playing;
        let mut rng = ctx.rng;
        self.init_game(&mut rng);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let completions = self.session_completions;
        if completions == 0 {
            return;
        }
        let reward = ((completions as f32) / 3.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 4.0 * reward),
            (StatId::Focus,        3.0 * reward),
            (StatId::Serenity,     3.0 * reward),
            (StatId::Sociability,  2.0),
            (StatId::Loyalty,      0.5 * reward),
        ]);
        let coins = (5.0 * reward) as i32;
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

        if self.state == State::Win {
            self.win_timer += dt;
            if buttons.was_just_pressed(Button::A) {
                let mut rng = ctx.rng;
                self.init_game(&mut rng);
                ctx.rng = rng;
                return None;
            }
            if self.win_timer >= WIN_RESET_DELAY {
                let mut rng = ctx.rng;
                self.init_game(&mut rng);
                ctx.rng = rng;
            }
            return None;
        }

        self.elapsed += dt;

        let col = self.cursor % self.cols;
        let row = self.cursor / self.cols;

        if buttons.was_just_pressed(Button::Up) && row > 0 {
            self.cursor -= self.cols;
        } else if buttons.was_just_pressed(Button::Down) && row < ROWS - 1 {
            self.cursor += self.cols;
        } else if buttons.was_just_pressed(Button::Left) && col > 0 {
            self.cursor -= 1;
        } else if buttons.was_just_pressed(Button::Right) && col < self.cols - 1 {
            self.cursor += 1;
        }

        if buttons.was_just_pressed(Button::A) {
            self.board[self.cursor] = if self.board[self.cursor] == FILLED {
                UNKNOWN
            } else {
                FILLED
            };
            if self.check_win_state() {
                self.enter_win_state();
            }
        } else if buttons.was_just_pressed(Button::B) {
            self.board[self.cursor] = if self.board[self.cursor] == CROSSED {
                UNKNOWN
            } else {
                CROSSED
            };
            if self.check_win_state() {
                self.enter_win_state();
            }
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.draw_col_clues(renderer);
        self.draw_row_clues(renderer);
        self.draw_grid(renderer);
        self.character.draw(renderer, 0);
        if self.state == State::Win {
            self.win_popup.draw(renderer, false);
        }
    }
}
