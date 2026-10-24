//! Sunny Spot minigame, a cat's take on Rush Hour. Slide the furniture
//! around until the cat bed (the solid block) can slide out of the gap in the
//! right wall and into the sunbeam.
//!
//! Puzzles are generated on the fly. A random layout is placed with the bed
//! already at the exit, every arrangement reachable from it is enumerated,
//! and a BFS outward from all the solved arrangements finds one that is far
//! from any of them. The board grows from 6x6 to 8x8 as levels progress.
//!
//! Layout (128x64):
//!   x=0..61    board (size depends on level), exit gap in the right wall
//!   x=66..127  level, moves and par (top), cat in its bed (bottom)

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{character::PoseId, furniture::draw_cat_bed_rim},
    context::{GameContext, StatId},
    entities::character::Character,
    input::{Button, Buttons},
    platform::time::Instant,
    rand::{rand_range_u32, xorshift32},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

/// Pieces in one puzzle, the bed included. Positions pack 3 bits each into
/// a `u64` state.
const MAX_PIECES: usize = 21;

/// Arrangements one generation attempt may enumerate before it is thrown
/// away. Bounds the scratch buffer and the time an attempt takes.
#[cfg(feature = "h2")]
const MAX_STATES: usize = 2048;
#[cfg(not(feature = "h2"))]
const MAX_STATES: usize = 4096;
const HASH_SIZE: usize = MAX_STATES * 2;
const UNSEEN: u8 = u8::MAX;

/// Hill-climbing steps per puzzle before settling for the hardest layout
/// found so far.
const MAX_STEPS: u32 = 400;
/// Steps a search carried over from the previous level must take before its
/// puzzle is used, so consecutive levels don't share a furniture layout.
const MIN_STEPS: u32 = 20;

/// Milliseconds of search per frame while waiting on a puzzle, and while
/// building the next level's puzzle during play. At least one step always
/// runs.
const FOREGROUND_BUDGET_MS: u64 = 60;
const BACKGROUND_BUDGET_MS: u64 = 15;

const UNDO_LEN: usize = 32;

/// Pixels per second the bed slides out once solved.
const EXIT_SPEED: f32 = 40.0;

const PANEL_X: i32 = 68;
/// Left edge of the cat bed, which spans the rest of the screen. The cat sits
/// in it at the same offset as in the bedroom.
const CAT_BED_X: i32 = 66;
const CAT_X: i32 = CAT_BED_X + 29;

/// The bed is always piece 0.
const GOAL: usize = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Piece {
    horizontal: bool,
    len: u8,
    /// Row of a horizontal piece, column of a vertical one.
    lane: u8,
}

const NO_PIECE: Piece = Piece { horizontal: true, len: 0, lane: 0 };

/// The fixed part of a puzzle. The variable part, where each piece sits
/// along its lane, is a packed `u64` state.
#[derive(Clone, Copy)]
struct Layout {
    n: u8,
    count: u8,
    pieces: [Piece; MAX_PIECES],
}

impl Layout {
    fn exit_row(&self) -> u8 {
        (self.n - 1) / 2
    }

    fn is_solved(&self, state: u64) -> bool {
        pos(state, GOAL) == self.n - 2
    }

    /// Board cells covered by piece `i` at `p`, one bit per cell at
    /// `row * 8 + col`.
    fn mask(&self, i: usize, p: u8) -> u64 {
        let piece = self.pieces[i];
        let mut m = 0u64;
        for k in 0..piece.len {
            let (r, c) = if piece.horizontal {
                (piece.lane, p + k)
            } else {
                (p + k, piece.lane)
            };
            m |= 1u64 << (r as u32 * 8 + c as u32);
        }
        m
    }

    fn occupancy(&self, state: u64) -> u64 {
        (0..self.count as usize).fold(0, |occ, i| occ | self.mask(i, pos(state, i)))
    }

    /// Piece covering `(row, col)`, if any.
    fn piece_at(&self, state: u64, row: u8, col: u8) -> Option<usize> {
        let bit = 1u64 << (row as u32 * 8 + col as u32);
        (0..self.count as usize).find(|&i| self.mask(i, pos(state, i)) & bit != 0)
    }

    /// Whether piece `i` can step one cell by `dir` (-1 or 1).
    fn can_step(&self, state: u64, i: usize, dir: i32) -> bool {
        let p = pos(state, i) as i32 + dir;
        if p < 0 || p + self.pieces[i].len as i32 > self.n as i32 {
            return false;
        }
        let others = self.occupancy(state) & !self.mask(i, pos(state, i));
        others & self.mask(i, p as u8) == 0
    }
}

/// Each piece's cells at every position along its lane, precomputed once per
/// layout so the search only does lookups.
struct MaskTable {
    count: usize,
    /// Furthest position along the lane.
    max: [u8; MAX_PIECES],
    masks: [[u64; 8]; MAX_PIECES],
}

impl MaskTable {
    fn new(layout: &Layout) -> Self {
        let mut t = Self { count: layout.count as usize, max: [0; MAX_PIECES], masks: [[0; 8]; MAX_PIECES] };
        for i in 0..t.count {
            t.max[i] = layout.n - layout.pieces[i].len;
            for p in 0..=t.max[i] {
                t.masks[i][p as usize] = layout.mask(i, p);
            }
        }
        t
    }

    /// Every state one move away: each piece slid any distance along its lane.
    fn for_each_neighbor(&self, state: u64, mut f: impl FnMut(u64)) {
        let mut occ = 0;
        for i in 0..self.count {
            occ |= self.masks[i][pos(state, i) as usize];
        }
        for i in 0..self.count {
            let m = &self.masks[i];
            let p = pos(state, i);
            let others = occ & !m[p as usize];
            let mut q = p;
            while q > 0 && others & m[q as usize - 1] == 0 {
                q -= 1;
                f(set_pos(state, i, q));
            }
            let mut q = p;
            while q < self.max[i] && others & m[q as usize + 1] == 0 {
                q += 1;
                f(set_pos(state, i, q));
            }
        }
    }
}

fn pos(state: u64, i: usize) -> u8 {
    ((state >> (3 * i)) & 7) as u8
}

fn set_pos(state: u64, i: usize, p: u8) -> u64 {
    let shift = 3 * i;
    (state & !(7u64 << shift)) | ((p as u64) << shift)
}

/// Board size, starting piece count and target par for a level (1-based).
fn level_params(level: u32) -> (u8, u8, u8) {
    match level {
        1..=4 => (6, 10, 4 + 3 * level as u8),
        5..=8 => (7, 15, 2 * level as u8),
        _ => (8, 19, (3 + level).min(22) as u8),
    }
}

/// Size tier of a board, used to weight rewards.
fn board_points(n: u8) -> u32 {
    (n - 5) as u32
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Working memory for one generation attempt. `static mut` because it is far
/// too large for the stack or the scene struct.
struct Scratch {
    /// Every arrangement found, in BFS discovery order.
    states: [u64; MAX_STATES],
    /// Open-addressing index into `states`, stored plus one so the
    /// all-zero buffer is empty and SCRATCH lands in `.bss`.
    hash: [u16; HASH_SIZE],
    /// Moves from the nearest solved arrangement.
    dist: [u8; MAX_STATES],
    queue: [u16; MAX_STATES],
    len: usize,
}

static mut SCRATCH: Scratch = Scratch {
    states: [0; MAX_STATES],
    hash: [0; HASH_SIZE],
    dist: [0; MAX_STATES],
    queue: [0; MAX_STATES],
    len: 0,
};

impl Scratch {
    fn clear(&mut self) {
        self.hash.fill(0);
        self.len = 0;
    }

    fn slot(&self, state: u64) -> usize {
        // 32-bit mixing; a 64-bit multiply is costly on the C6's rv32 core.
        let folded = state as u32 ^ ((state >> 32) as u32).rotate_left(16);
        let mut h = (folded.wrapping_mul(0x9E37_79B1) >> 16) as usize & (HASH_SIZE - 1);
        loop {
            let idx = self.hash[h] as usize;
            if idx == 0 || self.states[idx - 1] == state {
                return h;
            }
            h = (h + 1) & (HASH_SIZE - 1);
        }
    }

    fn index_of(&self, state: u64) -> Option<usize> {
        let idx = self.hash[self.slot(state)] as usize;
        idx.checked_sub(1)
    }

    /// Add `state` if it is new. False once the buffer is full.
    fn insert(&mut self, state: u64) -> bool {
        let h = self.slot(state);
        if self.hash[h] != 0 {
            return true;
        }
        if self.len == MAX_STATES {
            return false;
        }
        self.states[self.len] = state;
        self.len += 1;
        self.hash[h] = self.len as u16;
        true
    }

    /// Enumerate every arrangement reachable from `start`. False if there
    /// are more than fit.
    fn explore(&mut self, table: &MaskTable, start: u64) -> bool {
        self.clear();
        self.insert(start);
        let mut head = 0;
        let mut ok = true;
        while ok && head < self.len {
            let s = self.states[head];
            head += 1;
            table.for_each_neighbor(s, |next| ok &= self.insert(next));
        }
        ok
    }

    /// BFS outward from every solved arrangement. Returns the largest
    /// distance found.
    fn measure(&mut self, layout: &Layout, table: &MaskTable) -> u8 {
        let mut tail = 0;
        for i in 0..self.len {
            if layout.is_solved(self.states[i]) {
                self.dist[i] = 0;
                self.queue[tail] = i as u16;
                tail += 1;
            } else {
                self.dist[i] = UNSEEN;
            }
        }
        let mut head = 0;
        let mut max = 0;
        while head < tail {
            let i = self.queue[head] as usize;
            head += 1;
            let d = self.dist[i] + 1;
            table.for_each_neighbor(self.states[i], |next| {
                // Every neighbour is in the component, so the lookup can't miss.
                if let Some(j) = self.index_of(next) {
                    if self.dist[j] == UNSEEN {
                        self.dist[j] = d;
                        self.queue[tail] = j as u16;
                        tail += 1;
                        max = d;
                    }
                }
            });
        }
        max
    }

    /// A random arrangement exactly `d` moves from solved.
    fn pick_at(&self, d: u8, rng: &mut u32) -> u64 {
        let count = self.dist[..self.len].iter().filter(|&&x| x == d).count() as u32;
        let mut k = rand_range_u32(rng, 0, count.max(1) - 1);
        for i in 0..self.len {
            if self.dist[i] == d {
                if k == 0 {
                    return self.states[i];
                }
                k -= 1;
            }
        }
        self.states[0]
    }
}

/// Try to drop one random piece into a free spot of the solved arrangement.
fn add_piece(layout: &mut Layout, state: &mut u64, rng: &mut u32) {
    if layout.count as usize == MAX_PIECES {
        return;
    }
    let n = layout.n;
    let exit_row = layout.exit_row();
    let occ = layout.occupancy(*state);
    for _ in 0..30 {
        let horizontal = xorshift32(rng) & 1 == 0;
        // Bigger boards get more long pieces to stay crowded.
        let len = if rand_range_u32(rng, 0, 11) < 3 * (n as u32 - 5) { 3 } else { 2 };
        let lane = rand_range_u32(rng, 0, n as u32 - 1) as u8;
        // A sideways piece in the bed's row could never get out of its way.
        if horizontal && lane == exit_row {
            continue;
        }
        let p = rand_range_u32(rng, 0, (n - len) as u32) as u8;
        let i = layout.count as usize;
        layout.pieces[i] = Piece { horizontal, len, lane };
        if occ & layout.mask(i, p) == 0 {
            *state = set_pos(*state, i, p);
            layout.count += 1;
            return;
        }
    }
}

/// Remove a random piece other than the bed, moving the last piece into
/// its slot.
fn remove_piece(layout: &mut Layout, state: &mut u64, rng: &mut u32) {
    if layout.count < 2 {
        return;
    }
    let last = layout.count as usize - 1;
    let i = rand_range_u32(rng, 1, last as u32) as usize;
    layout.pieces[i] = layout.pieces[last];
    *state = set_pos(set_pos(*state, i, pos(*state, last)), last, 0);
    layout.count -= 1;
}

/// Hill-climbing search for a hard puzzle. It works on the solved
/// arrangement: each step adds, removes or moves one piece, and the change
/// stays unless the hardest reachable arrangement got easier.
#[derive(Clone, Copy)]
struct Climber {
    layout: Layout,
    solved: u64,
    /// Most moves any arrangement of `layout` needs, or None while it has
    /// too many arrangements to search.
    depth: Option<u8>,
}

impl Climber {
    fn new(n: u8, pieces: u8, rng: &mut u32) -> Self {
        let mut layout = Layout { n, count: 1, pieces: [NO_PIECE; MAX_PIECES] };
        layout.pieces[GOAL] = Piece { horizontal: true, len: 2, lane: layout.exit_row() };
        let mut solved = set_pos(0, GOAL, n - 2);
        for _ in 1..pieces {
            add_piece(&mut layout, &mut solved, rng);
        }
        let depth = depth_of(&layout, solved);
        Self { layout, solved, depth }
    }

    fn step(&mut self, rng: &mut u32) {
        let mut layout = self.layout;
        let mut solved = self.solved;
        // Too loose to search yet: crowd it until it fits, re-rolling a
        // piece once there is no room for more.
        if self.depth.is_none() {
            if layout.count as usize == MAX_PIECES {
                remove_piece(&mut layout, &mut solved, rng);
            }
            add_piece(&mut layout, &mut solved, rng);
            *self = Self { layout, solved, depth: depth_of(&layout, solved) };
            return;
        }
        match rand_range_u32(rng, 0, 2) {
            0 => remove_piece(&mut layout, &mut solved, rng),
            1 => add_piece(&mut layout, &mut solved, rng),
            _ => {
                remove_piece(&mut layout, &mut solved, rng);
                add_piece(&mut layout, &mut solved, rng);
            }
        }
        let depth = depth_of(&layout, solved);
        if depth >= self.depth {
            *self = Self { layout, solved, depth };
        }
    }

    /// The puzzle: a random arrangement `min(target, depth)` moves from
    /// solved, as (start state, par). None if no arrangement needs a move.
    fn puzzle(&self, target: u8, rng: &mut u32) -> Option<(u64, u8)> {
        let par = depth_of(&self.layout, self.solved)?.min(target);
        if par == 0 {
            return None;
        }
        Some((scratch().pick_at(par, rng), par))
    }
}

/// A puzzle ready to play.
#[derive(Clone, Copy)]
struct Puzzle {
    layout: Layout,
    start: u64,
    /// Fewest moves that solve it.
    par: u8,
}

/// Builds the puzzle for one level, a step at a time.
struct Generator {
    level: u32,
    climber: Option<Climber>,
    /// The climber came from the previous level's search.
    carried: bool,
    steps: u32,
    /// Time spent searching, for the timing log.
    busy_ms: u64,
    ready: Option<Puzzle>,
}

impl Generator {
    fn new(level: u32) -> Self {
        Self { level, climber: None, carried: false, steps: 0, busy_ms: 0, ready: None }
    }

    /// A search for `level` that carries on from `prev`'s layout when the
    /// board is the same size, so it starts near the last difficulty.
    fn follow(level: u32, prev: &Generator) -> Self {
        let mut next = Self::new(level);
        if level_params(level).0 == level_params(prev.level).0 {
            next.climber = prev.climber;
            next.carried = next.climber.is_some();
        }
        next
    }

    /// Search until the puzzle is ready or `budget_ms` has passed.
    fn run(&mut self, rng: &mut u32, budget_ms: u64) {
        if self.ready.is_some() {
            return;
        }
        let started = Instant::now();
        while self.ready.is_none() {
            self.step(rng);
            if started.elapsed().as_millis() >= budget_ms {
                break;
            }
        }
        self.busy_ms += started.elapsed().as_millis();
        if let Some(p) = self.ready {
            println!(
                "[SunnySpot] level {} ready: par {} after {} steps, {} ms",
                self.level, p.par, self.steps, self.busy_ms
            );
        }
    }

    fn step(&mut self, rng: &mut u32) {
        let (n, pieces, target) = level_params(self.level);
        let min_steps = if self.carried { MIN_STEPS } else { 0 };
        let climber = self.climber.get_or_insert_with(|| Climber::new(n, pieces, rng));
        let reached = climber.depth >= Some(target) && self.steps >= min_steps;
        if !reached && self.steps < MAX_STEPS {
            climber.step(rng);
            self.steps += 1;
            return;
        }
        // The climb never loses ground, so where it stands is the best found.
        match climber.puzzle(target, rng) {
            Some((start, par)) => self.ready = Some(Puzzle { layout: climber.layout, start, par }),
            // Nothing to solve at all; start over.
            None => {
                self.climber = None;
                self.carried = false;
                self.steps = 0;
            }
        }
    }
}

fn scratch() -> &'static mut Scratch {
    // SAFETY: generation runs on the single game-loop thread, one step at a
    // time, and nothing else touches SCRATCH.
    unsafe { &mut *core::ptr::addr_of_mut!(SCRATCH) }
}

/// Most moves any arrangement of `layout` reachable from `solved` needs,
/// leaving the distances in SCRATCH. None if there are too many to fit.
fn depth_of(layout: &Layout, solved: u64) -> Option<u8> {
    let table = MaskTable::new(layout);
    let s = scratch();
    s.explore(&table, solved).then(|| s.measure(layout, &table))
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Waiting on the puzzle search.
    Generating,
    Playing,
    /// The bed is sliding out into the sun.
    Exiting,
    Won,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SunnySpotAction {
    Retry,
    NewBoard,
}

const OPTIONS_ITEMS: &[MenuItem<SunnySpotAction>] = &[
    MenuItem { label: t!("Retry"),     icon: None, submenu: None, action: Some(SunnySpotAction::Retry),    confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("New Board"), icon: None, submenu: None, action: Some(SunnySpotAction::NewBoard), confirm: None, confirm_on_vacation: None },
];

pub struct SunnySpotScene {
    character: Character,
    win_popup: Popup,

    level: u32,
    puzzle: Puzzle,
    board: u64,
    moves: u32,
    cursor_row: u8,
    cursor_col: u8,
    /// Piece being slid, and where it was picked up.
    held: Option<(usize, u8)>,
    /// (piece, position before the move).
    undo: [(u8, u8); UNDO_LEN],
    undo_len: usize,
    exit_offset: f32,
    state: State,
    /// Search for the current level's puzzle while generating, then for the
    /// next level's during play.
    generator: Generator,
    /// Board-size points of puzzles solved this visit, for the exit rewards.
    session_points: u32,

    options_menu: Menu<SunnySpotAction>,
    menu_active: bool,
}

impl SunnySpotScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(CAT_X, 63)),
            win_popup: Popup::new(10, 8, 108, 48),

            level: 1,
            puzzle: Puzzle {
                layout: Layout { n: 6, count: 0, pieces: [NO_PIECE; MAX_PIECES] },
                start: 0,
                par: 0,
            },
            board: 0,
            moves: 0,
            cursor_row: 0,
            cursor_col: 0,
            held: None,
            undo: [(0, 0); UNDO_LEN],
            undo_len: 0,
            exit_offset: 0.0,
            state: State::Generating,
            generator: Generator::new(1),
            session_points: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    /// Wait for a puzzle for the current level. The pending search is kept
    /// if it is for this level, unless `fresh` asks for a new board; a new
    /// search carries on from the current one's layout.
    fn start_generating(&mut self, fresh: bool) {
        if fresh || self.generator.level != self.level {
            self.generator = Generator::follow(self.level, &self.generator);
        }
        self.state = State::Generating;
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn reset_board(&mut self) {
        self.board = self.puzzle.start;
        self.moves = 0;
        self.held = None;
        self.undo_len = 0;
        self.exit_offset = 0.0;
        self.cursor_row = self.puzzle.layout.exit_row();
        self.cursor_col = pos(self.board, GOAL);
        self.state = State::Playing;
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn push_undo(&mut self, piece: usize, from: u8) {
        if self.undo_len == UNDO_LEN {
            self.undo.copy_within(1.., 0);
            self.undo_len -= 1;
        }
        self.undo[self.undo_len] = (piece as u8, from);
        self.undo_len += 1;
    }

    fn pop_undo(&mut self) {
        if self.undo_len == 0 {
            return;
        }
        self.undo_len -= 1;
        let (piece, from) = self.undo[self.undo_len];
        self.board = set_pos(self.board, piece as usize, from);
        self.moves = self.moves.saturating_sub(1);
    }

    /// Put the held piece down, counting a move if it went anywhere.
    fn drop_held(&mut self) {
        if let Some((piece, from)) = self.held.take() {
            if pos(self.board, piece) != from {
                self.moves += 1;
                self.push_undo(piece, from);
            }
        }
    }

    fn move_cursor(&mut self, buttons: &mut Buttons) {
        let n = self.puzzle.layout.n;
        if buttons.was_just_pressed(Button::Up) && self.cursor_row > 0 {
            self.cursor_row -= 1;
        } else if buttons.was_just_pressed(Button::Down) && self.cursor_row < n - 1 {
            self.cursor_row += 1;
        } else if buttons.was_just_pressed(Button::Left) && self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if buttons.was_just_pressed(Button::Right) && self.cursor_col < n - 1 {
            self.cursor_col += 1;
        }
    }

    /// Slide the held piece one cell along its lane; the cursor rides along.
    fn slide_held(&mut self, piece: usize, buttons: &mut Buttons) {
        let horizontal = self.puzzle.layout.pieces[piece].horizontal;
        let dir = if horizontal {
            if buttons.was_just_pressed(Button::Left) {
                -1
            } else if buttons.was_just_pressed(Button::Right) {
                1
            } else {
                0
            }
        } else if buttons.was_just_pressed(Button::Up) {
            -1
        } else if buttons.was_just_pressed(Button::Down) {
            1
        } else {
            0
        };
        if dir == 0 || !self.puzzle.layout.can_step(self.board, piece, dir) {
            return;
        }
        let p = (pos(self.board, piece) as i32 + dir) as u8;
        self.board = set_pos(self.board, piece, p);
        if horizontal {
            self.cursor_col = (self.cursor_col as i32 + dir) as u8;
        } else {
            self.cursor_row = (self.cursor_row as i32 + dir) as u8;
        }

        if piece == GOAL && self.puzzle.layout.is_solved(self.board) {
            self.drop_held();
            self.state = State::Exiting;
            self.exit_offset = 0.0;
            self.character.set_pose(PoseId::SittingSideHappy);
        }
    }

    fn finish(&mut self) {
        self.state = State::Won;
        self.session_points += board_points(self.puzzle.layout.n);

        let mut text: String<64> = String::new();
        let _ = text.push_str(t!("Nap time!"));
        let _ = text.push('\n');
        let mut num: String<8> = String::new();
        let _ = write!(num, "{}", self.moves);
        crate::i18n::substitute(&mut text, t!("Moves: {n}"), &[("n", num.as_str())]);
        let _ = text.push('\n');
        let _ = text.push_str(t!("Par:"));
        let _ = write!(text, " {}\n", self.puzzle.par);
        let _ = text.push_str(t!("A: Next"));
        self.win_popup.set_text(text.as_str(), false, true);
    }

    fn open_menu(&mut self) {
        self.menu_active = true;
        self.options_menu.reset_to(OPTIONS_ITEMS);
    }

    fn handle_menu(&mut self, buttons: &mut Buttons) {
        match self.options_menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => {
                self.menu_active = false;
            }
            MenuResult::Action(action) => {
                self.menu_active = false;
                match action {
                    SunnySpotAction::Retry => self.reset_board(),
                    SunnySpotAction::NewBoard => self.start_generating(true),
                }
            }
        }
    }

    fn cell_px(&self) -> i32 {
        match self.puzzle.layout.n {
            6 => 10,
            7 => 8,
            _ => 7,
        }
    }

    /// Top-left of the board's outer border.
    fn origin(&self) -> (i32, i32) {
        let span = self.puzzle.layout.n as i32 * self.cell_px() + 2;
        (0, (64 - span) / 2)
    }

    fn draw_board(&self, r: &mut Renderer) {
        let cell = self.cell_px();
        let n = self.puzzle.layout.n as i32;
        let (ox, oy) = self.origin();
        let span = n * cell + 2;
        r.draw_rect(Point::new(ox, oy), Size::new(span as u32, span as u32), false);

        // Gap in the right wall for the bed to leave through.
        let exit_y = oy + 1 + self.puzzle.layout.exit_row() as i32 * cell;
        r.fill_rect_off(Point::new(ox + span - 1, exit_y + 1), Size::new(1, cell as u32 - 2));

        let playing = self.state == State::Playing;
        for i in 0..self.puzzle.layout.count as usize {
            let piece = self.puzzle.layout.pieces[i];
            let p = pos(self.board, i) as i32;
            let (row, col) = if piece.horizontal {
                (piece.lane as i32, p)
            } else {
                (p, piece.lane as i32)
            };
            let mut x = ox + 1 + col * cell + 1;
            let y = oy + 1 + row * cell + 1;
            if i == GOAL {
                x += self.exit_offset as i32;
            }
            let long = piece.len as i32 * cell - 2;
            let (w, h) = if piece.horizontal { (long, cell - 2) } else { (cell - 2, long) };
            let held = self.held.map_or(false, |(h, _)| h == i);
            draw_piece(r, x, y, w, h, i == GOAL, held);
        }

        // Dot on the cursor cell, cleared around so it shows over pieces
        // too and moves visibly while crossing a long one.
        if playing {
            let dot = if cell >= 8 { 2 } else { 1 };
            let inset = (cell - dot) / 2;
            let cx = ox + 1 + self.cursor_col as i32 * cell + inset;
            let cy = oy + 1 + self.cursor_row as i32 * cell + inset;
            let halo = (dot + 2) as u32;
            r.fill_rect_off(Point::new(cx - 1, cy - 1), Size::new(halo, halo));
            r.draw_rect(Point::new(cx, cy), Size::new(dot as u32, dot as u32), true);
        }
    }

    fn draw_panel(&self, r: &mut Renderer) {
        let mut buf: String<24> = String::new();
        let mut num: String<8> = String::new();
        let _ = write!(num, "{}", self.level);
        crate::i18n::substitute(&mut buf, t!("Level {n}"), &[("n", num.as_str())]);
        r.draw_text_small(buf.as_str(), Point::new(PANEL_X, 2));

        if self.state == State::Generating {
            r.draw_text("...", Point::new(PANEL_X, 9));
            return;
        }

        buf.clear();
        num.clear();
        let _ = write!(num, "{}", self.moves);
        crate::i18n::substitute(&mut buf, t!("Moves: {n}"), &[("n", num.as_str())]);
        r.draw_text_small(buf.as_str(), Point::new(PANEL_X, 9));

        buf.clear();
        let _ = buf.push_str(t!("Par:"));
        let _ = write!(buf, " {}", self.puzzle.par);
        r.draw_text_small(buf.as_str(), Point::new(PANEL_X, 16));
    }
}

/// A piece: hollow furniture, or the solid bed. The held one gets a checker
/// fill.
fn draw_piece(r: &mut Renderer, x: i32, y: i32, w: i32, h: i32, goal: bool, held: bool) {
    let (pos, size) = (Point::new(x, y), Size::new(w as u32, h as u32));
    r.draw_rect(pos, size, goal && !held);
    if held {
        for py in y + 1..y + h - 1 {
            for px in x + 1..x + w - 1 {
                r.draw_pixel(Point::new(px, py), (px + py) % 2 == 0);
            }
        }
    }
}

impl Scene for SunnySpotScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.level = 1;
        self.session_points = 0;
        self.menu_active = false;
        self.generator = Generator::new(1);
        self.start_generating(false);
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

        let mut rng = ctx.rng;
        if self.state == State::Generating {
            self.generator.run(&mut rng, FOREGROUND_BUDGET_MS);
            if let Some(puzzle) = self.generator.ready.take() {
                self.puzzle = puzzle;
                self.reset_board();
                self.generator = Generator::follow(self.level + 1, &self.generator);
            }
        } else {
            self.generator.run(&mut rng, BACKGROUND_BUDGET_MS);
        }
        ctx.rng = rng;

        if self.menu_active {
            self.handle_menu(buttons);
            return None;
        }

        match self.state {
            State::Generating => {}
            State::Exiting => {
                self.exit_offset += EXIT_SPEED * dt;
                if self.exit_offset >= (2 * self.cell_px()) as f32 {
                    self.finish();
                }
            }
            State::Won => {
                if buttons.was_just_pressed(Button::A) {
                    self.level += 1;
                    self.start_generating(false);
                }
            }
            State::Playing => {
                if buttons.was_just_pressed(Button::Menu2) {
                    self.drop_held();
                    self.open_menu();
                    return None;
                }
                match self.held {
                    Some((piece, _)) => {
                        if buttons.was_just_pressed(Button::A) || buttons.was_just_pressed(Button::B) {
                            self.drop_held();
                        } else {
                            self.slide_held(piece, buttons);
                        }
                    }
                    None => {
                        if buttons.was_just_pressed(Button::A) {
                            self.held = self
                                .puzzle
                                .layout
                                .piece_at(self.board, self.cursor_row, self.cursor_col)
                                .map(|i| (i, pos(self.board, i)));
                        } else if buttons.was_just_pressed(Button::B) {
                            self.pop_undo();
                        } else {
                            self.move_cursor(buttons);
                        }
                    }
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

        if self.state != State::Generating {
            self.draw_board(renderer);
        }
        self.draw_panel(renderer);
        self.character.draw(renderer, 0);
        // Rim after the cat so it sits inside the bed.
        draw_cat_bed_rim(renderer, CAT_BED_X);

        if self.state == State::Won {
            self.win_popup.draw(renderer, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    /// Fewest moves to solve, by a plain BFS forward from `start`.
    fn solve(layout: &Layout, start: u64) -> u32 {
        let table = MaskTable::new(layout);
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, 0)]);
        while let Some((s, d)) = queue.pop_front() {
            if layout.is_solved(s) {
                return d;
            }
            table.for_each_neighbor(s, |next| {
                if seen.insert(next) {
                    queue.push_back((next, d + 1));
                }
            });
        }
        panic!("unsolvable puzzle");
    }

    #[test]
    fn generated_par_is_the_shortest_solution() {
        // Levels in order, as play goes: fresh searches at each board size
        // and carried-over ones in between.
        let mut rng = 0x1234_5678;
        let mut generator = Generator::new(1);
        for level in 1..=10 {
            generator = Generator::follow(level, &generator);
            while generator.ready.is_none() {
                generator.step(&mut rng);
            }
            let puzzle = generator.ready.unwrap();
            assert!(!puzzle.layout.is_solved(puzzle.start));
            assert_eq!(solve(&puzzle.layout, puzzle.start), puzzle.par as u32);
        }
    }
}
