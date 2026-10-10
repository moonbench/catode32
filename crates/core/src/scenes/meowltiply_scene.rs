//! Meowltiply minigame, a take on 2048. The d-pad slides every tile as far
//! as it goes; two equal tiles that collide merge into one of double the
//! value. Build a 2048 tile to win, then keep going until the board locks.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::xorshift32,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

const N: usize = 4;
const CELLS: usize = N * N;

const CHAR_W: i32 = 6; // FONT_6X10
const SMALL_CHAR_W: i32 = 4; // FONT_4X6

// Wide tiles suit the 128x64 screen: the grid fills the whole display, so
// values up to 9999 fit in the regular font.
const CELL_W: i32 = 32;
const CELL_H: i32 = 16;
const TILE_W: i32 = CELL_W - 2;
const TILE_H: i32 = CELL_H - 2;

/// Exponent of the winning tile (2048).
const WIN_EXP: u8 = 11;
/// Tiles from this exponent (128) up are drawn filled.
const FILLED_EXP: u8 = 7;

/// Seconds a move takes to slide into place: three frames at the game's
/// 12 FPS, so the motion is actually visible.
const SLIDE_TIME: f32 = 0.24;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Won,
    Lose,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// Where one tile went during the last move. Two slides share a `to` when
/// they merged.
#[derive(Clone, Copy)]
struct Slide {
    from: u8,
    to: u8,
    exp: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MeowltiplyAction {
    NewBoard,
}

const OPTIONS_ITEMS: &[MenuItem<MeowltiplyAction>] = &[
    MenuItem { label: t!("New Board"), icon: None, submenu: None, action: Some(MeowltiplyAction::NewBoard), confirm: None, confirm_on_vacation: None },
];

pub struct MeowltiplyScene {
    popup: Popup,

    /// Tile exponents: 0 is empty, `e` holds the value `2^e`.
    grid: [u8; CELLS],
    state: State,
    score: i32,
    /// Score from finished games this visit, for the exit rewards.
    session_score: i32,
    /// Set once the player chooses to play on past 2048.
    kept_going: bool,
    new_best: bool,

    /// Every tile of the last move. While non-empty the board draws these
    /// mid-slide instead of `grid`, which also hides the freshly spawned tile.
    slides: Vec<Slide, CELLS>,
    anim_t: f32,

    options_menu: Menu<MeowltiplyAction>,
    menu_active: bool,
}

impl MeowltiplyScene {
    pub fn new() -> Self {
        let mut popup = Popup::new(10, 0, 108, 64);
        // Centres the four lines on the full-height card.
        popup.padding = 12;
        Self {
            popup,

            grid: [0; CELLS],
            state: State::Playing,
            score: 0,
            session_score: 0,
            kept_going: false,
            new_best: false,

            slides: Vec::new(),
            anim_t: 0.0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn new_game(&mut self, rng: &mut u32) {
        // Bank the score from the game that just ended before resetting.
        self.session_score += self.score;
        self.grid = [0; CELLS];
        self.score = 0;
        self.state = State::Playing;
        self.kept_going = false;
        self.new_best = false;
        self.slides.clear();
        self.spawn(rng);
        self.spawn(rng);
    }

    /// Cell index of the `k`th cell of `line`, counting from the edge the
    /// tiles slide toward.
    fn line_idx(dir: Dir, line: usize, k: usize) -> usize {
        match dir {
            Dir::Left => line * N + k,
            Dir::Right => line * N + (N - 1 - k),
            Dir::Up => k * N + line,
            Dir::Down => (N - 1 - k) * N + line,
        }
    }

    /// Slide every tile toward `dir`, merging equal pairs. A tile merges at
    /// most once per move. Returns false, changing nothing, if no tile could
    /// move.
    fn slide(&mut self, dir: Dir) -> bool {
        let mut next = [0u8; CELLS];
        let mut gained = 0;
        self.slides.clear();

        for line in 0..N {
            let mut out = 0;
            let mut can_merge = false;
            for k in 0..N {
                let from = Self::line_idx(dir, line, k);
                let exp = self.grid[from];
                if exp == 0 {
                    continue;
                }
                let to = if can_merge && next[Self::line_idx(dir, line, out - 1)] == exp {
                    let to = Self::line_idx(dir, line, out - 1);
                    next[to] = exp + 1;
                    gained += 1 << (exp + 1);
                    can_merge = false;
                    to
                } else {
                    let to = Self::line_idx(dir, line, out);
                    next[to] = exp;
                    out += 1;
                    can_merge = true;
                    to
                };
                let _ = self.slides.push(Slide { from: from as u8, to: to as u8, exp });
            }
        }

        if next == self.grid {
            self.slides.clear();
            return false;
        }
        self.grid = next;
        self.score += gained;
        self.anim_t = 0.0;
        true
    }

    /// Drop a 2 (or, one time in ten, a 4) into a random empty cell.
    fn spawn(&mut self, rng: &mut u32) -> Option<usize> {
        let empty = self.grid.iter().filter(|&&e| e == 0).count();
        if empty == 0 {
            return None;
        }
        let mut k = xorshift32(rng) as usize % empty;
        let exp = if xorshift32(rng) % 10 == 0 { 2 } else { 1 };
        for i in 0..CELLS {
            if self.grid[i] == 0 {
                if k == 0 {
                    self.grid[i] = exp;
                    return Some(i);
                }
                k -= 1;
            }
        }
        None
    }

    fn can_move(&self) -> bool {
        for i in 0..CELLS {
            let e = self.grid[i];
            if e == 0 {
                return true;
            }
            if i % N < N - 1 && self.grid[i + 1] == e {
                return true;
            }
            if i + N < CELLS && self.grid[i + N] == e {
                return true;
            }
        }
        false
    }

    /// Make a move and settle what follows: spawn, best score, win or loss.
    fn play(&mut self, dir: Dir, ctx: &mut GameContext) {
        if !self.slide(dir) {
            return;
        }
        let mut rng = ctx.rng;
        self.spawn(&mut rng);
        ctx.rng = rng;

        if self.score > ctx.meowltiply_high_score {
            ctx.meowltiply_high_score = self.score;
            self.new_best = true;
        }

        if !self.kept_going && self.grid.iter().any(|&e| e >= WIN_EXP) {
            self.state = State::Won;
            self.set_popup(t!("YOU WIN!"), t!("A: Keep Going"));
        } else if !self.can_move() {
            self.lose();
        }
    }

    fn lose(&mut self) {
        self.state = State::Lose;
        let title = if self.new_best { t!("NEW BEST!") } else { t!("GAME OVER") };
        self.set_popup(title, t!("A: New Game"));
    }

    fn set_popup(&mut self, title: &str, prompt: &str) {
        let mut text: String<96> = String::new();
        let _ = text.push_str(title);
        let mut n: String<12> = String::new();
        let _ = write!(n, "{}", self.score);
        crate::i18n::substitute(&mut text, t!("\n\nScore: {n}"), &[("n", n.as_str())]);
        let _ = text.push('\n');
        let _ = text.push_str(prompt);
        self.popup.set_text(text.as_str(), false, true);
    }

    fn open_menu(&mut self) {
        self.menu_active = true;
        self.options_menu.reset_to(OPTIONS_ITEMS);
    }

    fn handle_menu(&mut self, buttons: &mut Buttons, rng: &mut u32) {
        match self.options_menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => {
                self.menu_active = false;
            }
            MenuResult::Action(action) => {
                self.menu_active = false;
                match action {
                    MeowltiplyAction::NewBoard => self.new_game(rng),
                }
            }
        }
    }

    /// Advance the slide animation, dropping it once the tiles have arrived.
    fn tick_slide(&mut self, dt: f32) {
        if self.slides.is_empty() {
            return;
        }
        self.anim_t += dt;
        if self.anim_t >= SLIDE_TIME {
            self.slides.clear();
        }
    }

    fn tile_pos(i: usize) -> (i32, i32) {
        let col = (i % N) as i32;
        let row = (i / N) as i32;
        (col * CELL_W + 1, row * CELL_H + 1)
    }

    /// Where a sliding tile is drawn at this point in the animation.
    fn slide_pos(&self, s: Slide) -> (i32, i32) {
        let f = (self.anim_t / SLIDE_TIME).min(1.0);
        let (fx, fy) = Self::tile_pos(s.from as usize);
        let (tx, ty) = Self::tile_pos(s.to as usize);
        (
            fx + ((tx - fx) as f32 * f).round() as i32,
            fy + ((ty - fy) as f32 * f).round() as i32,
        )
    }

    fn draw_board(&self, r: &mut Renderer) {
        for i in 0..CELLS {
            if self.grid[i] == 0 || !self.slides.is_empty() {
                let (x, y) = Self::tile_pos(i);
                r.draw_pixel(Point::new(x + TILE_W / 2, y + TILE_H / 2), true);
            }
        }

        if self.slides.is_empty() {
            for i in 0..CELLS {
                if self.grid[i] != 0 {
                    let (x, y) = Self::tile_pos(i);
                    draw_tile(r, x, y, self.grid[i]);
                }
            }
            return;
        }

        for &s in &self.slides {
            let (x, y) = self.slide_pos(s);
            draw_tile(r, x, y, s.exp);
        }
    }
}

fn draw_tile(r: &mut Renderer, x: i32, y: i32, exp: u8) {
    let filled = exp >= FILLED_EXP;
    let size = Size::new(TILE_W as u32, TILE_H as u32);
    if filled {
        r.draw_rect(Point::new(x, y), size, true);
    } else {
        // Clear first so the empty-cell dot and any tile it slides over
        // don't show through.
        r.fill_rect_off(Point::new(x, y), size);
        r.draw_rect(Point::new(x, y), size, false);
    }
    for (cx, cy) in [
        (x, y),
        (x + TILE_W - 1, y),
        (x, y + TILE_H - 1),
        (x + TILE_W - 1, y + TILE_H - 1),
    ] {
        r.draw_pixel(Point::new(cx, cy), false);
    }

    let mut s: String<8> = String::new();
    let _ = write!(s, "{}", 1u32 << exp);
    let len = s.len() as i32;
    // Glyph advances include a trailing blank column, hence the +1. The y
    // offsets centre each font's digit ink in the tile.
    if len <= 4 {
        let pos = Point::new(x + (TILE_W - len * CHAR_W + 1) / 2, y + 2);
        if filled {
            r.draw_text_inverted(s.as_str(), pos);
        } else {
            r.draw_text(s.as_str(), pos);
        }
    } else {
        let pos = Point::new(x + (TILE_W - len * SMALL_CHAR_W + 1) / 2, y + 4);
        if filled {
            r.draw_text_small_inverted(s.as_str(), pos);
        } else {
            r.draw_text_small(s.as_str(), pos);
        }
    }
}

fn pressed_dir(buttons: &mut Buttons) -> Option<Dir> {
    if buttons.was_just_pressed(Button::Up) {
        Some(Dir::Up)
    } else if buttons.was_just_pressed(Button::Down) {
        Some(Dir::Down)
    } else if buttons.was_just_pressed(Button::Left) {
        Some(Dir::Left)
    } else if buttons.was_just_pressed(Button::Right) {
        Some(Dir::Right)
    } else {
        None
    }
}

impl Scene for MeowltiplyScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_score = 0;
        self.score = 0;
        let mut rng = ctx.rng;
        self.new_game(&mut rng);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let session = self.session_score + self.score;
        if session <= 0 {
            return;
        }
        let progress = (session as f32 / 2000.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 5.0 * progress),
            (StatId::Focus,        5.0 * progress),
            (StatId::Loyalty,      0.5 * progress),
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

        if self.menu_active {
            let mut rng = ctx.rng;
            self.handle_menu(buttons, &mut rng);
            ctx.rng = rng;
            return None;
        }

        match self.state {
            // The popup appears once the final slide has played out.
            State::Won | State::Lose if !self.slides.is_empty() => {}
            State::Lose => {
                if buttons.was_just_pressed(Button::A) {
                    let mut rng = ctx.rng;
                    self.new_game(&mut rng);
                    ctx.rng = rng;
                }
            }
            State::Won => {
                if buttons.was_just_pressed(Button::A) {
                    self.kept_going = true;
                    self.state = State::Playing;
                    if !self.can_move() {
                        self.lose();
                    }
                }
            }
            State::Playing => {
                if buttons.was_just_pressed(Button::Menu2) {
                    self.open_menu();
                    return None;
                }
                if let Some(dir) = pressed_dir(buttons) {
                    // A new move cuts the previous slide short.
                    self.slides.clear();
                    self.play(dir, ctx);
                }
            }
        }

        // Step after input so a fresh move already shows motion this frame.
        self.tick_slide(dt);

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }

        self.draw_board(renderer);

        if self.state != State::Playing && self.slides.is_empty() {
            self.popup.draw(renderer, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::led::Led;

    /// Build a scene from tile values (0 for empty), row by row.
    fn board(rows: [[u32; N]; N]) -> MeowltiplyScene {
        let mut s = MeowltiplyScene::new();
        for (r, row) in rows.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                s.grid[r * N + c] = if v == 0 { 0 } else { v.trailing_zeros() as u8 };
            }
        }
        s
    }

    fn values(s: &MeowltiplyScene) -> [[u32; N]; N] {
        let mut out = [[0; N]; N];
        for i in 0..CELLS {
            if s.grid[i] != 0 {
                out[i / N][i % N] = 1 << s.grid[i];
            }
        }
        out
    }

    fn row(s: &MeowltiplyScene, r: usize) -> [u32; N] {
        values(s)[r]
    }

    fn ctx() -> GameContext {
        GameContext::new(Led::new())
    }

    #[test]
    fn four_of_a_kind_merges_into_two_pairs() {
        let mut s = board([[2, 2, 2, 2], [0; 4], [0; 4], [0; 4]]);
        assert!(s.slide(Dir::Left));
        assert_eq!(row(&s, 0), [4, 4, 0, 0]);
        assert_eq!(s.score, 8);
    }

    #[test]
    fn merged_tile_does_not_merge_again() {
        let mut s = board([[2, 2, 4, 0], [0; 4], [0; 4], [0; 4]]);
        assert!(s.slide(Dir::Left));
        assert_eq!(row(&s, 0), [4, 4, 0, 0]);
        assert_eq!(s.score, 4);
    }

    #[test]
    fn gaps_close_before_merging() {
        let mut s = board([[2, 0, 2, 4], [0; 4], [0; 4], [0; 4]]);
        assert!(s.slide(Dir::Left));
        assert_eq!(row(&s, 0), [4, 4, 0, 0]);
    }

    #[test]
    fn pair_nearest_the_wall_merges_first() {
        let mut s = board([[2, 2, 2, 0], [0, 2, 2, 2], [0; 4], [0; 4]]);
        assert!(s.slide(Dir::Left));
        assert_eq!(row(&s, 0), [4, 2, 0, 0]);
        assert_eq!(row(&s, 1), [4, 2, 0, 0]);

        let mut s = board([[2, 2, 2, 0], [0; 4], [0; 4], [0; 4]]);
        assert!(s.slide(Dir::Right));
        assert_eq!(row(&s, 0), [0, 0, 2, 4]);
    }

    #[test]
    fn every_direction() {
        let start = [
            [2, 0, 0, 2],
            [0, 4, 0, 0],
            [2, 4, 0, 8],
            [0, 0, 0, 8],
        ];

        let mut s = board(start);
        assert!(s.slide(Dir::Left));
        assert_eq!(values(&s), [[4, 0, 0, 0], [4, 0, 0, 0], [2, 4, 8, 0], [8, 0, 0, 0]]);

        let mut s = board(start);
        assert!(s.slide(Dir::Right));
        assert_eq!(values(&s), [[0, 0, 0, 4], [0, 0, 0, 4], [0, 2, 4, 8], [0, 0, 0, 8]]);

        let mut s = board(start);
        assert!(s.slide(Dir::Up));
        assert_eq!(values(&s), [[4, 8, 0, 2], [0, 0, 0, 16], [0, 0, 0, 0], [0, 0, 0, 0]]);

        let mut s = board(start);
        assert!(s.slide(Dir::Down));
        assert_eq!(values(&s), [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 2], [4, 8, 0, 16]]);
    }

    #[test]
    fn blocked_move_changes_nothing() {
        let mut s = board([[2, 4, 0, 0], [8, 0, 0, 0], [0; 4], [0; 4]]);
        let before = s.grid;
        assert!(!s.slide(Dir::Left));
        assert!(!s.slide(Dir::Up));
        assert_eq!(s.grid, before);
        assert_eq!(s.score, 0);
        assert!(s.slides.is_empty());

        let mut c = ctx();
        let rng = c.rng;
        s.play(Dir::Left, &mut c);
        assert_eq!(s.grid, before); // no spawn either
        assert_eq!(c.rng, rng);
    }

    #[test]
    fn merging_slides_share_a_destination() {
        let mut s = board([[0, 2, 0, 2], [0; 4], [0; 4], [0; 4]]);
        assert!(s.slide(Dir::Left));
        assert_eq!(s.slides.len(), 2);
        let (a, b) = (s.slides[0], s.slides[1]);
        assert_eq!((a.from, a.to, a.exp), (1, 0, 1));
        assert_eq!((b.from, b.to, b.exp), (3, 0, 1));
    }

    #[test]
    fn spawn_only_fills_empty_cells() {
        let mut s = board([[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 0, 4], [4, 2, 4, 2]]);
        let mut rng = 12345;
        assert_eq!(s.spawn(&mut rng), Some(10));
        assert!(s.grid[10] == 1 || s.grid[10] == 2);
        assert_eq!(s.spawn(&mut rng), None);

        let mut s = MeowltiplyScene::new();
        let mut rng = 7;
        for _ in 0..200 {
            s.grid = [0; CELLS];
            s.grid[5] = 3;
            let i = s.spawn(&mut rng).unwrap();
            assert_ne!(i, 5);
            assert_eq!(s.grid[5], 3);
        }
    }

    #[test]
    fn full_board_needs_a_pair_to_move() {
        assert!(!board([[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 4, 2]]).can_move());
        // A vertical pair, then a horizontal one, in the bottom-right corner.
        assert!(board([[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 8], [4, 2, 4, 8]]).can_move());
        assert!(board([[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 8, 8]]).can_move());
    }

    #[test]
    fn locking_the_board_loses() {
        let mut s = board([[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 64], [0, 8, 16, 32]]);
        let mut c = ctx();
        s.play(Dir::Left, &mut c);
        // The spawn fills the bottom-right cell with a 2 or 4, which pairs
        // with neither 32 nor the 64 above it.
        assert!(s.state == State::Lose);
    }

    #[test]
    fn reaching_2048_wins_once() {
        let mut s = board([[1024, 1024, 0, 0], [0; 4], [0; 4], [0; 4]]);
        let mut c = ctx();
        s.play(Dir::Left, &mut c);
        assert!(s.state == State::Won);
        assert_eq!(s.score, 2048);
        assert_eq!(c.meowltiply_high_score, 2048);
        assert!(s.new_best);

        s.kept_going = true;
        s.state = State::Playing;
        s.grid[1] = 1;
        s.grid[2] = 1;
        s.play(Dir::Right, &mut c);
        assert!(s.state == State::Playing);
    }

    #[test]
    fn new_game_banks_the_score() {
        let mut s = MeowltiplyScene::new();
        let mut rng = 99;
        s.score = 500;
        s.new_game(&mut rng);
        assert_eq!(s.session_score, 500);
        assert_eq!(s.score, 0);
        assert_eq!(s.grid.iter().filter(|&&e| e != 0).count(), 2);
    }
}
