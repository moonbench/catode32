//! Magic 8 Ball tool. Ask a question, shake (A or the d-pad), and the
//! answer floats up in the ball's window. After a few seconds the view
//! pulls back to the cat reacting to the verdict; B flips between the two.
//!
//! Two views (128x64):
//!   ball    the ball (x=10..70) with the cat beside it
//!   answer  close up through the window: a dark ellipse filling the
//!           screen, the answer in the 6x10 font

use embedded_graphics::prelude::{Point, Size};
use heapless::Vec;

use crate::t;
use crate::{
    assets::character::PoseId,
    context::GameContext,
    entities::character::Character,
    input::{Button, Buttons},
    rand::rand_range_u32,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::{tiny_font, wrap},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Yes,
    Maybe,
    No,
}

impl Mood {
    fn pose(self) -> PoseId {
        match self {
            Mood::Yes => PoseId::SittingSideHappy,
            Mood::Maybe => PoseId::SittingSideAloof,
            Mood::No => PoseId::SittingSideAnnoyed,
        }
    }
}

const ANSWERS: &[(&str, Mood)] = &[
    (t!("Purr-sitively!"), Mood::Yes),
    (t!("The whiskers say yes"), Mood::Yes),
    (t!("Paws up!"), Mood::Yes),
    (t!("Yes, with treats"), Mood::Yes),
    (t!("Fur sure"), Mood::Yes),
    (t!("Tail held high: yes"), Mood::Yes),
    (t!("Go for it"), Mood::Yes),
    (t!("Clawsome idea"), Mood::Yes),
    (t!("The stars and the cat agree"), Mood::Yes),
    (t!("Meow means yes"), Mood::Yes),
    (t!("Ask after my nap"), Mood::Maybe),
    (t!("Purr-haps"), Mood::Maybe),
    (t!("Too sleepy to say"), Mood::Maybe),
    (t!("Shake me again"), Mood::Maybe),
    (t!("The yarn is tangled"), Mood::Maybe),
    (t!("Hiss. No."), Mood::No),
    (t!("Not a chance"), Mood::No),
    (t!("The tail says no"), Mood::No),
    (t!("Knock it off the table"), Mood::No),
    (t!("Cat-egorically not"), Mood::No),
];

const BALL_CX: i32 = 40;
const BALL_CY: i32 = 32;
const BALL_R: i32 = 30;
const FACE_R: i32 = 12;
const CAT_X: i32 = 104;
/// Centre of the prompt above the cat.
const PROMPT_CX: i32 = 100;
const PROMPT_CHARS: usize = 9;

/// The window, as an ellipse centred on the screen.
const WIN_A: i32 = 61;
const WIN_B: i32 = 31;
const LINE_H: i32 = 10;
const CHAR_W: i32 = 6;
/// Line widths that stay inside the ellipse: two wide lines near the
/// middle, or up to four narrower ones.
const WIDE_CHARS: usize = 18;
const NARROW_CHARS: usize = 14;

/// Seconds of shaking per press, and the most it can build up to.
const SHAKE_SECS: f32 = 0.9;
const SHAKE_MAX: f32 = 1.6;
/// Seconds for an answer to fade in.
const REVEAL_SECS: f32 = 0.7;
/// Seconds the close-up stays before pulling back to the cat.
const CLOSE_UP_SECS: f32 = 4.0;

/// 4x4 ordered-dither thresholds, 0..16.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    Shaking { left: f32 },
    /// `close_up` picks the view: the window, or the ball and the cat
    /// reacting.
    Answer { idx: usize, t: f32, close_up: bool },
}

pub struct EightBallScene {
    state: State,
    jitter: (i32, i32),
    character: Character,
}

/// Lay an answer out for the window: two wide lines if it fits, else up
/// to four narrower ones.
fn layout(text: &str) -> Vec<&str, 4> {
    let wide: Vec<&str, 4> = wrap::wrap(text, WIDE_CHARS);
    if wide.len() <= 2 {
        wide
    } else {
        wrap::wrap(text, NARROW_CHARS)
    }
}

fn inside_window(x: i32, y: i32) -> bool {
    let (dx, dy) = ((x - 64) as i64, (y - 32) as i64);
    let (a, b) = (WIN_A as i64, WIN_B as i64);
    // Strict, so the ellipse has no one-pixel nub at its tips.
    dx * dx * b * b + dy * dy * a * a < a * a * b * b
}

impl EightBallScene {
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            jitter: (0, 0),
            character: Character::new(Point::new(CAT_X, 63)),
        }
    }

    fn draw_ball(&self, r: &mut Renderer) {
        let (cx, cy) = (BALL_CX + self.jitter.0, BALL_CY + self.jitter.1);
        r.draw_circle_filled(Point::new(cx, cy), BALL_R);
        // A dark glint to give it some roundness.
        for k in 0..3 {
            r.draw_pixel(Point::new(cx - 20 + k, cy - 18 - k), false);
            r.draw_pixel(Point::new(cx - 21 + k, cy - 17 - k), false);
        }
        // White "8" disc in a dark ring.
        clear_disc(r, cx, cy, FACE_R + 2);
        r.draw_circle_filled(Point::new(cx, cy), FACE_R);
        tiny_font::draw_char_scaled(r, '8', cx - 4, cy - 5, 2, false);
    }

    fn draw_prompt(&self, r: &mut Renderer) {
        let text = t!("Ask, then press A");
        let lines: Vec<&str, 3> = wrap::wrap(text, PROMPT_CHARS);
        if lines.len() <= 2 {
            wrap::draw_centered(r, &lines, PROMPT_CX, 0, LINE_H);
        } else {
            // A long translation: fall back to the small font.
            let lines: Vec<&str, 3> = wrap::wrap(text, 13);
            wrap::draw_centered_small(r, &lines, PROMPT_CX, 1, 7);
        }
    }

    fn draw_answer(&self, r: &mut Renderer, idx: usize, t: f32) {
        // Lit ball all round, dark window in the middle.
        r.draw_rect(Point::new(0, 0), Size::new(128, 64), true);
        for y in (32 - WIN_B)..=(32 + WIN_B) {
            for x in (64 - WIN_A)..=(64 + WIN_A) {
                if inside_window(x, y) {
                    r.draw_pixel(Point::new(x, y), false);
                }
            }
        }

        let lines = layout(ANSWERS[idx].0);
        // Glyphs sit on rows 1..8 of the 10px cell; nudge up a pixel so
        // the block is centred on the window.
        let top = 32 - lines.len() as i32 * LINE_H / 2 - 1;
        for (i, line) in lines.iter().enumerate() {
            let w = line.chars().count() as i32 * CHAR_W;
            r.draw_text(line, Point::new(64 - w / 2, top + i as i32 * LINE_H));
        }

        // Fade in: hide each pixel until its dither threshold is passed.
        let level = ((t / REVEAL_SECS) * 16.0) as u8;
        if level < 16 {
            for y in (32 - WIN_B)..=(32 + WIN_B) {
                for x in (64 - WIN_A)..=(64 + WIN_A) {
                    if BAYER[(y & 3) as usize][(x & 3) as usize] >= level && inside_window(x, y) {
                        r.draw_pixel(Point::new(x, y), false);
                    }
                }
            }
        }
    }
}

impl Scene for EightBallScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.state = State::Idle;
        self.jitter = (0, 0);
        self.character.set_pose(PoseId::SittingSideNeutral);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.character.animate(dt);

        let shake = [Button::A, Button::Up, Button::Down, Button::Left, Button::Right]
            .into_iter()
            .any(|b| buttons.was_just_pressed(b));
        if shake {
            let left = match self.state {
                State::Shaking { left } => (left + SHAKE_SECS).min(SHAKE_MAX),
                _ => SHAKE_SECS,
            };
            self.state = State::Shaking { left };
            self.character.set_pose(PoseId::SittingSideLookingDown);
        }

        match self.state {
            State::Idle => {}
            State::Shaking { left } => {
                let left = left - dt;
                if left <= 0.0 {
                    let idx = rand_range_u32(&mut ctx.rng, 0, ANSWERS.len() as u32 - 1) as usize;
                    self.state = State::Answer { idx, t: 0.0, close_up: true };
                    self.jitter = (0, 0);
                    self.character.set_pose(ANSWERS[idx].1.pose());
                } else {
                    self.state = State::Shaking { left };
                    // Bigger wobble while the shake is fresh.
                    let amp = if left > 0.4 { 3 } else { 1 };
                    let span = (amp * 2) as u32;
                    self.jitter = (
                        rand_range_u32(&mut ctx.rng, 0, span) as i32 - amp,
                        rand_range_u32(&mut ctx.rng, 0, span) as i32 - amp,
                    );
                }
            }
            State::Answer { idx, t, close_up } => {
                let t2 = t + dt;
                // Pull back once, after the answer has had time to be read.
                let pull_back = t < CLOSE_UP_SECS && t2 >= CLOSE_UP_SECS;
                let flip = buttons.was_just_pressed(Button::B);
                let close_up = if pull_back { false } else { close_up != flip };
                self.state = State::Answer { idx, t: t2, close_up };
            }
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        match self.state {
            State::Answer { idx, t, close_up: true } => self.draw_answer(r, idx, t),
            State::Answer { close_up: false, .. } => {
                self.draw_ball(r);
                self.draw_prompt(r);
                self.character.draw(r, 0);
            }
            State::Idle => {
                self.draw_ball(r);
                self.draw_prompt(r);
                self.character.draw(r, 0);
            }
            State::Shaking { .. } => {
                self.draw_ball(r);
                self.character.draw(r, 0);
            }
        }
    }
}

fn clear_disc(r: &mut Renderer, cx: i32, cy: i32, radius: i32) {
    let r2 = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= r2 {
                r.draw_pixel(Point::new(cx + dx, cy + dy), false);
            }
        }
    }
}
