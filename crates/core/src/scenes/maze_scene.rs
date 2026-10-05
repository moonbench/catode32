//! Maze minigame. Guide the cat to the fish through a randomly generated maze.
//!
//! Maze grows with the session round: every `HARD_MODE_ROUND` rounds adds rows
//! (vertical scroll), every `WIDE_MODE_ROUND` rounds adds columns (horizontal
//! scroll). Even rounds carve reward rooms (3x3) with collectible diamonds.

use embedded_graphics::prelude::{Point, Size};
use heapless::Vec;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{items::FISH1, minigame_character::SITCAT1},
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::xorshift32,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::popup::Popup,
};

// Wall bitmask constants.
const WALL_N: u8 = 1;
const WALL_S: u8 = 2;
const WALL_E: u8 = 4;
const WALL_W: u8 = 8;
const ALL_WALLS: u8 = WALL_N | WALL_S | WALL_E | WALL_W;

// Grid layout.
const BASE_GRID_W: usize = 25;
const BASE_GRID_H: usize = 12;
const CELL_W: i32 = 5;
const CELL_H: i32 = 5;
const GRID_OFFSET_X: i32 = 1;
const GRID_OFFSET_Y: i32 = 2;

// Start / goal clear areas.
const START_CLEAR_W: usize = 4;
const START_CLEAR_H: usize = 4;
const GOAL_CLEAR_W: usize = 4;
const GOAL_CLEAR_H: usize = 3;

// Scaling thresholds.
const HARD_MODE_ROUND: u32 = 3;
const WIDE_MODE_ROUND: u32 = 5;
const MAX_SCALING_ROUND: u32 = 15;
const EXTRA_ROWS_PER_STEP: usize = 3;
const EXTRA_COLS_PER_STEP: usize = 3;

// Worst-case bounds (round 15+).
const MAX_EXTRA_ROWS: usize =
    (MAX_SCALING_ROUND as usize / HARD_MODE_ROUND as usize) * EXTRA_ROWS_PER_STEP; // 15
const MAX_EXTRA_COLS: usize =
    (MAX_SCALING_ROUND as usize / WIDE_MODE_ROUND as usize) * EXTRA_COLS_PER_STEP; //  9
const MAX_GRID_W: usize = BASE_GRID_W + MAX_EXTRA_COLS; // 34
const MAX_GRID_H: usize = BASE_GRID_H + MAX_EXTRA_ROWS; // 27
const MAX_CELLS: usize = MAX_GRID_W * MAX_GRID_H; // 918

// Prim's frontier upper bound: each cell pushes at most 4 entries (one per
// unvisited neighbour) when first visited. Round to power of two for headroom.
const MAX_FRONTIER: usize = 4096;

// Camera dead-zones (25% / 75% of screen on each axis).
const SCROLL_TOP: i32 = 16;
const SCROLL_BOT: i32 = 48;
const SCROLL_LEFT: i32 = 32;
const SCROLL_RIGHT: i32 = 96;

const WIN_DISPLAY_DURATION: f32 = 2.5;

const MAX_REWARDS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Playing,
    Win,
}

#[derive(Clone, Copy)]
enum GoalEntry {
    Left,
    Bottom,
}

#[derive(Clone, Copy)]
struct GoalArea {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    entry: GoalEntry,
}

pub struct MazeScene {
    win_popup: Popup,

    // Maze walls (row-major bitmasks).
    maze: [u8; MAX_CELLS],
    grid_w: usize,
    grid_h: usize,
    extra_rows: usize,
    extra_cols: usize,

    // Shared bool scratch: used by Prim's generator (visited) and then reused
    // at runtime as the path-membership set. They never overlap in time.
    cell_flag: [bool; MAX_CELLS],

    // Prim's frontier: (x, y, dir 0=N/1=S/2=E/3=W).
    frontier: Vec<(u8, u8, u8), MAX_FRONTIER>,

    // Camera.
    camera_x: i32,
    camera_y: i32,
    max_camera_x: i32,
    max_camera_y: i32,

    // Player.
    player_x: usize,
    player_y: usize,
    path: Vec<(u8, u8), MAX_CELLS>,

    // Goal.
    goal: GoalArea,

    // Rewards (even rounds).
    rewards: Vec<(u8, u8), MAX_REWARDS>,
    rewards_collected: [bool; MAX_REWARDS],
    coins_earned: u32,

    // Session.
    session_round: u32,
    session_completions: u32,

    // State + timers.
    state: State,
    anim_t: f32,
    win_timer: f32,
    b_held_time: f32,
    b_rewind_timer: f32,
}

impl MazeScene {
    pub fn new() -> Self {
        let mut win_popup = Popup::new(14, 16, 100, 32);
        win_popup.set_text(t!("Found it!"), false, true);
        Self {
            win_popup,
            maze: [0; MAX_CELLS],
            grid_w: BASE_GRID_W,
            grid_h: BASE_GRID_H,
            extra_rows: 0,
            extra_cols: 0,
            cell_flag: [false; MAX_CELLS],
            frontier: Vec::new(),
            camera_x: 0,
            camera_y: 0,
            max_camera_x: 0,
            max_camera_y: 0,
            player_x: 0,
            player_y: 0,
            path: Vec::new(),
            goal: GoalArea {
                x: 0,
                y: 0,
                w: GOAL_CLEAR_W,
                h: GOAL_CLEAR_H,
                entry: GoalEntry::Left,
            },
            rewards: Vec::new(),
            rewards_collected: [false; MAX_REWARDS],
            coins_earned: 0,
            session_round: 0,
            session_completions: 0,
            state: State::Playing,
            anim_t: 0.0,
            win_timer: 0.0,
            b_held_time: 0.0,
            b_rewind_timer: 0.0,
        }
    }

    fn reset_game(&mut self, rng: &mut u32) {
        if self.state == State::Win {
            self.session_completions += 1;
        }

        self.session_round += 1;
        let scaled_round = self.session_round.min(MAX_SCALING_ROUND) as usize;
        self.extra_rows = (scaled_round / HARD_MODE_ROUND as usize) * EXTRA_ROWS_PER_STEP;
        self.grid_h = BASE_GRID_H + self.extra_rows;
        self.max_camera_y = self.extra_rows as i32 * CELL_H;
        self.camera_y = self.max_camera_y;
        self.extra_cols = (scaled_round / WIDE_MODE_ROUND as usize) * EXTRA_COLS_PER_STEP;
        self.grid_w = BASE_GRID_W + self.extra_cols;
        self.max_camera_x = self.extra_cols as i32 * CELL_W;
        self.camera_x = 0;

        self.goal = Self::pick_goal_area(self.session_round, self.grid_w, self.grid_h, rng);
        self.generate_maze(rng);

        self.rewards.clear();
        for v in self.rewards_collected.iter_mut() {
            *v = false;
        }
        if self.session_round % 2 == 0 {
            let count = if self.session_round >= 12 {
                3
            } else if self.session_round >= 8 {
                2
            } else {
                1
            };
            self.setup_reward_areas(count, rng);
        }

        // Player start: column 4 (just right of the start area), bottom row.
        self.player_x = 4;
        self.player_y = self.grid_h - 1;

        // Reuse cell_flag as the path-membership set.
        for v in self.cell_flag.iter_mut() {
            *v = false;
        }
        self.path.clear();
        let _ = self.path.push((self.player_x as u8, self.player_y as u8));
        self.cell_flag[self.player_y * self.grid_w + self.player_x] = true;

        self.anim_t = 0.0;
        self.win_timer = 0.0;
        self.b_held_time = 0.0;
        self.b_rewind_timer = 0.0;
        self.state = State::Playing;
    }

    fn pick_goal_area(round: u32, gw: usize, gh: usize, rng: &mut u32) -> GoalArea {
        // Choice key: 0 = top_right, 1 = top_mid, 2 = mid_right.
        let choice = if round >= WIDE_MODE_ROUND {
            xorshift32(rng) % 3
        } else if round >= HARD_MODE_ROUND {
            xorshift32(rng) % 2
        } else {
            0
        };
        match choice {
            1 => GoalArea {
                x: (gw - GOAL_CLEAR_W) / 2,
                y: 0,
                w: GOAL_CLEAR_W,
                h: GOAL_CLEAR_H,
                entry: GoalEntry::Bottom,
            },
            2 => GoalArea {
                x: gw - GOAL_CLEAR_W,
                y: (gh - GOAL_CLEAR_H) / 2,
                w: GOAL_CLEAR_W,
                h: GOAL_CLEAR_H,
                entry: GoalEntry::Left,
            },
            _ => GoalArea {
                x: gw - GOAL_CLEAR_W,
                y: 0,
                w: GOAL_CLEAR_W,
                h: GOAL_CLEAR_H,
                entry: GoalEntry::Left,
            },
        }
    }

    fn generate_maze(&mut self, rng: &mut u32) {
        let gw = self.grid_w;
        let gh = self.grid_h;
        let total = gw * gh;

        for i in 0..total {
            self.maze[i] = ALL_WALLS;
        }
        for v in self.cell_flag.iter_mut() {
            *v = false;
        }

        // Clear walls inside start area (bottom-left) and mark visited.
        let sa_y = gh - START_CLEAR_H;
        for ay in sa_y..sa_y + START_CLEAR_H {
            for ax in 0..START_CLEAR_W {
                self.cell_flag[ay * gw + ax] = true;
                if ay > sa_y {
                    self.maze[ay * gw + ax] &= !WALL_N;
                    self.maze[(ay - 1) * gw + ax] &= !WALL_S;
                }
                if ax > 0 {
                    self.maze[ay * gw + ax] &= !WALL_W;
                    self.maze[ay * gw + ax - 1] &= !WALL_E;
                }
            }
        }
        // Clear walls inside goal area and mark visited.
        let ga = self.goal;
        for ay in ga.y..ga.y + ga.h {
            for ax in ga.x..ga.x + ga.w {
                self.cell_flag[ay * gw + ax] = true;
                if ay > ga.y {
                    self.maze[ay * gw + ax] &= !WALL_N;
                    self.maze[(ay - 1) * gw + ax] &= !WALL_S;
                }
                if ax > ga.x {
                    self.maze[ay * gw + ax] &= !WALL_W;
                    self.maze[ay * gw + ax - 1] &= !WALL_E;
                }
            }
        }

        // Connect start area to the maze proper at (START_CLEAR_W, gh-1).
        let start_gen_x = START_CLEAR_W;
        let start_gen_y = gh - 1;
        self.cell_flag[start_gen_y * gw + start_gen_x] = true;
        self.maze[start_gen_y * gw + start_gen_x] &= !WALL_W;
        self.maze[start_gen_y * gw + start_gen_x - 1] &= !WALL_E;

        self.frontier.clear();
        self.add_frontier(start_gen_x, start_gen_y);

        while !self.frontier.is_empty() {
            let idx = (xorshift32(rng) as usize) % self.frontier.len();
            let (x, y, dir) = self.frontier.swap_remove(idx);
            let (dx, dy, wall, opposite): (i32, i32, u8, u8) = match dir {
                0 => (0, -1, WALL_N, WALL_S),
                1 => (0, 1, WALL_S, WALL_N),
                2 => (1, 0, WALL_E, WALL_W),
                _ => (-1, 0, WALL_W, WALL_E),
            };
            let nx_i = x as i32 + dx;
            let ny_i = y as i32 + dy;
            if nx_i < 0 || ny_i < 0 || nx_i >= gw as i32 || ny_i >= gh as i32 {
                continue;
            }
            let nx = nx_i as usize;
            let ny = ny_i as usize;
            if !self.cell_flag[ny * gw + nx] {
                self.maze[y as usize * gw + x as usize] &= !wall;
                self.maze[ny * gw + nx] &= !opposite;
                self.cell_flag[ny * gw + nx] = true;
                self.add_frontier(nx, ny);
            }
        }

        // Connect goal area to maze proper.
        match self.goal.entry {
            GoalEntry::Left => {
                let ex = self.goal.x - 1;
                let ey = self.goal.y + self.goal.h - 1;
                self.maze[ey * gw + ex] &= !WALL_E;
                self.maze[ey * gw + ex + 1] &= !WALL_W;
            }
            GoalEntry::Bottom => {
                let ex = self.goal.x + self.goal.w / 2;
                let ey = self.goal.y + self.goal.h;
                self.maze[ey * gw + ex] &= !WALL_N;
                self.maze[(ey - 1) * gw + ex] &= !WALL_S;
            }
        }
    }

    fn add_frontier(&mut self, x: usize, y: usize) {
        let gw = self.grid_w as i32;
        let gh = self.grid_h as i32;
        for dir in 0..4u8 {
            let (dx, dy): (i32, i32) = match dir {
                0 => (0, -1),
                1 => (0, 1),
                2 => (1, 0),
                _ => (-1, 0),
            };
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && nx < gw && ny < gh {
                if !self.cell_flag[ny as usize * self.grid_w + nx as usize] {
                    let _ = self.frontier.push((x as u8, y as u8, dir));
                }
            }
        }
    }

    fn setup_reward_areas(&mut self, count: u32, rng: &mut u32) {
        let gw = self.grid_w;
        let gh = self.grid_h;
        let mut placed: Vec<(u8, u8), MAX_REWARDS> = Vec::new();

        for _ in 0..count {
            for _ in 0..100 {
                let rx = 1 + (xorshift32(rng) as usize) % (gw - 4);
                let ry = 1 + (xorshift32(rng) as usize) % (gh - 4);

                let in_start =
                    rx < START_CLEAR_W && ry + 2 >= gh - START_CLEAR_H;
                let ga = self.goal;
                let in_goal = rx + 2 >= ga.x
                    && rx < ga.x + ga.w
                    && ry + 2 >= ga.y
                    && ry < ga.y + ga.h;
                let too_close = placed.iter().any(|&(px, py)| {
                    (rx as i32 - px as i32).abs() < 4 && (ry as i32 - py as i32).abs() < 4
                });

                if !in_start && !in_goal && !too_close {
                    let _ = placed.push((rx as u8, ry as u8));
                    break;
                }
            }
        }

        for &(rx, ry) in &placed {
            // Clear internal walls of the 3x3 block.
            for dy in 0..3usize {
                for dx in 0..3usize {
                    let cx = rx as usize + dx;
                    let cy = ry as usize + dy;
                    if dy > 0 {
                        self.maze[cy * gw + cx] &= !WALL_N;
                        self.maze[(cy - 1) * gw + cx] &= !WALL_S;
                    }
                    if dx > 0 {
                        self.maze[cy * gw + cx] &= !WALL_W;
                        self.maze[cy * gw + cx - 1] &= !WALL_E;
                    }
                }
            }
            let _ = self.rewards.push((rx + 1, ry + 1));
        }
    }

    fn can_move(&self, dx: i32, dy: i32) -> bool {
        let cell = self.maze[self.player_y * self.grid_w + self.player_x];
        match (dx, dy) {
            (1, 0) => cell & WALL_E == 0,
            (-1, 0) => cell & WALL_W == 0,
            (0, -1) => cell & WALL_N == 0,
            (0, 1) => cell & WALL_S == 0,
            _ => false,
        }
    }

    fn update_camera(&mut self) {
        if self.max_camera_y != 0 {
            let player_py = GRID_OFFSET_Y
                + self.player_y as i32 * CELL_H
                + CELL_H / 2;
            let screen_py = player_py - self.camera_y;
            if screen_py < SCROLL_TOP {
                self.camera_y = (player_py - SCROLL_TOP).max(0);
            } else if screen_py > SCROLL_BOT {
                self.camera_y = (player_py - SCROLL_BOT).min(self.max_camera_y);
            }
        }

        if self.max_camera_x != 0 {
            let player_px = GRID_OFFSET_X
                + self.player_x as i32 * CELL_W
                + CELL_W / 2;
            let screen_px = player_px - self.camera_x;
            if screen_px < SCROLL_LEFT {
                self.camera_x = (player_px - SCROLL_LEFT).max(0);
            } else if screen_px > SCROLL_RIGHT {
                self.camera_x = (player_px - SCROLL_RIGHT).min(self.max_camera_x);
            }
        }
    }

    fn rewind_step(&mut self) {
        if self.path.len() <= 1 {
            return;
        }
        if let Some((lx, ly)) = self.path.pop() {
            self.cell_flag[ly as usize * self.grid_w + lx as usize] = false;
        }
        let &(px, py) = self.path.last().unwrap();
        self.player_x = px as usize;
        self.player_y = py as usize;
        self.update_camera();
    }

    fn move_player(&mut self, dx: i32, dy: i32) {
        if !self.can_move(dx, dy) {
            return;
        }
        let nx = (self.player_x as i32 + dx) as usize;
        let ny = (self.player_y as i32 + dy) as usize;
        let new_idx = ny * self.grid_w + nx;

        if self.cell_flag[new_idx] {
            // Backtrack: pop until the new position sits at the path tail.
            while let Some(&(lx, ly)) = self.path.last() {
                if lx as usize == nx && ly as usize == ny {
                    break;
                }
                self.path.pop();
                self.cell_flag[ly as usize * self.grid_w + lx as usize] = false;
            }
        } else {
            let _ = self.path.push((nx as u8, ny as u8));
            self.cell_flag[new_idx] = true;
        }

        self.player_x = nx;
        self.player_y = ny;
        self.update_camera();

        // Collect reward if standing on one.
        for i in 0..self.rewards.len() {
            let (rx, ry) = self.rewards[i];
            if !self.rewards_collected[i] && rx as usize == nx && ry as usize == ny {
                self.rewards_collected[i] = true;
                self.coins_earned += 1;
            }
        }

        // Win check: player inside goal area.
        let ga = self.goal;
        if nx >= ga.x && nx < ga.x + ga.w && ny >= ga.y && ny < ga.y + ga.h {
            self.state = State::Win;
            self.win_timer = 0.0;
        }
    }

    fn draw_maze(&self, renderer: &mut Renderer) {
        // Cull cells outside the screen.
        let min_cx = (((self.camera_x - GRID_OFFSET_X) / CELL_W).max(0) as usize).min(self.grid_w);
        let max_cx_excl = (((self.camera_x + 128 - GRID_OFFSET_X) / CELL_W + 1).max(0) as usize)
            .min(self.grid_w);
        let min_cy = (((self.camera_y - GRID_OFFSET_Y) / CELL_H).max(0) as usize).min(self.grid_h);
        let max_cy_excl = (((self.camera_y + 64 - GRID_OFFSET_Y) / CELL_H + 1).max(0) as usize)
            .min(self.grid_h);

        let cw1 = CELL_W - 1;
        let ch1 = CELL_H - 1;
        for cy in min_cy..max_cy_excl {
            for cx in min_cx..max_cx_excl {
                let walls = self.maze[cy * self.grid_w + cx];
                let px = GRID_OFFSET_X + cx as i32 * CELL_W - self.camera_x;
                let py = GRID_OFFSET_Y + cy as i32 * CELL_H - self.camera_y;
                if walls & WALL_N != 0 {
                    renderer.draw_line(Point::new(px, py), Point::new(px + cw1, py));
                }
                if walls & WALL_S != 0 {
                    renderer
                        .draw_line(Point::new(px, py + ch1), Point::new(px + cw1, py + ch1));
                }
                if walls & WALL_E != 0 {
                    renderer
                        .draw_line(Point::new(px + cw1, py), Point::new(px + cw1, py + ch1));
                }
                if walls & WALL_W != 0 {
                    renderer.draw_line(Point::new(px, py), Point::new(px, py + ch1));
                }
            }
        }
    }

    fn cell_center(&self, cx: usize, cy: usize) -> (i32, i32) {
        let px = GRID_OFFSET_X + cx as i32 * CELL_W + CELL_W / 2;
        let py = GRID_OFFSET_Y + cy as i32 * CELL_H + CELL_H / 2;
        (px, py)
    }

    fn draw_path(&self, renderer: &mut Renderer) {
        if self.path.len() < 2 {
            return;
        }
        for i in 0..self.path.len() - 1 {
            let (x1, y1) = self.path[i];
            let (x2, y2) = self.path[i + 1];
            let (px1, py1) = self.cell_center(x1 as usize, y1 as usize);
            let (px2, py2) = self.cell_center(x2 as usize, y2 as usize);
            renderer.draw_line(
                Point::new(px1 - self.camera_x, py1 - self.camera_y),
                Point::new(px2 - self.camera_x, py2 - self.camera_y),
            );
        }
    }

    fn draw_rewards(&self, renderer: &mut Renderer) {
        if self.rewards.is_empty() {
            return;
        }
        // Bob = int(sin(t * pi) * 2).
        let bob = (self.anim_t * core::f32::consts::PI).sin() * 2.0;
        let bob = bob as i32;
        let r: i32 = 2;
        for i in 0..self.rewards.len() {
            if self.rewards_collected[i] {
                continue;
            }
            let (rx, ry) = self.rewards[i];
            let (mut cx, mut cy) = self.cell_center(rx as usize, ry as usize);
            cx -= self.camera_x;
            cy -= self.camera_y + bob;
            // Hollow diamond outline.
            renderer.draw_line(Point::new(cx, cy - r), Point::new(cx + r, cy));
            renderer.draw_line(Point::new(cx + r, cy), Point::new(cx, cy + r));
            renderer.draw_line(Point::new(cx, cy + r), Point::new(cx - r, cy));
            renderer.draw_line(Point::new(cx - r, cy), Point::new(cx, cy - r));
        }
    }

    fn draw_goal(&self, renderer: &mut Renderer) {
        let px = GRID_OFFSET_X + self.goal.x as i32 * CELL_W + 2 - self.camera_x;
        let py = GRID_OFFSET_Y + self.goal.y as i32 * CELL_H + 2 - self.camera_y;
        renderer.draw_sprite(
            &FISH1,
            Point::new(px, py),
            SpriteOpts {
                transparent: true,
                transparent_color: false,
                ..Default::default()
            },
        );
    }

    fn draw_player(&self, renderer: &mut Renderer) {
        let x = 3 - self.camera_x;
        let y = 44 + self.max_camera_y - self.camera_y;
        renderer.draw_sprite(
            &SITCAT1,
            Point::new(x, y),
            SpriteOpts {
                transparent: true,
                transparent_color: false,
                ..Default::default()
            },
        );
    }

    fn draw_position_indicator(&self, renderer: &mut Renderer) {
        // Blink: visible half the time at 2 Hz.
        let phase = self.anim_t - (self.anim_t / 0.5).floor() * 0.5;
        if phase >= 0.25 {
            return;
        }
        let (cx, cy) = self.cell_center(self.player_x, self.player_y);
        renderer.draw_rect(
            Point::new(cx - 1 - self.camera_x, cy - 1 - self.camera_y),
            Size::new(3, 3),
            false,
        );
    }
}

impl Scene for MazeScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        // Only the round counter resets each visit. Completion count and coin
        // tally intentionally carry over.
        self.session_round = 0;
        let mut rng = ctx.rng;
        self.reset_game(&mut rng);
        ctx.rng = rng;
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let completions = self.session_completions;
        if completions == 0 {
            return;
        }
        let scale = ((completions as f32) / 2.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Intelligence, 3.0 * scale),
            (StatId::Curiosity,    3.0 * scale),
            (StatId::Focus,        3.0 * scale),
            (StatId::Sociability,  2.0),
            (StatId::Loyalty,      1.0 * scale),
        ]);
        let coins = (5.0 * scale) as i32 + self.coins_earned as i32;
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

        self.anim_t += dt;

        // Win-state path: A or timer advances to the next round.
        if self.state == State::Win {
            self.win_timer += dt;
            if buttons.was_just_pressed(Button::A) {
                let mut rng = ctx.rng;
                self.reset_game(&mut rng);
                ctx.rng = rng;
                return None;
            }
            if self.win_timer >= WIN_DISPLAY_DURATION {
                let mut rng = ctx.rng;
                self.reset_game(&mut rng);
                ctx.rng = rng;
            }
            return None;
        }

        // Held-B rewind with acceleration. This advances before the
        // edge-trigger handler.
        if buttons.is_pressed(Button::B) {
            if self.path.len() > 1 {
                self.b_held_time += dt;
                self.b_rewind_timer += dt;
                let interval = (0.3 - self.b_held_time * 0.12).max(0.05);
                if self.b_rewind_timer >= interval {
                    self.b_rewind_timer -= interval;
                    self.rewind_step();
                }
            }
        } else {
            self.b_held_time = 0.0;
            self.b_rewind_timer = 0.0;
        }

        // Edge-triggered input.
        if buttons.was_just_pressed(Button::B) {
            self.rewind_step();
            self.b_held_time = 0.0;
            self.b_rewind_timer = 0.0;
        } else if buttons.was_just_pressed(Button::Up) {
            self.move_player(0, -1);
        } else if buttons.was_just_pressed(Button::Down) {
            self.move_player(0, 1);
        } else if buttons.was_just_pressed(Button::Left) {
            self.move_player(-1, 0);
        } else if buttons.was_just_pressed(Button::Right) {
            self.move_player(1, 0);
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.draw_maze(renderer);
        self.draw_path(renderer);
        self.draw_rewards(renderer);
        self.draw_goal(renderer);
        self.draw_player(renderer);
        self.draw_position_indicator(renderer);

        if self.state == State::Win {
            self.win_popup.draw(renderer, false);
        }
    }
}
