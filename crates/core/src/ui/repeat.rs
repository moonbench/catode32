//! Hold-to-repeat for value controls: one step on a tap, then a step
//! every frame once the button has been held a moment.

use crate::input::{Button, Buttons};

/// Seconds a button is held before it starts repeating.
pub const HOLD_DELAY: f32 = 0.35;

#[derive(Clone, Copy, Default)]
pub struct HoldRepeat {
    held: f32,
}

/// One step's worth of input from [`HoldRepeat::poll`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Step {
    /// -1 for the `down` button, 1 for `up`.
    pub dir: i32,
    /// 0 for a tap; seconds held for a repeat, so callers can speed up.
    pub held: f32,
}

impl HoldRepeat {
    pub const fn new() -> Self {
        Self { held: 0.0 }
    }

    pub fn reset(&mut self) {
        self.held = 0.0;
    }

    /// Poll a pair of buttons. Call every frame while the control has
    /// focus, so taps are seen.
    pub fn poll(&mut self, buttons: &mut Buttons, down: Button, up: Button, dt: f32) -> Option<Step> {
        let down_tap = buttons.was_just_pressed(down);
        let up_tap = buttons.was_just_pressed(up);
        if down_tap || up_tap {
            self.held = 0.0;
            return Some(Step { dir: if up_tap { 1 } else { -1 }, held: 0.0 });
        }
        let dir = if buttons.is_pressed(up) {
            1
        } else if buttons.is_pressed(down) {
            -1
        } else {
            self.held = 0.0;
            return None;
        };
        self.held += dt;
        (self.held >= HOLD_DELAY).then_some(Step { dir, held: self.held })
    }
}
