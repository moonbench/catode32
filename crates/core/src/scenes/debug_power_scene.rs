use core::fmt::Write as _;

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
use crate::t;

use crate::{
    context::{GameContext, PowerAction},
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
};

const ROW_HEIGHT: i32 = 16;
const CONTENT_WIDTH: u32 = 128;

struct Entry {
    label: &'static str,
    action: PowerAction,
}

const ENTRIES: &[Entry] = &[
    Entry { label: t!("Reboot"),      action: PowerAction::Reboot },
    Entry { label: t!("Light Sleep"), action: PowerAction::LightSleep },
    Entry { label: t!("Deep Sleep"),  action: PowerAction::DeepSleep },
];

pub struct DebugPowerScene {
    selected: usize,
}

impl DebugPowerScene {
    pub fn new() -> Self {
        Self { selected: 0 }
    }
}

impl Scene for DebugPowerScene {
    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::B)
            || buttons.was_just_pressed(Button::Menu1)
            || buttons.was_just_pressed(Button::Menu2)
        {
            return Some(ctx.last_main_scene);
        }
        if buttons.was_just_pressed(Button::Up) && self.selected > 0 {
            self.selected -= 1;
        }
        if buttons.was_just_pressed(Button::Down) && self.selected + 1 < ENTRIES.len() {
            self.selected += 1;
        }
        if buttons.was_just_pressed(Button::A) {
            ctx.pending_power = Some(ENTRIES[self.selected].action);
            return Some(ctx.last_main_scene);
        }
        None
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        // Below the three 16px rows. Shown on-device because brownouts only
        // happen off USB, where there is no serial log to read.
        let mut line: String<32> = String::new();
        let _ = write!(line, "Reset: {}", ctx.last_reset_reason);
        renderer.draw_text(line.as_str(), Point::new(2, ENTRIES.len() as i32 * ROW_HEIGHT + 4));

        for (i, entry) in ENTRIES.iter().enumerate() {
            let y = i as i32 * ROW_HEIGHT;
            let selected = i == self.selected;
            if selected {
                renderer.draw_rect(
                    Point::new(0, y),
                    Size::new(CONTENT_WIDTH, ROW_HEIGHT as u32),
                    true,
                );
            }
            let text_y = y + (ROW_HEIGHT - 8) / 2;
            if selected {
                renderer.draw_text_inverted(entry.label, Point::new(2, text_y));
            } else {
                renderer.draw_text(entry.label, Point::new(2, text_y));
            }
        }
    }
}
