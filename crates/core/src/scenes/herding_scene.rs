//! Herding minigame. "+" strays bounce around the arena while the cat runs
//! along its walls. Cutting a line from wall to wall fills in whichever side
//! holds fewer strays (catching any inside). Fill 90% to clear the level.

use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::minigame_assets::{CAT_THIN_DOWN, CAT_THIN_RIGHT},
    board::DISPLAY_WIDTH,
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::xorshift32,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
};

const CHAR_W: i32 = 6; // FONT_6X10

// Arena layout. The 4px margin lets the cat head overhang the outer wall.
const ORIGIN_X: i32 = 4;
const ORIGIN_Y: i32 = 4;
const GRID_W: i32 = 120;
const GRID_H: i32 = 56;
const CELLS: usize = (GRID_W * GRID_H) as usize; // 6720
const ROW_BYTES: usize = GRID_W as usize / 8; // 15
const BORDER: i32 = 2 * GRID_W + 2 * (GRID_H - 2); // 348
const INTERIOR: i32 = (GRID_W - 2) * (GRID_H - 2); // 6372

const CLEAR_PERCENT: i32 = 90;

// Cell values. Labels from LABEL0 up only exist in the middle of a carve.
const OPEN: u8 = 0;
const WALL: u8 = 1;
const TRAIL: u8 = 2;
const LABEL0: u8 = 3;

const PLAYER_SPEED: f32 = 30.0; // px per second
const ENEMY_BASE_SPEED: f32 = 15.0; // px per second on each axis
const ENEMY_SPEED_PER_LEVEL: f32 = 1.0;
const ENEMY_MAX_SPEED: f32 = 26.0;
const MAX_ENEMIES: usize = 12;

/// Pending scanline seeds. Overflow falls back to a slower sweep.
const FILL_STACK: usize = 256;

/// Seconds the arena stays frozen after being caught or clearing a level.
const END_PAUSE: f32 = 1.0;

/// Offsets covered by an enemy: a 5x5 plus. Used for drawing, wall bounces
/// and trail hits alike.
const PLUS: [(i32, i32); 9] = [
    (0, 0),
    (-1, 0), (-2, 0), (1, 0), (2, 0),
    (0, -1), (0, -2), (0, 1), (0, 2),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Playing,
    Lose,
    Win,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Running along the edge of the open area.
    OnWall,
    /// Out in the open area, laying a trail.
    Drawing,
}

#[derive(Clone, Copy)]
struct Enemy {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
}

pub struct HerdingScene {
    state: State,
    level: i32,

    grid: [u8; CELLS],
    /// 1bpp cache of every lit cell (WALL or TRAIL), in the row layout
    /// `draw_sprite_raw` reads, so the arena draws in one call.
    pixels: [u8; CELLS / 8],
    /// Interior cells that are WALL.
    filled: i32,
    fill_stack: Vec<(u8, u8), FILL_STACK>,

    mode: Mode,
    px: i32,
    py: i32,
    dir: (i32, i32),
    /// Direction of the last step actually taken. Drawing ignores presses
    /// that reverse it so the cat can't turn straight back onto its trail.
    last_dir: (i32, i32),
    facing: (i32, i32),
    player_progress: f32,

    enemies: Vec<Enemy, MAX_ENEMIES>,
    enemy_progress: f32,

    end_timer: f32,

    session_carved: i32,
    session_levels: i32,
}

impl HerdingScene {
    pub fn new() -> Self {
        Self {
            state: State::Ready,
            level: 1,
            grid: [OPEN; CELLS],
            pixels: [0; CELLS / 8],
            filled: 0,
            fill_stack: Vec::new(),
            mode: Mode::OnWall,
            px: 0,
            py: 0,
            dir: (0, 0),
            last_dir: (0, -1),
            facing: (0, -1),
            player_progress: 0.0,
            enemies: Vec::new(),
            enemy_progress: 0.0,
            end_timer: 0.0,
            session_carved: 0,
            session_levels: 0,
        }
    }

    fn idx(x: i32, y: i32) -> usize {
        (y * GRID_W + x) as usize
    }

    /// Cell value, treating anything off the grid as WALL.
    fn cell(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= GRID_W || y >= GRID_H {
            return WALL;
        }
        self.grid[Self::idx(x, y)]
    }

    fn set_lit(&mut self, x: i32, y: i32) {
        self.pixels[y as usize * ROW_BYTES + x as usize / 8] |= 0x80 >> (x as usize % 8);
    }

    fn rebuild_pixels(&mut self) {
        for b in self.pixels.iter_mut() {
            *b = 0;
        }
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                if self.grid[Self::idx(x, y)] != OPEN {
                    self.set_lit(x, y);
                }
            }
        }
    }

    /// A WALL cell touching the open area (8-way). These are the cells the
    /// cat can run along, including corners.
    fn is_edge(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= GRID_W || y >= GRID_H || self.grid[Self::idx(x, y)] != WALL {
            return false;
        }
        for ny in y - 1..=y + 1 {
            for nx in x - 1..=x + 1 {
                if self.cell(nx, ny) == OPEN {
                    return true;
                }
            }
        }
        false
    }

    fn start_level(&mut self, rng: &mut u32) {
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let border = x == 0 || y == 0 || x == GRID_W - 1 || y == GRID_H - 1;
                self.grid[Self::idx(x, y)] = if border { WALL } else { OPEN };
            }
        }
        self.rebuild_pixels();
        self.filled = 0;

        self.mode = Mode::OnWall;
        self.px = GRID_W / 2;
        self.py = GRID_H - 1;
        self.dir = (0, 0);
        self.last_dir = (0, -1);
        self.facing = (0, -1);
        self.player_progress = 0.0;

        // Spawn clear of the walls and away from the cat on the bottom wall.
        self.enemies.clear();
        let count = (self.level.max(1) as usize).min(MAX_ENEMIES);
        for _ in 0..count {
            let x = 3 + (xorshift32(rng) % (GRID_W - 6) as u32) as i32; // 3..=GRID_W-4
            let y = 3 + (xorshift32(rng) % (GRID_H - 18) as u32) as i32; // 3..=GRID_H-16
            let dx = if xorshift32(rng) & 1 == 0 { 1 } else { -1 };
            let dy = if xorshift32(rng) & 1 == 0 { 1 } else { -1 };
            let _ = self.enemies.push(Enemy { x, y, dx, dy });
        }
        self.enemy_progress = 0.0;
        self.end_timer = 0.0;
    }

    fn enemy_speed(&self) -> f32 {
        (ENEMY_BASE_SPEED + ENEMY_SPEED_PER_LEVEL * (self.level - 1) as f32).min(ENEMY_MAX_SPEED)
    }

    fn lose(&mut self) {
        self.state = State::Lose;
        self.end_timer = END_PAUSE;
    }

    fn win(&mut self) {
        self.state = State::Win;
        self.end_timer = END_PAUSE;
        self.session_levels += 1;
    }

    fn handle_input(&mut self, buttons: &mut Buttons) {
        let pressed = if buttons.was_just_pressed(Button::Up) {
            Some((0, -1))
        } else if buttons.was_just_pressed(Button::Down) {
            Some((0, 1))
        } else if buttons.was_just_pressed(Button::Left) {
            Some((-1, 0))
        } else if buttons.was_just_pressed(Button::Right) {
            Some((1, 0))
        } else {
            None
        };
        if let Some(d) = pressed {
            self.press(d);
        }
    }

    fn press(&mut self, d: (i32, i32)) {
        match self.mode {
            Mode::OnWall => {
                let (nx, ny) = (self.px + d.0, self.py + d.1);
                if self.cell(nx, ny) == OPEN {
                    // Launch straight off the wall so Drawing always starts
                    // out in the open.
                    self.mode = Mode::Drawing;
                    self.dir = d;
                    self.player_progress = 0.0;
                    self.advance_trail(nx, ny);
                } else if self.is_edge(nx, ny) {
                    self.dir = d;
                }
            }
            Mode::Drawing => {
                if d != (-self.last_dir.0, -self.last_dir.1) {
                    self.dir = d;
                }
            }
        }
    }

    fn step_player(&mut self) {
        let (dx, dy) = self.dir;
        if dx == 0 && dy == 0 {
            return;
        }
        let (nx, ny) = (self.px + dx, self.py + dy);
        match self.mode {
            Mode::OnWall => {
                // Slide until the edge turns a corner.
                if self.is_edge(nx, ny) {
                    self.move_to(nx, ny);
                } else {
                    self.dir = (0, 0);
                }
            }
            Mode::Drawing => match self.cell(nx, ny) {
                OPEN => self.advance_trail(nx, ny),
                TRAIL => self.lose(),
                _ => {
                    self.move_to(nx, ny);
                    self.mode = Mode::OnWall;
                    self.dir = (0, 0);
                    self.carve();
                    self.snap_to_edge();
                    if self.filled * 100 >= CLEAR_PERCENT * INTERIOR {
                        self.win();
                    }
                }
            },
        }
    }

    fn move_to(&mut self, x: i32, y: i32) {
        self.px = x;
        self.py = y;
        self.last_dir = self.dir;
        self.facing = self.dir;
    }

    fn advance_trail(&mut self, x: i32, y: i32) {
        self.move_to(x, y);
        self.grid[Self::idx(x, y)] = TRAIL;
        self.set_lit(x, y);
        if self.enemies.iter().any(|e| Self::covers(e, x, y)) {
            self.lose();
        }
    }

    fn covers(e: &Enemy, x: i32, y: i32) -> bool {
        (x == e.x && (y - e.y).abs() <= 2) || (y == e.y && (x - e.x).abs() <= 2)
    }

    /// Any part of a plus centered at (cx, cy) would overlap a wall.
    fn blocked(&self, cx: i32, cy: i32) -> bool {
        PLUS.iter().any(|&(ox, oy)| self.cell(cx + ox, cy + oy) == WALL)
    }

    fn step_enemies(&mut self) {
        for i in 0..self.enemies.len() {
            let mut e = self.enemies[i];
            let block_x = self.blocked(e.x + e.dx, e.y);
            let block_y = self.blocked(e.x, e.y + e.dy);
            if block_x {
                e.dx = -e.dx;
            }
            if block_y {
                e.dy = -e.dy;
            }
            if !block_x && !block_y && self.blocked(e.x + e.dx, e.y + e.dy) {
                // Glancing a corner head-on: bounce straight back.
                e.dx = -e.dx;
                e.dy = -e.dy;
            }
            // Fall back to a single axis so narrow corridors don't pin it.
            if !self.blocked(e.x + e.dx, e.y + e.dy) {
                e.x += e.dx;
                e.y += e.dy;
            } else if !self.blocked(e.x, e.y + e.dy) {
                e.y += e.dy;
            } else if !self.blocked(e.x + e.dx, e.y) {
                e.x += e.dx;
            }
            self.enemies[i] = e;

            if self.mode == Mode::Drawing
                && PLUS.iter().any(|&(ox, oy)| self.cell(e.x + ox, e.y + oy) == TRAIL)
            {
                self.lose();
                return;
            }
        }
    }

    /// Wall off the finished trail, then fill every region except the one
    /// holding the most enemies (ties keep the larger region). Enemies in
    /// the filled regions are removed.
    fn carve(&mut self) {
        for c in self.grid.iter_mut() {
            if *c == TRAIL {
                *c = WALL;
            }
        }

        // Label only the regions that hold enemies, so there are at most
        // MAX_ENEMIES labels. Enemy-free regions stay OPEN and get filled.
        let mut areas: Vec<i32, MAX_ENEMIES> = Vec::new();
        let mut counts = [0u8; MAX_ENEMIES];
        for i in 0..self.enemies.len() {
            let e = self.enemies[i];
            let mut v = self.grid[Self::idx(e.x, e.y)];
            if v == OPEN {
                v = LABEL0 + areas.len() as u8;
                let area = self.flood(e.x, e.y, OPEN, v);
                let _ = areas.push(area);
            }
            if v >= LABEL0 {
                counts[(v - LABEL0) as usize] += 1;
            }
        }

        let mut keep = 0;
        for l in 1..areas.len() {
            if counts[l] > counts[keep] || (counts[l] == counts[keep] && areas[l] > areas[keep]) {
                keep = l;
            }
        }
        let keeper = LABEL0 + keep as u8;

        let mut walls = 0;
        for c in self.grid.iter_mut() {
            *c = if *c == keeper { OPEN } else { WALL };
            if *c == WALL {
                walls += 1;
            }
        }
        self.rebuild_pixels();

        let filled = walls - BORDER;
        self.session_carved += filled - self.filled;
        self.filled = filled;

        let grid = &self.grid;
        self.enemies.retain(|e| grid[Self::idx(e.x, e.y)] == OPEN);
    }

    /// Scanline flood fill of the 4-connected `from` region containing
    /// (sx, sy), relabelling it `to`. Returns the number of cells changed.
    fn flood(&mut self, sx: i32, sy: i32, from: u8, to: u8) -> i32 {
        let mut area = 0;
        let mut overflow = false;
        self.fill_stack.clear();
        let _ = self.fill_stack.push((sx as u8, sy as u8));
        while let Some((x, y)) = self.fill_stack.pop() {
            let (x, y) = (x as i32, y as i32);
            if self.grid[Self::idx(x, y)] != from {
                continue;
            }
            let mut x1 = x;
            while x1 > 0 && self.grid[Self::idx(x1 - 1, y)] == from {
                x1 -= 1;
            }
            let mut x2 = x;
            while x2 < GRID_W - 1 && self.grid[Self::idx(x2 + 1, y)] == from {
                x2 += 1;
            }
            for fx in x1..=x2 {
                self.grid[Self::idx(fx, y)] = to;
            }
            area += x2 - x1 + 1;

            // Seed each run of `from` cells directly above and below.
            for ny in [y - 1, y + 1] {
                if ny < 0 || ny >= GRID_H {
                    continue;
                }
                let mut in_run = false;
                for fx in x1..=x2 {
                    let hit = self.grid[Self::idx(fx, ny)] == from;
                    if hit && !in_run && self.fill_stack.push((fx as u8, ny as u8)).is_err() {
                        overflow = true;
                    }
                    in_run = hit;
                }
            }
        }

        if overflow {
            // Some seeds were dropped. Grow the region one sweep at a time
            // until nothing left over borders it.
            let w = GRID_W as usize;
            loop {
                let mut changed = false;
                for y in 1..GRID_H - 1 {
                    for x in 1..GRID_W - 1 {
                        let i = Self::idx(x, y);
                        if self.grid[i] == from
                            && (self.grid[i - 1] == to
                                || self.grid[i + 1] == to
                                || self.grid[i - w] == to
                                || self.grid[i + w] == to)
                        {
                            self.grid[i] = to;
                            area += 1;
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        area
    }

    /// After a carve the landing cell can end up buried. Move to the
    /// nearest cell still on the edge of the open area.
    fn snap_to_edge(&mut self) {
        if self.is_edge(self.px, self.py) {
            return;
        }
        for r in 1..GRID_W {
            for oy in -r..=r {
                for ox in -r..=r {
                    if ox.abs() != r && oy.abs() != r {
                        continue;
                    }
                    let (x, y) = (self.px + ox, self.py + oy);
                    if self.is_edge(x, y) {
                        self.px = x;
                        self.py = y;
                        return;
                    }
                }
            }
        }
    }

    fn apply_rewards(&self, ctx: &mut GameContext) {
        if self.session_carved <= 0 {
            return;
        }
        let progress = (self.session_carved as f32 / 6000.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Focus,        4.0 * progress),
            (StatId::Intelligence, 2.0 * progress),
            (StatId::Playfulness,  3.0 * progress),
            (StatId::Loyalty,      0.5 * progress),
        ]);
        let coins = (3.0 * progress) as i32 + 2 * self.session_levels;
        if coins > 0 {
            ctx.coins += coins;
        }
    }

    /// Grid cells under the lit pixels of the cat head, centered on the cat.
    fn head_cells(&self) -> impl Iterator<Item = (i32, i32)> {
        // (data, w, h, mirror_h, mirror_v), matching SnakeScene::draw_head.
        let (data, w, h, mh, mv): (&'static [u8], i32, i32, bool, bool) = match self.facing {
            (0, 1)  => (CAT_THIN_DOWN,  6, 8, false, false), // down
            (0, -1) => (CAT_THIN_DOWN,  6, 8, false, true),  // up
            (1, 0)  => (CAT_THIN_RIGHT, 8, 6, false, false), // right
            _       => (CAT_THIN_RIGHT, 8, 6, true,  false), // left
        };
        let (left, top) = (self.px - w / 2, self.py - h / 2);
        let row_bytes = (w + 7) / 8;
        (0..h).flat_map(move |y| {
            (0..w).filter_map(move |x| {
                let sx = if mh { w - 1 - x } else { x };
                let sy = if mv { h - 1 - y } else { y };
                let byte = data[(sy * row_bytes + sx / 8) as usize];
                if byte & (0x80 >> (sx % 8)) != 0 {
                    Some((left + x, top + y))
                } else {
                    None
                }
            })
        })
    }

    /// Solid filled area: WALL that doesn't touch the open area, so not the
    /// 1px wall the cat runs along.
    fn is_fill(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < GRID_W && y < GRID_H
            && self.grid[Self::idx(x, y)] == WALL
            && !self.is_edge(x, y)
    }

    /// Draw the head lit, except over solid fill where it's drawn dark so it
    /// stays readable. Walls, the trail and enemies don't invert it.
    fn draw_head(&self, renderer: &mut Renderer) {
        for (x, y) in self.head_cells() {
            renderer.draw_pixel(Point::new(ORIGIN_X + x, ORIGIN_Y + y), !self.is_fill(x, y));
        }
    }
}

fn draw_centered(renderer: &mut Renderer, text: &str, y: i32) {
    let w = text.chars().count() as i32 * CHAR_W;
    renderer.draw_text(text, Point::new((DISPLAY_WIDTH as i32 - w) / 2, y));
}

impl Scene for HerdingScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_carved = 0;
        self.session_levels = 0;
        self.level = 1;
        let mut rng = ctx.rng;
        self.start_level(&mut rng);
        ctx.rng = rng;
        self.state = State::Ready;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        self.apply_rewards(ctx);
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

        // Polled every frame so a press during the freeze isn't replayed
        // once it ends.
        let a = buttons.was_just_pressed(Button::A);

        match self.state {
            State::Ready => {
                if a {
                    self.state = State::Playing;
                }
                return None;
            }
            State::Lose | State::Win => {
                if self.end_timer > 0.0 {
                    self.end_timer = (self.end_timer - dt).max(0.0);
                } else if a {
                    if self.state == State::Win {
                        self.level += 1;
                    }
                    let mut rng = ctx.rng;
                    self.start_level(&mut rng);
                    ctx.rng = rng;
                    self.state = State::Playing;
                }
                return None;
            }
            State::Playing => {}
        }

        self.handle_input(buttons);

        self.player_progress += PLAYER_SPEED * dt;
        while self.state == State::Playing && self.player_progress >= 1.0 {
            self.player_progress -= 1.0;
            self.step_player();
        }

        self.enemy_progress += self.enemy_speed() * dt;
        while self.state == State::Playing && self.enemy_progress >= 1.0 {
            self.enemy_progress -= 1.0;
            self.step_enemies();
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        let frozen = self.end_timer > 0.0;
        match self.state {
            State::Ready => {
                draw_centered(renderer, t!("HERDING"), 16);
                draw_centered(renderer, t!("A: START"), 34);
                return;
            }
            State::Lose if !frozen => {
                draw_centered(renderer, t!("A: RETRY"), 27);
                return;
            }
            State::Win if !frozen => {
                let mut pct: String<8> = String::new();
                let _ = write!(pct, "{}%", self.filled * 100 / INTERIOR);
                let mut n: String<8> = String::new();
                let _ = write!(n, "{}", self.level + 1);
                let mut next: String<24> = String::new();
                crate::i18n::substitute(&mut next, t!("Level {n}"), &[("n", n.as_str())]);
                draw_centered(renderer, t!("YOU WIN!"), 6);
                draw_centered(renderer, &pct, 19);
                draw_centered(renderer, &next, 32);
                draw_centered(renderer, t!("A: START"), 48);
                return;
            }
            _ => {}
        }

        renderer.draw_sprite_raw(
            &self.pixels,
            GRID_W as u16,
            GRID_H as u16,
            Point::new(ORIGIN_X, ORIGIN_Y),
            SpriteOpts { transparent: true, ..Default::default() },
        );

        for e in &self.enemies {
            let (cx, cy) = (ORIGIN_X + e.x, ORIGIN_Y + e.y);
            renderer.draw_rect(Point::new(cx - 2, cy), Size::new(5, 1), true);
            renderer.draw_rect(Point::new(cx, cy - 2), Size::new(1, 5), true);
        }

        // Blink the head while frozen after being caught.
        let blink_off = self.state == State::Lose && (self.end_timer * 8.0) as i32 % 2 == 1;
        if !blink_off {
            self.draw_head(renderer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: (i32, i32) = (0, -1);
    const DOWN: (i32, i32) = (0, 1);
    const LEFT: (i32, i32) = (-1, 0);
    const RIGHT: (i32, i32) = (1, 0);

    fn arena(enemies: &[(i32, i32)]) -> HerdingScene {
        let mut s = HerdingScene::new();
        let mut rng = 1;
        s.start_level(&mut rng);
        s.enemies.clear();
        for &(x, y) in enemies {
            let _ = s.enemies.push(Enemy { x, y, dx: 1, dy: 1 });
        }
        s.state = State::Playing;
        s
    }

    /// Press `d`, then take `steps` player steps.
    fn go(s: &mut HerdingScene, d: (i32, i32), steps: i32) {
        s.press(d);
        for _ in 0..steps {
            s.step_player();
        }
    }

    fn pixels_match_grid(s: &HerdingScene) -> bool {
        (0..GRID_H).all(|y| {
            (0..GRID_W).all(|x| {
                let lit = s.pixels[y as usize * ROW_BYTES + x as usize / 8] & (0x80 >> (x % 8)) != 0;
                lit == (s.grid[HerdingScene::idx(x, y)] != OPEN)
            })
        })
    }

    #[test]
    fn straight_cut_fills_empty_side() {
        let mut s = arena(&[(90, 20)]);
        go(&mut s, UP, 54); // launch + 53 steps reaches y=1, then onto the top wall
        assert!(s.mode == Mode::OnWall);
        assert_eq!((s.px, s.py), (60, 0));
        assert_eq!(s.filled, 60 * 54); // columns 1..=59 plus the trail column
        assert_eq!(s.cell(30, 20), WALL);
        assert_eq!(s.cell(90, 20), OPEN);
        assert_eq!(s.enemies.len(), 1);
        assert!(s.is_edge(s.px, s.py));
        assert!(pixels_match_grid(&s));
        assert!(s.state == State::Playing);
    }

    #[test]
    fn staircase_cut_fills_under_the_steps() {
        let mut s = arena(&[(30, 15)]);
        s.px = 70;
        go(&mut s, UP, 19); // (70,54)..(70,35)
        go(&mut s, RIGHT, 25); // ..(95,35)
        go(&mut s, UP, 15); // ..(95,20)
        go(&mut s, RIGHT, 24); // ..(118,20), then into the right wall
        assert!(s.mode == Mode::OnWall);
        let pocket = 48 * 19 + 23 * 15;
        let trail = 20 + 25 + 15 + 23;
        assert_eq!(s.filled, pocket + trail);
        assert_eq!(s.cell(100, 50), WALL);
        assert_eq!(s.cell(100, 25), WALL);
        assert_eq!(s.cell(80, 40), WALL);
        assert_eq!(s.cell(80, 25), OPEN);
        assert_eq!(s.cell(30, 15), OPEN);
        assert!(pixels_match_grid(&s));
    }

    #[test]
    fn tie_fills_smaller_side_and_catches_its_enemy() {
        let mut s = arena(&[(20, 20), (100, 20)]);
        s.px = 40;
        go(&mut s, UP, 54);
        assert_eq!(s.enemies.len(), 1);
        assert_eq!(s.enemies[0].x, 100);
        assert_eq!(s.cell(20, 20), WALL);
    }

    #[test]
    fn more_enemies_side_is_kept_even_if_smaller() {
        let mut s = arena(&[(10, 10), (20, 30), (100, 20)]);
        s.px = 40;
        go(&mut s, UP, 54);
        assert_eq!(s.enemies.len(), 2);
        assert_eq!(s.cell(100, 20), WALL);
        assert_eq!(s.cell(10, 10), OPEN);
    }

    #[test]
    fn corner_slides_and_mid_bottom_launches() {
        let mut s = arena(&[(90, 20)]);
        go(&mut s, LEFT, 200);
        assert_eq!((s.px, s.py), (0, GRID_H - 1)); // slid non-stop to the corner
        assert_eq!(s.dir, (0, 0));
        go(&mut s, UP, 3);
        assert!(s.mode == Mode::OnWall); // slides up the side wall
        assert_eq!((s.px, s.py), (0, GRID_H - 4));

        let mut s = arena(&[(90, 20)]);
        s.press(UP);
        assert!(s.mode == Mode::Drawing);
        assert_eq!(s.cell(60, GRID_H - 2), TRAIL);
    }

    #[test]
    fn slide_stops_at_convex_corner() {
        let mut s = arena(&[(30, 15)]);
        s.px = 90;
        go(&mut s, UP, 19); // (90,54)..(90,35)
        go(&mut s, RIGHT, 29); // into the right wall
        assert!(s.mode == Mode::OnWall);
        assert_eq!((s.px, s.py), (119, 35));
        go(&mut s, LEFT, 100);
        assert_eq!((s.px, s.py), (90, 35)); // top-left corner of the filled block
        assert!(s.mode == Mode::OnWall);
        s.press(LEFT);
        assert!(s.mode == Mode::Drawing);
    }

    #[test]
    fn reversing_while_drawing_is_ignored() {
        let mut s = arena(&[(90, 20)]);
        go(&mut s, UP, 5);
        s.press(DOWN);
        assert_eq!(s.dir, UP);
    }

    #[test]
    fn running_into_own_trail_loses() {
        let mut s = arena(&[(100, 10)]);
        go(&mut s, UP, 9); // (60,45)
        go(&mut s, RIGHT, 3); // (63,45)
        go(&mut s, DOWN, 3); // (63,48)
        go(&mut s, LEFT, 3); // (60,48) is trail
        assert!(s.state == State::Lose);
    }

    #[test]
    fn enemy_touching_trail_loses() {
        let mut s = arena(&[(62, 40)]);
        go(&mut s, UP, 13); // trail reaches (60,41)
        assert!(s.state == State::Playing);
        s.step_player(); // (60,40) is under the enemy's left arm
        assert!(s.state == State::Lose);
    }

    #[test]
    fn enemies_never_overlap_walls() {
        let mut s = arena(&[(30, 15), (50, 10), (10, 30)]);
        s.px = 70;
        go(&mut s, UP, 19);
        go(&mut s, RIGHT, 25);
        go(&mut s, UP, 15);
        go(&mut s, RIGHT, 24);
        for _ in 0..5000 {
            s.step_enemies();
            for e in &s.enemies {
                assert!(!s.blocked(e.x, e.y));
            }
        }
    }

    #[test]
    fn clearing_90_percent_wins() {
        // Cutting at x=105 fills about 89%, just short.
        let mut s = arena(&[(115, 10)]);
        s.px = 105;
        go(&mut s, UP, 54);
        assert!(s.state == State::Playing);

        // Cutting at x=110 fills about 93%.
        let mut s = arena(&[(115, 10)]);
        s.px = 110;
        go(&mut s, UP, 54);
        assert!(s.state == State::Win);
        assert!(s.filled * 100 >= CLEAR_PERCENT * INTERIOR);
    }
}
