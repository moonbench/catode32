//! Lights Out minigame. Toggle all cells off. Pressing a cell flips it and
//! its orthogonal neighbours.

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
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

const MAX_N: usize = 6;
const MAX_TOTAL: usize = MAX_N * MAX_N;

// Difficulty: scramble-press count per grid size.
const PRESS_MIN_4: u32 = 3;
const PRESS_MIN_5: u32 = 3;
const PRESS_MIN_6: u32 = 4;
const PRESS_MAX_4: u32 = 10;
const PRESS_MAX_5: u32 = 15;
const PRESS_MAX_6: u32 = 20;

// Total pixel size per cell (inner fill + 1px gap on each side).
fn cell_px(n: usize) -> i32 {
    match n {
        4 => 12,
        5 => 10,
        _ => 8,
    }
}

const GRID_AREA_W: i32 = 90;
const GRID_AREA_H: i32 = 64;

const WIN_RESET_DELAY: f32 = 3.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Win,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LightsOutAction {
    Retry,
    NewBoard,
    Size4,
    Size5,
    Size6,
}

const SIZE_ITEMS: &[MenuItem<LightsOutAction>] = &[
    MenuItem { label: t!("4x4 Easy"),   icon: None, submenu: None, action: Some(LightsOutAction::Size4), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("5x5 Normal"), icon: None, submenu: None, action: Some(LightsOutAction::Size5), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("6x6 Hard"),   icon: None, submenu: None, action: Some(LightsOutAction::Size6), confirm: None, confirm_on_vacation: None },
];

const OPTIONS_ITEMS: &[MenuItem<LightsOutAction>] = &[
    MenuItem { label: t!("Retry"),     icon: None, submenu: None,             action: Some(LightsOutAction::Retry),    confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("New Board"), icon: None, submenu: None,             action: Some(LightsOutAction::NewBoard), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Grid Size"), icon: None, submenu: Some(SIZE_ITEMS), action: None,                            confirm: None, confirm_on_vacation: None },
];

pub struct LightsOutScene {
    character: Character,
    win_popup: Popup,

    grid_size: usize,
    grid: [u8; MAX_TOTAL],
    seed: [u8; MAX_TOTAL],
    seed_len: usize,
    cursor: usize,
    state: State,
    win_timer: f32,
    move_count: u32,
    par: u32,
    session_wins: u32,

    options_menu: Menu<LightsOutAction>,
    menu_active: bool,
}

impl LightsOutScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(100, 63)),
            win_popup: Popup::new(10, 14, 108, 36),

            grid_size: 5,
            grid: [0; MAX_TOTAL],
            seed: [0; MAX_TOTAL],
            seed_len: 0,
            cursor: 0,
            state: State::Playing,
            win_timer: 0.0,
            move_count: 0,
            par: 0,
            session_wins: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn init_game(&mut self, size: Option<usize>, rng: &mut u32) {
        if let Some(s) = size {
            self.grid_size = s;
        }
        let n = self.grid_size;
        let total = n * n;

        for i in 0..MAX_TOTAL {
            self.grid[i] = 0;
        }

        let (min_p, max_p) = match n {
            4 => (PRESS_MIN_4, PRESS_MAX_4),
            5 => (PRESS_MIN_5, PRESS_MAX_5),
            _ => (PRESS_MIN_6, PRESS_MAX_6),
        };
        let target = max_p.min(min_p + self.session_wins / 2);

        // Scramble: press `target` distinct cells. Track which were pressed via the
        // grid itself isn't safe (toggles cascade) so use a bitmask.
        let mut pressed_mask = [false; MAX_TOTAL];
        let mut pressed_count: u32 = 0;
        let mut attempts: u32 = 0;
        self.seed_len = 0;
        while pressed_count < target && attempts < target * 4 {
            attempts += 1;
            let idx = (xorshift32(rng) % total as u32) as usize;
            if !pressed_mask[idx] {
                pressed_mask[idx] = true;
                pressed_count += 1;
                self.seed[self.seed_len] = idx as u8;
                self.seed_len += 1;
                self.apply_toggle(idx);
            }
        }

        self.par = self.seed_len as u32;
        self.reset_board();
    }

    fn reset_board(&mut self) {
        let n = self.grid_size;
        let total = n * n;
        for i in 0..MAX_TOTAL {
            self.grid[i] = 0;
        }
        for k in 0..self.seed_len {
            let idx = self.seed[k] as usize;
            self.apply_toggle(idx);
        }
        self.cursor = total / 2;
        self.state = State::Playing;
        self.win_timer = 0.0;
        self.move_count = 0;
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn apply_toggle(&mut self, idx: usize) {
        let n = self.grid_size as i32;
        let row = (idx as i32) / n;
        let col = (idx as i32) % n;
        const DELTAS: [(i32, i32); 5] = [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)];
        for (dr, dc) in DELTAS {
            let r = row + dr;
            let c = col + dc;
            if r >= 0 && r < n && c >= 0 && c < n {
                let k = (r * n + c) as usize;
                self.grid[k] ^= 1;
            }
        }
    }

    fn check_win(&self) -> bool {
        let total = self.grid_size * self.grid_size;
        for i in 0..total {
            if self.grid[i] != 0 {
                return false;
            }
        }
        true
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
                    LightsOutAction::Retry => self.reset_board(),
                    LightsOutAction::NewBoard => self.init_game(None, rng),
                    LightsOutAction::Size4 => self.init_game(Some(4), rng),
                    LightsOutAction::Size5 => self.init_game(Some(5), rng),
                    LightsOutAction::Size6 => self.init_game(Some(6), rng),
                }
            }
        }
    }

    fn draw_grid(&self, r: &mut Renderer) {
        let n = self.grid_size;
        let px = cell_px(n);
        let span = n as i32 * px;
        let ox = (GRID_AREA_W - span) / 2;
        let oy = (GRID_AREA_H - span) / 2;
        let inner = (px - 2) as u32;

        for i in 0..n * n {
            let row = (i / n) as i32;
            let col = (i % n) as i32;
            let cx = ox + col * px;
            let cy = oy + row * px;
            let filled = self.grid[i] != 0;
            r.draw_rect(
                Point::new(cx + 1, cy + 1),
                Size::new(inner, inner),
                filled,
            );
            if i == self.cursor && self.state == State::Playing {
                r.draw_rect(
                    Point::new(cx, cy),
                    Size::new(px as u32, px as u32),
                    false,
                );
            }
        }
    }

    fn draw_sidebar(&self, r: &mut Renderer) {
        let mut moves: String<8> = String::new();
        let _ = write!(moves, "{}", self.move_count);
        r.draw_text(moves.as_str(), Point::new(92, 2));
        let mut par: String<12> = String::new();
        let _ = par.push_str(t!("Par:"));
        let _ = write!(par, "{}", self.par);
        r.draw_text(par.as_str(), Point::new(82, 12));
    }

    fn set_win_popup(&mut self) {
        let mut text: String<48> = String::new();
        let _ = text.push_str(t!("All off!"));
        let _ = text.push_str("\n\n");
        let mut n: String<8> = String::new();
        let _ = write!(n, "{}", self.move_count);
        crate::i18n::substitute(&mut text, t!("Moves: {n}"), &[("n", n.as_str())]);
        self.win_popup.set_text(text.as_str(), false, true);
    }
}

impl Scene for LightsOutScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_wins = 0;
        let mut rng = ctx.rng;
        self.init_game(None, &mut rng);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.session_wins == 0 {
            return;
        }
        let scale = (self.session_wins as f32 / 5.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 5.0 * scale),
            (StatId::Focus,        5.0 * scale),
            (StatId::Loyalty,      0.5 * scale),
        ]);
        let coins = 3 * self.session_wins as i32;
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

        if self.menu_active {
            let mut rng = ctx.rng;
            self.handle_menu(buttons, &mut rng);
            ctx.rng = rng;
            return None;
        }

        if self.state == State::Win {
            self.win_timer += dt;
            if buttons.was_just_pressed(Button::A) {
                let mut rng = ctx.rng;
                self.init_game(None, &mut rng);
                ctx.rng = rng;
                return None;
            }
            if self.win_timer >= WIN_RESET_DELAY {
                let mut rng = ctx.rng;
                self.init_game(None, &mut rng);
                ctx.rng = rng;
            }
            return None;
        }

        if buttons.was_just_pressed(Button::Menu2) {
            self.open_menu();
            return None;
        }

        let n = self.grid_size;
        let row = self.cursor / n;
        let col = self.cursor % n;
        if buttons.was_just_pressed(Button::Up) && row > 0 {
            self.cursor -= n;
        } else if buttons.was_just_pressed(Button::Down) && row < n - 1 {
            self.cursor += n;
        } else if buttons.was_just_pressed(Button::Left) && col > 0 {
            self.cursor -= 1;
        } else if buttons.was_just_pressed(Button::Right) && col < n - 1 {
            self.cursor += 1;
        }

        if buttons.was_just_pressed(Button::A) {
            self.apply_toggle(self.cursor);
            self.move_count += 1;
            if self.check_win() {
                self.session_wins += 1;
                self.state = State::Win;
                self.win_timer = 0.0;
                self.set_win_popup();
                self.character.set_pose(PoseId::SittingSideHappy);
            }
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }

        self.draw_grid(renderer);
        self.draw_sidebar(renderer);
        self.character.draw(renderer, 0);

        if self.state == State::Win {
            self.win_popup.draw(renderer, false);
        }
    }
}
