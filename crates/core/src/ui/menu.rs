use embedded_graphics::prelude::{Point, Size};
use heapless::Vec;

use crate::{
    assets::icons,
    input::{Button, Buttons},
    render::{Renderer, SpriteOpts},
    ui::{
        confirm::{Confirm, ConfirmResult},
        list_nav::ListNav,
        scrollbar::Scrollbar,
    },
};

pub const VISIBLE_ITEMS: usize = 4;
pub const ROW_HEIGHT: i32 = 16;
pub const DEFAULT_CONTENT_WIDTH: i32 = 120;
pub const DEFAULT_SCROLLBAR_X: i32 = 126;
const ICON_X: i32 = 2;
const ICON_TEXT_GAP: i32 = 3;
const ARROW_INSET_FROM_RIGHT: i32 = 10;
const TRACK_HEIGHT: u32 = 64;
const MIN_THUMB_HEIGHT: u32 = 4;
const MAX_DEPTH: usize = 4;

#[derive(Clone, Copy)]
pub struct MenuItem<A: Copy + 'static> {
    pub label: &'static str,
    pub icon: Option<&'static [u8]>,
    pub submenu: Option<&'static [MenuItem<A>]>,
    pub action: Option<A>,
    pub confirm: Option<&'static str>,
    /// When the caller passes `vacation_active = true` to `handle_input`,
    /// this prompt is shown instead of `confirm`. Lets scene-switch items
    /// gate themselves behind an "End vacation?" prompt only while the
    /// player is on a vacation scene.
    pub confirm_on_vacation: Option<&'static str>,
}

pub enum MenuResult<A> {
    Continue,
    Closed,
    Action(A),
}

#[derive(Clone, Copy)]
struct Frame<A: Copy + 'static> {
    items: &'static [MenuItem<A>],
    nav: ListNav,
}

pub struct Menu<A: Copy + 'static> {
    current: Frame<A>,
    stack: Vec<Frame<A>, MAX_DEPTH>,
    content_width: i32,
    scrollbar_x: i32,
    confirm: Confirm,
    /// Action that fires once the open `Confirm` returns `Confirmed`.
    pending_action: Option<A>,
}

impl<A: Copy + 'static> Menu<A> {
    pub fn new(items: &'static [MenuItem<A>]) -> Self {
        Self::with_width(items, DEFAULT_CONTENT_WIDTH, DEFAULT_SCROLLBAR_X)
    }

    pub fn with_width(
        items: &'static [MenuItem<A>],
        content_width: i32,
        scrollbar_x: i32,
    ) -> Self {
        Self {
            current: Frame { items, nav: ListNav::new() },
            stack: Vec::new(),
            content_width,
            scrollbar_x,
            confirm: Confirm::new(),
            pending_action: None,
        }
    }

    /// Reset to the root menu (clears any submenu stack and pending confirmation).
    pub fn reset_to(&mut self, items: &'static [MenuItem<A>]) {
        self.current = Frame { items, nav: ListNav::new() };
        self.stack.clear();
        self.confirm.close();
        self.pending_action = None;
    }

    /// Highlight item `index` of the current list (clamped), scrolled into view.
    pub fn select(&mut self, index: usize) {
        let len = self.current.items.len();
        self.current.nav.reset();
        for _ in 0..index.min(len.saturating_sub(1)) {
            self.current.nav.down(len, VISIBLE_ITEMS);
        }
    }

    pub fn handle_input(&mut self, buttons: &mut Buttons, vacation_active: bool) -> MenuResult<A> {
        if self.confirm.is_open() {
            return match self.confirm.handle_input(buttons) {
                ConfirmResult::Pending => MenuResult::Continue,
                ConfirmResult::Confirmed => match self.pending_action.take() {
                    Some(action) => MenuResult::Action(action),
                    None => MenuResult::Continue,
                },
                ConfirmResult::Cancelled => {
                    self.pending_action = None;
                    MenuResult::Continue
                }
            };
        }

        if buttons.was_just_pressed(Button::Menu1) || buttons.was_just_pressed(Button::Menu2) {
            return MenuResult::Closed;
        }

        if buttons.was_just_pressed(Button::Up) {
            self.current.nav.up(VISIBLE_ITEMS);
        }
        if buttons.was_just_pressed(Button::Down) {
            self.current.nav.down(self.current.items.len(), VISIBLE_ITEMS);
        }

        if buttons.was_just_pressed(Button::B) {
            if self.stack.is_empty() {
                return MenuResult::Closed;
            }
            self.exit_submenu();
        }
        if buttons.was_just_pressed(Button::Left) && !self.stack.is_empty() {
            self.exit_submenu();
        }

        if buttons.was_just_pressed(Button::Right) {
            if let Some(item) = self.current.items.get(self.current.nav.selected) {
                if let Some(submenu) = item.submenu {
                    self.enter_submenu(submenu);
                }
            }
        }

        if buttons.was_just_pressed(Button::A) {
            if let Some(item) = self.current.items.get(self.current.nav.selected) {
                if let Some(submenu) = item.submenu {
                    self.enter_submenu(submenu);
                } else if let Some(action) = item.action {
                    let confirm_text = if vacation_active {
                        item.confirm_on_vacation.or(item.confirm)
                    } else {
                        item.confirm
                    };
                    if let Some(text) = confirm_text {
                        self.open_confirm(action, text);
                    } else {
                        return MenuResult::Action(action);
                    }
                }
            }
        }

        MenuResult::Continue
    }

    pub fn draw(&self, renderer: &mut Renderer) {
        let range = self.current.nav.visible_range(self.current.items.len(), VISIBLE_ITEMS);
        for (i, idx) in range.enumerate() {
            let item = &self.current.items[idx];
            let y = (i as i32) * ROW_HEIGHT;
            let selected = idx == self.current.nav.selected;
            draw_menu_row(
                renderer,
                item.label,
                item.icon,
                item.submenu.is_some(),
                y,
                selected,
                self.content_width,
            );
        }
        let bar = Scrollbar::new(self.scrollbar_x, 0, TRACK_HEIGHT, MIN_THUMB_HEIGHT);
        bar.draw(
            renderer,
            self.current.items.len(),
            VISIBLE_ITEMS,
            self.current.nav.scroll,
        );
        self.confirm.draw(renderer);
    }

    fn enter_submenu(&mut self, submenu: &'static [MenuItem<A>]) {
        let _ = self.stack.push(self.current);
        self.current = Frame { items: submenu, nav: ListNav::new() };
    }

    fn exit_submenu(&mut self) {
        if let Some(frame) = self.stack.pop() {
            self.current = frame;
        }
    }

    fn open_confirm(&mut self, action: A, text: &str) {
        self.pending_action = Some(action);
        self.confirm.open(text);
    }
}

/// Draw one row of a menu-style list: optional icon, label, and a `>` arrow
/// when the row leads into a submenu. The selected row gets an inverted-fill
/// background. Used by `Menu`, `LocationMenu`, and anywhere else that wants
/// the same row look.
pub fn draw_menu_row(
    renderer: &mut Renderer,
    label: &str,
    icon: Option<&'static [u8]>,
    has_submenu_arrow: bool,
    y: i32,
    selected: bool,
    content_width: i32,
) {
    if selected {
        renderer.draw_rect(
            Point::new(0, y),
            Size::new(content_width as u32, ROW_HEIGHT as u32),
            true,
        );
    }

    let mut text_x = ICON_X;
    if let Some(icon) = icon {
        let icon_y = y + (ROW_HEIGHT - icons::ICON_HEIGHT as i32) / 2;
        renderer.draw_sprite_raw(
            icon,
            icons::ICON_WIDTH,
            icons::ICON_HEIGHT,
            Point::new(ICON_X, icon_y),
            SpriteOpts {
                transparent: !selected,
                invert: selected,
                ..Default::default()
            },
        );
        text_x = ICON_X + icons::ICON_WIDTH as i32 + ICON_TEXT_GAP;
    }

    let text_y = y + (ROW_HEIGHT - 10) / 2;
    if selected {
        renderer.draw_text_inverted(label, Point::new(text_x, text_y));
    } else {
        renderer.draw_text(label, Point::new(text_x, text_y));
    }

    if has_submenu_arrow {
        let arrow_x = content_width - ARROW_INSET_FROM_RIGHT;
        if selected {
            renderer.draw_text_inverted(">", Point::new(arrow_x, text_y));
        } else {
            renderer.draw_text(">", Point::new(arrow_x, text_y));
        }
    }
}
