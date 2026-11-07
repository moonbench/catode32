//! Game of Life toy. Draw on a full-screen 128x64 wrapping world and watch it
//! evolve. No win or lose state.

use embedded_graphics::prelude::{Point, Size};
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
};

const W: i32 = 128;
const H: usize = 64;

// Hold-to-repeat for cursor movement.
const REPEAT_DELAY: f32 = 0.25;
const REPEAT_INTERVAL: f32 = 0.04;

const BLINK_PERIOD: f32 = 0.25;
// While running, the cursor only shows for a moment after it moves.
const CURSOR_SHOW_TIME: f32 = 1.5;
const LABEL_TIME: f32 = 1.0;

// Generations per second; 0 means one per frame.
const SPEED_SLOW: f32 = 2.0;
const SPEED_NORMAL: f32 = 8.0;
const SPEED_FAST: f32 = 20.0;
const SPEED_MAX: f32 = 0.0;
const MAX_STEPS_PER_FRAME: u32 = 4;

const GLIDER: &[&str] = &[
    ".#.",
    "..#",
    "###",
];

const GLIDER_GUN: &[&str] = &[
    "........................#...........",
    "......................#.#...........",
    "............##......##............##",
    "...........#...#....##............##",
    "##........#.....#...##..............",
    "##........#...#.##....#.#...........",
    "..........#.....#.......#...........",
    "...........#...#....................",
    "............##......................",
];

const PULSAR: &[&str] = &[
    "..###...###..",
    ".............",
    "#....#.#....#",
    "#....#.#....#",
    "#....#.#....#",
    "..###...###..",
    ".............",
    "..###...###..",
    "#....#.#....#",
    "#....#.#....#",
    "#....#.#....#",
    ".............",
    "..###...###..",
];

const R_PENTOMINO: &[&str] = &[
    ".##",
    "##.",
    ".#.",
];

const ACORN: &[&str] = &[
    ".#.....",
    "...#...",
    "##..###",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pattern {
    Glider,
    GliderGun,
    Pulsar,
    RPentomino,
    Acorn,
}

#[derive(Clone, Copy)]
enum LifeAction {
    Speed(f32),
    Step,
    Clear,
    Random,
    Pattern(Pattern),
}

const SPEED_ITEMS: &[MenuItem<LifeAction>] = &[
    MenuItem { label: t!("Slow"),   icon: None, submenu: None, action: Some(LifeAction::Speed(SPEED_SLOW)),   confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Normal"), icon: None, submenu: None, action: Some(LifeAction::Speed(SPEED_NORMAL)), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Fast"),   icon: None, submenu: None, action: Some(LifeAction::Speed(SPEED_FAST)),   confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Max"),    icon: None, submenu: None, action: Some(LifeAction::Speed(SPEED_MAX)),    confirm: None, confirm_on_vacation: None },
];

const PATTERN_ITEMS: &[MenuItem<LifeAction>] = &[
    MenuItem { label: t!("Glider"),      icon: None, submenu: None, action: Some(LifeAction::Pattern(Pattern::Glider)),     confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Glider Gun"),  icon: None, submenu: None, action: Some(LifeAction::Pattern(Pattern::GliderGun)),  confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Pulsar"),      icon: None, submenu: None, action: Some(LifeAction::Pattern(Pattern::Pulsar)),     confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("R-pentomino"), icon: None, submenu: None, action: Some(LifeAction::Pattern(Pattern::RPentomino)), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Acorn"),       icon: None, submenu: None, action: Some(LifeAction::Pattern(Pattern::Acorn)),      confirm: None, confirm_on_vacation: None },
];

const OPTIONS_ITEMS: &[MenuItem<LifeAction>] = &[
    MenuItem { label: t!("Speed"),     icon: None, submenu: Some(SPEED_ITEMS),   action: None,                    confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Step"),      icon: None, submenu: None,                action: Some(LifeAction::Step),   confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Patterns"),  icon: None, submenu: Some(PATTERN_ITEMS), action: None,                    confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Random"),    icon: None, submenu: None,                action: Some(LifeAction::Random), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Clear All"), icon: None, submenu: None,                action: Some(LifeAction::Clear),  confirm: None, confirm_on_vacation: None },
];

const DIRS: [(Button, i32, i32); 4] = [
    (Button::Up, 0, -1),
    (Button::Down, 0, 1),
    (Button::Left, -1, 0),
    (Button::Right, 1, 0),
];

pub struct LifeScene {
    /// One row per screen line; bit `x` is column `x`.
    cells: [u128; H],

    cursor_x: i32,
    cursor_y: i32,
    /// Per-direction hold timers; negative while inside the initial delay.
    hold: [f32; 4],
    /// Value A-held cursor moves paint onto each new cell.
    paint_value: bool,

    running: bool,
    speed: f32,
    step_timer: f32,

    blink_timer: f32,
    cursor_shown_timer: f32,
    label_timer: f32,

    cells_edited: u32,
    generations_run: u32,

    options_menu: Menu<LifeAction>,
    menu_active: bool,
}

impl LifeScene {
    pub fn new() -> Self {
        Self {
            cells: [0; H],

            cursor_x: W / 2,
            cursor_y: H as i32 / 2,
            hold: [0.0; 4],
            paint_value: true,

            running: false,
            speed: SPEED_NORMAL,
            step_timer: 0.0,

            blink_timer: 0.0,
            cursor_shown_timer: 0.0,
            label_timer: 0.0,

            cells_edited: 0,
            generations_run: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn get(&self, x: i32, y: i32) -> bool {
        (self.cells[y as usize] >> x) & 1 == 1
    }

    fn set(&mut self, x: i32, y: i32, on: bool) {
        let x = x.rem_euclid(W);
        let y = y.rem_euclid(H as i32) as usize;
        if on {
            self.cells[y] |= 1u128 << x;
        } else {
            self.cells[y] &= !(1u128 << x);
        }
    }

    /// Advance one generation. Each row is updated 128 cells at a time: the
    /// eight neighbour masks are summed into a bit-sliced counter (ones, twos,
    /// and a sticky "four or more"), and edges wrap via rotation.
    fn step(&mut self) {
        let mut next = [0u128; H];
        for y in 0..H {
            let above = self.cells[(y + H - 1) % H];
            let row = self.cells[y];
            let below = self.cells[(y + 1) % H];
            let neighbours = [
                above.rotate_left(1), above, above.rotate_right(1),
                row.rotate_left(1),          row.rotate_right(1),
                below.rotate_left(1), below, below.rotate_right(1),
            ];
            let (mut ones, mut twos, mut fours) = (0u128, 0u128, 0u128);
            for m in neighbours {
                let carry1 = ones & m;
                ones ^= m;
                let carry2 = twos & carry1;
                twos ^= carry1;
                fours |= carry2;
            }
            // Alive next if count is 3, or 2 and already alive.
            next[y] = twos & !fours & (ones | row);
        }
        self.cells = next;
        self.generations_run += 1;
    }

    fn randomize(&mut self, rng: &mut u32) {
        for row in self.cells.iter_mut() {
            // AND of two random words gives roughly 25% density.
            let mut r = 0u128;
            for _ in 0..4 {
                r = (r << 32) | (xorshift32(rng) & xorshift32(rng)) as u128;
            }
            *row = r;
        }
    }

    fn stamp_ascii(&mut self, rows: &[&str], ox: i32, oy: i32) {
        for (y, line) in rows.iter().enumerate() {
            for (x, c) in line.bytes().enumerate() {
                if c == b'#' {
                    self.set(ox + x as i32, oy + y as i32, true);
                }
            }
        }
    }

    fn stamp_ascii_centered(&mut self, rows: &[&str]) {
        let w = rows[0].len() as i32;
        let h = rows.len() as i32;
        self.stamp_ascii(rows, (W - w) / 2, (H as i32 - h) / 2);
    }

    fn load_pattern(&mut self, pattern: Pattern) {
        match pattern {
            Pattern::Glider => self.stamp_ascii(GLIDER, 4, 4),
            Pattern::GliderGun => self.stamp_ascii(GLIDER_GUN, 4, 4),
            Pattern::Pulsar => self.stamp_ascii_centered(PULSAR),
            Pattern::RPentomino => self.stamp_ascii_centered(R_PENTOMINO),
            Pattern::Acorn => self.stamp_ascii_centered(ACORN),
        }
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
                    LifeAction::Speed(s) => self.speed = s,
                    LifeAction::Step => {
                        self.running = false;
                        self.step();
                    }
                    LifeAction::Clear => {
                        self.cells = [0; H];
                        self.running = false;
                    }
                    LifeAction::Random => self.randomize(rng),
                    LifeAction::Pattern(p) => {
                        self.cells = [0; H];
                        self.running = false;
                        self.load_pattern(p);
                    }
                }
            }
        }
    }

    /// Steps to move in direction `i` this frame, with hold-to-repeat.
    fn repeat_steps(&mut self, buttons: &mut Buttons, i: usize, dt: f32) -> u32 {
        let button = DIRS[i].0;
        if buttons.was_just_pressed(button) {
            self.hold[i] = -REPEAT_DELAY;
            return 1;
        }
        if !buttons.is_pressed(button) {
            return 0;
        }
        self.hold[i] += dt;
        let mut steps = 0;
        while self.hold[i] >= REPEAT_INTERVAL {
            self.hold[i] -= REPEAT_INTERVAL;
            steps += 1;
        }
        steps
    }

    fn update_cursor(&mut self, buttons: &mut Buttons, dt: f32) {
        let painting = buttons.is_pressed(Button::A);
        for i in 0..DIRS.len() {
            let (_, dx, dy) = DIRS[i];
            for _ in 0..self.repeat_steps(buttons, i, dt) {
                self.cursor_x = (self.cursor_x + dx).rem_euclid(W);
                self.cursor_y = (self.cursor_y + dy).rem_euclid(H as i32);
                self.cursor_shown_timer = CURSOR_SHOW_TIME;
                self.blink_timer = 0.0;
                if painting && self.get(self.cursor_x, self.cursor_y) != self.paint_value {
                    self.set(self.cursor_x, self.cursor_y, self.paint_value);
                    self.cells_edited += 1;
                }
            }
        }

        if buttons.was_just_pressed(Button::A) {
            self.paint_value = !self.get(self.cursor_x, self.cursor_y);
            self.set(self.cursor_x, self.cursor_y, self.paint_value);
            self.cells_edited += 1;
            self.cursor_shown_timer = CURSOR_SHOW_TIME;
            self.blink_timer = 0.0;
        }
    }

    fn cursor_visible(&self) -> bool {
        let shown = !self.running || self.cursor_shown_timer > 0.0;
        // Starts on the inverted phase so a move or toggle is seen at once.
        shown && (self.blink_timer / BLINK_PERIOD) as u32 % 2 == 0
    }
}

impl Scene for LifeScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.cells_edited = 0;
        self.generations_run = 0;
        self.running = false;
        self.menu_active = false;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.cells_edited == 0 && self.generations_run == 0 {
            return;
        }
        let gens = (self.generations_run as f32 / 500.0).min(1.0);
        let edits = (self.cells_edited as f32 / 50.0).min(1.0);
        let scale = (gens + edits).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Curiosity,    3.0 * scale),
            (StatId::Intelligence, 2.0 * scale),
            (StatId::Serenity,     1.0 * scale),
        ]);
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

        if buttons.was_just_pressed(Button::Menu2) {
            self.menu_active = true;
            self.options_menu.reset_to(OPTIONS_ITEMS);
            return None;
        }

        self.blink_timer += dt;
        self.cursor_shown_timer = (self.cursor_shown_timer - dt).max(0.0);
        self.label_timer = (self.label_timer - dt).max(0.0);

        if buttons.was_just_pressed(Button::B) {
            self.running = !self.running;
            self.step_timer = 0.0;
            self.label_timer = LABEL_TIME;
        }

        self.update_cursor(buttons, dt);

        if self.running {
            if self.speed == SPEED_MAX {
                self.step();
            } else {
                let interval = 1.0 / self.speed;
                self.step_timer += dt;
                let mut steps = 0;
                while self.step_timer >= interval && steps < MAX_STEPS_PER_FRAME {
                    self.step_timer -= interval;
                    self.step();
                    steps += 1;
                }
                self.step_timer = self.step_timer.min(interval);
            }
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        if self.menu_active {
            self.options_menu.draw(renderer);
            return;
        }

        for (y, &row) in self.cells.iter().enumerate() {
            let mut bits = row;
            while bits != 0 {
                let x = bits.trailing_zeros() as i32;
                renderer.draw_pixel(Point::new(x, y as i32), true);
                bits &= bits - 1;
            }
        }

        if self.cursor_visible() {
            let on = !self.get(self.cursor_x, self.cursor_y);
            renderer.draw_pixel(Point::new(self.cursor_x, self.cursor_y), on);
        }

        if self.label_timer > 0.0 {
            let label = if self.running { t!("Running") } else { t!("Paused") };
            let w = label.chars().count() as u32 * 4 + 2;
            renderer.fill_rect_off(Point::new(0, 0), Size::new(w, 8));
            renderer.draw_text_small(label, Point::new(1, 1));
        }
    }
}
