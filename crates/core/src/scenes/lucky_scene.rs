//! Lucky Numbers tool. Six numbers from 0-99 and a fortune for the pet's
//! current in-game day. Both come from the pet's seed and the day number,
//! so they hold steady all day and change at midnight.
//!
//! Layout (128x64):
//!   y=1..17   six lottery balls
//!   y=20..63  "Day N" while the balls roll, then the fortune (6x10 font)

use core::fmt::Write;
use embedded_graphics::prelude::Point;
use heapless::String;

use crate::t;
use crate::{
    context::GameContext,
    input::{Button, Buttons},
    pet_seed::{absorb, next_expanded},
    rand::rand_range_u32,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::wrap,
};

pub const LUCKY_COUNT: usize = 6;
/// Numbers are drawn from 0..=LUCKY_MAX.
pub const LUCKY_MAX: u8 = 99;

/// Keeps this stream apart from other users of the pet seed.
const SALT: u64 = 0x4C55_434B_594E_554D; // "LUCKYNUM"

pub const FORTUNES: &[&str] = &[
    t!("A sunbeam will find you before noon."),
    t!("Someone will open a door for you today. Take your time walking through."),
    t!("Knock one thing off a shelf today. Just one."),
    t!("A nap taken now is worth two taken later."),
    t!("An empty box holds great possibility."),
    t!("Patience at the food bowl will be rewarded."),
    t!("Today, the red dot will finally be caught."),
    t!("Stretch first. Big decisions can wait."),
    t!("A warm lap is closer than you think."),
    t!("Ignore them, and they will come to you."),
    t!("Fortune favours the cat who sits on the keyboard."),
    t!("Land on your feet, then act like you meant it."),
    t!("An unexpected treat is heading your way."),
    t!("Purr loudly. Good things are listening."),
    t!("Today's great mystery is under the couch."),
    t!("Blink slowly at a friend. They'll understand."),
    t!("The bird at the window is a sign. Of what, nobody knows."),
    t!("Your whiskers know the way. Follow them."),
    t!("Rain outside means cozy inside."),
    t!("Small paws can move big things."),
    t!("Fresh water and a quiet corner bring clarity."),
    t!("Today is a good day to be fluffy."),
    t!("Curiosity is safe today. Mostly."),
    t!("Someone will say your name in a happy voice."),
    t!("Groom your goals and they will shine."),
    t!("A new smell will lead to a new friend."),
    t!("The zoomies arrive at midnight. Be ready."),
    t!("Sit in the box that doesn't fit. Make it fit."),
    t!("Luck is a ball of yarn. Chase it gently."),
    t!("Even a short nap is a fresh start."),
];

/// Today's numbers (sorted, distinct) and an index into [`FORTUNES`].
pub fn lucky_for(pet_seed: u64, day: u32) -> ([u8; LUCKY_COUNT], usize) {
    let mut s = SALT;
    absorb(&mut s, pet_seed);
    absorb(&mut s, day as u64);

    // Partial Fisher-Yates over 0..=LUCKY_MAX.
    let mut pool = [0u8; LUCKY_MAX as usize + 1];
    for (i, v) in pool.iter_mut().enumerate() {
        *v = i as u8;
    }
    let n = pool.len();
    for i in 0..LUCKY_COUNT {
        let j = i + (next_expanded(&mut s) % (n - i) as u64) as usize;
        pool.swap(i, j);
    }
    let mut picks = [0u8; LUCKY_COUNT];
    picks.copy_from_slice(&pool[..LUCKY_COUNT]);
    picks.sort_unstable();

    let fortune = (next_expanded(&mut s) % FORTUNES.len() as u64) as usize;
    (picks, fortune)
}

const BALL_R: i32 = 8;
const BALL_Y: i32 = 9;
const BALL_PITCH: i32 = 20;
/// Text area under the balls.
const TEXT_TOP: i32 = 20;
const TEXT_H: i32 = 64 - TEXT_TOP;
const LINE_H: i32 = 11;
const LINE_CHARS: usize = 21;
/// Seconds before the first ball lands, then between balls.
const FIRST_REVEAL: f32 = 0.4;
const REVEAL_GAP: f32 = 0.35;
/// Seconds between digit changes on a ball still tumbling.
const TUMBLE: f32 = 0.06;

pub struct LuckyScene {
    numbers: [u8; LUCKY_COUNT],
    fortune: usize,
    day: u32,
    /// Seconds since the reveal began.
    t: f32,
    tumble_t: f32,
    /// What unrevealed balls currently show.
    tumbling: [u8; LUCKY_COUNT],
}

impl LuckyScene {
    pub fn new() -> Self {
        Self {
            numbers: [0; LUCKY_COUNT],
            fortune: 0,
            day: u32::MAX,
            t: 0.0,
            tumble_t: 0.0,
            tumbling: [0; LUCKY_COUNT],
        }
    }

    fn revealed(&self) -> usize {
        if self.t < FIRST_REVEAL {
            return 0;
        }
        (((self.t - FIRST_REVEAL) / REVEAL_GAP) as usize + 1).min(LUCKY_COUNT)
    }

    fn done_time() -> f32 {
        FIRST_REVEAL + REVEAL_GAP * (LUCKY_COUNT - 1) as f32
    }

    fn refresh(&mut self, ctx: &GameContext) {
        let (numbers, fortune) = lucky_for(ctx.pet_seed, ctx.day_number);
        self.numbers = numbers;
        self.fortune = fortune;
        self.day = ctx.day_number;
        self.t = 0.0;
    }
}

impl Scene for LuckyScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.refresh(ctx);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        // Midnight passed while watching: draw again.
        if ctx.day_number != self.day {
            self.refresh(ctx);
        }
        if buttons.was_just_pressed(Button::A) {
            self.t = if self.t < Self::done_time() { Self::done_time() } else { 0.0 };
        }
        self.t += dt;
        self.tumble_t += dt;
        if self.tumble_t >= TUMBLE {
            self.tumble_t = 0.0;
            for v in self.tumbling.iter_mut() {
                *v = rand_range_u32(&mut ctx.rng, 0, LUCKY_MAX as u32) as u8;
            }
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        let revealed = self.revealed();
        let x0 = 64 - BALL_PITCH * (LUCKY_COUNT as i32 - 1) / 2;
        for i in 0..LUCKY_COUNT {
            let cx = x0 + i as i32 * BALL_PITCH;
            let landed = i < revealed;
            let value = if landed { self.numbers[i] } else { self.tumbling[i] };
            let mut s: String<2> = String::new();
            let _ = write!(s, "{:02}", value);
            // 6x10 digits are 5px wide with a 1px gap and sit on rows 1..7
            // of their cell, so two of them centre at (-5, -4).
            let text_at = |dy: i32| Point::new(cx - 5, BALL_Y - 4 + dy);
            if landed {
                r.draw_circle_filled(Point::new(cx, BALL_Y), BALL_R);
                r.draw_text_inverted(&s, text_at(0));
            } else {
                // Still spinning: hollow ball, bouncing a pixel.
                let bob = if (i + (self.t / TUMBLE) as usize) % 2 == 0 { -1 } else { 0 };
                r.draw_circle_outline(Point::new(cx, BALL_Y + bob), BALL_R);
                r.draw_text(&s, text_at(bob));
            }
        }

        if revealed == LUCKY_COUNT {
            let lines: heapless::Vec<&str, 4> = wrap::wrap(FORTUNES[self.fortune], LINE_CHARS);
            let top = TEXT_TOP + (TEXT_H - lines.len() as i32 * LINE_H) / 2;
            wrap::draw_centered(r, &lines, 64, top, LINE_H);
        } else {
            let mut day: String<24> = String::new();
            let _ = write!(day, "{} {}", t!("Day"), self.day);
            wrap::draw_centered(r, &[day.as_str()], 64, TEXT_TOP + (TEXT_H - LINE_H) / 2, LINE_H);
        }
    }
}
