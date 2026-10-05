#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    context::GameContext,
    input::Buttons,
    render::Renderer,
    scene::{Scene, SceneId},
    ui::settings::{SettingItem, SettingValue, Settings, SettingsResult},
};

// time_speed range 0.5..20.0 step 0.5 stored as tenths.
const DIVISOR: i32 = 10;
const SPEED_MIN: i32 = 5;
const SPEED_MAX: i32 = 200;
const SPEED_STEP: i32 = 5;

pub struct DebugTimeScene {
    settings: Settings,
}

impl DebugTimeScene {
    pub fn new() -> Self {
        Self {
            settings: Settings::new(),
        }
    }
}

impl Scene for DebugTimeScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        let current = (ctx.time_speed * DIVISOR as f32).round() as i32;
        let items = [SettingItem::fixed(
            t!("Speed"),
            current.clamp(SPEED_MIN, SPEED_MAX),
            SPEED_MIN,
            SPEED_MAX,
            SPEED_STEP,
            DIVISOR,
        )];
        self.settings.open(&items);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        match self.settings.handle_input(buttons) {
            SettingsResult::Continue | SettingsResult::Activated(_) => None,
            SettingsResult::Closed => {
                if let Some(SettingValue::Fixed { value, divisor, .. }) = self.settings.value(0) {
                    ctx.time_speed = *value as f32 / *divisor as f32;
                }
                Some(ctx.last_main_scene)
            }
        }
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.settings.draw(renderer);
    }
}
