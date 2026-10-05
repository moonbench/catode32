//! Debug scene that exposes a handful of context-mutating actions:
//! save now, edit coins, edit pet seed, reset plants, reset stats,
//! factory-reset.

use core::fmt::Write as _;

use crate::platform::power::software_reset;
use heapless::String;
use crate::t;

use crate::{
    context::GameContext,
    input::{Button, Buttons},
    render::Renderer,
    save,
    scene::{Scene, SceneId},
    storage,
    ui::{
        confirm::{Confirm, ConfirmResult},
        keyboard::{Charset, OnScreenKeyboard},
        popup::Popup,
        settings::{SettingItem, SettingValue, Settings, SettingsResult},
    },
};

// Index assignments for `Settings`. Matches the order of the items pushed in
// `open_settings` so `Activated(i)` maps to a known action.
const IDX_SAVE: usize = 0;
const IDX_COINS: usize = 1;
const IDX_SEED: usize = 2;
const IDX_RESET_PLANTS: usize = 3;
const IDX_RESET_STATS: usize = 4;
const IDX_FACTORY_RESET: usize = 5;

const COINS_MIN: i32 = 0;
const COINS_MAX: i32 = 99_999;
const COINS_STEP: i32 = 1;

const SEED_HEX_LEN: usize = 16;

/// Which destructive action is waiting on the open `Confirm` dialog. Drives
/// what we actually run when the user hits A.
#[derive(Clone, Copy)]
enum PendingConfirm {
    ResetPlants,
    ResetStats,
    FactoryReset,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Settings,
    Seed,
    Confirm,
    SavedNotice,
}

pub struct DebugContextScene {
    settings: Settings,
    keyboard: OnScreenKeyboard,
    confirm: Confirm,
    pending: Option<PendingConfirm>,
    notice: Popup,
    mode: Mode,
}

impl DebugContextScene {
    pub fn new() -> Self {
        let mut notice = Popup::new(0, 16, 128, 32);
        notice.padding = 4;
        Self {
            settings: Settings::new(),
            keyboard: OnScreenKeyboard::new(Charset::Hex, SEED_HEX_LEN),
            confirm: Confirm::new(),
            pending: None,
            notice,
            mode: Mode::Settings,
        }
    }

    fn open_settings(&mut self, ctx: &GameContext) {
        let coins = ctx.coins.clamp(COINS_MIN, COINS_MAX);
        let items = [
            SettingItem::action(t!("Save now")),
            SettingItem::int(t!("Coins"), coins, COINS_MIN, COINS_MAX, COINS_STEP),
            SettingItem::action(t!("Seed")),
            SettingItem::action(t!("Reset Plants")),
            SettingItem::action(t!("Reset stats")),
            SettingItem::action(t!("Delete Context")),
        ];
        self.settings.open(&items);
        self.mode = Mode::Settings;
    }

    fn open_seed_keyboard(&mut self, ctx: &GameContext) {
        let mut hex: String<SEED_HEX_LEN> = String::new();
        let _ = write!(&mut hex, "{:016X}", ctx.pet_seed);
        self.keyboard.open(hex.as_str());
        self.mode = Mode::Seed;
    }

    fn apply_seed_text(&self, ctx: &mut GameContext, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        match u64::from_str_radix(trimmed, 16) {
            Ok(seed) => {
                ctx.reseed(seed);
                println!("[DebugContext] Reseeded to {:016X}", seed);
            }
            Err(_) => {
                println!("[DebugContext] Bad seed hex: {}", trimmed);
            }
        }
    }

    fn apply_coins_from_settings(&self, ctx: &mut GameContext) {
        if let Some(SettingValue::Int { value, .. }) = self.settings.value(IDX_COINS) {
            ctx.coins = *value;
        }
    }

    fn run_action(&mut self, ctx: &mut GameContext, idx: usize) {
        match idx {
            IDX_SAVE => {
                // Apply any pending coins edit before persisting so the
                // displayed value matches what gets written.
                self.apply_coins_from_settings(ctx);
                let ok = save::save(ctx);
                let msg = if ok { "Saved" } else { "Save failed" };
                self.notice.set_text(msg, false, true);
                self.mode = Mode::SavedNotice;
            }
            IDX_SEED => self.open_seed_keyboard(ctx),
            IDX_RESET_PLANTS => self.open_confirm(PendingConfirm::ResetPlants, t!("Reset all plants?")),
            IDX_RESET_STATS => self.open_confirm(PendingConfirm::ResetStats, t!("Reset all stats to defaults?")),
            IDX_FACTORY_RESET => self.open_confirm(
                PendingConfirm::FactoryReset,
                t!("Factory reset?\nAll data lost!"),
            ),
            _ => {}
        }
    }

    fn open_confirm(&mut self, what: PendingConfirm, text: &str) {
        self.pending = Some(what);
        self.confirm.open(text);
        self.mode = Mode::Confirm;
    }

    fn run_confirmed(&mut self, ctx: &mut GameContext) {
        let Some(what) = self.pending.take() else {
            return;
        };
        match what {
            PendingConfirm::ResetPlants => {
                ctx.reset_plants_to_starter();
                println!("[DebugContext] Plants reset to starter");
            }
            PendingConfirm::ResetStats => {
                ctx.reset_stats_to_defaults();
                println!("[DebugContext] Stats reset to defaults");
            }
            PendingConfirm::FactoryReset => {
                storage::erase_all();
                println!("[DebugContext] Factory reset, rebooting");
                software_reset();
            }
        }
    }
}

impl Scene for DebugContextScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.open_settings(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        match self.mode {
            Mode::Settings => match self.settings.handle_input(buttons) {
                SettingsResult::Continue => None,
                SettingsResult::Closed => {
                    self.apply_coins_from_settings(ctx);
                    Some(ctx.last_main_scene)
                }
                SettingsResult::Activated(i) => {
                    self.run_action(ctx, i);
                    None
                }
            },
            Mode::Seed => {
                if let Some(text) = self.keyboard.handle_input(buttons) {
                    self.apply_seed_text(ctx, text.as_str());
                    self.open_settings(ctx);
                }
                None
            }
            Mode::Confirm => {
                match self.confirm.handle_input(buttons) {
                    ConfirmResult::Pending => {}
                    ConfirmResult::Confirmed => {
                        self.run_confirmed(ctx);
                        // run_confirmed may have rebooted; if we're still
                        // running, return to the settings list.
                        self.open_settings(ctx);
                    }
                    ConfirmResult::Cancelled => {
                        self.pending = None;
                        self.open_settings(ctx);
                    }
                }
                None
            }
            Mode::SavedNotice => {
                if buttons.was_just_pressed(Button::A)
                    || buttons.was_just_pressed(Button::B)
                    || buttons.was_just_pressed(Button::Menu1)
                    || buttons.was_just_pressed(Button::Menu2)
                {
                    self.open_settings(ctx);
                }
                None
            }
        }
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        match self.mode {
            Mode::Settings => self.settings.draw(renderer),
            Mode::Seed => self.keyboard.draw(renderer),
            Mode::Confirm => {
                // Render the settings list behind the dialog so the modal
                // feels layered (same as Menu's confirm overlay).
                self.settings.draw(renderer);
                self.confirm.draw(renderer);
            }
            Mode::SavedNotice => {
                self.settings.draw(renderer);
                self.notice.draw(renderer, false);
            }
        }
    }
}
