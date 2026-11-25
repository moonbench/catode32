//! Hide & Seek minigame, a cat's take on Battleship. You and your pet each
//! hide four napping boxes (3, 2, 2 and 1 cells long) in an 8x8 yard, then
//! take turns pouncing on squares. Whoever finds all the other's boxes first
//! wins the round.
//!
//! The pet only sees what its own pounces turned up. How it picks squares
//! depends on its intelligence: kittens paw around their hits, clever pets
//! hunt on a checkerboard and follow lines, and geniuses count every way the
//! hidden boxes could still fit.
//!
//! Two 57px grids sit side by side, your yard on the left and the pet's on
//! the right, under a 6px status strip.

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
    rand::rand_range_u32,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::popup::Popup,
};

const N: usize = 8;
const CELLS: usize = N * N;

/// Box lengths, hidden largest first.
const BOXES: [usize; 4] = [3, 2, 2, 1];
const NBOX: usize = BOXES.len();

// Grid geometry: 6px cells separated by 1px lines, two grids side by side.
const PITCH: i32 = 7;
const CELL: i32 = 6;
const SPAN: i32 = N as i32 * PITCH;
const LEFT_X: i32 = 2;
const RIGHT_X: i32 = 69;
const GRID_Y: i32 = 7;
/// Left edge of the hint panel beside your yard while hiding.
const PANEL_X: i32 = 64;
/// FONT_4X6 advance.
const SMALL_CHAR_W: i32 = 4;

// What the seeker knows about a cell.
const UNKNOWN: u8 = 0;
const MISS: u8 = 1;
const HIT: u8 = 2;
/// Hit, and part of a box that has been found. Only appears in a `View`.
const FOUND: u8 = 3;

const PET_THINK_DELAY: f32 = 0.6;
const RESULT_DELAY: f32 = 0.7;
const END_DELAY: f32 = 0.5;
/// Half-period of the ghost box and pet pounce blink.
const BLINK: f32 = 0.15;

/// Pets at or above these intelligences play the stronger AIs.
const CLEVER_INTELLIGENCE: f32 = 40.0;
const GENIUS_INTELLIGENCE: f32 = 75.0;
/// How often a kitten forgets its hits and pounces anywhere.
const KITTEN_WANDER_PCT: u32 = 20;

/// The cell `(dr, dc)` away from `cell`, if it's on the board.
fn step(cell: usize, dr: i32, dc: i32) -> Option<usize> {
    let r = (cell / N) as i32 + dr;
    let c = (cell % N) as i32 + dc;
    if r >= 0 && r < N as i32 && c >= 0 && c < N as i32 {
        Some(r as usize * N + c as usize)
    } else {
        None
    }
}

fn neighbours(cell: usize) -> impl Iterator<Item = usize> {
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .filter_map(move |(dr, dc)| step(cell, dr, dc))
}

/// Whether a box of `len` starting at `cell` stays on the board.
fn fits(cell: usize, len: usize, horiz: bool) -> bool {
    let (r, c) = (cell / N, cell % N);
    if horiz { c + len <= N } else { r + len <= N }
}

/// The cells a box of `len` starting at `cell` covers. Check `fits` first.
fn span(cell: usize, len: usize, horiz: bool) -> impl Iterator<Item = usize> {
    let stride = if horiz { 1 } else { N };
    (0..len).map(move |i| cell + i * stride)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Placement {
    cell: u8,
    len: u8,
    horiz: bool,
}

impl Placement {
    const NONE: Placement = Placement { cell: 0, len: 0, horiz: true };

    fn cells(self) -> impl Iterator<Item = usize> {
        span(self.cell as usize, self.len as usize, self.horiz)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Miss,
    Hit,
    /// The pounce uncovered the last cell of this box.
    Found(u8),
}

/// One side's yard: where its boxes are and where it has been pounced on.
#[derive(Clone)]
struct Yard {
    /// 0 for open grass, otherwise box index + 1.
    owner: [u8; CELLS],
    /// UNKNOWN, MISS or HIT.
    shot: [u8; CELLS],
    boxes: [Placement; NBOX],
    placed: usize,
}

impl Yard {
    fn new() -> Self {
        Self {
            owner: [0; CELLS],
            shot: [UNKNOWN; CELLS],
            boxes: [Placement::NONE; NBOX],
            placed: 0,
        }
    }

    /// Length of the next box to hide, if any are left.
    fn next_len(&self) -> Option<usize> {
        BOXES.get(self.placed).copied()
    }

    fn can_place(&self, cell: usize, len: usize, horiz: bool) -> bool {
        fits(cell, len, horiz) && span(cell, len, horiz).all(|c| self.owner[c] == 0)
    }

    /// Hide the next box at `cell`. Boxes may touch but not overlap.
    fn place(&mut self, cell: usize, horiz: bool) -> bool {
        let Some(len) = self.next_len() else { return false };
        if !self.can_place(cell, len, horiz) {
            return false;
        }
        for c in span(cell, len, horiz) {
            self.owner[c] = self.placed as u8 + 1;
        }
        self.boxes[self.placed] = Placement { cell: cell as u8, len: len as u8, horiz };
        self.placed += 1;
        true
    }

    /// Hide every remaining box somewhere random.
    fn place_random(&mut self, rng: &mut u32) {
        while self.next_len().is_some() {
            let horiz = rand_range_u32(rng, 0, 1) == 0;
            let cell = rand_range_u32(rng, 0, CELLS as u32 - 1) as usize;
            self.place(cell, horiz);
        }
    }

    fn pounce(&mut self, cell: usize) -> Outcome {
        match self.owner[cell] {
            0 => {
                self.shot[cell] = MISS;
                Outcome::Miss
            }
            o => {
                self.shot[cell] = HIT;
                let b = o as usize - 1;
                if self.is_found(b) { Outcome::Found(b as u8) } else { Outcome::Hit }
            }
        }
    }

    fn is_found(&self, b: usize) -> bool {
        b < self.placed && self.boxes[b].cells().all(|c| self.shot[c] == HIT)
    }

    fn all_found(&self) -> bool {
        self.placed == NBOX && (0..NBOX).all(|b| self.is_found(b))
    }

    /// What the seeker knows: pounce results, found boxes, and the lengths
    /// still hidden. Never where the hidden boxes are.
    fn view(&self) -> View {
        let mut view = View { cell: self.shot, hidden: [0; NBOX], n_hidden: 0 };
        for b in 0..NBOX {
            if self.is_found(b) {
                for c in self.boxes[b].cells() {
                    view.cell[c] = FOUND;
                }
            } else {
                view.hidden[view.n_hidden] = BOXES[b] as u8;
                view.n_hidden += 1;
            }
        }
        view
    }
}

/// The seeker's knowledge of a yard. The pet AI works from this alone.
#[derive(Clone, PartialEq, Eq, Debug)]
struct View {
    /// UNKNOWN, MISS, HIT (box not yet found) or FOUND.
    cell: [u8; CELLS],
    hidden: [u8; NBOX],
    n_hidden: usize,
}

impl View {
    fn hidden(&self) -> &[u8] {
        &self.hidden[..self.n_hidden]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PetLevel {
    /// Random pounces, pawing around hits most of the time.
    Kitten,
    /// Checkerboard hunting, then following lines of hits.
    Clever,
    /// Pounces where the most possible box placements overlap.
    Genius,
}

fn pet_level(intelligence: f32) -> PetLevel {
    if intelligence >= GENIUS_INTELLIGENCE {
        PetLevel::Genius
    } else if intelligence >= CLEVER_INTELLIGENCE {
        PetLevel::Clever
    } else {
        PetLevel::Kitten
    }
}

/// A set of candidate cells to pick from at random.
struct Picks {
    buf: [u8; CELLS],
    len: usize,
}

impl Picks {
    fn new() -> Self {
        Self { buf: [0; CELLS], len: 0 }
    }

    fn push(&mut self, cell: usize) {
        if !self.buf[..self.len].contains(&(cell as u8)) {
            self.buf[self.len] = cell as u8;
            self.len += 1;
        }
    }

    fn pick(&self, rng: &mut u32) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        let i = rand_range_u32(rng, 0, self.len as u32 - 1) as usize;
        Some(self.buf[i] as usize)
    }
}

fn unknown_cells(view: &View) -> Picks {
    let mut out = Picks::new();
    for i in 0..CELLS {
        if view.cell[i] == UNKNOWN {
            out.push(i);
        }
    }
    out
}

/// Unknown cells next to a hit on a box that hasn't been found yet.
fn neighbour_targets(view: &View) -> Picks {
    let mut out = Picks::new();
    for i in 0..CELLS {
        if view.cell[i] == HIT {
            for n in neighbours(i) {
                if view.cell[n] == UNKNOWN {
                    out.push(n);
                }
            }
        }
    }
    out
}

/// Unknown cells at either end of a line of two or more unresolved hits.
fn line_targets(view: &View) -> Picks {
    let mut out = Picks::new();
    let is_hit = |c: Option<usize>| c.map_or(false, |c| view.cell[c] == HIT);
    for i in 0..CELLS {
        if view.cell[i] != HIT {
            continue;
        }
        for (dr, dc) in [(0, 1), (1, 0)] {
            // Start from the first hit of each run.
            if is_hit(step(i, -dr, -dc)) {
                continue;
            }
            let mut end = i;
            while is_hit(step(end, dr, dc)) {
                end = step(end, dr, dc).unwrap();
            }
            if end == i {
                continue;
            }
            for c in [step(i, -dr, -dc), step(end, dr, dc)].into_iter().flatten() {
                if view.cell[c] == UNKNOWN {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Unknown cells on one colour of a checkerboard. Every box of 2+ covers
/// both colours, so this halves the hunt, but only once the 1-cell box
/// is found.
fn parity_hunt(view: &View) -> Picks {
    let smallest = view.hidden().iter().copied().min().unwrap_or(1);
    let mut out = Picks::new();
    for i in 0..CELLS {
        if view.cell[i] == UNKNOWN && (smallest < 2 || (i / N + i % N) % 2 == 0) {
            out.push(i);
        }
    }
    out
}

/// For each unknown cell, how many ways the hidden boxes could cover it.
/// While there are unresolved hits, only placements through them count,
/// weighted towards those covering several.
fn density(view: &View) -> [u16; CELLS] {
    let chasing = view.cell.contains(&HIT);
    let mut score = [0u16; CELLS];
    for &len in view.hidden() {
        let len = len as usize;
        for horiz in [true, false] {
            for start in 0..CELLS {
                if !fits(start, len, horiz) {
                    continue;
                }
                let mut hits = 0u16;
                let mut blocked = false;
                for c in span(start, len, horiz) {
                    match view.cell[c] {
                        MISS | FOUND => blocked = true,
                        HIT => hits += 1,
                        _ => {}
                    }
                }
                if blocked || (chasing && hits == 0) {
                    continue;
                }
                let weight = if chasing { hits * hits } else { 1 };
                for c in span(start, len, horiz) {
                    if view.cell[c] == UNKNOWN {
                        score[c] += weight;
                    }
                }
            }
        }
    }
    score
}

fn densest(view: &View) -> Picks {
    let score = density(view);
    let best = score.iter().copied().max().unwrap_or(0);
    let mut out = Picks::new();
    if best > 0 {
        for i in 0..CELLS {
            if score[i] == best {
                out.push(i);
            }
        }
    }
    out
}

/// Where the pet pounces next. `None` only if every cell has been pounced.
fn choose_pounce(view: &View, level: PetLevel, rng: &mut u32) -> Option<usize> {
    let pick = match level {
        PetLevel::Kitten => {
            if rand_range_u32(rng, 1, 100) > KITTEN_WANDER_PCT {
                neighbour_targets(view).pick(rng)
            } else {
                None
            }
        }
        PetLevel::Clever => line_targets(view)
            .pick(rng)
            .or_else(|| neighbour_targets(view).pick(rng))
            .or_else(|| parity_hunt(view).pick(rng)),
        PetLevel::Genius => densest(view).pick(rng),
    };
    pick.or_else(|| unknown_cells(view).pick(rng))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Hiding,
    PlayerTurn,
    PetTurn,
    /// A pounce just landed; its outcome shows before the turn passes.
    Landed { by_player: bool, outcome: Outcome },
    PlayerWin,
    PetWin,
}

pub struct HideSeekScene {
    character: Character,
    result_popup: Popup,

    round_number: u32,
    player_wins: u32,
    pet_wins: u32,
    level: PetLevel,

    /// Your yard. The pet pounces here.
    home: Yard,
    /// The pet's yard. You pounce here.
    away: Yard,

    hide_cursor: usize,
    hide_horiz: bool,
    aim: usize,
    last_pet_pounce: Option<usize>,

    timer: f32,
    blink: f32,
    state: State,
}

impl HideSeekScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(96, 63)),
            result_popup: Popup::new(14, 12, 100, 40),

            round_number: 0,
            player_wins: 0,
            pet_wins: 0,
            level: PetLevel::Kitten,

            home: Yard::new(),
            away: Yard::new(),

            hide_cursor: 0,
            hide_horiz: true,
            aim: 0,
            last_pet_pounce: None,

            timer: 0.0,
            blink: 0.0,
            state: State::Hiding,
        }
    }

    fn reset_round(&mut self, rng: &mut u32) {
        self.home = Yard::new();
        self.away = Yard::new();
        self.away.place_random(rng);

        self.hide_cursor = 0;
        self.hide_horiz = true;
        self.aim = 3 * N + 3;
        self.last_pet_pounce = None;
        self.timer = 0.0;
        self.state = State::Hiding;
    }

    fn start_battle(&mut self) {
        self.timer = 0.0;
        self.state = if self.round_number % 2 == 0 {
            State::PlayerTurn
        } else {
            State::PetTurn
        };
    }

    /// Keep the box being hidden on the board after a rotate or a new box.
    fn clamp_hide_cursor(&mut self) {
        let Some(len) = self.home.next_len() else { return };
        let (mut r, mut c) = (self.hide_cursor / N, self.hide_cursor % N);
        if self.hide_horiz {
            c = c.min(N - len);
        } else {
            r = r.min(N - len);
        }
        self.hide_cursor = r * N + c;
    }

    fn update_hiding(&mut self, buttons: &mut Buttons, rng: &mut u32) {
        if buttons.was_just_pressed(Button::Menu2) {
            self.home.place_random(rng);
            self.start_battle();
            return;
        }
        let Some(len) = self.home.next_len() else { return };

        if buttons.was_just_pressed(Button::B) {
            self.hide_horiz = !self.hide_horiz;
            self.clamp_hide_cursor();
        }

        if let Some((dr, dc)) = dpad(buttons) {
            if let Some(next) = step(self.hide_cursor, dr, dc) {
                if fits(next, len, self.hide_horiz) {
                    self.hide_cursor = next;
                }
            }
        }

        if buttons.was_just_pressed(Button::A) && self.home.place(self.hide_cursor, self.hide_horiz) {
            if self.home.next_len().is_none() {
                self.start_battle();
            } else {
                self.clamp_hide_cursor();
            }
        }
    }

    fn land(&mut self, by_player: bool, outcome: Outcome) {
        self.timer = 0.0;
        self.blink = 0.0;
        self.state = State::Landed { by_player, outcome };
    }

    fn finish(&mut self, player_won: bool) {
        self.timer = 0.0;
        if player_won {
            self.player_wins += 1;
            self.state = State::PlayerWin;
            self.character.set_pose(PoseId::SittingSideAnnoyed);
        } else {
            self.pet_wins += 1;
            self.state = State::PetWin;
            self.character.set_pose(PoseId::SittingSideHappy);
        }

        let mut msg: String<64> = String::new();
        let _ = msg.push_str(if player_won { t!("You Win!") } else { t!("You Lose!") });
        let _ = write!(msg, "\n{}-{}\n", self.player_wins, self.pet_wins);
        let _ = msg.push_str(t!("A: New Game"));
        self.result_popup.set_text(msg.as_str(), false, true);
    }

    fn blink_on(&self) -> bool {
        self.blink < BLINK
    }

    fn draw_grid(r: &mut Renderer, gx: i32) {
        for i in 0..=N as i32 {
            let x = gx + i * PITCH;
            r.draw_line(Point::new(x, GRID_Y), Point::new(x, GRID_Y + SPAN));
            let y = GRID_Y + i * PITCH;
            r.draw_line(Point::new(gx, y), Point::new(gx + SPAN, y));
        }
    }

    fn draw_hiding(&self, r: &mut Renderer) {
        Self::draw_grid(r, LEFT_X);
        for b in 0..self.home.placed {
            draw_box(r, LEFT_X, self.home.boxes[b], false);
        }

        // The box being hidden: solid where it fits, blinking where it doesn't.
        if let Some(len) = self.home.next_len() {
            let ghost = Placement {
                cell: self.hide_cursor as u8,
                len: len as u8,
                horiz: self.hide_horiz,
            };
            let ok = self.home.can_place(self.hide_cursor, len, self.hide_horiz);
            if ok || self.blink_on() {
                draw_box(r, LEFT_X, ghost, true);
            }
        }

        r.draw_text_small(t!("You"), Point::new(LEFT_X, 0));
        draw_status(r, LEFT_X, t!("Hide!"));

        let mut buf: String<32> = String::new();
        let _ = write!(buf, "{} {}-{} {}", t!("You"), self.player_wins, self.pet_wins, t!("Pet"));
        r.draw_text_small(buf.as_str(), Point::new(PANEL_X, 0));
        r.draw_text_small(t!("A: hide"), Point::new(PANEL_X, 10));
        r.draw_text_small(t!("B: turn"), Point::new(PANEL_X, 17));

        self.character.draw(r, 0);
    }

    fn draw_battle(&self, r: &mut Renderer) {
        Self::draw_grid(r, LEFT_X);
        Self::draw_grid(r, RIGHT_X);

        // Your yard: your boxes, with every pounce the pet has made.
        for b in 0..NBOX {
            draw_box(r, LEFT_X, self.home.boxes[b], self.home.is_found(b));
        }
        for i in 0..CELLS {
            let (x, y) = cell_origin(LEFT_X, i);
            match self.home.shot[i] {
                MISS => draw_glyph(r, x, y, MISS, true),
                HIT => r.draw_rect(Point::new(x + 1, y + 1), Size::new(4, 4), true),
                _ => {}
            }
        }
        if let (State::Landed { by_player: false, .. }, Some(c)) = (self.state, self.last_pet_pounce) {
            if self.blink_on() {
                draw_cursor(r, LEFT_X, c, self.home.view().cell[c]);
            }
        }

        // The pet's yard: what you've found. Its hidden boxes show at the end.
        let view = self.away.view();
        let reveal = matches!(self.state, State::PlayerWin | State::PetWin);
        for b in 0..NBOX {
            if self.away.is_found(b) {
                draw_box(r, RIGHT_X, self.away.boxes[b], true);
            } else if reveal {
                draw_box(r, RIGHT_X, self.away.boxes[b], false);
            }
        }
        for i in 0..CELLS {
            if matches!(view.cell[i], MISS | HIT) {
                let (x, y) = cell_origin(RIGHT_X, i);
                draw_glyph(r, x, y, view.cell[i], true);
            }
        }
        if self.state == State::PlayerTurn {
            draw_cursor(r, RIGHT_X, self.aim, view.cell[self.aim]);
        }

        r.draw_text_small(t!("You"), Point::new(LEFT_X, 0));
        r.draw_text_small(t!("Pet"), Point::new(RIGHT_X, 0));
        match self.state {
            State::PetTurn => draw_status(r, LEFT_X, "..."),
            State::Landed { by_player, outcome } => {
                let text = match outcome {
                    Outcome::Miss => t!("Miss"),
                    Outcome::Hit => t!("Hit!"),
                    Outcome::Found(_) => t!("Found it!"),
                };
                draw_status(r, if by_player { RIGHT_X } else { LEFT_X }, text);
            }
            State::PlayerWin | State::PetWin => self.result_popup.draw(r, false),
            _ => {}
        }
    }
}

fn dpad(buttons: &mut Buttons) -> Option<(i32, i32)> {
    if buttons.was_just_pressed(Button::Up) {
        Some((-1, 0))
    } else if buttons.was_just_pressed(Button::Down) {
        Some((1, 0))
    } else if buttons.was_just_pressed(Button::Left) {
        Some((0, -1))
    } else if buttons.was_just_pressed(Button::Right) {
        Some((0, 1))
    } else {
        None
    }
}

/// Top-left pixel of a cell's 6x6 interior in the grid at `gx`.
fn cell_origin(gx: i32, cell: usize) -> (i32, i32) {
    (
        gx + 1 + (cell % N) as i32 * PITCH,
        GRID_Y + 1 + (cell / N) as i32 * PITCH,
    )
}

/// A box drawn as one shape across its cells, covering the grid lines
/// between them. Filled once found (or as the ghost while hiding).
fn draw_box(r: &mut Renderer, gx: i32, p: Placement, filled: bool) {
    if p.len == 0 {
        return;
    }
    let (x, y) = cell_origin(gx, p.cell as usize);
    let long = p.len as i32 * PITCH - 1;
    let (w, h) = if p.horiz { (long, CELL) } else { (CELL, long) };
    r.fill_rect_off(Point::new(x, y), Size::new(w as u32, h as u32));
    r.draw_rect(Point::new(x + 1, y + 1), Size::new(w as u32 - 2, h as u32 - 2), filled);
}

/// A miss dot or a hit X inside the cell at `(x, y)`.
fn draw_glyph(r: &mut Renderer, x: i32, y: i32, state: u8, on: bool) {
    match state {
        MISS => {
            for (dx, dy) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
                r.draw_pixel(Point::new(x + dx, y + dy), on);
            }
        }
        HIT => {
            for d in 1..5 {
                r.draw_pixel(Point::new(x + d, y + d), on);
                r.draw_pixel(Point::new(x + 5 - d, y + d), on);
            }
        }
        FOUND => {
            if on {
                r.draw_rect(Point::new(x + 1, y + 1), Size::new(4, 4), true);
            } else {
                r.fill_rect_off(Point::new(x + 1, y + 1), Size::new(4, 4));
            }
        }
        _ => {}
    }
}

/// An inverted cell, with its contents drawn dark.
fn draw_cursor(r: &mut Renderer, gx: i32, cell: usize, state: u8) {
    let (x, y) = cell_origin(gx, cell);
    r.draw_rect(Point::new(x, y), Size::new(CELL as u32, CELL as u32), true);
    draw_glyph(r, x, y, state, false);
}

/// Short status text right-aligned above the grid at `gx`.
fn draw_status(r: &mut Renderer, gx: i32, text: &str) {
    let w = text.chars().count() as i32 * SMALL_CHAR_W;
    r.draw_text_small(text, Point::new(gx + SPAN + 1 - w, 0));
}

impl Scene for HideSeekScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.round_number = 0;
        self.player_wins = 0;
        self.pet_wins = 0;
        self.level = pet_level(ctx.intelligence);
        self.character.set_pose(PoseId::SittingSideNeutral);
        let mut rng = ctx.rng;
        self.reset_round(&mut rng);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let ended = matches!(self.state, State::PlayerWin | State::PetWin);
        let total_rounds = self.round_number + if ended { 1 } else { 0 };
        if total_rounds == 0 {
            return;
        }
        let scale = (total_rounds as f32 / 3.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 4.0 * scale),
            (StatId::Focus,        4.0 * scale),
            (StatId::Sociability,  3.0 * scale),
            (StatId::Fulfillment,  3.0 * scale),
            (StatId::Loyalty,      1.0 * scale),
        ]);
        ctx.coins += 4 * total_rounds as i32;
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
        self.blink += dt;
        if self.blink >= 2.0 * BLINK {
            self.blink -= 2.0 * BLINK;
        }

        let mut rng = ctx.rng;
        match self.state {
            State::Hiding => self.update_hiding(buttons, &mut rng),
            State::PlayerTurn => {
                if let Some((dr, dc)) = dpad(buttons) {
                    if let Some(next) = step(self.aim, dr, dc) {
                        self.aim = next;
                    }
                }
                if buttons.was_just_pressed(Button::A) && self.away.shot[self.aim] == UNKNOWN {
                    let outcome = self.away.pounce(self.aim);
                    self.land(true, outcome);
                }
            }
            State::PetTurn => {
                self.timer += dt;
                if self.timer >= PET_THINK_DELAY {
                    let view = self.home.view();
                    if let Some(c) = choose_pounce(&view, self.level, &mut rng) {
                        self.last_pet_pounce = Some(c);
                        let outcome = self.home.pounce(c);
                        self.land(false, outcome);
                    }
                }
            }
            State::Landed { by_player, .. } => {
                self.timer += dt;
                if self.timer >= RESULT_DELAY {
                    self.timer = 0.0;
                    if by_player {
                        if self.away.all_found() {
                            self.finish(true);
                        } else {
                            self.state = State::PetTurn;
                        }
                    } else if self.home.all_found() {
                        self.finish(false);
                    } else {
                        self.state = State::PlayerTurn;
                    }
                }
            }
            State::PlayerWin | State::PetWin => {
                self.timer += dt;
                if buttons.was_just_pressed(Button::A) && self.timer >= END_DELAY {
                    self.round_number += 1;
                    self.reset_round(&mut rng);
                }
            }
        }
        ctx.rng = rng;

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.state == State::Hiding {
            self.draw_hiding(renderer);
        } else {
            self.draw_battle(renderer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_cells(y: &Yard) -> usize {
        y.owner.iter().filter(|&&o| o != 0).count()
    }

    /// Let the pet seek through `yard` until everything's found. Returns the
    /// number of pounces.
    fn play_out(yard: &mut Yard, level: PetLevel, rng: &mut u32) -> usize {
        let mut pounces = 0;
        while !yard.all_found() {
            let view = yard.view();
            let c = choose_pounce(&view, level, rng).expect("cells left to pounce");
            assert_eq!(yard.shot[c], UNKNOWN, "{:?} pounced twice on {}", level, c);
            yard.pounce(c);
            pounces += 1;
            assert!(pounces <= CELLS);
        }
        pounces
    }

    #[test]
    fn random_hiding_places_every_box_without_overlap() {
        let total: usize = BOXES.iter().sum();
        for seed in 1..1000u32 {
            let mut rng = seed;
            let mut y = Yard::new();
            y.place_random(&mut rng);
            assert_eq!(y.placed, NBOX);
            assert_eq!(box_cells(&y), total, "seed {}", seed);
            for b in 0..NBOX {
                assert_eq!(y.boxes[b].len as usize, BOXES[b]);
                for c in y.boxes[b].cells() {
                    assert_eq!(y.owner[c] as usize, b + 1);
                }
            }
        }
    }

    #[test]
    fn placement_respects_bounds_and_overlap() {
        let mut y = Yard::new();
        assert!(!y.place(6, true)); // 3 long from column 6 runs off the edge
        assert!(!y.place(6 * N, false)); // ...and from row 6
        assert!(y.place(0, true)); // 3: cells 0..3
        assert!(!y.place(2, false)); // 2 can't overlap cell 2
        assert!(y.place(3, true)); // 2 may touch: cells 3, 4
        assert!(y.place(N, true)); // 2: cells 8, 9
        assert!(y.place(63, true)); // 1 fits in the corner
        assert!(!y.place(20, true)); // nothing left to hide
        assert_eq!(box_cells(&y), 8);
    }

    #[test]
    fn pounces_report_hits_and_found_boxes() {
        let mut y = Yard::new();
        y.place(0, true); // 3
        y.place(N, true); // 2
        y.place(2 * N, true); // 2
        y.place(63, true); // 1

        assert_eq!(y.pounce(5), Outcome::Miss);
        assert_eq!(y.pounce(63), Outcome::Found(3));
        assert_eq!(y.pounce(0), Outcome::Hit);
        assert_eq!(y.pounce(1), Outcome::Hit);
        assert_eq!(y.pounce(2), Outcome::Found(0));
        assert_eq!(y.pounce(N), Outcome::Hit);
        assert_eq!(y.pounce(N + 1), Outcome::Found(1));
        assert!(!y.all_found());
        assert_eq!(y.pounce(2 * N), Outcome::Hit);
        assert_eq!(y.pounce(2 * N + 1), Outcome::Found(2));
        assert!(y.all_found());

        let v = y.view();
        assert_eq!(v.cell[0], FOUND);
        assert_eq!(v.cell[5], MISS);
        assert!(v.hidden().is_empty());
    }

    #[test]
    fn view_hides_where_unfound_boxes_are() {
        // Different layouts, same pounce results: the pet sees the same thing,
        // so it can't cheat.
        let mut a = Yard::new();
        a.place(0, true);
        a.place(2 * N, true);
        a.place(4 * N, true);
        a.place(6 * N, true);
        let mut b = Yard::new();
        b.place(5, false);
        b.place(4 * N + 6, true);
        b.place(7 * N + 3, true);
        b.place(6 * N + 1, true);
        for c in [N + 3, 3 * N + 3, 5 * N + 2, 7 * N + 7] {
            assert_eq!(a.pounce(c), Outcome::Miss);
            assert_eq!(b.pounce(c), Outcome::Miss);
        }
        assert_eq!(a.view(), b.view());
        assert_eq!(density(&a.view()), density(&b.view()));
    }

    #[test]
    fn every_level_finishes_without_repeating_a_pounce() {
        for level in [PetLevel::Kitten, PetLevel::Clever, PetLevel::Genius] {
            for seed in 1..200u32 {
                let mut rng = seed;
                let mut y = Yard::new();
                y.place_random(&mut rng);
                play_out(&mut y, level, &mut rng);
            }
        }
    }

    #[test]
    fn clever_follows_a_line_of_hits() {
        let mut y = Yard::new();
        y.place(3 * N + 2, true); // 3: cells 26, 27, 28
        y.place(0, true);
        y.place(N * 7, true);
        y.place(7, true);
        y.pounce(3 * N + 3);
        y.pounce(3 * N + 2);
        let view = y.view();
        let targets = line_targets(&view);
        let mut got: [u8; 2] = [targets.buf[0], targets.buf[1]];
        got.sort();
        assert_eq!(targets.len, 2);
        assert_eq!(got, [(3 * N + 1) as u8, (3 * N + 4) as u8]);
    }

    #[test]
    fn intelligence_picks_the_level() {
        assert_eq!(pet_level(0.0), PetLevel::Kitten);
        assert_eq!(pet_level(50.0), PetLevel::Clever);
        assert_eq!(pet_level(90.0), PetLevel::Genius);
    }
}
