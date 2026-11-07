//! Mini Golf. Putt a ball round randomly generated greens a few screens big,
//! nine holes to a round. Left/right aims, A starts a power gauge that swings
//! up and down, and A again putts at whatever it reads.
//!
//! Greens are laid out on a grid of 16px cells by a turtle that places
//! straights, turns, forks around an island and wide rooms with an obstacle
//! in the middle. The boundary between open and solid cells is traced into
//! rectilinear loops whose corners are then cut at assorted angles, so the
//! ball bounces off line segments rather than cells. Cut ends always land on
//! an 8px lattice.

use core::f32::consts::PI;
use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    context::{GameContext, StatId},
    input::{Button, Buttons},
    rand::{rand_bool, rand_range_u32},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::confirm::{Confirm, ConfirmResult},
    ui::menu::{Menu, MenuItem, MenuResult},
    ui::popup::Popup,
};

const SCREEN_W: i32 = 128;
const SCREEN_H: i32 = 64;

const CELL: i32 = 16;
/// Generation grid. Greens are cropped to what the turtle used.
const GW: usize = 26;
const GH: usize = 16;
const GCELLS: usize = GW * GH;
/// World px between the grid and the world edge, so edge walls are visible.
const MARGIN: i32 = 2;
const MAX_SEGS: usize = 200;
const MAX_PIECES: usize = 10;
const MAX_LOOPS: usize = 24;
/// Narrowest opening allowed between walls, a little over the ball's width.
const MIN_GAP: f32 = 10.0;
const MAX_GRASS: usize = 600;
/// One chance at a grass dot per square of this many px off the green.
const GRASS_PITCH: i32 = 8;

const HOLES: usize = 9;

// Cell surfaces. Slopes are `SLOPE + dir`, dir pointing downhill.
const SOLID: u8 = 0;
const FAIRWAY: u8 = 1;
const ROUGH: u8 = 2;
const SLOPE: u8 = 3;

/// E, S, W, N in screen coordinates. A right turn is `(d + 1) % 4`.
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

// Physics, in px and seconds. Speeds are kept low so the ball can be
// watched rolling; scaling speeds by k and accelerations by k² keeps the
// distances the same.
const BALL_R: f32 = 2.0;
const HOLE_R: f32 = 3.0;
const FRICTION: f32 = 18.0;
const ROUGH_FRICTION: f32 = 61.0;
const SLOPE_ACCEL: f32 = 29.0;
const V_MAX: f32 = 90.0;
const V_MIN: f32 = 5.0;
const RESTITUTION: f32 = 0.75;
/// Faster than this and the ball rolls over the hole. Well under half of
/// `V_MAX`, so a hard putt runs past and comes back off the wall too fast.
const V_CAPTURE: f32 = 32.0;
/// Pull towards the cup while the ball is over it.
const HOLE_PULL: f32 = 90.0;
/// Longest substep, kept under the ball radius so walls can't be skipped.
const MAX_STEP: f32 = 1.0;
const STOP_SPEED: f32 = 4.0;
const STOP_TIME: f32 = 0.3;

/// Distance a comfortable putt covers, for estimating par.
const SHOT_REACH: i32 = 160;

const POWER_PERIOD: f32 = 1.4;
const AIM_TAP: f32 = 3.0 * PI / 180.0;
const AIM_RATE: f32 = 90.0 * PI / 180.0;
const AIM_HOLD_DELAY: f32 = 0.25;
/// Radius of the ring drawn round the ball while aiming.
const AIM_RING: i32 = 5;
/// Length of the aim line beyond the ring.
const AIM_LEN: f32 = 14.0;

const SPLASH_TIME: f32 = 2.0;
const SUNK_TIME: f32 = 2.5;
/// Strokes over par before the ball is picked up.
const MAX_OVER: u8 = 4;

// Camera dead zone in screen px.
const CAM_LEFT: f32 = 40.0;
const CAM_RIGHT: f32 = 88.0;
const CAM_TOP: f32 = 20.0;
const CAM_BOTTOM: f32 = 44.0;
/// How quickly the camera eases into its aiming spot, per second.
const CAM_PAN: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MiniGolfAction {
    NewRound,
    NewGreen,
}

const OPTIONS_ITEMS: &[MenuItem<MiniGolfAction>] = &[
    MenuItem { label: t!("New Round"), icon: None, submenu: None, action: Some(MiniGolfAction::NewRound), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("New Green"), icon: None, submenu: None, action: Some(MiniGolfAction::NewGreen), confirm: None, confirm_on_vacation: None },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Splash,
    Aim,
    Power,
    Rolling,
    Sunk,
    Scorecard,
}

// ---------------------------------------------------------------------------
// Layout: the turtle walk over the generation grid.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Straight,
    /// Three wide with a solid island down the middle.
    Fork,
    /// Three wide and open, with an obstacle in the middle.
    Room,
    /// Open putting area at the end, two or three wide, holding the hole.
    Green,
}

/// A placed piece. The rect is inclusive and in grid cells.
#[derive(Clone, Copy)]
struct Piece {
    kind: Kind,
    dir: u8,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

struct Layout {
    /// Piece id + 1 for every cell a piece claimed, islands included.
    owner: [u8; GCELLS],
    open: [bool; GCELLS],
    pieces: Vec<Piece, MAX_PIECES>,
    /// Last cell of the lane so far. Starts one behind the tee.
    head: (i32, i32),
    dir: u8,
    start: (i32, i32),
    hole: (i32, i32),
}

fn grid_idx(x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || x >= GW as i32 || y >= GH as i32 {
        None
    } else {
        Some(y as usize * GW + x as usize)
    }
}

impl Layout {
    fn new(start: (i32, i32), dir: u8) -> Self {
        let (dx, dy) = DIRS[dir as usize];
        Self {
            owner: [0; GCELLS],
            open: [false; GCELLS],
            pieces: Vec::new(),
            head: (start.0 - dx, start.1 - dy),
            dir,
            start,
            hole: start,
        }
    }

    fn last_kind(&self) -> Option<Kind> {
        self.pieces.last().map(|p| p.kind)
    }

    /// Place a piece off the head. It may only touch the previous piece, and
    /// only side-on at the head, so lanes never merge by accident.
    fn try_place(&mut self, kind: Kind, dir: u8, len: i32) -> bool {
        let span = if kind == Kind::Straight { (0, 0) } else { (-1, 1) };
        self.try_place_span(kind, dir, len, span)
    }

    /// As `try_place`, with the piece spanning `lo..=hi` cells across the
    /// lane (right of travel is positive), so it can open out to one side.
    fn try_place_span(&mut self, kind: Kind, dir: u8, len: i32, (lo, hi): (i32, i32)) -> bool {
        if self.pieces.is_full() {
            return false;
        }
        let (dx, dy) = DIRS[dir as usize];
        let (px, py) = (-dy, dx);
        let (hx, hy) = self.head;
        let id = self.pieces.len() as u8 + 1;
        let prev = id - 1;

        let mut cells: Vec<(i32, i32, bool), 16> = Vec::new();
        for k in 1..=len {
            for l in lo..=hi {
                let island = kind == Kind::Fork && l == 0 && k > 1 && k < len;
                let _ = cells.push((hx + dx * k + px * l, hy + dy * k + py * l, !island));
            }
        }
        let entry = (hx + dx, hy + dy);

        for &(x, y, _) in &cells {
            let Some(i) = grid_idx(x, y) else { return false };
            if self.owner[i] != 0 {
                return false;
            }
            for ny in y - 1..=y + 1 {
                for nx in x - 1..=x + 1 {
                    let Some(n) = grid_idx(nx, ny) else { continue };
                    let o = self.owner[n];
                    if o == 0 {
                        continue;
                    }
                    if o != prev {
                        return false;
                    }
                    let side_on = nx == x || ny == y;
                    if side_on && !((nx, ny) == self.head && (x, y) == entry) {
                        return false;
                    }
                }
            }
        }

        // Two open cells meeting only at a corner would make walls cross.
        let open_at = |x: i32, y: i32| -> bool {
            if let Some(&(_, _, o)) = cells.iter().find(|c| c.0 == x && c.1 == y) {
                return o;
            }
            grid_idx(x, y).map_or(false, |i| self.open[i])
        };
        for &(x, y, _) in &cells {
            for by in y - 1..=y {
                for bx in x - 1..=x {
                    let tl = open_at(bx, by);
                    let tr = open_at(bx + 1, by);
                    let bl = open_at(bx, by + 1);
                    let br = open_at(bx + 1, by + 1);
                    if (tl && br && !tr && !bl) || (tr && bl && !tl && !br) {
                        return false;
                    }
                }
            }
        }

        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(x, y, o) in &cells {
            let i = grid_idx(x, y).unwrap();
            self.owner[i] = id;
            self.open[i] = o;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let _ = self.pieces.push(Piece { kind, dir, x0, y0, x1, y1 });
        self.head = (hx + dx * len, hy + dy * len);
        self.dir = dir;
        true
    }
}

fn gen_layout(rng: &mut u32) -> Option<Layout> {
    let dir = rand_range_u32(rng, 0, 3) as u8;
    let sx = rand_range_u32(rng, 6, GW as u32 - 7) as i32;
    let sy = rand_range_u32(rng, 4, GH as u32 - 5) as i32;
    let mut lay = Layout::new((sx, sy), dir);
    if !lay.try_place(Kind::Straight, dir, rand_range_u32(rng, 2, 3) as i32) {
        return None;
    }

    let target = rand_range_u32(rng, 4, 6) as usize;
    let mut forks = 0;
    let mut rooms = 0;
    while lay.pieces.len() < target {
        let mut placed = false;
        for _ in 0..12 {
            let after_straight = lay.last_kind() == Some(Kind::Straight);
            let roll = rand_range_u32(rng, 0, 99);
            let (kind, d, len) = if after_straight && roll < 20 && forks < 2 {
                (Kind::Fork, lay.dir, rand_range_u32(rng, 3, 5) as i32)
            } else if after_straight && roll < 35 && rooms < 2 {
                (Kind::Room, lay.dir, rand_range_u32(rng, 2, 3) as i32)
            } else if after_straight {
                // Carrying straight on would only lengthen the last piece.
                let turn = if rand_bool(rng, 0.5) { 1 } else { 3 };
                (Kind::Straight, (lay.dir + turn) % 4, rand_range_u32(rng, 2, 3) as i32)
            } else {
                // Forks and rooms are only left the way they came in.
                (Kind::Straight, lay.dir, rand_range_u32(rng, 2, 4) as i32)
            };
            if lay.try_place(kind, d, len) {
                match kind {
                    Kind::Fork => forks += 1,
                    Kind::Room => rooms += 1,
                    _ => {}
                }
                placed = true;
                break;
            }
        }
        if !placed {
            break;
        }
    }
    if !finish(&mut lay, rng) || lay.pieces.len() < 4 {
        return None;
    }
    let (x0, y0, x1, y1) = bbox(&lay);
    // A green should need scrolling to see all of it.
    if (x1 - x0 + 1) * CELL <= SCREEN_W && (y1 - y0 + 1) * CELL <= SCREEN_H {
        return None;
    }
    Some(lay)
}

/// Place the hole: at the end of a lane, on an open green, or at a turn with
/// a short spur beyond it to overshoot into.
fn finish(lay: &mut Layout, rng: &mut u32) -> bool {
    // Forks and rooms are only left by a lane, so the hole isn't in one.
    if lay.last_kind() != Some(Kind::Straight) && !lay.try_place(Kind::Straight, lay.dir, 2) {
        return false;
    }
    let roll = rand_range_u32(rng, 0, 99);
    if roll < 55 {
        for _ in 0..8 {
            if try_green(lay, rng) {
                return true;
            }
        }
    } else if roll < 75 {
        let corner = lay.head;
        let turn = if rand_bool(rng, 0.5) { 1 } else { 3 };
        for t in [turn, 4 - turn] {
            if lay.try_place(Kind::Straight, (lay.dir + t) % 4, rand_range_u32(rng, 1, 2) as i32) {
                lay.hole = corner;
                return true;
            }
        }
    }
    lay.hole = lay.head;
    true
}

/// An open green two or three wide, straight on or round a turn, with the
/// hole anywhere but the mouth.
fn try_green(lay: &mut Layout, rng: &mut u32) -> bool {
    // Lopsided spans let a green flare out round a turn.
    const SPANS: [(i32, i32); 5] = [(-1, 1), (0, 2), (-2, 0), (0, 1), (-1, 0)];
    let turn = match rand_range_u32(rng, 0, 3) {
        0 => 1,
        1 => 3,
        _ => 0,
    };
    let dir = (lay.dir + turn) % 4;
    let span = SPANS[rand_range_u32(rng, 0, SPANS.len() as u32 - 1) as usize];
    let (dx, dy) = DIRS[dir as usize];
    let entry = (lay.head.0 + dx, lay.head.1 + dy);
    if !lay.try_place_span(Kind::Green, dir, rand_range_u32(rng, 2, 3) as i32, span) {
        return false;
    }
    let p = *lay.pieces.last().unwrap();
    loop {
        let c = (
            rand_range_u32(rng, p.x0 as u32, p.x1 as u32) as i32,
            rand_range_u32(rng, p.y0 as u32, p.y1 as u32) as i32,
        );
        if c != entry {
            lay.hole = c;
            return true;
        }
    }
}

fn bbox(lay: &Layout) -> (i32, i32, i32, i32) {
    lay.pieces.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |b, p| {
        (b.0.min(p.x0), b.1.min(p.y0), b.2.max(p.x1), b.3.max(p.y1))
    })
}

/// A plain S-bend, in case generation keeps failing.
fn fallback_layout() -> Layout {
    let mut lay = Layout::new((2, 2), 0);
    lay.try_place(Kind::Straight, 0, 8);
    lay.try_place(Kind::Straight, 1, 3);
    lay.try_place(Kind::Straight, 2, 6);
    lay.hole = lay.head;
    lay
}

// ---------------------------------------------------------------------------
// Green: the cropped grid plus wall segments, ready to play.

/// A wall from `a` to `b`, with the fairway on its right (screen coords).
#[derive(Clone, Copy, Debug)]
struct Seg {
    ax: i16,
    ay: i16,
    bx: i16,
    by: i16,
}

struct Green {
    w: usize,
    h: usize,
    /// `y * w + x`.
    cells: [u8; GCELLS],
    /// Cut by a diagonal wall or obstacle, so it stays plain fairway.
    clipped: [bool; GCELLS],
    segs: Vec<Seg, MAX_SEGS>,
    /// (first seg, seg count) of each closed wall loop.
    loops: Vec<(u8, u8), MAX_LOOPS>,
    tee: (f32, f32),
    tee_cell: (i32, i32),
    tee_dir: u8,
    hole: (f32, f32),
    hole_cell: (i32, i32),
    /// Cells of the green round the hole, if it ends on one.
    hole_area: Option<(i32, i32, i32, i32)>,
    par: u8,
    /// Specks of grass off the green, in world px.
    grass: Vec<(i16, i16), MAX_GRASS>,
}

fn cell_center(c: (i32, i32)) -> (f32, f32) {
    (
        (c.0 * CELL + CELL / 2 + MARGIN) as f32,
        (c.1 * CELL + CELL / 2 + MARGIN) as f32,
    )
}

impl Green {
    const fn empty() -> Self {
        Self {
            w: 0,
            h: 0,
            cells: [SOLID; GCELLS],
            clipped: [false; GCELLS],
            segs: Vec::new(),
            loops: Vec::new(),
            tee: (0.0, 0.0),
            tee_cell: (0, 0),
            tee_dir: 0,
            hole: (0.0, 0.0),
            hole_cell: (0, 0),
            hole_area: None,
            par: 3,
            grass: Vec::new(),
        }
    }

    fn generate(rng: &mut u32) -> Self {
        for _ in 0..40 {
            if let Some(lay) = gen_layout(rng) {
                if let Some(g) = Self::build(&lay, rng, true) {
                    return g;
                }
            }
        }
        Self::build(&fallback_layout(), rng, false).unwrap()
    }

    fn world_w(&self) -> i32 {
        self.w as i32 * CELL + 2 * MARGIN
    }

    fn world_h(&self) -> i32 {
        self.h as i32 * CELL + 2 * MARGIN
    }

    fn cell(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            SOLID
        } else {
            self.cells[y as usize * self.w + x as usize]
        }
    }

    fn is_clipped(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h
            && self.clipped[y as usize * self.w + x as usize]
    }

    fn open(&self, x: i32, y: i32) -> bool {
        self.cell(x, y) != SOLID
    }

    fn cell_at(x: f32, y: f32) -> (i32, i32) {
        (
            ((x - MARGIN as f32) / CELL as f32).floor() as i32,
            ((y - MARGIN as f32) / CELL as f32).floor() as i32,
        )
    }

    /// Mark every cell overlapping a px rect as clipped.
    fn clip_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let cx0 = (x0 - MARGIN).div_euclid(CELL);
        let cy0 = (y0 - MARGIN).div_euclid(CELL);
        let cx1 = (x1 - MARGIN - 1).div_euclid(CELL);
        let cy1 = (y1 - MARGIN - 1).div_euclid(CELL);
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                if cx >= 0 && cy >= 0 && (cx as usize) < self.w && (cy as usize) < self.h {
                    self.clipped[cy as usize * self.w + cx as usize] = true;
                }
            }
        }
    }

    fn push_loop(&mut self, pts: &[(i32, i32)]) -> Option<()> {
        let first = self.segs.len();
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            // Cuts from both ends of an edge can meet in the middle.
            if a == b {
                continue;
            }
            self.segs
                .push(Seg { ax: a.0 as i16, ay: a.1 as i16, bx: b.0 as i16, by: b.1 as i16 })
                .ok()?;
        }
        self.loops.push((first as u8, (self.segs.len() - first) as u8)).ok()
    }

    fn build(lay: &Layout, rng: &mut u32, check_gaps: bool) -> Option<Self> {
        let (bx0, by0, bx1, by1) = bbox(lay);
        let mut g = Self::empty();
        g.w = (bx1 - bx0 + 1) as usize;
        g.h = (by1 - by0 + 1) as usize;
        for y in 0..g.h {
            for x in 0..g.w {
                let i = grid_idx(x as i32 + bx0, y as i32 + by0).unwrap();
                g.cells[y * g.w + x] = if lay.open[i] { FAIRWAY } else { SOLID };
            }
        }
        g.tee_cell = (lay.start.0 - bx0, lay.start.1 - by0);
        g.hole_cell = (lay.hole.0 - bx0, lay.hole.1 - by0);
        g.hole_area = lay.pieces.iter().find(|p| p.kind == Kind::Green)
            .map(|p| (p.x0 - bx0, p.y0 - by0, p.x1 - bx0, p.y1 - by0));
        g.tee = cell_center(g.tee_cell);
        g.hole = cell_center(g.hole_cell);
        g.tee_dir = lay.pieces[0].dir;

        g.build_walls(rng)?;
        for p in &lay.pieces {
            if p.kind == Kind::Room {
                g.add_obstacle(p, bx0, by0, rng)?;
            }
        }
        g.add_surfaces(lay, bx0, by0, rng);
        let turns = lay.pieces.windows(2).filter(|w| w[0].dir != w[1].dir).count() as i32;
        g.par = g.estimate_par(turns)?;
        if check_gaps && !g.gaps_ok() {
            return None;
        }
        g.add_grass(rng);
        Some(g)
    }

    /// Trace the open/solid boundary into loops and cut their corners.
    fn build_walls(&mut self, rng: &mut u32) -> Option<()> {
        const VW: usize = GW + 1;
        const NONE: u8 = 255;
        let vw = self.w + 1;
        let vh = self.h + 1;
        let mut out = [NONE; VW * (GH + 1)];
        let mut set = |x: i32, y: i32, d: u8| -> Option<()> {
            let v = y as usize * vw + x as usize;
            if out[v] != NONE {
                return None;
            }
            out[v] = d;
            Some(())
        };
        // Clockwise round each open cell, so the fairway is on the right.
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                if !self.open(x, y) {
                    continue;
                }
                if !self.open(x, y - 1) {
                    set(x, y, 0)?;
                }
                if !self.open(x + 1, y) {
                    set(x + 1, y, 1)?;
                }
                if !self.open(x, y + 1) {
                    set(x + 1, y + 1, 2)?;
                }
                if !self.open(x - 1, y) {
                    set(x, y + 1, 3)?;
                }
            }
        }

        let mut seen = [false; VW * (GH + 1)];
        for start in 0..vw * vh {
            if out[start] == NONE || seen[start] {
                continue;
            }
            let next = |v: usize| {
                let (dx, dy) = DIRS[out[v] as usize];
                ((v / vw) as i32 + dy) as usize * vw + ((v % vw) as i32 + dx) as usize
            };
            // First lap finds the direction the loop arrives at `start` in.
            let mut v = start;
            let mut d_in;
            loop {
                d_in = out[v];
                v = next(v);
                if v == start {
                    break;
                }
            }
            // Second lap keeps the corners: (vx, vy, outgoing dir).
            let mut corners: Vec<(i32, i32, u8), 96> = Vec::new();
            loop {
                seen[v] = true;
                if out[v] != d_in {
                    corners.push(((v % vw) as i32, (v / vw) as i32, out[v])).ok()?;
                }
                d_in = out[v];
                v = next(v);
                if v == start {
                    break;
                }
            }
            self.cut_loop(&corners, rng)?;
        }
        Some(())
    }

    /// Corners by the tee or the hole, or anywhere round a green, only get
    /// small cuts. Big ones would funnel the ball onto the cup.
    fn touches_special(&self, vx: i32, vy: i32) -> bool {
        if let Some((x0, y0, x1, y1)) = self.hole_area {
            if vx >= x0 && vx <= x1 + 1 && vy >= y0 && vy <= y1 + 1 {
                return true;
            }
        }
        [self.tee_cell, self.hole_cell]
            .iter()
            .any(|&(cx, cy)| (cx == vx - 1 || cx == vx) && (cy == vy - 1 || cy == vy))
    }

    fn cut_loop(&mut self, corners: &[(i32, i32, u8)], rng: &mut u32) -> Option<()> {
        let n = corners.len();
        let mut pts: Vec<(i32, i32), 192> = Vec::new();
        let px = |c: (i32, i32, u8)| (c.0 * CELL + MARGIN, c.1 * CELL + MARGIN);
        for i in 0..n {
            let c = corners[i];
            let prev = corners[(i + n - 1) % n];
            let next = corners[(i + 1) % n];
            let din = prev.2;
            let dout = c.2;
            let len_in = ((c.0 - prev.0).abs() + (c.1 - prev.1).abs()) * CELL;
            let len_out = ((next.0 - c.0).abs() + (next.1 - c.1).abs()) * CELL;
            // Right turns are the fairway's outer corners, where the cut adds
            // wall. Left turns are wall tips, where it takes wall away.
            let right = dout == (din + 1) % 4;
            let max_leg = if self.touches_special(c.0, c.1) {
                8
            } else if right {
                32
            } else {
                16
            };
            // Each end of an edge may use half of it.
            let cap_a = (len_in / 2).min(max_leg) / 8;
            let cap_b = (len_out / 2).min(max_leg) / 8;
            let (cx, cy) = px(c);
            if cap_a == 0 || cap_b == 0 || rand_bool(rng, 0.2) {
                pts.push((cx, cy)).ok()?;
                continue;
            }
            let mut a = 8 * rand_range_u32(rng, 1, cap_a as u32) as i32;
            let mut b = 8 * rand_range_u32(rng, 1, cap_b as u32) as i32;
            if rand_bool(rng, 0.35) {
                a = a.min(b);
                b = a;
            }
            // Keep the inside of a one-cell lane's turn roomy: this leaves
            // at least ~9px to the inner corner.
            if right && a + b > 40 {
                if a > b { a = 40 - b } else { b = 40 - a }
            }
            let (ix, iy) = DIRS[din as usize];
            let (ox, oy) = DIRS[dout as usize];
            let p0 = (cx - ix * a, cy - iy * a);
            let p1 = (cx + ox * b, cy + oy * b);
            pts.push(p0).ok()?;
            pts.push(p1).ok()?;
            if right {
                let xs = [cx, p0.0, p1.0];
                let ys = [cy, p0.1, p1.1];
                self.clip_rect(
                    *xs.iter().min().unwrap(),
                    *ys.iter().min().unwrap(),
                    *xs.iter().max().unwrap(),
                    *ys.iter().max().unwrap(),
                );
            }
        }
        self.push_loop(&pts)
    }

    /// A small diamond or square in the middle of a room.
    fn add_obstacle(&mut self, p: &Piece, bx0: i32, by0: i32, rng: &mut u32) -> Option<()> {
        let cx = (p.x0 - bx0) * CELL + (p.x1 - p.x0 + 1) * CELL / 2 + MARGIN;
        let cy = (p.y0 - by0) * CELL + (p.y1 - p.y0 + 1) * CELL / 2 + MARGIN;
        // Points run anticlockwise so the open side is on the right.
        let (rx, ry) = match rand_range_u32(rng, 0, 3) {
            0 => {
                self.push_loop(&[(cx - 4, cy - 4), (cx - 4, cy + 4), (cx + 4, cy + 4), (cx + 4, cy - 4)])?;
                (4, 4)
            }
            1 => {
                self.push_loop(&[(cx, cy - 6), (cx - 6, cy), (cx, cy + 6), (cx + 6, cy)])?;
                (6, 6)
            }
            // Stretched across the room, so it splits the way.
            2 if p.dir % 2 == 0 => {
                self.push_loop(&[(cx, cy - 8), (cx - 4, cy), (cx, cy + 8), (cx + 4, cy)])?;
                (4, 8)
            }
            _ => {
                self.push_loop(&[(cx, cy - 4), (cx - 8, cy), (cx, cy + 4), (cx + 8, cy)])?;
                (8, 4)
            }
        };
        self.clip_rect(cx - rx, cy - ry, cx + rx, cy + ry);
        Some(())
    }

    fn add_surfaces(&mut self, lay: &Layout, bx0: i32, by0: i32, rng: &mut u32) {
        let mut roughs = 0;
        let mut slopes = 0;
        for (i, p) in lay.pieces.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let (px, py) = DIRS[(p.dir as usize + 1) % 4];
            let roll = rand_range_u32(rng, 0, 99);
            if roll < 30 && roughs < 2 && !matches!(p.kind, Kind::Room | Kind::Green) {
                roughs += 1;
                // A rough fork only roughs one side, so there's a choice.
                let side = if rand_bool(rng, 0.5) { 1 } else { -1 };
                for y in p.y0..=p.y1 {
                    for x in p.x0..=p.x1 {
                        if p.kind == Kind::Fork {
                            let (cx, cy) = (x * 2 - p.x0 - p.x1, y * 2 - p.y0 - p.y1);
                            if (cx * px + cy * py).signum() != side {
                                continue;
                            }
                        }
                        self.paint(x - bx0, y - by0, ROUGH);
                    }
                }
            } else if roll < 60 && slopes < 2 && p.kind != Kind::Fork {
                slopes += 1;
                let turn = match rand_range_u32(rng, 0, 9) {
                    0..=3 => 0,
                    4..=5 => 2,
                    6..=7 => 1,
                    _ => 3,
                };
                let d = (p.dir + turn) % 4;
                for y in p.y0..=p.y1 {
                    for x in p.x0..=p.x1 {
                        self.paint(x - bx0, y - by0, SLOPE + d);
                    }
                }
            }
        }
    }

    fn paint(&mut self, x: i32, y: i32, surface: u8) {
        if !self.open(x, y) || self.is_clipped(x, y) || (x, y) == self.tee_cell || (x, y) == self.hole_cell {
            return;
        }
        self.cells[y as usize * self.w + x as usize] = surface;
    }

    /// Shortest grid path from tee to hole, with one stroke per reach of
    /// distance plus a little for lots of corners and rough on the way.
    fn estimate_par(&self, turns: i32) -> Option<u8> {
        let n = self.w * self.h;
        let mut from = [u16::MAX; GCELLS];
        let mut queue = [0u16; GCELLS];
        let (mut qh, mut qt) = (0, 0);
        let start = self.tee_cell.1 as usize * self.w + self.tee_cell.0 as usize;
        let goal = self.hole_cell.1 as usize * self.w + self.hole_cell.0 as usize;
        from[start] = start as u16;
        queue[qt] = start as u16;
        qt += 1;
        while qh < qt {
            let i = queue[qh] as usize;
            qh += 1;
            if i == goal {
                break;
            }
            let (x, y) = ((i % self.w) as i32, (i / self.w) as i32);
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if !self.open(nx, ny) {
                    continue;
                }
                let j = ny as usize * self.w + nx as usize;
                if j < n && from[j] == u16::MAX {
                    from[j] = i as u16;
                    queue[qt] = j as u16;
                    qt += 1;
                }
            }
        }
        if from[goal] == u16::MAX {
            return None;
        }
        let (mut steps, mut rough) = (0, false);
        let mut i = goal;
        while i != start {
            let j = from[i] as usize;
            steps += 1;
            rough |= self.cells[i] == ROUGH;
            i = j;
        }
        let len = steps * CELL;
        let par = 1 + (len + SHOT_REACH - 1) / SHOT_REACH + (turns >= 3) as i32 + rough as i32;
        Some(par.clamp(2, 6) as u8)
    }

    /// Walls `i` and `j` are on the same loop and at most two apart, so
    /// they meet at a corner rather than facing across a gap.
    fn near_on_loop(&self, i: usize, j: usize) -> bool {
        self.loops.iter().any(|&(first, len)| {
            let (first, len) = (first as usize, len as usize);
            let r = first..first + len;
            if !r.contains(&i) || !r.contains(&j) {
                return false;
            }
            let d = i.abs_diff(j);
            d.min(len - d) <= 2
        })
    }

    /// No two walls face each other across the fairway closer than
    /// `MIN_GAP`. Walls don't cross, so the closest approach of two of them
    /// is from one's end to the other.
    fn gaps_ok(&self) -> bool {
        let pad = MIN_GAP as i16;
        for (i, s) in self.segs.iter().enumerate() {
            for (j, t) in self.segs.iter().enumerate().skip(i + 1) {
                if s.ax.max(s.bx) + pad < t.ax.min(t.bx) || t.ax.max(t.bx) + pad < s.ax.min(s.bx)
                    || s.ay.max(s.by) + pad < t.ay.min(t.by) || t.ay.max(t.by) + pad < s.ay.min(s.by)
                    || self.near_on_loop(i, j)
                {
                    continue;
                }
                let ends = [
                    ((s.ax as f32, s.ay as f32), t),
                    ((s.bx as f32, s.by as f32), t),
                    ((t.ax as f32, t.ay as f32), s),
                    ((t.bx as f32, t.by as f32), s),
                ];
                for ((x, y), other) in ends {
                    let (cx, cy) = closest_on_seg(other, x, y);
                    let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                    if d2 >= MIN_GAP * MIN_GAP {
                        continue;
                    }
                    // Only a squeeze if it's fairway between them, not the
                    // solid inside of an obstacle or a wall.
                    if d2 < 0.25 || self.inside((x + cx) / 2.0, (y + cy) / 2.0) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Scatter grass off the green, clear of the walls so they stay crisp.
    fn add_grass(&mut self, rng: &mut u32) {
        for by in (0..self.world_h()).step_by(GRASS_PITCH as usize) {
            for bx in (0..self.world_w()).step_by(GRASS_PITCH as usize) {
                if !rand_bool(rng, 0.45) {
                    continue;
                }
                let x = bx + rand_range_u32(rng, 0, GRASS_PITCH as u32 - 1) as i32;
                let y = by + rand_range_u32(rng, 0, GRASS_PITCH as u32 - 1) as i32;
                let (fx, fy) = (x as f32, y as f32);
                if self.inside(fx, fy) || self.segs.iter().any(|s| seg_dist2(s, fx, fy) < 9.0) {
                    continue;
                }
                if self.grass.push((x as i16, y as i16)).is_err() {
                    return;
                }
            }
        }
    }

    /// Even-odd test against every wall loop: on the fairway or not.
    fn inside(&self, x: f32, y: f32) -> bool {
        let mut inside = false;
        for s in &self.segs {
            let (ax, ay, bx, by) = (s.ax as f32, s.ay as f32, s.bx as f32, s.by as f32);
            if (ay > y) != (by > y) && x < ax + (y - ay) * (bx - ax) / (by - ay) {
                inside = !inside;
            }
        }
        inside
    }

    fn collide(&self, b: &mut Ball) {
        for s in &self.segs {
            let (ax, ay, bx, by) = (s.ax as f32, s.ay as f32, s.bx as f32, s.by as f32);
            if b.x + BALL_R < ax.min(bx) || b.x - BALL_R > ax.max(bx)
                || b.y + BALL_R < ay.min(by) || b.y - BALL_R > ay.max(by)
            {
                continue;
            }
            let (ex, ey) = (bx - ax, by - ay);
            let len2 = ex * ex + ey * ey;
            let (cx, cy) = closest_on_seg(s, b.x, b.y);
            let (dx, dy) = (b.x - cx, b.y - cy);
            let d2 = dx * dx + dy * dy;
            if d2 >= BALL_R * BALL_R {
                continue;
            }
            let d = d2.sqrt();
            let (nx, ny) = if d > 1e-4 {
                (dx / d, dy / d)
            } else {
                let l = len2.sqrt();
                (-ey / l, ex / l)
            };
            b.x = cx + nx * BALL_R;
            b.y = cy + ny * BALL_R;
            let vn = b.vx * nx + b.vy * ny;
            if vn < 0.0 {
                b.vx -= (1.0 + RESTITUTION) * vn * nx;
                b.vy -= (1.0 + RESTITUTION) * vn * ny;
            }
        }
    }
}

fn closest_on_seg(s: &Seg, x: f32, y: f32) -> (f32, f32) {
    let (ax, ay) = (s.ax as f32, s.ay as f32);
    let (ex, ey) = (s.bx as f32 - ax, s.by as f32 - ay);
    let t = (((x - ax) * ex + (y - ay) * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
    (ax + ex * t, ay + ey * t)
}

fn seg_dist2(s: &Seg, x: f32, y: f32) -> f32 {
    let (cx, cy) = closest_on_seg(s, x, y);
    (x - cx) * (x - cx) + (y - cy) * (y - cy)
}

// ---------------------------------------------------------------------------
// Ball physics.

#[derive(Clone, Copy, Debug)]
struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Roll {
    Moving,
    Stopped,
    Sunk,
}

fn roll(g: &Green, b: &mut Ball, still: &mut f32, dt: f32) -> Roll {
    let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
    let n = ((speed * dt / MAX_STEP).ceil() as u32).clamp(1, 64);
    let h = dt / n as f32;
    for _ in 0..n {
        let (cx, cy) = Green::cell_at(b.x, b.y);
        let surface = g.cell(cx, cy);
        if surface >= SLOPE {
            let (sx, sy) = DIRS[(surface - SLOPE) as usize];
            b.vx += sx as f32 * SLOPE_ACCEL * h;
            b.vy += sy as f32 * SLOPE_ACCEL * h;
        }
        let friction = if surface == ROUGH { ROUGH_FRICTION } else { FRICTION };
        let s = (b.vx * b.vx + b.vy * b.vy).sqrt();
        if s > 0.0 {
            let k = (s - friction * h).max(0.0) / s;
            b.vx *= k;
            b.vy *= k;
        }

        let (hx, hy) = (g.hole.0 - b.x, g.hole.1 - b.y);
        let hd = (hx * hx + hy * hy).sqrt();
        if hd < HOLE_R {
            if s < V_CAPTURE {
                b.x = g.hole.0;
                b.y = g.hole.1;
                b.vx = 0.0;
                b.vy = 0.0;
                return Roll::Sunk;
            }
            if hd > 1e-3 {
                b.vx += hx / hd * HOLE_PULL * h;
                b.vy += hy / hd * HOLE_PULL * h;
            }
        }

        b.x += b.vx * h;
        b.y += b.vy * h;
        g.collide(b);
    }

    let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
    if speed < STOP_SPEED {
        *still += dt;
        if *still >= STOP_TIME {
            b.vx = 0.0;
            b.vy = 0.0;
            return Roll::Stopped;
        }
    } else {
        *still = 0.0;
    }
    Roll::Moving
}

/// Gauge reading, 0..1, swinging smoothly up and down.
fn gauge(phase: f32) -> f32 {
    (1.0 - phase.cos()) / 2.0
}

// ---------------------------------------------------------------------------

pub struct MiniGolfScene {
    green: Green,
    hole_no: usize,
    strokes: u8,
    card: [u8; HOLES],
    pars: [u8; HOLES],

    state: State,
    timer: f32,
    ball: Ball,
    shot_from: (f32, f32),
    aim: f32,
    aim_hold: f32,
    phase: f32,
    still: f32,
    cam_x: f32,
    cam_y: f32,

    popup: Popup,
    /// Asks before leaving the scorecard, so a quick A doesn't skip it.
    confirm: Confirm,

    session_holes: u32,
    session_bonus: i32,

    options_menu: Menu<MiniGolfAction>,
    menu_active: bool,
}

impl MiniGolfScene {
    pub fn new() -> Self {
        Self {
            green: Green::empty(),
            hole_no: 0,
            strokes: 0,
            card: [0; HOLES],
            pars: [0; HOLES],

            state: State::Splash,
            timer: 0.0,
            ball: Ball { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0 },
            shot_from: (0.0, 0.0),
            aim: 0.0,
            aim_hold: 0.0,
            phase: 0.0,
            still: 0.0,
            cam_x: 0.0,
            cam_y: 0.0,

            popup: Popup::new(24, 16, 80, 30),
            confirm: Confirm::new(),

            session_holes: 0,
            session_bonus: 0,

            options_menu: Menu::new(OPTIONS_ITEMS),
            menu_active: false,
        }
    }

    fn start_round(&mut self, ctx: &mut GameContext) {
        self.confirm.close();
        self.hole_no = 0;
        self.card = [0; HOLES];
        self.pars = [0; HOLES];
        self.start_hole(ctx);
    }

    fn start_hole(&mut self, ctx: &mut GameContext) {
        let mut rng = ctx.rng;
        self.green = Green::generate(&mut rng);
        ctx.rng = rng;
        self.pars[self.hole_no] = self.green.par;
        self.strokes = 0;
        let (x, y) = self.green.tee;
        self.ball = Ball { x, y, vx: 0.0, vy: 0.0 };
        self.aim = self.green.tee_dir as f32 * PI / 2.0;
        self.state = State::Splash;
        self.timer = 0.0;
        let mut text: String<32> = String::new();
        let _ = write!(text, "{} {}\n{} {}", t!("Hole"), self.hole_no + 1, t!("Par"), self.green.par);
        self.popup.set_text(&text, false, true);
        self.update_camera(true);
    }

    fn finish_hole(&mut self, strokes: u8, text: &str) {
        self.card[self.hole_no] = strokes;
        self.session_holes += 1;
        self.state = State::Sunk;
        self.timer = 0.0;
        self.popup.set_text(text, false, true);
    }

    fn sunk_text(&self) -> &'static str {
        let rel = self.strokes as i32 - self.green.par as i32;
        match (self.strokes, rel) {
            (1, _) => t!("Hole in one!"),
            (_, i32::MIN..=-2) => t!("Eagle!"),
            (_, -1) => t!("Birdie!"),
            (_, 0) => t!("Par"),
            (_, 1) => t!("Bogey"),
            _ => t!("Double bogey"),
        }
    }

    fn round_rel(&self) -> i32 {
        (0..HOLES)
            .filter(|&i| self.card[i] > 0)
            .map(|i| self.card[i] as i32 - self.pars[i] as i32)
            .sum()
    }

    /// Ease the camera towards the midpoint of ball and hole, as far as the
    /// ball can stay in the dead zone, so there's a view of what's ahead and
    /// the camera doesn't jump once the ball rolls. The tee shot keeps the
    /// default camera.
    fn pan_to_hole(&mut self, dt: f32) {
        if self.strokes == 0 {
            return;
        }
        let fit = |b: f32, h: f32, screen: i32, margin: f32, world: i32| {
            let want = (b + h) / 2.0 - screen as f32 / 2.0;
            clamp_cam(want.clamp(b - screen as f32 + margin, b - margin), world, screen)
        };
        let g = &self.green;
        let tx = fit(self.ball.x, g.hole.0, SCREEN_W, CAM_LEFT, g.world_w());
        let ty = fit(self.ball.y, g.hole.1, SCREEN_H, CAM_TOP, g.world_h());
        let k = (CAM_PAN * dt).min(1.0);
        self.cam_x += (tx - self.cam_x) * k;
        self.cam_y += (ty - self.cam_y) * k;
    }

    fn update_camera(&mut self, snap: bool) {
        let (bx, by) = (self.ball.x, self.ball.y);
        if snap {
            self.cam_x = bx - SCREEN_W as f32 / 2.0;
            self.cam_y = by - SCREEN_H as f32 / 2.0;
        } else {
            let (sx, sy) = (bx - self.cam_x, by - self.cam_y);
            if sx < CAM_LEFT {
                self.cam_x = bx - CAM_LEFT;
            } else if sx > CAM_RIGHT {
                self.cam_x = bx - CAM_RIGHT;
            }
            if sy < CAM_TOP {
                self.cam_y = by - CAM_TOP;
            } else if sy > CAM_BOTTOM {
                self.cam_y = by - CAM_BOTTOM;
            }
        }
        self.cam_x = clamp_cam(self.cam_x, self.green.world_w(), SCREEN_W);
        self.cam_y = clamp_cam(self.cam_y, self.green.world_h(), SCREEN_H);
    }

    fn handle_menu(&mut self, ctx: &mut GameContext, buttons: &mut Buttons) {
        match self.options_menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => self.menu_active = false,
            MenuResult::Action(action) => {
                self.menu_active = false;
                match action {
                    MiniGolfAction::NewRound => self.start_round(ctx),
                    MiniGolfAction::NewGreen => self.start_hole(ctx),
                }
            }
        }
    }

    fn update_aim(&mut self, buttons: &mut Buttons, dt: f32) {
        let left = buttons.is_pressed(Button::Left);
        let right = buttons.is_pressed(Button::Right);
        if buttons.was_just_pressed(Button::Left) {
            self.aim -= AIM_TAP;
            self.aim_hold = 0.0;
        } else if buttons.was_just_pressed(Button::Right) {
            self.aim += AIM_TAP;
            self.aim_hold = 0.0;
        } else if left || right {
            self.aim_hold += dt;
            if self.aim_hold > AIM_HOLD_DELAY {
                self.aim += if left { -AIM_RATE * dt } else { AIM_RATE * dt };
            }
        }
        self.aim = self.aim.rem_euclid(2.0 * PI);
    }

    fn putt(&mut self) {
        let v = V_MIN + gauge(self.phase) * (V_MAX - V_MIN);
        self.ball.vx = self.aim.cos() * v;
        self.ball.vy = self.aim.sin() * v;
        self.shot_from = (self.ball.x, self.ball.y);
        self.strokes += 1;
        self.still = 0.0;
        self.state = State::Rolling;
    }

    fn after_roll(&mut self, result: Roll) {
        match result {
            Roll::Moving => {}
            Roll::Sunk => {
                let text = self.sunk_text();
                self.finish_hole(self.strokes, text);
            }
            Roll::Stopped => {
                if !self.green.inside(self.ball.x, self.ball.y) {
                    // Escaped through a crack somehow: back to where it was hit.
                    self.ball.x = self.shot_from.0;
                    self.ball.y = self.shot_from.1;
                }
                let limit = self.green.par + MAX_OVER;
                if self.strokes >= limit {
                    self.finish_hole(limit, t!("Picked up"));
                } else {
                    self.state = State::Aim;
                }
            }
        }
    }

    fn next_hole(&mut self, ctx: &mut GameContext) {
        if self.hole_no + 1 < HOLES {
            self.hole_no += 1;
            self.start_hole(ctx);
        } else {
            let rel = self.round_rel();
            self.session_bonus += if rel <= 0 { 3 } else if rel <= HOLES as i32 { 1 } else { 0 };
            self.state = State::Scorecard;
        }
    }

    // --- drawing -----------------------------------------------------------

    fn cam(&self) -> (i32, i32) {
        (self.cam_x as i32, self.cam_y as i32)
    }

    fn draw_surfaces(&self, r: &mut Renderer) {
        let g = &self.green;
        let (camx, camy) = self.cam();
        let cx0 = (camx - MARGIN).div_euclid(CELL);
        let cy0 = (camy - MARGIN).div_euclid(CELL);
        let cx1 = (camx + SCREEN_W - MARGIN).div_euclid(CELL);
        let cy1 = (camy + SCREEN_H - MARGIN).div_euclid(CELL);
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let s = g.cell(cx, cy);
                let ox = cx * CELL + MARGIN - camx;
                let oy = cy * CELL + MARGIN - camy;
                if s == ROUGH {
                    // Leave a gap along walls so the checker doesn't merge in.
                    let x0 = if g.open(cx - 1, cy) { 0 } else { 2 };
                    let x1 = if g.open(cx + 1, cy) { CELL } else { CELL - 2 };
                    let y0 = if g.open(cx, cy - 1) { 0 } else { 2 };
                    let y1 = if g.open(cx, cy + 1) { CELL } else { CELL - 2 };
                    for y in y0..y1 {
                        for x in x0..x1 {
                            if (ox + camx + x + oy + camy + y) % 2 == 0 {
                                r.draw_pixel(Point::new(ox + x, oy + y), true);
                            }
                        }
                    }
                } else if s >= SLOPE {
                    self.draw_slope(r, cx, cy, ox, oy, s - SLOPE);
                }
            }
        }
    }

    /// Chevrons near both lane walls pointing downhill, and a dotted line
    /// where the slope meets flat ground.
    fn draw_slope(&self, r: &mut Renderer, cx: i32, cy: i32, ox: i32, oy: i32, d: u8) {
        let g = &self.green;
        let s = g.cell(cx, cy);
        let (dx, dy) = DIRS[d as usize];
        let (px, py) = (-dy, dx);
        let (mx, my) = (ox + CELL / 2, oy + CELL / 2);
        // A quarter cell either side of the middle, so neighbouring cells
        // continue the same even 8px spacing.
        for side in [-CELL / 4, CELL / 4] {
            let (bx, by) = (mx + px * side - dx, my + py * side - dy);
            for (a, l) in [(0, -2), (1, -1), (2, 0), (1, 1), (0, 2)] {
                r.draw_pixel(Point::new(bx + dx * a + px * l, by + dy * a + py * l), true);
            }
        }
        for (i, (nx, ny)) in DIRS.iter().enumerate() {
            // Solid neighbours too: a corner cut can open them up, and where
            // a wall does run along the edge the dots sit under it.
            if g.cell(cx + nx, cy + ny) == s {
                continue;
            }
            for k in (1..CELL).step_by(2) {
                let p = match i {
                    0 => Point::new(ox + CELL, oy + k),
                    1 => Point::new(ox + k, oy + CELL),
                    2 => Point::new(ox, oy + k),
                    _ => Point::new(ox + k, oy),
                };
                r.draw_pixel(p, true);
            }
        }
    }

    fn draw_course(&self, r: &mut Renderer) {
        let g = &self.green;
        let (camx, camy) = self.cam();
        for &(x, y) in &g.grass {
            let p = Point::new(x as i32 - camx, y as i32 - camy);
            if p.x >= 0 && p.y >= 0 && p.x < SCREEN_W && p.y < SCREEN_H {
                r.draw_pixel(p, true);
            }
        }
        self.draw_surfaces(r);
        for s in &g.segs {
            let a = Point::new(s.ax as i32 - camx, s.ay as i32 - camy);
            let b = Point::new(s.bx as i32 - camx, s.by as i32 - camy);
            r.draw_line(a, b);
            // Thicken towards the wall side (left of travel), keeping the
            // fairway its full width.
            let (ex, ey) = (b.x - a.x, b.y - a.y);
            let off = if ey.abs() >= ex.abs() {
                Point::new(ey.signum(), 0)
            } else {
                Point::new(0, -ex.signum())
            };
            r.draw_line(a + off, b + off);
        }

        // Tee markers either side of the tee.
        let (tx, ty) = (g.tee.0 as i32 - camx, g.tee.1 as i32 - camy);
        let (dx, dy) = DIRS[g.tee_dir as usize];
        for side in [-5, 5] {
            r.draw_rect(Point::new(tx - dy * side - 1, ty + dx * side - 1), Size::new(2, 2), true);
        }

        // Cup and flag.
        let (hx, hy) = (g.hole.0 as i32 - camx, g.hole.1 as i32 - camy);
        r.fill_rect_off(Point::new(hx - 3, hy - 3), Size::new(7, 7));
        r.draw_circle_outline(Point::new(hx, hy), 3);
        r.draw_line(Point::new(hx, hy), Point::new(hx, hy - 11));
        for (row, w) in [(11, 4), (10, 3), (9, 2), (8, 1)] {
            r.draw_line(Point::new(hx + 1, hy - row), Point::new(hx + w, hy - row));
        }

        if self.state != State::Sunk {
            let (bx, by) = self.ball_screen();
            // Clear a disc one pixel wider than the ball so it reads on rough.
            for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    if dx * dx + dy * dy <= 10 {
                        r.draw_pixel(Point::new(bx + dx, by + dy), false);
                    }
                }
            }
            r.draw_circle_filled(Point::new(bx, by), 2);
            if matches!(self.state, State::Aim | State::Power) {
                r.draw_circle_outline(Point::new(bx, by), AIM_RING);
            }
        }
    }

    fn ball_screen(&self) -> (i32, i32) {
        let (camx, camy) = self.cam();
        (self.ball.x as i32 - camx, self.ball.y as i32 - camy)
    }

    fn draw_aim(&self, r: &mut Renderer) {
        let (bx, by) = (self.ball.x - self.cam_x, self.ball.y - self.cam_y);
        let (c, s) = (self.aim.cos(), self.aim.sin());
        let from = (AIM_RING + 2) as f32;
        let to = from + AIM_LEN;
        r.draw_line(
            Point::new((bx + c * from + 0.5).floor() as i32, (by + s * from + 0.5).floor() as i32),
            Point::new((bx + c * to + 0.5).floor() as i32, (by + s * to + 0.5).floor() as i32),
        );
    }

    fn draw_power(&self, r: &mut Renderer) {
        const W: i32 = 5;
        const H: i32 = 23;
        let (bx, by) = self.ball_screen();
        let x = if bx - AIM_RING - 3 - W >= 0 { bx - AIM_RING - 3 - W } else { bx + AIM_RING + 3 };
        let y = (by - H / 2).clamp(0, SCREEN_H - H);
        r.fill_rect_off(Point::new(x - 1, y - 1), Size::new(W as u32 + 2, H as u32 + 2));
        r.draw_rect(Point::new(x, y), Size::new(W as u32, H as u32), false);
        let fill = (gauge(self.phase) * (H - 2) as f32 + 0.5) as i32;
        if fill > 0 {
            r.draw_rect(Point::new(x + 1, y + H - 1 - fill), Size::new(W as u32 - 2, fill as u32), true);
        }
    }

    fn draw_hud(&self, r: &mut Renderer) {
        let mut text: String<12> = String::new();
        let _ = write!(text, "S{}/P{}", self.strokes, self.green.par);
        let w = text.len() as i32 * 4 + 1;
        let (bx, by) = self.ball_screen();
        let mut x = SCREEN_W - w;
        if by < 12 && bx > x - 6 {
            x = 0;
        }
        r.draw_rect(Point::new(x, 0), Size::new(w as u32, 7), true);
        r.draw_text_small_inverted(&text, Point::new(x + 1, 1));
    }

    fn draw_scorecard(&self, r: &mut Renderer) {
        r.draw_text(t!("Round over"), Point::new(2, 1));
        let rel = self.round_rel();
        let mut buf: String<8> = String::new();
        if rel == 0 {
            let _ = write!(buf, "E");
        } else {
            let _ = write!(buf, "{:+}", rel);
        }
        r.draw_text(&buf, Point::new(SCREEN_W - 2 - buf.len() as i32 * 6, 1));

        const X0: i32 = 22;
        const COL: i32 = 10;
        const TOT_X: i32 = X0 + COL * HOLES as i32 + 2;
        let rows = [(16, t!("Hole")), (28, t!("Par")), (40, "")];
        for (y, label) in rows {
            r.draw_text_small(label, Point::new(1, y));
        }
        r.draw_line(Point::new(0, 24), Point::new(SCREEN_W - 1, 24));
        r.draw_line(Point::new(0, 36), Point::new(SCREEN_W - 1, 36));
        r.draw_line(Point::new(X0 - 2, 14), Point::new(X0 - 2, 48));
        r.draw_line(Point::new(TOT_X - 2, 14), Point::new(TOT_X - 2, 48));
        // The player's row is marked with a ball.
        r.draw_rect(Point::new(8, 41), Size::new(3, 3), true);

        let (mut par_sum, mut card_sum) = (0, 0);
        for i in 0..HOLES {
            let x = X0 + COL * i as i32 + 2;
            buf.clear();
            let _ = write!(buf, "{}", i + 1);
            r.draw_text_small(&buf, Point::new(x, 16));
            buf.clear();
            let _ = write!(buf, "{}", self.pars[i]);
            r.draw_text_small(&buf, Point::new(x, 28));
            par_sum += self.pars[i] as i32;
            card_sum += self.card[i] as i32;
            buf.clear();
            let _ = write!(buf, "{}", self.card[i]);
            if self.card[i] < self.pars[i] {
                // Under par gets inverted.
                r.draw_rect(Point::new(x - 1, 39), Size::new(6, 8), true);
                r.draw_text_small_inverted(&buf, Point::new(x, 40));
            } else {
                r.draw_text_small(&buf, Point::new(x, 40));
            }
        }
        for (y, v) in [(28, par_sum), (40, card_sum)] {
            buf.clear();
            let _ = write!(buf, "{}", v);
            r.draw_text_small(&buf, Point::new(TOT_X, y));
        }
    }
}

fn clamp_cam(cam: f32, world: i32, screen: i32) -> f32 {
    if world <= screen {
        ((world - screen) / 2) as f32
    } else {
        cam.clamp(0.0, (world - screen) as f32)
    }
}

impl Scene for MiniGolfScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.session_holes = 0;
        self.session_bonus = 0;
        self.menu_active = false;
        self.start_round(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if self.session_holes == 0 {
            return;
        }
        let progress = (self.session_holes as f32 / HOLES as f32).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Focus,       5.0 * progress),
            (StatId::Playfulness, 3.0 * progress),
            (StatId::Serenity,    1.0 * progress),
        ]);
        let coins = (3.0 * progress) as i32 + self.session_bonus;
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
            self.handle_menu(ctx, buttons);
            return None;
        }

        if buttons.was_just_pressed(Button::Menu2) {
            self.menu_active = true;
            self.options_menu.reset_to(OPTIONS_ITEMS);
            return None;
        }

        match self.state {
            State::Splash => {
                self.timer += dt;
                if buttons.was_just_pressed(Button::A) || self.timer >= SPLASH_TIME {
                    self.state = State::Aim;
                }
            }
            State::Aim => {
                self.update_aim(buttons, dt);
                self.pan_to_hole(dt);
                if buttons.was_just_pressed(Button::A) {
                    self.phase = 0.0;
                    self.state = State::Power;
                }
            }
            State::Power => {
                self.phase = (self.phase + dt * 2.0 * PI / POWER_PERIOD) % (2.0 * PI);
                self.pan_to_hole(dt);
                if buttons.was_just_pressed(Button::A) {
                    self.putt();
                } else if buttons.was_just_pressed(Button::B) {
                    self.state = State::Aim;
                }
            }
            State::Rolling => {
                let result = roll(&self.green, &mut self.ball, &mut self.still, dt);
                self.update_camera(false);
                self.after_roll(result);
            }
            State::Sunk => {
                self.timer += dt;
                if buttons.was_just_pressed(Button::A) || self.timer >= SUNK_TIME {
                    self.next_hole(ctx);
                }
            }
            State::Scorecard => {
                if self.confirm.is_open() {
                    if self.confirm.handle_input(buttons) == ConfirmResult::Confirmed {
                        self.start_round(ctx);
                    }
                } else if buttons.was_just_pressed(Button::A) {
                    self.confirm.open(t!("New round?"));
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
        if self.state == State::Scorecard {
            self.draw_scorecard(renderer);
            if self.confirm.is_open() {
                self.confirm.draw(renderer);
            }
            return;
        }
        self.draw_course(renderer);
        match self.state {
            State::Aim => {
                self.draw_aim(renderer);
                self.draw_hud(renderer);
            }
            State::Power => {
                self.draw_aim(renderer);
                self.draw_power(renderer);
                self.draw_hud(renderer);
            }
            State::Splash | State::Sunk => self.popup.draw(renderer, false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greens_are_playable() {
        for seed in 1..200u32 {
            let mut rng = seed.wrapping_mul(2654435761);
            let g = Green::generate(&mut rng);
            assert!(g.open(g.tee_cell.0, g.tee_cell.1), "seed {}", seed);
            assert!(g.open(g.hole_cell.0, g.hole_cell.1), "seed {}", seed);
            assert!((2..=6).contains(&g.par), "seed {} par {}", seed, g.par);
            assert!(g.inside(g.tee.0, g.tee.1), "seed {}", seed);
            assert!(g.inside(g.hole.0, g.hole.1), "seed {}", seed);
            assert!(g.world_w() > SCREEN_W || g.world_h() > SCREEN_H, "seed {}", seed);
        }
    }

    #[test]
    fn full_power_putts_stay_on_the_green() {
        for seed in 1..30u32 {
            let mut rng = seed.wrapping_mul(2654435761);
            let g = Green::generate(&mut rng);
            for k in 0..12 {
                let a = k as f32 * PI / 6.0 + 0.1;
                let mut b = Ball { x: g.tee.0, y: g.tee.1, vx: a.cos() * V_MAX, vy: a.sin() * V_MAX };
                let mut still = 0.0;
                for _ in 0..240 {
                    let r = roll(&g, &mut b, &mut still, 1.0 / 12.0);
                    assert!(g.inside(b.x, b.y), "seed {} dir {} at {:?}", seed, k, b);
                    if r != Roll::Moving {
                        break;
                    }
                }
            }
        }
    }
}
