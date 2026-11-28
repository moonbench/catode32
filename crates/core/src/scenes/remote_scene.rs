//! Remote tool: the device pairs over Bluetooth as a keyboard and remote
//! control. Camera mode works a phone's shutter, Slides moves through a
//! presentation, Media controls playback.
//!
//! Bluetooth has the radio to itself while this is open: WiFi scans and
//! ESP-NOW wait until it's left. The screen lists what each button sends
//! in the current mode; Menu2 switches mode (or forgets pairings). Pairings are kept in their own flash sector so phones
//! reconnect after a reboot without pairing again.

use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};

use crate::t;
use crate::{
    ble::{self, bonds, keys, BondRecord, Mode, StartConfig, Status, MAX_BONDS},
    context::GameContext,
    input::{Button, Buttons},
    led::{scale_rgb, LedLatch},
    platform::{rng::device_id, time::now_ms},
    render::Renderer,
    scene::{Scene, SceneId},
    storage,
    ui::menu::{Menu, MenuItem, MenuResult},
};

#[derive(Clone, Copy)]
enum Action {
    Mode(usize),
    Forget,
}

/// Menu2 opens straight onto the modes, with Forget at the end.
const OPTIONS: &[MenuItem<Action>] = &[
    MenuItem { label: t!("Camera"), icon: None, submenu: None, action: Some(Action::Mode(0)), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Slides"), icon: None, submenu: None, action: Some(Action::Mode(1)), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Media"),  icon: None, submenu: None, action: Some(Action::Mode(2)), confirm: None, confirm_on_vacation: None },
    MenuItem { label: t!("Forget pairings"), icon: None, submenu: None, action: Some(Action::Forget), confirm: Some(t!("Forget every paired device?")), confirm_on_vacation: None },
];

/// How long the LED blips, and the minimum time a tapped button's cell
/// stays highlighted, so quick taps are still visible.
const SENT_FLASH_MS: u64 = 150;
const BLUE: (u8, u8, u8) = (0, 60, 255);
const LED_LEVEL: u8 = 120;
/// Longest wait for Bluetooth to shut down when leaving.
const STOP_TIMEOUT_MS: u64 = 1500;

/// Characters across the screen in the 6x10 font.
const LINE_CHARS: usize = 21;
const BANNER_H: u32 = 11;
/// The key grid sits between the banner and the info lines.
const GRID_TOP: i32 = 12;
const GRID_ROWS: i32 = 3;
const ROW_H: i32 = 10;
const CELL_W: u32 = 63;
const INFO_Y: i32 = 42;
/// The Menu2 hint sits on a solid bar along the bottom, like the banner.
const FOOTER_TOP: i32 = 53;

/// A button and what it does in a mode.
pub type KeyCell = Option<(Button, &'static str)>;

/// The on-screen key guide for `mode`: rows of two cells. Each button keeps
/// the same cell in every mode that uses it.
pub fn key_rows(mode: Mode) -> &'static [[KeyCell; 2]] {
    match mode {
        // B sends Volume Down, which camera apps also take as the shutter.
        Mode::Camera => &[[Some((Button::A, t!("Photo"))), Some((Button::B, t!("Photo")))]],
        Mode::Slides => &[
            [Some((Button::Left, t!("Back"))), Some((Button::Right, t!("Next")))],
            [Some((Button::Up, t!("Start"))), Some((Button::Down, t!("Blank")))],
            [Some((Button::A, t!("Next"))), Some((Button::B, t!("Back")))],
        ],
        Mode::Media => &[
            [Some((Button::Left, t!("Prev"))), Some((Button::Right, t!("Next")))],
            [Some((Button::Up, t!("Vol+"))), Some((Button::Down, t!("Vol-")))],
            [Some((Button::A, t!("Play/Pause"))), Some((Button::B, t!("Mute")))],
        ],
    }
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Camera => t!("Camera"),
        Mode::Slides => t!("Slides"),
        Mode::Media => t!("Media"),
    }
}

fn text_width(text: &str) -> i32 {
    text.chars().count() as i32 * 6
}

/// A 6x8 picture of a d-pad button (or its letter for A/B) at `(x, y)`.
fn draw_button_glyph(r: &mut Renderer, button: Button, x: i32, y: i32, on: bool) {
    let line = |r: &mut Renderer, x0, y0, x1, y1| r.draw_line_color(Point::new(x0, y0), Point::new(x1, y1), on);
    match button {
        Button::Left => (0..4).for_each(|k| line(r, x + 1 + k, y + 4 - k, x + 1 + k, y + 4 + k)),
        Button::Right => (0..4).for_each(|k| line(r, x + 4 - k, y + 4 - k, x + 4 - k, y + 4 + k)),
        Button::Up => (0..4).for_each(|k| line(r, x + 2 - k, y + 2 + k, x + 2 + k, y + 2 + k)),
        Button::Down => (0..4).for_each(|k| line(r, x + 2 - k, y + 5 - k, x + 2 + k, y + 5 - k)),
        _ => {
            let label = button.label();
            if on {
                r.draw_text(label, Point::new(x, y));
            } else {
                r.draw_text_inverted(label, Point::new(x, y));
            }
        }
    }
}

pub struct RemoteScene {
    mode: usize,
    menu: Menu<Action>,
    menu_open: bool,
    /// The radio was busy (an ESP-NOW playdate) when we tried to start.
    blocked: bool,
    bonds: Vec<BondRecord, MAX_BONDS>,
    sent_at: Option<u64>,
    /// Buttons held this frame (bit per `Button`), and the last one tapped.
    held: u8,
    tapped: Option<(Button, u64)>,
    led: LedLatch,
    now: u64,
}

impl RemoteScene {
    pub fn new() -> Self {
        Self {
            mode: 0,
            menu: Menu::new(OPTIONS),
            menu_open: false,
            blocked: false,
            bonds: Vec::new(),
            sent_at: None,
            held: 0,
            tapped: None,
            led: LedLatch::new(),
            now: 0,
        }
    }

    fn mode(&self) -> Mode {
        keys::MODES[self.mode]
    }

    fn start(&mut self, ctx: &mut GameContext) {
        if ble::status() == Status::Unavailable {
            return;
        }
        // Bluetooth can't share the radio; wait for WiFi/ESP-NOW to finish.
        self.blocked = ctx.radio_users > 0;
        if self.blocked {
            return;
        }
        self.bonds = storage::open_bond_partition()
            .map(|mut p| bonds::load(&mut p))
            .unwrap_or_default();
        ble::start(StartConfig {
            name: ble::device_name(&ctx.pet_name),
            address: ble::static_address(device_id()),
            bonds: self.bonds.clone(),
        });
    }

    /// Store a bond the BLE task reports, on this (the game) thread.
    fn save_new_bond(&mut self) {
        if let Some(bond) = ble::take_new_bond() {
            bonds::remember(&mut self.bonds, bond);
            if let Some(mut p) = storage::open_bond_partition() {
                bonds::store(&mut p, &self.bonds);
            }
        }
    }

    fn forget(&mut self, ctx: &mut GameContext) {
        ble::stop_and_wait(STOP_TIMEOUT_MS);
        self.bonds.clear();
        if let Some(mut p) = storage::open_bond_partition() {
            bonds::store(&mut p, &[]);
        }
        self.start(ctx);
    }

    fn handle_menu(&mut self, ctx: &mut GameContext, buttons: &mut Buttons) {
        match self.menu.handle_input(buttons, false) {
            MenuResult::Continue => {}
            MenuResult::Closed => self.menu_open = false,
            MenuResult::Action(action) => {
                self.menu_open = false;
                match action {
                    Action::Mode(i) => {
                        self.mode = i.min(keys::MODES.len() - 1);
                        ctx.tool_prefs.remote_mode = self.mode as u8;
                    }
                    Action::Forget => self.forget(ctx),
                }
            }
        }
    }

    /// The connection state shown in the banner.
    fn state_text(&self, status: Status) -> &'static str {
        if self.blocked {
            return t!("Radio busy");
        }
        match status {
            Status::Unavailable => t!("Unavailable"),
            Status::Off | Status::Starting => t!("Starting..."),
            Status::Advertising => t!("Not connected"),
            Status::Connected { secure: false } => t!("Pairing..."),
            Status::Connected { secure: true } => t!("Connected"),
            Status::Failed => t!("Failed"),
        }
    }

    /// One line under the key guide: what to do next, if anything.
    fn info_text(&self, ctx: &GameContext, status: Status) -> String<64> {
        let mut line = String::new();
        if self.blocked || status == Status::Failed {
            let _ = line.push_str(t!("A: try again"));
            return line;
        }
        match status {
            Status::Unavailable => {
                let _ = line.push_str(t!("Real device only"));
            }
            Status::Advertising => {
                let name = ble::device_name(&ctx.pet_name);
                let pair = t!("Pair with");
                if pair.chars().count() + 1 + name.chars().count() <= LINE_CHARS {
                    let _ = line.push_str(pair);
                    let _ = line.push(' ');
                }
                let _ = line.push_str(&name);
            }
            Status::Connected { secure: true } if self.mode() == Mode::Camera => {
                let _ = line.push_str(t!("Open the camera app"));
            }
            _ => {}
        }
        line
    }

    fn highlighted(&self, button: Button) -> bool {
        self.held & (1 << button as u8) != 0
            || self.tapped.is_some_and(|(b, t)| b == button && self.now - t < SENT_FLASH_MS)
    }

    /// "Mode: state" on a solid bar. If a translation runs long, the mode
    /// name is shortened rather than the state.
    fn draw_banner(&self, r: &mut Renderer, state: &str) {
        let mode = mode_label(self.mode());
        let state_len = state.chars().count();
        let mut text: String<64> = String::new();
        let room = LINE_CHARS.saturating_sub(state_len + 2);
        if mode.chars().count() <= room {
            let _ = text.push_str(mode);
        } else {
            mode.chars().take(room.saturating_sub(1)).for_each(|c| {
                let _ = text.push(c);
            });
            let _ = text.push('.');
        }
        let _ = text.push_str(": ");
        let _ = text.push_str(state);
        r.draw_rect(Point::new(0, 0), Size::new(128, BANNER_H), true);
        r.draw_text_inverted(&text, Point::new(64 - text_width(&text) / 2, 1));
    }

    fn draw_keys(&self, r: &mut Renderer) {
        let rows = key_rows(self.mode());
        // Fewer rows sit in the middle of the grid area.
        let top = GRID_TOP + (GRID_ROWS - rows.len() as i32) * ROW_H / 2;
        for (i, row) in rows.iter().enumerate() {
            let y = top + i as i32 * ROW_H;
            for (col, cell) in row.iter().enumerate() {
                let Some((button, label)) = *cell else { continue };
                let x = col as i32 * 65;
                let on = !self.highlighted(button);
                if !on {
                    r.draw_rect(Point::new(x, y), Size::new(CELL_W, ROW_H as u32), true);
                }
                draw_button_glyph(r, button, x + 2, y, on);
                if on {
                    r.draw_text(label, Point::new(x + 11, y));
                } else {
                    r.draw_text_inverted(label, Point::new(x + 11, y));
                }
            }
        }
    }
}

impl Scene for RemoteScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        self.mode = (ctx.tool_prefs.remote_mode as usize).min(keys::MODES.len() - 1);
        self.menu_open = false;
        self.led.forget();
        self.start(ctx);
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        if !ble::stop_and_wait(STOP_TIMEOUT_MS) {
            crate::println!("[BLE] did not stop in time");
        }
        ctx.keep_awake = false;
        self.led.forget();
        self.led.off(&mut ctx.led);
    }

    fn update(&mut self, ctx: &mut GameContext, buttons: &mut Buttons, _dt: f32) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        self.now = now_ms();
        self.save_new_bond();
        let status = ble::status();

        if self.menu_open {
            self.handle_menu(ctx, buttons);
        } else if buttons.was_just_pressed(Button::Menu2) {
            self.menu.reset_to(OPTIONS);
            self.menu.select(self.mode);
            self.menu_open = true;
        } else if self.blocked || status == Status::Failed {
            if buttons.was_just_pressed(Button::A) {
                ble::stop_and_wait(STOP_TIMEOUT_MS);
                self.start(ctx);
            }
        } else {
            for button in keys::KEY_BUTTONS {
                if !buttons.was_just_pressed(button) {
                    continue;
                }
                self.tapped = Some((button, self.now));
                if let Some(key) = keys::key_for(self.mode(), button) {
                    if ble::send(key) {
                        self.sent_at = Some(self.now);
                    }
                }
            }
        }

        self.held = if self.menu_open { 0 } else { buttons.pressed_mask() };
        let flashing = self.sent_at.is_some_and(|t| self.now - t < SENT_FLASH_MS);
        let rgb = if flashing { scale_rgb(BLUE, LED_LEVEL) } else { (0, 0, 0) };
        self.led.show(&mut ctx.led, rgb);
        ctx.keep_awake = status.is_active();
        None
    }

    fn draw(&self, ctx: &GameContext, r: &mut Renderer, _dt_ms: u64) {
        if self.menu_open {
            self.menu.draw(r);
            return;
        }
        let status = ble::status();
        self.draw_banner(r, self.state_text(status));
        self.draw_keys(r);

        let info = self.info_text(ctx, status);
        let info: &str = &info;
        if !info.is_empty() {
            let shown = &info[..info.char_indices().nth(LINE_CHARS).map_or(info.len(), |(i, _)| i)];
            r.draw_text(shown, Point::new(64 - text_width(shown) / 2, INFO_Y));
        }

        let hint = t!("M2: change mode");
        r.draw_rect(Point::new(0, FOOTER_TOP), Size::new(128, BANNER_H), true);
        r.draw_text_inverted(hint, Point::new(64 - text_width(hint) / 2, FOOTER_TOP + 1));
    }
}
