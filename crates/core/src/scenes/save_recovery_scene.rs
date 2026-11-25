//! Shown at boot when a save exists but couldn't be read.
//!
//! Storage is locked while this is up (see `save::load`), so nothing can
//! overwrite the unreadable save. The player can leave it in place and wait
//! for a firmware update that reads it, or explicitly erase it to start over.

use crate::{
    context::GameContext,
    input::{Button, Buttons},
    platform::power::software_reset,
    render::Renderer,
    scene::{Scene, SceneId},
    storage,
    t,
    ui::{
        confirm::{Confirm, ConfirmResult},
        popup::Popup,
    },
};

pub struct SaveRecoveryScene {
    info: Popup,
    confirm: Confirm,
}

impl SaveRecoveryScene {
    pub fn new() -> Self {
        let mut info = Popup::new(0, 0, 128, 64);
        info.padding = 4;
        Self {
            info,
            confirm: Confirm::new(),
        }
    }
}

impl Scene for SaveRecoveryScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.info.set_text(
            t!("Your cat's save couldn't be read. It is kept safe and nothing will overwrite it. A firmware update may recover it. Press A to erase it and start over."),
            true,
            false,
        );
    }

    fn update(
        &mut self,
        _ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        if self.confirm.is_open() {
            match self.confirm.handle_input(buttons) {
                ConfirmResult::Confirmed => {
                    if storage::erase_all() {
                        println!("[Recovery] Unreadable save erased by the player, rebooting");
                        software_reset();
                    }
                    println!("[Recovery] Erase failed; save still locked");
                }
                ConfirmResult::Cancelled | ConfirmResult::Pending => {}
            }
            return None;
        }
        if buttons.was_just_pressed(Button::Up) {
            self.info.scroll_up();
        } else if buttons.was_just_pressed(Button::Down) {
            self.info.scroll_down();
        } else if buttons.was_just_pressed(Button::A) {
            self.confirm.open(t!("Erase old save?\nThat cat is lost!"));
        }
        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.info.draw(renderer, true);
        if self.confirm.is_open() {
            self.confirm.draw(renderer);
        }
    }
}
