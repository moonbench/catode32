//! Yarnball, a numberlink minigame. Join each pair of matching numbers with
//! a strand of yarn; strands can't cross. Boards grow as the player wins and
//! start over on each visit.
//!
//! Layout (128x64):
//!   x=0..91    board, up to 13x9 cells of 6px with 1px gaps
//!   x=92..127  level (top), cat (bottom)

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
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
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
    ui::tiny_font,
};

const MAX_ROWS: usize = 9;
const MAX_COLS: usize = 13;
const MAX_CELLS: usize = MAX_ROWS * MAX_COLS;
const MAX_PAIRS: usize = 9;

/// Shortest strand in a generated solution, endpoints included.
const MIN_STRAND: usize = 3;
/// Backbite moves per cell when shuffling the solution path.
const SHUFFLE_PER_CELL: usize = 20;
/// Cuts to try per path, and paths to try, before falling back to even cuts.
const CUT_TRIES: u32 = 50;
const PATH_TRIES: u32 = 20;

// Board layout.
const PITCH: i32 = 7;
const CELL_IN: u32 = (PITCH - 1) as u32;
const BOARD_AREA_W: i32 = 92;
const BOARD_AREA_H: i32 = 64;
/// Centre of the panel right of the board, over the cat's head.
const PANEL_CX: i32 = 110;

/// Half-period of the cursor blink while drawing a strand.
const BLINK_HALF: f32 = 0.25;

const DIRS: [Button; 4] = [Button::Up, Button::Down, Button::Left, Button::Right];

/// Cells in visiting order, as indices into a `rows * cols` board.
type Path = Vec<u8, MAX_CELLS>;

/// Board size, pair count and how many pairs the obvious routes must fail
/// to join (rows, cols, pairs, stuck) for a level (1-based). See
/// `naive_stuck`.
fn level_params(level: u32) -> (usize, usize, usize, u32) {
    match level {
        0 | 1 => (5, 7, 4, 0),
        2 => (6, 9, 5, 1),
        3 => (7, 11, 6, 1),
        4 => (8, 12, 8, 2),
        _ => (9, 13, 9, 2),
    }
}

/// The cell next to `i` towards `DIRS[d]`, if it stays on the board.
fn neighbour(i: usize, d: usize, rows: usize, cols: usize) -> Option<usize> {
    let (r, c) = (i / cols, i % cols);
    match d {
        0 if r > 0 => Some(i - cols),
        1 if r + 1 < rows => Some(i + cols),
        2 if c > 0 => Some(i - 1),
        3 if c + 1 < cols => Some(i + 1),
        _ => None,
    }
}

fn adjacent(a: usize, b: usize, cols: usize) -> bool {
    let (ra, ca) = ((a / cols) as i32, (a % cols) as i32);
    let (rb, cb) = ((b / cols) as i32, (b % cols) as i32);
    (ra - rb).abs() + (ca - cb).abs() == 1
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Won,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum YarnballAction {
    NewBoard,
    Clear,
}

const OPTIONS_ITEMS: &[MenuItem<YarnballAction>] = &[
    MenuItem { label: t!("New Board"), icon: None, submenu: None, action: Some(YarnballAction::NewBoard), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Clear"),     icon: None, submenu: None, action: Some(YarnballAction::Clear),    confirm: None, confirm_on_vacation: None },
];

pub struct YarnballScene {
    character: Character,
    win_popup: Popup,

    rows: usize,
    cols: usize,
    pairs: usize,
    /// Endpoint number per cell: 1..=pairs, 0 for none.
    ends: [u8; MAX_CELLS],
    /// The player's strand for each pair, starting at the endpoint it was
    /// drawn from.
    strands: [Path; MAX_PAIRS],
    /// Pair number whose strand covers each cell, 0 for none.
    owner: [u8; MAX_CELLS],

    cursor: usize,
    /// Pair index whose strand the d-pad is extending.
    drawing: Option<usize>,
    blink_timer: f32,
    state: State,
    level: u32,
    /// Levels of boards solved this visit, for the exit rewards.
    session_points: u32,

    options_menu: Menu<YarnballAction>,
    menu_active: bool,
}

impl YarnballScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(PANEL_CX, 63)),
            win_popup: Popup::new(10, 14, 108, 36),

            rows: 0,
            cols: 0,
            pairs: 0,
            ends: [0; MAX_CELLS],
            strands: core::array::from_fn(|_| Path::new()),
            owner: [0; MAX_CELLS],

            cursor: 0,
            drawing: None,
            blink_timer: 0.0,
            state: State::Playing,
            level: 1,
            session_points: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn new_board(&mut self, ctx: &mut GameContext) {
        let (rows, cols, pairs, stuck) = level_params(self.level);
        self.rows = rows;
        self.cols = cols;
        self.pairs = pairs;
        let mut rng = ctx.rng;
        self.ends = generate(rows, cols, pairs, stuck, &mut rng);
        ctx.rng = rng;
        self.clear_all();
        self.cursor = (rows / 2) * cols + cols / 2;
        self.state = State::Playing;
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn clear_all(&mut self) {
        for p in 0..MAX_PAIRS {
            self.strands[p].clear();
        }
        self.owner = [0; MAX_CELLS];
        self.drawing = None;
    }

    fn clear_strand(&mut self, p: usize) {
        self.truncate(p, 0);
    }

    /// Cut pair `p`'s strand down to its first `len` cells.
    fn truncate(&mut self, p: usize, len: usize) {
        for &c in &self.strands[p][len..] {
            self.owner[c as usize] = 0;
        }
        self.strands[p].truncate(len);
    }

    fn push(&mut self, p: usize, cell: usize) {
        let _ = self.strands[p].push(cell as u8);
        self.owner[cell] = p as u8 + 1;
    }

    /// Whether pair `p`'s strand runs from one of its endpoints to the other.
    fn is_connected(&self, p: usize) -> bool {
        let s = &self.strands[p];
        let n = p as u8 + 1;
        s.len() >= 2 && self.ends[s[0] as usize] == n && self.ends[s[s.len() - 1] as usize] == n
    }

    /// A on an endpoint starts its strand over from there; A on a strand
    /// cuts it after that cell and carries on drawing.
    fn start_drawing(&mut self) {
        let i = self.cursor;
        if self.ends[i] != 0 {
            let p = self.ends[i] as usize - 1;
            self.clear_strand(p);
            self.push(p, i);
            self.drawing = Some(p);
        } else if self.owner[i] != 0 {
            let p = self.owner[i] as usize - 1;
            if let Some(k) = self.strands[p].iter().position(|&c| c as usize == i) {
                self.truncate(p, k + 1);
            }
            self.drawing = Some(p);
        }
        self.blink_timer = 0.0;
    }

    /// Extend pair `p`'s strand towards `DIRS[d]`. Stepping onto the strand
    /// itself backs up to that cell; other pairs' cells block the way.
    fn step(&mut self, p: usize, d: usize) {
        let Some(&head) = self.strands[p].last() else {
            self.drawing = None;
            return;
        };
        let Some(t) = neighbour(head as usize, d, self.rows, self.cols) else {
            return;
        };
        if let Some(k) = self.strands[p].iter().position(|&c| c as usize == t) {
            self.truncate(p, k + 1);
        } else if self.owner[t] != 0 || (self.ends[t] != 0 && self.ends[t] as usize != p + 1) {
            return;
        } else {
            self.push(p, t);
            if self.ends[t] != 0 {
                self.drawing = None;
                self.cursor = t;
                self.check_board();
                return;
            }
        }
        self.cursor = t;
    }

    fn check_board(&mut self) {
        if !(0..self.pairs).all(|p| self.is_connected(p)) {
            return;
        }
        self.state = State::Won;
        self.session_points += self.level;
        self.level += 1;
        let mut text: String<48> = String::new();
        let _ = text.push_str(t!("Well done!"));
        let _ = text.push('\n');
        let _ = text.push_str(t!("A: New Game"));
        self.win_popup.set_text(text.as_str(), false, true);
        self.character.set_pose(PoseId::SittingSideHappy);
    }

    fn update_board(&mut self, buttons: &mut Buttons) {
        let dir = (0..4).find(|&d| buttons.was_just_pressed(DIRS[d]));

        if let Some(p) = self.drawing {
            if let Some(d) = dir {
                self.step(p, d);
            } else if buttons.was_just_pressed(Button::A) || buttons.was_just_pressed(Button::B) {
                self.drawing = None;
            }
            return;
        }

        if let Some(d) = dir {
            if let Some(i) = neighbour(self.cursor, d, self.rows, self.cols) {
                self.cursor = i;
            }
        } else if buttons.was_just_pressed(Button::A) {
            self.start_drawing();
        } else if buttons.was_just_pressed(Button::B) {
            let i = self.cursor;
            let n = if self.ends[i] != 0 { self.ends[i] } else { self.owner[i] };
            if n != 0 {
                self.clear_strand(n as usize - 1);
            }
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
                    YarnballAction::NewBoard => self.new_board(ctx),
                    YarnballAction::Clear => self.clear_all(),
                }
            }
        }
    }

    fn origin(&self) -> (i32, i32) {
        let w = self.cols as i32 * PITCH + 1;
        let h = self.rows as i32 * PITCH + 1;
        ((BOARD_AREA_W - w) / 2, (BOARD_AREA_H - h) / 2)
    }

    /// Top-left pixel inside cell `i`.
    fn cell_pos(&self, i: usize) -> (i32, i32) {
        let (ox, oy) = self.origin();
        let (r, c) = ((i / self.cols) as i32, (i % self.cols) as i32);
        (ox + c * PITCH + 1, oy + r * PITCH + 1)
    }

    fn draw_board(&self, r: &mut Renderer) {
        let (ox, oy) = self.origin();
        let w = self.cols as i32 * PITCH + 1;
        let h = self.rows as i32 * PITCH + 1;
        r.draw_rect(Point::new(ox, oy), Size::new(w as u32, h as u32), false);
        for row in 1..self.rows as i32 {
            for col in 1..self.cols as i32 {
                r.draw_pixel(Point::new(ox + col * PITCH, oy + row * PITCH), true);
            }
        }

        // Strands: 2px strokes joining the centres of consecutive cells.
        for p in 0..self.pairs {
            for seg in self.strands[p].windows(2) {
                let (ax, ay) = self.cell_pos(seg[0] as usize);
                let (bx, by) = self.cell_pos(seg[1] as usize);
                let pos = Point::new(ax.min(bx) + 2, ay.min(by) + 2);
                let size = Size::new(((ax - bx).abs() + 2) as u32, ((ay - by).abs() + 2) as u32);
                r.draw_rect(pos, size, true);
            }
        }

        // Endpoints over the strands, inverted once their pair is joined.
        for i in 0..self.rows * self.cols {
            let n = self.ends[i];
            if n == 0 {
                continue;
            }
            let (x, y) = self.cell_pos(i);
            let done = self.is_connected(n as usize - 1);
            if done {
                r.draw_rect(Point::new(x, y), Size::new(CELL_IN, CELL_IN), true);
            } else {
                r.fill_rect_off(Point::new(x, y), Size::new(CELL_IN, CELL_IN));
            }
            tiny_font::draw_digit(r, n, Point::new(x + 1, y + 1), done);
        }

        // Cursor: an outline over the grid lines round its cell.
        let blink_on = self.blink_timer < BLINK_HALF;
        if self.state == State::Playing && (self.drawing.is_none() || blink_on) {
            let (x, y) = self.cell_pos(self.cursor);
            r.draw_rect(Point::new(x - 1, y - 1), Size::new(CELL_IN + 2, CELL_IN + 2), false);
        }
    }

    fn draw_level(&self, r: &mut Renderer) {
        let mut level: String<24> = String::new();
        let mut num: String<8> = String::new();
        {
            use core::fmt::Write as _;
            let _ = write!(num, "{}", self.level);
        }
        crate::i18n::substitute(&mut level, t!("Level {n}"), &[("n", num.as_str())]);
        let x = PANEL_CX - 2 * level.chars().count() as i32;
        r.draw_text_small(level.as_str(), Point::new(x.max(BOARD_AREA_W), 2));
    }
}

// ---------------------------------------------------------------------------
// Board generation.
// ---------------------------------------------------------------------------

/// A random path through every cell: a snake across the rows, shuffled with
/// backbite moves (join an end to a neighbour, reverse the loop that makes).
fn random_path(rows: usize, cols: usize, rng: &mut u32) -> Path {
    let n = rows * cols;
    let mut path = Path::new();
    for r in 0..rows {
        for k in 0..cols {
            let c = if r % 2 == 0 { k } else { cols - 1 - k };
            let _ = path.push((r * cols + c) as u8);
        }
    }
    let mut pos = [0u8; MAX_CELLS];
    for (k, &c) in path.iter().enumerate() {
        pos[c as usize] = k as u8;
    }

    for _ in 0..n * SHUFFLE_PER_CELL {
        let roll = xorshift32(rng);
        let from_end = roll & 4 != 0;
        let end = if from_end { path[n - 1] } else { path[0] };
        let Some(v) = neighbour(end as usize, (roll & 3) as usize, rows, cols) else {
            continue;
        };
        let j = pos[v] as usize;
        let range = if from_end { j + 1..n } else { 0..j };
        path[range.clone()].reverse();
        for k in range {
            pos[path[k] as usize] = k as u8;
        }
    }
    path
}

/// Number each run of `path` between consecutive `bounds` as one pair.
fn place_ends(path: &Path, bounds: &[usize], rng: &mut u32) -> [u8; MAX_CELLS] {
    let pairs = bounds.len() - 1;
    let mut numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    for k in (1..pairs).rev() {
        let j = xorshift32(rng) as usize % (k + 1);
        numbers.swap(k, j);
    }
    let mut ends = [0; MAX_CELLS];
    for k in 0..pairs {
        ends[path[bounds[k]] as usize] = numbers[k];
        ends[path[bounds[k + 1] - 1] as usize] = numbers[k];
    }
    ends
}

/// Random run boundaries splitting `path` into `pairs` strands, each at
/// least `MIN_STRAND` long with ends that don't touch. None if this roll
/// didn't fit.
fn random_cut(path: &Path, pairs: usize, cols: usize, rng: &mut u32) -> Option<Vec<usize, { MAX_PAIRS + 1 }>> {
    let n = path.len();
    let mut bounds: Vec<usize, { MAX_PAIRS + 1 }> = Vec::new();
    let _ = bounds.push(0);
    for _ in 1..pairs {
        let _ = bounds.push(1 + xorshift32(rng) as usize % (n - 1));
    }
    let _ = bounds.push(n);
    bounds.sort_unstable();
    let fits = bounds.windows(2).all(|w| {
        w[1] - w[0] >= MIN_STRAND && !adjacent(path[w[0]] as usize, path[w[1] - 1] as usize, cols)
    });
    fits.then_some(bounds)
}

/// Join `a` to `b` by a shortest route through cells that are neither
/// `taken` nor another pair's endpoint, marking the route taken. Neighbours
/// are tried in `dirs` order, which picks among equally short routes.
fn route(a: usize, b: usize, ends: &[u8; MAX_CELLS], taken: &mut [bool; MAX_CELLS], rows: usize, cols: usize, dirs: [usize; 4]) -> bool {
    const NONE: u8 = u8::MAX;
    let mut prev = [NONE; MAX_CELLS];
    let mut queue = [0u8; MAX_CELLS];
    let (mut head, mut tail) = (0, 1);
    queue[0] = a as u8;
    prev[a] = a as u8;
    while head < tail {
        let i = queue[head] as usize;
        head += 1;
        if i == b {
            let mut k = b;
            while k != a {
                taken[k] = true;
                k = prev[k] as usize;
            }
            taken[a] = true;
            return true;
        }
        for d in dirs {
            if let Some(t) = neighbour(i, d, rows, cols) {
                if prev[t] == NONE && !taken[t] && (ends[t] == 0 || t == b) {
                    prev[t] = i as u8;
                    queue[tail] = t as u8;
                    tail += 1;
                }
            }
        }
    }
    false
}

/// How many pairs the obvious way of playing can't join: take pairs from
/// nearest to farthest and draw each by a shortest route around what's
/// already drawn. A few tie-breaks are tried and the kindest one counts, so
/// a board scores above zero only if direct routes really do collide.
fn naive_stuck(ends: &[u8; MAX_CELLS], rows: usize, cols: usize, pairs: usize) -> u32 {
    let mut pair_ends = [(0usize, 0usize); MAX_PAIRS];
    let mut seen = [false; MAX_PAIRS];
    for i in 0..rows * cols {
        if ends[i] != 0 {
            let p = ends[i] as usize - 1;
            if seen[p] {
                pair_ends[p].1 = i;
            } else {
                pair_ends[p] = (i, i);
                seen[p] = true;
            }
        }
    }
    let dist = |p: usize| {
        let (a, b) = pair_ends[p];
        ((a / cols) as i32 - (b / cols) as i32).abs() + ((a % cols) as i32 - (b % cols) as i32).abs()
    };
    let mut order = [0usize; MAX_PAIRS];
    for (k, v) in order.iter_mut().enumerate() {
        *v = k;
    }
    order[..pairs].sort_unstable_by_key(|&p| dist(p));

    let mut best = u32::MAX;
    for dirs in [[0, 1, 2, 3], [3, 2, 1, 0], [2, 0, 3, 1], [1, 3, 0, 2]] {
        let mut taken = [false; MAX_CELLS];
        let mut stuck = 0;
        for &p in &order[..pairs] {
            let (a, b) = pair_ends[p];
            if !route(a, b, ends, &mut taken, rows, cols, dirs) {
                stuck += 1;
            }
        }
        best = best.min(stuck);
    }
    best
}

/// Endpoints for a board. They come from cutting one path through every
/// cell, so the board always has a solution. Boards where the obvious
/// routes get stuck on fewer than `stuck` pairs are passed over, keeping
/// the hardest one seen in case none gets there.
fn generate(rows: usize, cols: usize, pairs: usize, stuck: u32, rng: &mut u32) -> [u8; MAX_CELLS] {
    let mut path = Path::new();
    let mut best: Option<(u32, [u8; MAX_CELLS])> = None;
    for _ in 0..PATH_TRIES {
        path = random_path(rows, cols, rng);
        for _ in 0..CUT_TRIES {
            let Some(bounds) = random_cut(&path, pairs, cols, rng) else {
                continue;
            };
            let ends = place_ends(&path, &bounds, rng);
            let score = naive_stuck(&ends, rows, cols, pairs);
            if score >= stuck {
                return ends;
            }
            if best.map_or(true, |(s, _)| score > s) {
                best = Some((score, ends));
            }
        }
    }
    if let Some((_, ends)) = best {
        return ends;
    }
    // Out of luck: even runs. Always long enough, though ends may touch.
    let n = path.len();
    let mut bounds: Vec<usize, { MAX_PAIRS + 1 }> = Vec::new();
    for k in 0..=pairs {
        let _ = bounds.push(k * n / pairs);
    }
    place_ends(&path, &bounds, rng)
}

impl Scene for YarnballScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.level = 1;
        self.session_points = 0;
        self.menu_active = false;
        self.new_board(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.session_points == 0 {
            return;
        }
        let reward = (self.session_points as f32 / 2.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 5.0 * reward),
            (StatId::Focus,        5.0 * reward),
            (StatId::Serenity,     2.0 * reward),
            (StatId::Loyalty,      0.5 * reward),
        ]);
        ctx.coins += 3 * self.session_points as i32;
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
        self.blink_timer = (self.blink_timer + dt) % (2.0 * BLINK_HALF);

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

        self.update_board(buttons);
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }

        self.draw_board(renderer);
        self.draw_level(renderer);
        self.character.draw(renderer, 0);

        if self.state == State::Won {
            self.win_popup.draw(renderer, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boards_have_separated_pairs() {
        for level in 1..=5 {
            let (rows, cols, pairs, stuck) = level_params(level);
            for seed in 1..=6 {
                let mut rng = seed * 7919;
                let ends = generate(rows, cols, pairs, stuck, &mut rng);
                assert!(ends.iter().all(|&n| n as usize <= pairs));
                for n in 1..=pairs as u8 {
                    let mut cells = (0..rows * cols).filter(|&i| ends[i] == n);
                    let (a, b) = (cells.next().unwrap(), cells.next().unwrap());
                    assert!(cells.next().is_none());
                    assert!(!adjacent(a, b, cols));
                }
            }
        }
    }

    #[test]
    fn strands_cannot_cross_and_joining_all_wins() {
        let mut s = YarnballScene::new();
        s.rows = 2;
        s.cols = 3;
        s.pairs = 2;
        s.ends[..6].copy_from_slice(&[1, 0, 1, 2, 0, 2]);

        s.cursor = 0;
        s.start_drawing();
        s.step(0, 3);
        s.step(0, 3);
        assert!(s.is_connected(0));
        assert_eq!(s.drawing, None);

        s.cursor = 3;
        s.start_drawing();
        s.step(1, 0); // Up into pair 1's strand: blocked.
        assert_eq!(s.cursor, 3);
        s.step(1, 3);
        assert!(s.state == State::Playing);
        s.step(1, 3);
        assert!(s.state == State::Won);
    }
}
