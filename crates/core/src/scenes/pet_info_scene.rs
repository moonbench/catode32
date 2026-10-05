//! Pet info, a rich scrollable biography page.

use core::fmt::Write;

use embedded_graphics::prelude::Point;
use heapless::{String, Vec};
use crate::t;

use crate::{
    assets::{
        character::{PoseId, ALL_POSES},
        icons,
    },
    context::{FavWeather, FoodItem, FoodKind, GameContext, MealEntry, ToyVariant, PET_NAME_MAX},
    input::{Button, Buttons},
    pet_seed::{PetGender, Temperament},
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::keyboard::{Charset, OnScreenKeyboard},
};

/// Visible lines on screen (8 rows x 8 px = 64 px).
const VISIBLE: usize = 8;
/// Pixel width of a single glyph in the current font (FONT_6X10).
const CHAR_W: i32 = 6;
/// Full-width column count, leaving room for scroll arrows on the right
/// edge. Arrows render at x=119; reserving ~16 px of right margin gives
/// (128 - 16 - 3) / 6 ~= 18 chars.
const FULL_CPL: usize = 18;

const LINE_CAP: usize = 24;
const LINES_CAP: usize = 96;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Browsing,
    Editing,
}

pub struct PetInfoScene {
    state: State,
    keyboard: OnScreenKeyboard,
    portrait_poses: Vec<PoseId, 64>,
    portrait: Option<PoseId>,
    lines: Vec<String<LINE_CAP>, LINES_CAP>,
    scroll: usize,
    max_scroll: usize,
    /// x offset for lines that sit beside the headshot.
    narrow_x: i32,
    /// How many leading lines use `narrow_x`.
    narrow_lines: usize,
}

impl PetInfoScene {
    pub fn new() -> Self {
        Self {
            state: State::Browsing,
            keyboard: OnScreenKeyboard::new(Charset::Full, PET_NAME_MAX),
            portrait_poses: Vec::new(),
            portrait: None,
            lines: Vec::new(),
            scroll: 0,
            max_scroll: 0,
            narrow_x: 0,
            narrow_lines: 0,
        }
    }

    fn build_portrait_poses(out: &mut Vec<PoseId, 64>) {
        out.clear();
        for &id in ALL_POSES {
            let name = id.name();
            if name.starts_with("costume_sitting.") || name.starts_with("sitting_back.") {
                continue;
            }
            if id.data().eyes.is_some() {
                let _ = out.push(id);
            }
        }
    }

    fn build_content(&mut self, ctx: &GameContext) {
        self.lines.clear();
        if self.portrait_poses.is_empty() {
            return;
        }

        // Choose portrait pose from the stored seed.
        let portrait_idx = ((ctx.pet_seed >> 40) as usize) % self.portrait_poses.len();
        let pose_id = self.portrait_poses[portrait_idx];
        self.portrait = Some(pose_id);
        let pose = pose_id.data();
        let head_w = pose.head.sprite.width as i32;
        let head_h = pose.head.sprite.height as i32;
        self.narrow_lines = ((head_h + 7) / 8) as usize;
        self.narrow_x = head_w + 6;
        let narrow_cpl = (((128 - self.narrow_x - 3) / CHAR_W) as usize).max(1);

        // Pronouns from stored gender. Lowercase forms are runtime-derived
        // from the translated uppercase ones so languages with different
        // pronouns (e.g. es "Ella" → "ella", de "Sie" → "sie") work.
        let gender = ctx.pet_gender.unwrap_or(PetGender::Queen);
        let (she, her_cap, g_noun) = match gender {
            PetGender::Tom => (t!("He"), t!("His"), t!("tom")),
            PetGender::Queen => (t!("She"), t!("Her"), t!("queen")),
        };
        let she_l: String<8> = to_lower(she);
        let her: String<8> = to_lower(her_cap);

        // Intro paragraph 1, floated beside headshot.
        let name = if ctx.pet_name.is_empty() {
            "?"
        } else {
            ctx.pet_name.as_str()
        };
        let days = ctx.day_number;
        let temper = current_temperament(ctx);
        let sign_lower: String<24> = match ctx.star_sign {
            Some(s) => to_lower(s.label()),
            None => {
                let mut q: String<24> = String::new();
                let _ = q.push('?');
                q
            }
        };

        let mut days_buf: String<8> = String::new();
        let _ = write!(&mut days_buf, "{}", days);
        let mut intro1: String<96> = String::new();
        substitute(
            &mut intro1,
            t!("{name} is a {days} day old {gnoun}."),
            &[("name", name), ("days", days_buf.as_str()), ("gnoun", g_noun)],
        );
        wrap_intro(
            &mut self.lines,
            intro1.as_str(),
            narrow_cpl,
            FULL_CPL,
            self.narrow_lines,
        );
        push_blank(&mut self.lines);

        let temper_lower: String<24> = to_lower(temper.label());
        let mut intro2: String<64> = String::new();
        substitute(
            &mut intro2,
            t!("{she} is a {temper} {sign}."),
            &[("she", she), ("temper", temper_lower.as_str()), ("sign", sign_lower.as_str())],
        );
        wrap_full(&mut self.lines, intro2.as_str(), FULL_CPL);
        push_blank(&mut self.lines);

        // Body paragraphs. Each label is translated then lowercased so it
        // reads naturally embedded in a sentence (e.g. "favorite meal is
        // kibble").
        let meal: String<24> = to_lower(food_label(ctx.fav_meal));
        let snack: String<24> = to_lower(food_label(ctx.fav_snack));
        let toy: String<24> = to_lower(toy_label(ctx.fav_toy));
        let room: String<24> = to_lower(location_label(ctx.fav_location));
        let weather = weather_label(ctx.fav_weather);

        let mut buf: String<96> = String::new();

        substitute(
            &mut buf,
            t!("{Her} favorite meal is {meal}, and {her} favorite snack is {snack}."),
            &[
                ("Her", her_cap),
                ("meal", meal.as_str()),
                ("her", her.as_str()),
                ("snack", snack.as_str()),
            ],
        );
        wrap_full(&mut self.lines, buf.as_str(), FULL_CPL);
        push_blank(&mut self.lines);
        buf.clear();

        substitute(
            &mut buf,
            t!("{She} loves to hang out in the {room}, and {she_l} really enjoys {weather} days."),
            &[
                ("She", she),
                ("room", room.as_str()),
                ("she_l", she_l.as_str()),
                ("weather", weather),
            ],
        );
        wrap_full(&mut self.lines, buf.as_str(), FULL_CPL);
        push_blank(&mut self.lines);
        buf.clear();

        substitute(
            &mut buf,
            t!("Of all the toys, {she_l} loves to play with the {toy} the most."),
            &[("she_l", she_l.as_str()), ("toy", toy.as_str())],
        );
        wrap_full(&mut self.lines, buf.as_str(), FULL_CPL);
        push_blank(&mut self.lines);
        buf.clear();

        // Sickness.
        if ctx.sickness >= 7.0 {
            substitute(&mut buf, t!("{she} feels very sick."), &[("she", she)]);
            wrap_full(&mut self.lines, buf.as_str(), FULL_CPL);
            push_blank(&mut self.lines);
            buf.clear();
        } else if ctx.sickness >= 3.0 {
            substitute(&mut buf, t!("{she} feels pretty sick."), &[("she", she)]);
            wrap_full(&mut self.lines, buf.as_str(), FULL_CPL);
            push_blank(&mut self.lines);
            buf.clear();
        } else if ctx.sickness > 0.0 {
            substitute(&mut buf, t!("{she} feels a little sick."), &[("she", she)]);
            wrap_full(&mut self.lines, buf.as_str(), FULL_CPL);
            push_blank(&mut self.lines);
            buf.clear();
        }

        // Mood sentences. Each low stat contributes one sentence; if none are
        // low, fall back to the "feeling great" line.
        let mut any_mood = false;
        for &(val, threshold, template) in &mood_checks(ctx) {
            if val < threshold {
                if any_mood {
                    push_blank(&mut self.lines);
                }
                let mut sentence: String<64> = String::new();
                expand_template(&mut sentence, template, she, her.as_str());
                wrap_full(&mut self.lines, sentence.as_str(), FULL_CPL);
                any_mood = true;
            }
        }
        if !any_mood {
            let mut sentence: String<64> = String::new();
            substitute(
                &mut sentence,
                t!("It seems like {she_l}'s feeling pretty great at the moment!"),
                &[("she_l", she_l.as_str())],
            );
            wrap_full(&mut self.lines, sentence.as_str(), FULL_CPL);
        }
        push_blank(&mut self.lines);

        // Meal variety: 4+ of last 5 meals are the same kind.
        if recent_meal_dominance(ctx) >= 4 {
            let mut s: String<64> = String::new();
            substitute(
                &mut s,
                t!("{she} wishes {she_l} had more variety in {her} meals."),
                &[("she", she), ("she_l", she_l.as_str()), ("her", her.as_str())],
            );
            wrap_full(&mut self.lines, s.as_str(), FULL_CPL);
            push_blank(&mut self.lines);
        }

        // Familiar location.
        if ctx.in_familiar_location {
            let mut s: String<32> = String::new();
            substitute(&mut s, t!("{she} feels at home here."), &[("she", she)]);
            wrap_full(&mut self.lines, s.as_str(), FULL_CPL);
            push_blank(&mut self.lines);
        }

        // Strip trailing blanks, add two spacers, then the Change Name row.
        while self
            .lines
            .last()
            .map(|l| l.is_empty())
            .unwrap_or(false)
        {
            self.lines.pop();
        }
        push_blank(&mut self.lines);
        push_blank(&mut self.lines);
        let mut cn: String<LINE_CAP> = String::new();
        let _ = cn.push_str(t!("Change Name"));
        let _ = self.lines.push(cn);

        self.max_scroll = self.lines.len().saturating_sub(VISIBLE);
    }

    fn handle_input(&mut self, buttons: &mut Buttons, ctx: &mut GameContext) -> Option<SceneId> {
        match self.state {
            State::Editing => {
                if let Some(name) = self.keyboard.handle_input(buttons) {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() {
                        ctx.pet_name.clear();
                        for c in trimmed.chars().take(PET_NAME_MAX) {
                            if ctx.pet_name.push(c).is_err() {
                                break;
                            }
                        }
                    }
                    self.state = State::Browsing;
                    self.build_content(ctx);
                }
                None
            }
            State::Browsing => {
                if buttons.was_just_pressed(Button::B) {
                    return Some(ctx.last_main_scene);
                }
                if buttons.was_just_pressed(Button::Up) && self.scroll > 0 {
                    self.scroll -= 1;
                } else if buttons.was_just_pressed(Button::Down) && self.scroll < self.max_scroll {
                    self.scroll += 1;
                } else if buttons.was_just_pressed(Button::A) && self.scroll == self.max_scroll {
                    self.keyboard.open(ctx.pet_name.as_str());
                    self.state = State::Editing;
                }
                None
            }
        }
    }

    fn draw_browsing(&self, renderer: &mut Renderer) {
        let total = self.lines.len();

        // Headshot floats with scroll for the first `narrow_lines` rows.
        if let Some(pose_id) = self.portrait {
            if self.scroll < self.narrow_lines {
                let y = -(self.scroll as i32) * 8;
                let pose = pose_id.data();
                renderer.draw_sprite(&pose.head.sprite, Point::new(0, y), SpriteOpts::default());
                if let Some(eyes) = pose.eyes {
                    let ex = pose.head.eye_x as i32 - eyes.anchor_x as i32;
                    let ey = y + pose.head.eye_y as i32 - eyes.anchor_y as i32;
                    renderer.draw_sprite(&eyes.sprite, Point::new(ex, ey), SpriteOpts::default());
                }
            }
        }
        let rows_remaining = self.narrow_lines.saturating_sub(self.scroll);

        for i in 0..VISIBLE {
            let line_idx = self.scroll + i;
            if line_idx >= total {
                break;
            }
            let y = (i * 8) as i32;
            let x = if i < rows_remaining { self.narrow_x } else { 3 };
            let line = &self.lines[line_idx];
            let is_change_name = line_idx == total - 1;
            let at_bottom = self.scroll == self.max_scroll;
            if is_change_name && at_bottom {
                renderer.draw_rect(
                    Point::new(0, y),
                    embedded_graphics::prelude::Size::new(128, 8),
                    true,
                );
                renderer.draw_text_inverted(line.as_str(), Point::new(x, y));
            } else {
                renderer.draw_text(line.as_str(), Point::new(x, y));
            }
        }

        if self.scroll > 0 {
            renderer.draw_sprite_raw(
                icons::UP_ARROW,
                icons::ARROW_W,
                icons::ARROW_H,
                Point::new(119, 0),
                SpriteOpts::default(),
            );
        }
        if self.scroll < self.max_scroll {
            renderer.draw_sprite_raw(
                icons::DOWN_ARROW,
                icons::ARROW_W,
                icons::ARROW_H,
                Point::new(119, 56),
                SpriteOpts::default(),
            );
        }
    }
}

impl Scene for PetInfoScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        Self::build_portrait_poses(&mut self.portrait_poses);
        self.state = State::Browsing;
        self.scroll = 0;
        self.build_content(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        _dt: f32,
    ) -> Option<SceneId> {
        self.handle_input(buttons, ctx)
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        match self.state {
            State::Editing => self.keyboard.draw(renderer),
            State::Browsing => self.draw_browsing(renderer),
        }
    }
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn push_blank(lines: &mut Vec<String<LINE_CAP>, LINES_CAP>) {
    let _ = lines.push(String::new());
}

/// Wrap text to `cpl` chars per line, hyphenating words that exceed it.
fn wrap_full(out: &mut Vec<String<LINE_CAP>, LINES_CAP>, text: &str, cpl: usize) {
    let cpl = cpl.min(LINE_CAP);
    let mut current: String<LINE_CAP> = String::new();
    for raw_word in text.split(' ') {
        let mut word = raw_word;
        // Hyphenation: append fragments before letting the leftover fall
        // through to the normal append path.
        while word.len() > cpl - 1 && cpl > 1 {
            if !current.is_empty() {
                let _ = out.push(current.clone());
                current.clear();
            }
            let mut frag: String<LINE_CAP> = String::new();
            for c in word.chars().take(cpl - 1) {
                let _ = frag.push(c);
            }
            let _ = frag.push('-');
            let _ = out.push(frag);
            word = &word[cpl - 1..];
        }
        let needs_space = !current.is_empty();
        let extra = if needs_space { 1 } else { 0 } + word.len();
        if current.len() + extra <= cpl {
            if needs_space {
                let _ = current.push(' ');
            }
            let _ = current.push_str(word);
        } else {
            if !current.is_empty() {
                let _ = out.push(current.clone());
                current.clear();
            }
            let _ = current.push_str(word);
        }
    }
    let _ = out.push(current);
}

/// Same as `wrap_full` but the first `narrow_lines` lines use `narrow_cpl`
/// while subsequent lines use `full_cpl`.
fn wrap_intro(
    out: &mut Vec<String<LINE_CAP>, LINES_CAP>,
    text: &str,
    narrow_cpl: usize,
    full_cpl: usize,
    narrow_lines: usize,
) {
    let initial_count = out.len();
    let mut current: String<LINE_CAP> = String::new();
    let mut cpl = narrow_cpl.min(LINE_CAP);

    for raw_word in text.split(' ') {
        let mut word = raw_word;
        // Recompute column width if we've crossed the narrow boundary.
        if out.len() - initial_count >= narrow_lines {
            cpl = full_cpl.min(LINE_CAP);
        }
        while word.len() > cpl - 1 && cpl > 1 {
            if !current.is_empty() {
                let _ = out.push(current.clone());
                current.clear();
                if out.len() - initial_count >= narrow_lines {
                    cpl = full_cpl.min(LINE_CAP);
                }
            }
            let mut frag: String<LINE_CAP> = String::new();
            for c in word.chars().take(cpl - 1) {
                let _ = frag.push(c);
            }
            let _ = frag.push('-');
            let _ = out.push(frag);
            word = &word[cpl - 1..];
            if out.len() - initial_count >= narrow_lines {
                cpl = full_cpl.min(LINE_CAP);
            }
        }
        let needs_space = !current.is_empty();
        let extra = if needs_space { 1 } else { 0 } + word.len();
        if current.len() + extra <= cpl {
            if needs_space {
                let _ = current.push(' ');
            }
            let _ = current.push_str(word);
        } else {
            if !current.is_empty() {
                let _ = out.push(current.clone());
                current.clear();
                if out.len() - initial_count >= narrow_lines {
                    cpl = full_cpl.min(LINE_CAP);
                }
            }
            let _ = current.push_str(word);
        }
    }
    if !current.is_empty() {
        let _ = out.push(current);
    }
}

/// Substitute `{s}` (subject pronoun) and `{h}` (possessive pronoun)
/// placeholders in mood-check templates.
fn expand_template(out: &mut String<64>, template: &str, she: &str, her: &str) {
    let mut remaining = template;
    while let Some(idx) = remaining.find('{') {
        let (head, tail) = remaining.split_at(idx);
        let _ = out.push_str(head);
        if tail.starts_with("{s}") {
            let _ = out.push_str(she);
            remaining = &tail[3..];
        } else if tail.starts_with("{h}") {
            let _ = out.push_str(her);
            remaining = &tail[3..];
        } else {
            // Unknown placeholder, pass through literally.
            let _ = out.push('{');
            remaining = &tail[1..];
        }
    }
    let _ = out.push_str(remaining);
}

use crate::i18n::{substitute, to_lower};

/// Returns the dominant trait index (0..5) based on the live trait values.
fn current_temperament(ctx: &GameContext) -> Temperament {
    let values = [
        ctx.courage,
        ctx.loyalty,
        ctx.mischievousness,
        ctx.curiosity,
        ctx.sociability,
    ];
    let mut best = 0usize;
    let mut best_v = values[0];
    for (i, v) in values.iter().enumerate().skip(1) {
        if *v > best_v {
            best_v = *v;
            best = i;
        }
    }
    Temperament::from_dominant_index(best)
}

/// All mood checks, in order.
fn mood_checks(ctx: &GameContext) -> [(f32, f32, &'static str); 18] {
    [
        (ctx.health, 35.0, t!("{s}'s not feeling {h} best.")),
        (ctx.fullness, 30.0, t!("{s}'s feeling hungry.")),
        (ctx.energy, 30.0, t!("{s}'s feeling tired.")),
        (ctx.comfort, 30.0, t!("{s} seems uncomfortable.")),
        (ctx.cleanliness, 25.0, t!("{s}'s feeling grubby.")),
        (ctx.fitness, 20.0, t!("{s}'s feeling sluggish.")),
        (ctx.focus, 25.0, t!("{s}'s feeling scattered.")),
        (ctx.intelligence, 20.0, t!("{s}'s understimulated.")),
        (ctx.curiosity, 25.0, t!("{s}'s feeling bored.")),
        (ctx.playfulness, 30.0, t!("{s}'s not feeling playful.")),
        (ctx.affection, 25.0, t!("{s} wants more attention.")),
        (ctx.fulfillment, 25.0, t!("{s}'s feeling unfulfilled.")),
        (ctx.serenity, 25.0, t!("{s} seems restless.")),
        (ctx.sociability, 20.0, t!("{s}'s been withdrawn.")),
        (ctx.courage, 20.0, t!("{s}'s been timid lately.")),
        (ctx.loyalty, 20.0, t!("{s} seems detached.")),
        (ctx.mischievousness, 20.0, t!("{s}'s been very subdued.")),
        (ctx.maturity, 20.0, t!("{s}'s been impulsive.")),
    ]
}

/// Count the most-repeated meal in `ctx.recent_meals`.
fn recent_meal_dominance(ctx: &GameContext) -> u8 {
    let meals = &ctx.recent_meals;
    if meals.len() < 4 {
        return 0;
    }
    let mut best: u8 = 0;
    for kind in [
        FoodKind::Kibble,
        FoodKind::WetFood,
        FoodKind::Treat,
        FoodKind::Fish,
        FoodKind::CaughtSnack,
    ] {
        let count = meals
            .iter()
            .filter(|m| match **m {
                MealEntry::Item(item) => item.kind() == kind,
                MealEntry::CaughtSnack => kind == FoodKind::CaughtSnack,
            })
            .count() as u8;
        if count > best {
            best = count;
        }
    }
    best
}

// ----------------------------------------------------------------------
// Display labels — return the translated label as-is. Callers lowercase
// at the use-site so embedded forms ("favorite meal is kibble") work in
// languages whose translated label differs in form from the source key.
// ----------------------------------------------------------------------

fn food_label(item: Option<FoodItem>) -> &'static str {
    match item {
        None => "?",
        Some(i) => i.label(),
    }
}

fn toy_label(toy: Option<ToyVariant>) -> &'static str {
    match toy {
        None => "?",
        Some(t) => t.label(),
    }
}

fn location_label(loc: Option<SceneId>) -> &'static str {
    match loc {
        Some(SceneId::Inside) => t!("Living Room"),
        Some(SceneId::Bedroom) => t!("Bedroom"),
        Some(SceneId::Kitchen) => t!("Kitchen"),
        Some(SceneId::Outside) => t!("Outside"),
        Some(SceneId::Treehouse) => t!("Treehouse"),
        _ => "?",
    }
}

fn weather_label(w: Option<FavWeather>) -> &'static str {
    match w {
        None => "?",
        Some(FavWeather::Sunny) => t!("sunny"),
        Some(FavWeather::Rainy) => t!("rainy"),
        Some(FavWeather::Snowy) => t!("snowy"),
        Some(FavWeather::Overcast) => t!("cloudy"),
    }
}
