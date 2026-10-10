//! Sudokat minigame, a cat's take on sudoku. Fill the 9x9 grid so every row,
//! column and 3x3 box holds each digit once. The board lives in the context,
//! so an unfinished puzzle survives leaving the game or a reboot.
//!
//! Layout (128x64):
//!   x=4..67    board, nine 6px cells and ten 1px lines
//!   x=68..127  number to place (top), lock when on a given, cat (bottom)

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{character::PoseId, minigame_assets::LOCK},
    context::{GameContext, StatId},
    entities::character::Character,
    input::{Button, Buttons},
    rand::xorshift32,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

const N: usize = 9;
const CELLS: usize = N * N;

/// One digit per cell, row by row. 0 is blank.
type Grid = [u8; CELLS];

/// Bits 1..=9: a row, column or box holding every digit.
const ALL_DIGITS: u16 = 0x3FE;

/// `selected` value that erases instead of placing a digit.
const ERASE: u8 = 10;

/// Search nodes one uniqueness check may visit before it gives up and
/// treats the puzzle as ambiguous. Keeps generation time bounded on device.
const SEARCH_BUDGET: u32 = 20_000;

// Board layout.
const PITCH: i32 = 7;
const BOARD_X: i32 = 8;
const BOARD_SIZE: i32 = N as i32 * PITCH + 1; // 64
const CELL_IN: u32 = (PITCH - 1) as u32;
const GLYPH_W: u16 = 4;
const GLYPH_H: u16 = 5;
/// Glyph y offset in its cell by row within the box, so no digit touches a
/// box line.
const GLYPH_DY: [i32; 3] = [1, 0, 0];

// Panel layout.
/// Centre of the selector, over the cat's head.
const PANEL_CX: i32 = 96;
const SEL_W: i32 = 14;
const SEL_H: i32 = 16;
const SEL_X: i32 = PANEL_CX - SEL_W / 2;
const SEL_Y: i32 = 2;
const ARROW_GAP: i32 = 2;
/// Right of the selector, clear of the right arrow.
const LOCK_X: i32 = SEL_X + SEL_W + 9;

/// 4x5 digits 1-9, one row per byte, high nibble.
const GLYPHS: [[u8; 5]; 9] = [
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Difficulty::Easy,
            1 => Difficulty::Normal,
            _ => Difficulty::Hard,
        }
    }

    /// Clues to dig down to. Generation stops short if no more can go.
    fn target_clues(self) -> usize {
        match self {
            Difficulty::Easy => 38,
            Difficulty::Normal => 30,
            Difficulty::Hard => 24,
        }
    }

    /// Reward weight for solving a board.
    fn points(self) -> u32 {
        self as u32 + 1
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Won,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The d-pad moves the cursor.
    Board,
    /// The d-pad picks the number to place.
    Select,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SudokatAction {
    NewBoard,
    Easy,
    Normal,
    Hard,
}

const DIFFICULTY_ITEMS: &[MenuItem<SudokatAction>] = &[
    MenuItem { label: t!("Easy"),   icon: None, submenu: None, action: Some(SudokatAction::Easy),   confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Normal"), icon: None, submenu: None, action: Some(SudokatAction::Normal), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Hard"),   icon: None, submenu: None, action: Some(SudokatAction::Hard),   confirm: None, confirm_on_vacation: None },
];

const OPTIONS_ITEMS: &[MenuItem<SudokatAction>] = &[
    MenuItem { label: t!("New Board"),  icon: None, submenu: None,                   action: Some(SudokatAction::NewBoard), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Difficulty"), icon: None, submenu: Some(DIFFICULTY_ITEMS), action: None,                          confirm: None, confirm_on_vacation: None },
];

pub struct SudokatScene {
    character: Character,
    win_popup: Popup,

    givens: Grid,
    entries: Grid,
    difficulty: Difficulty,
    cursor: usize,
    mode: Mode,
    /// Number A places: 1-9, or `ERASE`.
    selected: u8,
    state: State,
    /// Difficulty points of boards solved this visit, for the exit rewards.
    session_points: u32,

    options_menu: Menu<SudokatAction>,
    menu_active: bool,
}

impl SudokatScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(100, 63)),
            win_popup: Popup::new(10, 14, 108, 36),

            givens: [0; CELLS],
            entries: [0; CELLS],
            difficulty: Difficulty::Easy,
            cursor: CELLS / 2,
            mode: Mode::Board,
            selected: 1,
            state: State::Playing,
            session_points: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn cells(&self) -> Grid {
        let mut out = self.givens;
        for i in 0..CELLS {
            if out[i] == 0 {
                out[i] = self.entries[i];
            }
        }
        out
    }

    fn new_board(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.givens = generate(self.difficulty, &mut rng);
        ctx.rng = rng;
        self.entries = [0; CELLS];
        self.cursor = CELLS / 2;
        self.mode = Mode::Board;
        self.state = State::Playing;
        self.character.set_pose(PoseId::SittingSideNeutral);
        self.store(ctx);
    }

    /// Copy the board into the context so the next save picks it up.
    fn store(&self, ctx: &mut GameContext) {
        ctx.sudokat_givens = self.givens;
        ctx.sudokat_entries = self.entries;
        ctx.sudokat_difficulty = self.difficulty as u8;
    }

    /// Write the selected number into the cursor cell. Givens are locked.
    /// Returns whether anything changed.
    fn place(&mut self) -> bool {
        let i = self.cursor;
        if self.givens[i] != 0 {
            return false;
        }
        let v = if self.selected == ERASE { 0 } else { self.selected };
        if self.entries[i] == v {
            return false;
        }
        self.entries[i] = v;
        true
    }

    /// Settle the board after a change: win, or let the cat frown at a full
    /// board with a mistake in it.
    fn check_board(&mut self) {
        let cells = self.cells();
        if is_solved(&cells) {
            self.state = State::Won;
            self.session_points += self.difficulty.points();
            let mut text: String<48> = String::new();
            let _ = text.push_str(t!("Well done!"));
            let _ = text.push('\n');
            let _ = text.push_str(t!("A: New Game"));
            self.win_popup.set_text(text.as_str(), false, true);
            self.character.set_pose(PoseId::SittingSideHappy);
        } else if cells.iter().all(|&v| v != 0) {
            self.character.set_pose(PoseId::SittingSideAnnoyed);
        } else {
            self.character.set_pose(PoseId::SittingSideNeutral);
        }
    }

    fn move_cursor(&mut self, buttons: &mut Buttons) {
        let mut row = self.cursor / N;
        let mut col = self.cursor % N;
        if buttons.was_just_pressed(Button::Up) {
            row = (row + N - 1) % N;
        } else if buttons.was_just_pressed(Button::Down) {
            row = (row + 1) % N;
        } else if buttons.was_just_pressed(Button::Left) {
            col = (col + N - 1) % N;
        } else if buttons.was_just_pressed(Button::Right) {
            col = (col + 1) % N;
        }
        self.cursor = row * N + col;
    }

    fn cycle_selected(&mut self, buttons: &mut Buttons) {
        if buttons.was_just_pressed(Button::Left) {
            self.selected = if self.selected == 1 { ERASE } else { self.selected - 1 };
        } else if buttons.was_just_pressed(Button::Right) {
            self.selected = if self.selected == ERASE { 1 } else { self.selected + 1 };
        }
    }

    fn open_menu(&mut self) {
        self.menu_active = true;
        self.options_menu.reset_to(OPTIONS_ITEMS);
    }

    fn handle_menu(&mut self, buttons: &mut Buttons, ctx: &mut GameContext) {
        match self.options_menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => {
                self.menu_active = false;
            }
            MenuResult::Action(action) => {
                self.menu_active = false;
                match action {
                    SudokatAction::NewBoard => {}
                    SudokatAction::Easy => self.difficulty = Difficulty::Easy,
                    SudokatAction::Normal => self.difficulty = Difficulty::Normal,
                    SudokatAction::Hard => self.difficulty = Difficulty::Hard,
                }
                self.new_board(ctx);
            }
        }
    }

    fn draw_board(&self, r: &mut Renderer) {
        // Only the box lines are drawn; cells inside a box go unmarked.
        for k in 0..=3 {
            let p = k * 3 * PITCH;
            r.draw_line(Point::new(BOARD_X + p, 0), Point::new(BOARD_X + p, BOARD_SIZE - 1));
            r.draw_line(Point::new(BOARD_X, p), Point::new(BOARD_X + BOARD_SIZE - 1, p));
        }

        let cells = self.cells();
        for i in 0..CELLS {
            let row = (i / N) as i32;
            let col = (i % N) as i32;
            let x = BOARD_X + col * PITCH + 1;
            let y = row * PITCH + 1;
            let is_cursor = i == self.cursor && self.state == State::Playing;
            if is_cursor {
                r.draw_rect(Point::new(x, y), Size::new(CELL_IN, CELL_IN), true);
            }
            if cells[i] != 0 {
                let pos = Point::new(x + 1, y + GLYPH_DY[(row % 3) as usize]);
                draw_glyph(r, cells[i], pos, is_cursor);
            }
        }
    }

    fn draw_panel(&self, r: &mut Renderer) {
        // Rounded box holding the number to place, drawn at double size.
        r.draw_rect(Point::new(SEL_X, SEL_Y), Size::new(SEL_W as u32, SEL_H as u32), false);
        for (cx, cy) in [
            (SEL_X, SEL_Y),
            (SEL_X + SEL_W - 1, SEL_Y),
            (SEL_X, SEL_Y + SEL_H - 1),
            (SEL_X + SEL_W - 1, SEL_Y + SEL_H - 1),
        ] {
            r.draw_pixel(Point::new(cx, cy), false);
        }
        let gx = PANEL_CX - GLYPH_W as i32;
        let gy = SEL_Y + (SEL_H - 2 * GLYPH_H as i32) / 2;
        if self.selected == ERASE {
            // 2px strokes to match the doubled digits.
            for k in 0..7 {
                let y = gy + 1 + k;
                r.draw_rect(Point::new(gx + k, y), Size::new(2, 1), true);
                r.draw_rect(Point::new(gx + 6 - k, y), Size::new(2, 1), true);
            }
        } else {
            draw_glyph_2x(r, self.selected, gx, gy);
        }

        if self.mode == Mode::Select {
            let cy = SEL_Y + SEL_H / 2;
            draw_arrow(r, SEL_X - ARROW_GAP - 1, cy, -1);
            draw_arrow(r, SEL_X + SEL_W + ARROW_GAP, cy, 1);
        }

        if self.state == State::Playing && self.givens[self.cursor] != 0 {
            let y = SEL_Y + (SEL_H - LOCK.height as i32) / 2;
            r.draw_sprite(&LOCK, Point::new(LOCK_X, y), SpriteOpts::default());
        }
    }
}

fn draw_glyph(r: &mut Renderer, digit: u8, pos: Point, inverted: bool) {
    let opts = SpriteOpts {
        transparent: true,
        transparent_color: inverted,
        invert: inverted,
        ..SpriteOpts::default()
    };
    r.draw_sprite_raw(&GLYPHS[digit as usize - 1], GLYPH_W, GLYPH_H, pos, opts);
}

fn draw_glyph_2x(r: &mut Renderer, digit: u8, x: i32, y: i32) {
    let glyph = &GLYPHS[digit as usize - 1];
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..GLYPH_W as i32 {
            if bits & (0x80 >> gx) != 0 {
                r.draw_rect(Point::new(x + gx * 2, y + gy as i32 * 2), Size::new(2, 2), true);
            }
        }
    }
}

/// A 4x7 arrowhead with its base at column `base_x`, pointing along `dir`.
fn draw_arrow(r: &mut Renderer, base_x: i32, cy: i32, dir: i32) {
    for k in 0..4 {
        let x = base_x + dir * k;
        let half = 3 - k;
        r.draw_line(Point::new(x, cy - half), Point::new(x, cy + half));
    }
}

// ---------------------------------------------------------------------------
// Puzzle logic.
// ---------------------------------------------------------------------------

fn box_of(i: usize) -> usize {
    (i / 27) * 3 + (i % N) / 3
}

/// The `k`th cell of unit `u`: rows 0-8, then columns, then boxes.
fn unit_cell(u: usize, k: usize) -> usize {
    match u {
        0..=8 => u * N + k,
        9..=17 => k * N + (u - 9),
        _ => {
            let b = u - 18;
            (b / 3) * 27 + (b % 3) * 3 + (k / 3) * N + k % 3
        }
    }
}

/// Which digits each row, column and box already holds.
struct Masks {
    rows: [u16; N],
    cols: [u16; N],
    boxes: [u16; N],
}

impl Masks {
    fn of(grid: &Grid) -> Self {
        let mut m = Masks { rows: [0; N], cols: [0; N], boxes: [0; N] };
        for i in 0..CELLS {
            if grid[i] != 0 {
                m.toggle(i, grid[i]);
            }
        }
        m
    }

    /// Add or remove digit `d` at cell `i`.
    fn toggle(&mut self, i: usize, d: u8) {
        let bit = 1 << d;
        self.rows[i / N] ^= bit;
        self.cols[i % N] ^= bit;
        self.boxes[box_of(i)] ^= bit;
    }

    fn candidates(&self, i: usize) -> u16 {
        !(self.rows[i / N] | self.cols[i % N] | self.boxes[box_of(i)]) & ALL_DIGITS
    }

    fn unit(&self, u: usize) -> u16 {
        match u {
            0..=8 => self.rows[u],
            9..=17 => self.cols[u - 9],
            _ => self.boxes[u - 18],
        }
    }
}

/// Every cell filled and every row, column and box holding 1-9 once.
fn is_solved(grid: &Grid) -> bool {
    if grid.iter().any(|&v| v == 0) {
        return false;
    }
    (0..3 * N).all(|u| {
        let mut seen = 0u16;
        for k in 0..N {
            seen |= 1 << grid[unit_cell(u, k)];
        }
        seen == ALL_DIGITS
    })
}

/// The blank with the fewest candidates, or None if the grid is full. A
/// blank with no candidates comes back with an empty mask.
fn best_blank(grid: &Grid, m: &Masks) -> Option<(usize, u16)> {
    let mut best: Option<(usize, u16)> = None;
    let mut best_n = u32::MAX;
    for i in 0..CELLS {
        if grid[i] != 0 {
            continue;
        }
        let c = m.candidates(i);
        let n = c.count_ones();
        if n < best_n {
            best = Some((i, c));
            best_n = n;
            if n <= 1 {
                break;
            }
        }
    }
    best
}

fn shuffle(items: &mut [u8], rng: &mut u32) {
    for k in (1..items.len()).rev() {
        let j = xorshift32(rng) as usize % (k + 1);
        items.swap(k, j);
    }
}

/// Complete `grid` with a random solution, trying digits in shuffled order.
fn fill(grid: &mut Grid, m: &mut Masks, rng: &mut u32) -> bool {
    let Some((i, cands)) = best_blank(grid, m) else {
        return true;
    };
    let mut digits = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    shuffle(&mut digits, rng);
    for d in digits {
        if cands & (1 << d) == 0 {
            continue;
        }
        grid[i] = d;
        m.toggle(i, d);
        if fill(grid, m, rng) {
            return true;
        }
        m.toggle(i, d);
        grid[i] = 0;
    }
    false
}

/// Count solutions, stopping at `limit`. Running out of `budget` reports
/// `limit`, so the caller treats the puzzle as ambiguous.
fn count(grid: &mut Grid, m: &mut Masks, limit: u32, budget: &mut u32) -> u32 {
    let Some((i, cands)) = best_blank(grid, m) else {
        return 1;
    };
    let mut found = 0;
    for d in 1..=9u8 {
        if cands & (1 << d) == 0 {
            continue;
        }
        if *budget == 0 {
            return limit;
        }
        *budget -= 1;
        grid[i] = d;
        m.toggle(i, d);
        found += count(grid, m, limit - found, budget);
        m.toggle(i, d);
        grid[i] = 0;
        if found >= limit {
            break;
        }
    }
    found
}

fn count_solutions(grid: &Grid, limit: u32, budget: u32) -> u32 {
    let mut g = *grid;
    let mut m = Masks::of(&g);
    let mut budget = budget;
    count(&mut g, &mut m, limit, &mut budget)
}

/// Whether the puzzle solves with naked singles (a cell with one candidate
/// left) and hidden singles (a digit with one place left in a unit) alone.
/// Every step is forced, so success also proves the solution is unique.
fn solves_with_singles(grid: &Grid) -> bool {
    let mut g = *grid;
    let mut m = Masks::of(&g);
    loop {
        let mut progress = false;
        for i in 0..CELLS {
            if g[i] != 0 {
                continue;
            }
            let c = m.candidates(i);
            if c == 0 {
                return false;
            }
            if c.count_ones() == 1 {
                let d = c.trailing_zeros() as u8;
                g[i] = d;
                m.toggle(i, d);
                progress = true;
            }
        }
        for u in 0..3 * N {
            for d in 1..=9u8 {
                let bit = 1 << d;
                if m.unit(u) & bit != 0 {
                    continue;
                }
                let mut spot = None;
                let mut spots = 0;
                for k in 0..N {
                    let i = unit_cell(u, k);
                    if g[i] == 0 && m.candidates(i) & bit != 0 {
                        spot = Some(i);
                        spots += 1;
                    }
                }
                match (spots, spot) {
                    (0, _) => return false,
                    (1, Some(i)) => {
                        g[i] = d;
                        m.toggle(i, d);
                        progress = true;
                    }
                    _ => {}
                }
            }
        }
        if !progress {
            return g.iter().all(|&v| v != 0);
        }
    }
}

/// Build a puzzle with exactly one solution. Fills a random solution, then
/// digs out cells in 180-degree pairs, keeping each removal only while the
/// puzzle stays fair: singles-solvable on Easy and Normal, unique on Hard.
fn generate(difficulty: Difficulty, rng: &mut u32) -> Grid {
    let mut solution = [0; CELLS];
    let mut m = Masks::of(&solution);
    fill(&mut solution, &mut m, rng);

    let mut puzzle = solution;
    let mut clues = CELLS;
    // Each index stands for the pair (i, 80 - i); 40 is the centre cell.
    let mut order = [0u8; CELLS / 2 + 1];
    for (k, v) in order.iter_mut().enumerate() {
        *v = k as u8;
    }
    shuffle(&mut order, rng);

    for &i in &order {
        if clues <= difficulty.target_clues() {
            break;
        }
        let (a, b) = (i as usize, CELLS - 1 - i as usize);
        puzzle[a] = 0;
        puzzle[b] = 0;
        let fair = match difficulty {
            Difficulty::Hard => count_solutions(&puzzle, 2, SEARCH_BUDGET) == 1,
            _ => solves_with_singles(&puzzle),
        };
        if fair {
            clues -= if a == b { 1 } else { 2 };
        } else {
            puzzle[a] = solution[a];
            puzzle[b] = solution[b];
        }
    }
    puzzle
}

impl Scene for SudokatScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_points = 0;
        self.difficulty = Difficulty::from_u8(ctx.sudokat_difficulty);
        self.givens = ctx.sudokat_givens;
        self.entries = ctx.sudokat_entries;
        // Pick up where the player left off, unless that board was solved.
        if self.givens.iter().all(|&v| v == 0) || is_solved(&self.cells()) {
            self.new_board(ctx);
        } else {
            self.check_board();
        }
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.session_points == 0 {
            return;
        }
        let reward = (self.session_points as f32 / 2.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 6.0 * reward),
            (StatId::Focus,        5.0 * reward),
            (StatId::Serenity,     3.0 * reward),
            (StatId::Loyalty,      0.5 * reward),
        ]);
        ctx.coins += 5 * self.session_points as i32;
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

        if self.menu_active {
            self.handle_menu(buttons, ctx);
            return None;
        }

        if self.state == State::Won {
            if buttons.was_just_pressed(Button::A) {
                self.new_board(ctx);
            }
            return None;
        }

        if buttons.was_just_pressed(Button::Menu2) {
            self.open_menu();
            return None;
        }

        match self.mode {
            Mode::Board => {
                self.move_cursor(buttons);
                if buttons.was_just_pressed(Button::A) {
                    if self.place() {
                        self.store(ctx);
                        self.check_board();
                    }
                } else if buttons.was_just_pressed(Button::B) {
                    self.mode = Mode::Select;
                }
            }
            Mode::Select => {
                self.cycle_selected(buttons);
                if buttons.was_just_pressed(Button::A) || buttons.was_just_pressed(Button::B) {
                    self.mode = Mode::Board;
                }
            }
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }

        self.draw_board(renderer);
        self.draw_panel(renderer);
        self.character.draw(renderer, 0);

        if self.state == State::Won {
            self.win_popup.draw(renderer, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved_board(seed: u32) -> Grid {
        let mut grid = [0; CELLS];
        let mut m = Masks::of(&grid);
        let mut rng = seed;
        assert!(fill(&mut grid, &mut m, &mut rng));
        grid
    }

    #[test]
    fn every_board_has_one_solution() {
        for difficulty in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
            for seed in 1..=8 {
                let mut rng = seed * 7919;
                let puzzle = generate(difficulty, &mut rng);
                assert_eq!(count_solutions(&puzzle, 2, u32::MAX), 1);
                if difficulty != Difficulty::Hard {
                    assert!(solves_with_singles(&puzzle));
                }
            }
        }
    }

    #[test]
    fn win_needs_a_full_valid_grid() {
        let grid = solved_board(42);
        assert!(is_solved(&grid));

        let mut swapped = grid;
        swapped.swap(0, 1);
        assert!(!is_solved(&swapped));

        let mut blank = grid;
        blank[40] = 0;
        assert!(!is_solved(&blank));
    }

    #[test]
    fn givens_cannot_be_overwritten() {
        let mut s = SudokatScene::new();
        s.givens[0] = 5;
        s.cursor = 0;
        s.selected = 3;
        assert!(!s.place());
        assert_eq!(s.cells()[0], 5);
        assert_eq!(s.entries[0], 0);

        s.cursor = 1;
        assert!(s.place());
        assert_eq!(s.cells()[1], 3);
        s.selected = ERASE;
        assert!(s.place());
        assert_eq!(s.cells()[1], 0);
    }
}
