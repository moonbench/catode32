use embedded_graphics::prelude::{Point, Size};
use crate::t;

use crate::{
    behavior::{
        AffectionVariant, AttentionVariant, BehaviorId, BehaviorManager, EatingSource, GiftKind,
        NextBehavior, PlayVariant, TrainingKind,
    },
    context::{FoodItem, GameContext},
    entities::character::Character,
    input::{Button, Buttons},
    render::Renderer,
    scene::{Scene, SceneId},
    ui::scrollbar::Scrollbar,
};

const LINES_VISIBLE: usize = 7;
const LINE_HEIGHT: i32 = 8;
const FLOOR_Y: i32 = 60;
const CHAR_X: i32 = 100;
const CHAR_Y: i32 = 60;
const SCENE_X_MIN: i32 = 10;
const SCENE_X_MAX: i32 = 118;

struct Entry {
    label: &'static str,
    next: NextBehavior,
    target: BehaviorId,
}

const ENTRIES: &[Entry] = &[
    Entry { label: t!("Idle"),              next: NextBehavior::Idle,                                          target: BehaviorId::Idle },
    Entry { label: t!("Sleeping"),          next: NextBehavior::Sleeping,                                      target: BehaviorId::Sleeping },
    Entry { label: t!("Napping"),           next: NextBehavior::Napping,                                       target: BehaviorId::Napping },
    Entry { label: t!("Stretching"),        next: NextBehavior::Stretching,                                    target: BehaviorId::Stretching },
    Entry { label: t!("Kneading"),          next: NextBehavior::Kneading,                                      target: BehaviorId::Kneading },
    Entry { label: t!("Lounging"),          next: NextBehavior::Lounging,                                      target: BehaviorId::Lounging },
    Entry { label: t!("Investigating"),     next: NextBehavior::Investigating,                                 target: BehaviorId::Investigating },
    Entry { label: "Pawing",              next: NextBehavior::Pawing,                                        target: BehaviorId::Pawing },
    Entry { label: t!("Startled"),          next: NextBehavior::Startled,                                      target: BehaviorId::Startled },
    Entry { label: t!("Observing"),         next: NextBehavior::Observing,                                     target: BehaviorId::Observing },
    Entry { label: t!("Chattering"),        next: NextBehavior::Chattering,                                    target: BehaviorId::Chattering },
    Entry { label: t!("Zoomies"),           next: NextBehavior::Zoomies,                                       target: BehaviorId::Zoomies },
    Entry { label: t!("Vocalizing"),        next: NextBehavior::Vocalizing,                                    target: BehaviorId::Vocalizing },
    Entry { label: t!("Self Grooming"),     next: NextBehavior::SelfGrooming(false),                           target: BehaviorId::SelfGrooming },
    Entry { label: "Groom (after meal)",    next: NextBehavior::SelfGrooming(true),                            target: BehaviorId::SelfGrooming },
    Entry { label: t!("Being Groomed"),     next: NextBehavior::BeingGroomed,                                  target: BehaviorId::BeingGroomed },
    Entry { label: t!("Hunting"),           next: NextBehavior::Hunting,                                       target: BehaviorId::Hunting },
    Entry { label: "Gift (fish)",           next: NextBehavior::GiftBringing(GiftKind::Fish),                  target: BehaviorId::GiftBringing },
    Entry { label: "Gift (mouse)",          next: NextBehavior::GiftBringing(GiftKind::Mouse),                 target: BehaviorId::GiftBringing },
    Entry { label: t!("Pacing"),            next: NextBehavior::Pacing,                                        target: BehaviorId::Pacing },
    Entry { label: t!("Meandering"),        next: NextBehavior::Meandering,                                    target: BehaviorId::Meandering },
    Entry { label: t!("Sulking"),           next: NextBehavior::Sulking,                                       target: BehaviorId::Sulking },
    Entry { label: t!("Mischief"),          next: NextBehavior::Mischief,                                      target: BehaviorId::Mischief },
    Entry { label: t!("Hiding"),            next: NextBehavior::Hiding,                                        target: BehaviorId::Hiding },
    Entry { label: "Train (intel)",         next: NextBehavior::Training(TrainingKind::Intelligence),          target: BehaviorId::Training },
    Entry { label: "Train (behav)",         next: NextBehavior::Training(TrainingKind::Behavior),              target: BehaviorId::Training },
    Entry { label: "Train (fit)",           next: NextBehavior::Training(TrainingKind::Fitness),               target: BehaviorId::Training },
    Entry { label: "Train (social)",        next: NextBehavior::Training(TrainingKind::Sociability),           target: BehaviorId::Training },
    Entry { label: "Play (ball)",           next: NextBehavior::Playing(PlayVariant::Ball, false),                    target: BehaviorId::Playing },
    Entry { label: "Play (string)",         next: NextBehavior::Playing(PlayVariant::String, false),                  target: BehaviorId::Playing },
    Entry { label: "Play (feather)",        next: NextBehavior::Playing(PlayVariant::Feather, false),                 target: BehaviorId::Playing },
    Entry { label: "Play (mouse)",          next: NextBehavior::Playing(PlayVariant::Mouse, false),                   target: BehaviorId::Playing },
    Entry { label: "Play (hand)",           next: NextBehavior::Playing(PlayVariant::Hand, false),                    target: BehaviorId::Playing },
    Entry { label: "Play (laser)",          next: NextBehavior::Playing(PlayVariant::Laser, false),                   target: BehaviorId::Playing },
    Entry { label: "Play (bubbles)",        next: NextBehavior::Playing(PlayVariant::Bubbles, false),                 target: BehaviorId::Playing },
    Entry { label: "Affection (kiss)",      next: NextBehavior::Affection(AffectionVariant::Kiss),             target: BehaviorId::Affection },
    Entry { label: "Affection (pets)",      next: NextBehavior::Affection(AffectionVariant::Pets),             target: BehaviorId::Affection },
    Entry { label: "Affection (scratch)",   next: NextBehavior::Affection(AffectionVariant::Scratching),       target: BehaviorId::Affection },
    Entry { label: "Attention (psst)",      next: NextBehavior::Attention(AttentionVariant::Psst),             target: BehaviorId::Attention },
    Entry { label: "Attention (bird)",      next: NextBehavior::Attention(AttentionVariant::PointBird),        target: BehaviorId::Attention },
    Entry { label: t!("Hearing (exclaim)"), next: NextBehavior::Hearing(Some("exclaim")),                      target: BehaviorId::Hearing },
    Entry { label: t!("Hearing (heart)"),   next: NextBehavior::Hearing(Some("heart")),                        target: BehaviorId::Hearing },
    Entry { label: t!("Hearing (note)"),    next: NextBehavior::Hearing(Some("note")),                         target: BehaviorId::Hearing },
    Entry { label: "Eat (kibble)",          next: NextBehavior::Eating(EatingSource::Item(FoodItem::Kibble)),  target: BehaviorId::Eating },
    Entry { label: "Eat (wet)",             next: NextBehavior::Eating(EatingSource::Item(FoodItem::Chicken)), target: BehaviorId::Eating },
    Entry { label: "Eat (fish)",            next: NextBehavior::Eating(EatingSource::Item(FoodItem::Tuna)),    target: BehaviorId::Eating },
    Entry { label: "Eat (snack)",           next: NextBehavior::Eating(EatingSource::CaughtSnack),             target: BehaviorId::Eating },
    Entry { label: "Eat (treat)",           next: NextBehavior::Eating(EatingSource::Item(FoodItem::Treats)),  target: BehaviorId::Eating },
    Entry { label: "Greeting",              next: NextBehavior::Greeting,                                      target: BehaviorId::Greeting },
];

pub struct DebugBehaviorsScene {
    character: Character,
    behaviors: BehaviorManager,
    selected: usize,
    scroll: usize,
}

impl DebugBehaviorsScene {
    pub fn new() -> Self {
        Self {
            character: Character::new(Point::new(CHAR_X, CHAR_Y)),
            behaviors: BehaviorManager::new(),
            selected: 0,
            scroll: 0,
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    fn move_down(&mut self) {
        if self.selected + 1 < ENTRIES.len() {
            self.selected += 1;
        }
        if self.selected >= self.scroll + LINES_VISIBLE {
            self.scroll = self.selected + 1 - LINES_VISIBLE;
        }
    }

    fn trigger_selected(&mut self, ctx: &mut GameContext) {
        let next = ENTRIES[self.selected].next;
        self.behaviors.trigger(next, ctx, &mut self.character);
    }
}

impl Scene for DebugBehaviorsScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        ctx.scene_x_min = SCENE_X_MIN;
        ctx.scene_x_max = SCENE_X_MAX;
        self.character.reseed_anim();
        self.behaviors.start(ctx, &mut self.character);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::B) || buttons.was_just_pressed(Button::Menu1) {
            return Some(ctx.last_main_scene);
        }
        if buttons.was_just_pressed(Button::Up) {
            self.move_up();
        }
        if buttons.was_just_pressed(Button::Down) {
            self.move_down();
        }
        if buttons.was_just_pressed(Button::A) {
            self.trigger_selected(ctx);
        }

        self.behaviors.update(ctx, &mut self.character, dt);
        let pose = self.behaviors.current_pose();
        self.character.set_pose(pose);
        self.character.animate(dt);

        // Behaviors may request a scene transition (e.g. go_to). Consume it so
        // the debug scene doesn't end up jumping somewhere unexpected.
        ctx.pending_scene.take();
        None
    }

    fn draw(&self, ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        renderer.draw_line(Point::new(0, FLOOR_Y), Point::new(128, FLOOR_Y));

        self.draw_list(renderer);
        self.draw_progress(renderer);

        self.character.draw(renderer, 0);
        let char_screen = Point::new(self.character.pos.x, self.character.pos.y);
        self.behaviors
            .draw_overlay(renderer, ctx, char_screen, self.character.mirror_h);
    }
}

impl DebugBehaviorsScene {
    fn draw_list(&self, renderer: &mut Renderer) {
        let visible_end = (self.scroll + LINES_VISIBLE).min(ENTRIES.len());
        let current_id = self.behaviors.current_id();
        for (row, idx) in (self.scroll..visible_end).enumerate() {
            let entry = &ENTRIES[idx];
            let y = row as i32 * LINE_HEIGHT;
            let selected = idx == self.selected;
            if selected {
                renderer.draw_rect(Point::new(0, y), Size::new(128, LINE_HEIGHT as u32), true);
            }

            let mut buf = [0u8; 24];
            let active_marker = entry.target == current_id;
            let written = format_label(&mut buf, entry.label, active_marker);
            let text = core::str::from_utf8(&buf[..written]).unwrap_or(entry.label);

            if selected {
                renderer.draw_text_inverted(text, Point::new(1, y));
            } else {
                renderer.draw_text(text, Point::new(1, y));
            }
        }

        let track_height = (LINES_VISIBLE as i32 * LINE_HEIGHT) as u32;
        let bar = Scrollbar::right_edge(0, track_height);
        bar.draw(renderer, ENTRIES.len(), LINES_VISIBLE, self.scroll);
    }

    fn draw_progress(&self, renderer: &mut Renderer) {
        let progress = self.behaviors.current_progress();
        if progress <= 0.0 {
            return;
        }
        let width = (progress.clamp(0.0, 1.0) * 128.0) as i32;
        if width > 0 {
            renderer.draw_rect(
                Point::new(0, FLOOR_Y),
                Size::new(width as u32, 4),
                true,
            );
        }
    }
}

/// Copy `label` into `buf`, appending `*` if `active`. Returns the byte length
/// written. The label is truncated if it would overflow `buf`.
fn format_label(buf: &mut [u8], label: &str, active: bool) -> usize {
    let max = if active { buf.len().saturating_sub(1) } else { buf.len() };
    let bytes = label.as_bytes();
    let n = bytes.len().min(max);
    buf[..n].copy_from_slice(&bytes[..n]);
    if active && n < buf.len() {
        buf[n] = b'*';
        n + 1
    } else {
        n
    }
}
