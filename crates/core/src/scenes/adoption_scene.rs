//! Adoption scene: first-run cat selection and onboarding.
//!
//! States:
//!   Grid:    2x2 grid of candidate cats; D-pad to navigate, A to inspect
//!   Profile: per-cat info card; B = back, A = adopt
//!   Confirm: confirmation prompt; B = back, A = confirm
//!   Naming:  on-screen keyboard for naming the cat
//!   Moment:  adoption moment, cat walks in, shows love bubble, fades to inside


use embedded_graphics::prelude::{Point, Size};
use heapless::{String, Vec};
use crate::t;
use crate::i18n::{substitute, to_lower};

use crate::{
    assets::{character::PoseId, icons},
    character::{draw_pose, PoseAnim},
    context::{FavWeather, GameContext, PET_NAME_MAX},
    input::{Button, Buttons},
    pet_names,
    pet_seed::{derive_favorites, derive_trait_offsets, DerivedFavorites, PetGender, StarSign, Temperament},
    rand,
    render::{Renderer, SpriteOpts},
    scene::{Scene, SceneId},
    ui::{
        bubble::{self, BubbleIcon},
        keyboard::{Charset, OnScreenKeyboard},
        popup::Popup,
    },
};

const TITLE_H: i32 = 9;
const CELL_W: i32 = 64;
const CELL_H: i32 = 27;
const PROFILE_TEXT_CAP: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Grid,
    Profile,
    Confirm,
    Naming,
    Moment,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MomentPhase {
    Walking,
    Sitting,
}

#[derive(Clone, Copy)]
struct Candidate {
    seed: u64,
    favs: DerivedFavorites,
    offsets: [i32; 5],
    name: &'static str,
    /// Index into `portrait_poses`, the pose to render in the grid cell.
    portrait_idx: usize,
}

pub struct AdoptionScene {
    state: State,
    selected: usize,
    viewing: usize,
    candidates: Vec<Candidate, 4>,
    portrait_poses: Vec<PoseId, 64>,
    popup: Popup,
    keyboard: OnScreenKeyboard,
    moment_phase: MomentPhase,
    moment_x: f32,
    moment_timer: f32,
    bubble_prog: f32,
    moment_pose: PoseId,
    moment_anim: PoseAnim,
    /// Guards the post-adoption save against repeat-firing while the
    /// scene-transition fade plays. `update_moment` keeps returning
    /// `Some(Inside)` for several frames once its timer elapses.
    saved: bool,
}

impl AdoptionScene {
    pub fn new() -> Self {
        Self {
            state: State::Grid,
            selected: 0,
            viewing: 0,
            candidates: Vec::new(),
            portrait_poses: Vec::new(),
            popup: Popup::new(0, 0, 128, 55),
            keyboard: OnScreenKeyboard::new(Charset::Full, PET_NAME_MAX),
            moment_phase: MomentPhase::Walking,
            moment_x: 0.0,
            moment_timer: 0.0,
            saved: false,
            bubble_prog: 0.0,
            moment_pose: PoseId::WalkingSideNeutral,
            moment_anim: PoseAnim::new(1),
        }
    }

    fn build_portrait_poses(out: &mut Vec<PoseId, 64>) {
        out.clear();
        for &id in crate::assets::character::ALL_POSES {
            let name = id.name();
            if name.starts_with("costume_sitting.") || name.starts_with("sitting_back.") {
                continue;
            }
            let pose = id.data();
            // Every pose has a head, but eyes are optional; require both.
            if pose.eyes.is_some() {
                let _ = out.push(id);
            }
        }
    }

    fn generate_candidates(&mut self, ctx: &mut GameContext) {
        self.candidates.clear();
        let portraits = &self.portrait_poses;
        if portraits.is_empty() {
            return;
        }

        // Track which (head_ptr, eyes_ptr) and (temperament, star_sign) combos
        // are already taken so each candidate feels distinct.
        let mut used_portraits: Vec<(usize, usize), 4> = Vec::new();
        let mut used_tom_id: Vec<(usize, StarSign), 2> = Vec::new();
        let mut used_queen_id: Vec<(usize, StarSign), 2> = Vec::new();
        let mut toms: Vec<Candidate, 2> = Vec::new();
        let mut queens: Vec<Candidate, 2> = Vec::new();

        // Cap iterations. At 12 FPS we don't want a runaway loop if the
        // entropy/uniqueness conditions can't be satisfied.
        for _ in 0..1024 {
            if toms.len() >= 2 && queens.len() >= 2 {
                break;
            }
            let seed = next_seed(ctx);
            let favs = derive_favorites(seed);
            let offs = derive_trait_offsets(seed);
            let portrait_idx = ((seed >> 40) as usize) % portraits.len();
            let pose_id = portraits[portrait_idx];
            let pose = pose_id.data();
            let head_ptr = pose.head as *const _ as usize;
            let eyes_ptr = pose.eyes.map(|e| e as *const _ as usize).unwrap_or(0);
            let portrait_key = (head_ptr, eyes_ptr);

            let dom = dominant_trait_index(&offs);
            let identity = (dom, favs.star_sign);
            let used_id = match favs.pet_gender {
                PetGender::Tom => &mut used_tom_id,
                PetGender::Queen => &mut used_queen_id,
            };
            if used_portraits.iter().any(|p| *p == portrait_key)
                || used_id
                    .iter()
                    .any(|(d, s)| *d == identity.0 && *s == identity.1)
            {
                continue;
            }

            let gendered = match favs.pet_gender {
                PetGender::Tom => pet_names::TOM_NAMES,
                PetGender::Queen => pet_names::QUEEN_NAMES,
            };
            let name = pet_names::pick_name(seed, gendered);
            let cand = Candidate {
                seed,
                favs,
                offsets: offs,
                name,
                portrait_idx,
            };

            match favs.pet_gender {
                PetGender::Tom if toms.len() < 2 => {
                    let _ = toms.push(cand);
                    let _ = used_portraits.push(portrait_key);
                    let _ = used_tom_id.push(identity);
                }
                PetGender::Queen if queens.len() < 2 => {
                    let _ = queens.push(cand);
                    let _ = used_portraits.push(portrait_key);
                    let _ = used_queen_id.push(identity);
                }
                _ => {}
            }
        }

        // Arrange [tom0, queen0, tom1, queen1] then Fisher-Yates shuffle.
        let mut order: Vec<Candidate, 4> = Vec::new();
        if let Some(c) = toms.first() {
            let _ = order.push(*c);
        }
        if let Some(c) = queens.first() {
            let _ = order.push(*c);
        }
        if let Some(c) = toms.get(1) {
            let _ = order.push(*c);
        }
        if let Some(c) = queens.get(1) {
            let _ = order.push(*c);
        }
        for i in (1..order.len()).rev() {
            let j = rand::rand_range_u32(&mut ctx.rng, 0, i as u32) as usize;
            order.swap(i, j);
        }
        self.candidates = order;
    }

    fn open_profile(&mut self) {
        if let Some(cand) = self.candidates.get(self.viewing) {
            let text = profile_text(cand);
            self.popup.set_text(text.as_str(), true, false);
        }
    }

    fn open_naming(&mut self) {
        if let Some(cand) = self.candidates.get(self.viewing) {
            self.keyboard.open(cand.name);
        }
        self.state = State::Naming;
    }

    fn do_adopt(&mut self, ctx: &mut GameContext, name: &str) {
        let Some(cand) = self.candidates.get(self.viewing).copied() else {
            return;
        };
        ctx.pet_seed = cand.seed;
        ctx.pet_gender = Some(cand.favs.pet_gender);
        ctx.star_sign = Some(cand.favs.star_sign);
        ctx.fav_weather = Some(cand.favs.fav_weather);
        ctx.fav_meal = Some(cand.favs.fav_meal);
        ctx.least_fav_meal = Some(cand.favs.least_fav_meal);
        ctx.fav_snack = Some(cand.favs.fav_snack);
        ctx.least_fav_snack = Some(cand.favs.least_fav_snack);
        ctx.fav_toy = Some(cand.favs.fav_toy);
        ctx.least_fav_toy = Some(cand.favs.least_fav_toy);
        ctx.fav_location = Some(cand.favs.fav_location);
        ctx.least_fav_location = Some(cand.favs.least_fav_location);

        // Personality trait offsets centred on 50.
        let traits: [&mut f32; 5] = [
            &mut ctx.courage,
            &mut ctx.loyalty,
            &mut ctx.mischievousness,
            &mut ctx.curiosity,
            &mut ctx.sociability,
        ];
        for (slot, off) in traits.into_iter().zip(cand.offsets.iter()) {
            *slot = 50.0 + *off as f32;
        }

        // Pet name (trimmed; fallback to "Cat" when empty).
        let trimmed = name.trim();
        ctx.pet_name.clear();
        let chosen = if trimmed.is_empty() { "Cat" } else { trimmed };
        for c in chosen.chars().take(PET_NAME_MAX) {
            if ctx.pet_name.push(c).is_err() {
                break;
            }
        }

        ctx.first_impressions = true;
        ctx.milestone_fed = false;
        ctx.milestone_petted = false;
        ctx.milestone_played = false;
        ctx.milestone_groomed = false;
        ctx.milestone_store = false;

        // TODO(save_load): once the save layer exists, call `backup::write_adoption`
        // with the full candidate list + chosen seed + name.

        // Kick off the adoption moment.
        self.moment_x = -20.0;
        self.moment_timer = 0.0;
        self.bubble_prog = 0.0;
        self.moment_phase = MomentPhase::Walking;
        self.moment_pose = PoseId::WalkingSideNeutral;
        self.moment_anim = PoseAnim::new(seed_to_u32(cand.seed));
        self.moment_anim.reseed_for(self.moment_pose.data());
        self.state = State::Moment;
    }

    // ------------------------------------------------------------------
    // Input
    // ------------------------------------------------------------------

    fn input_grid(&mut self, buttons: &mut Buttons) -> Option<SceneId> {
        let mut col = self.selected % 2;
        let mut row = self.selected / 2;
        if buttons.was_just_pressed(Button::Up) || buttons.was_just_pressed(Button::Down) {
            row = 1 - row;
        } else if buttons.was_just_pressed(Button::Left)
            || buttons.was_just_pressed(Button::Right)
        {
            col = 1 - col;
        }
        self.selected = (row * 2 + col).min(self.candidates.len().saturating_sub(1));

        if buttons.was_just_pressed(Button::A) {
            self.viewing = self.selected;
            self.open_profile();
            self.state = State::Profile;
        }
        None
    }

    fn input_profile(&mut self, buttons: &mut Buttons) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::B) {
            self.state = State::Grid;
        } else if buttons.was_just_pressed(Button::A) {
            self.state = State::Confirm;
        } else if buttons.was_just_pressed(Button::Down) {
            self.popup.scroll_down();
        } else if buttons.was_just_pressed(Button::Up) {
            self.popup.scroll_up();
        }
        None
    }

    fn input_confirm(&mut self, buttons: &mut Buttons) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::A) {
            self.open_naming();
        } else if buttons.was_just_pressed(Button::B) {
            self.state = State::Profile;
        }
        None
    }

    fn input_naming(&mut self, buttons: &mut Buttons, ctx: &mut GameContext) -> Option<SceneId> {
        if let Some(name) = self.keyboard.handle_input(buttons) {
            self.do_adopt(ctx, name.as_str());
        }
        None
    }

    fn update_moment(&mut self, dt: f32) -> Option<SceneId> {
        match self.moment_phase {
            MomentPhase::Walking => {
                self.moment_x += 30.0 * dt;
                self.moment_anim.update(self.moment_pose.data(), dt);
                if self.moment_x >= 64.0 {
                    self.moment_x = 64.0;
                    self.moment_phase = MomentPhase::Sitting;
                    self.moment_timer = 0.0;
                    self.bubble_prog = 0.0;
                    self.moment_pose = PoseId::SittingForwardHappy;
                    self.moment_anim.reseed_for(self.moment_pose.data());
                }
            }
            MomentPhase::Sitting => {
                self.moment_timer += dt;
                self.bubble_prog = (self.moment_timer / 4.5).min(1.0);
                self.moment_anim.update(self.moment_pose.data(), dt);
                if self.moment_timer >= 5.0 {
                    return Some(SceneId::Inside);
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Draw
    // ------------------------------------------------------------------

    fn draw_grid(&self, renderer: &mut Renderer) {
        let title = t!("Adoptable Pets");
        let title_x = (128 - title.len() as i32 * 8) / 2;
        renderer.draw_text(title, Point::new(title_x, 0));

        for (i, cand) in self.candidates.iter().enumerate() {
            let col = (i % 2) as i32;
            let row = (i / 2) as i32;
            let cx = col * CELL_W;
            let cy = TITLE_H + row * CELL_H;
            self.draw_cell(renderer, cand, cx, cy, i == self.selected);
        }
    }

    fn draw_cell(&self, renderer: &mut Renderer, cand: &Candidate, cx: i32, cy: i32, selected: bool) {
        let pose_id = self.portrait_poses[cand.portrait_idx];
        let pose = pose_id.data();
        let head = pose.head;
        let head_w = head.sprite.width as i32;
        let hx = cx + (CELL_W - head_w) / 2;
        let hy = cy + 1;
        renderer.draw_sprite(
            &head.sprite,
            Point::new(hx, hy),
            SpriteOpts::default(),
        );
        if let Some(eyes) = pose.eyes {
            let ex = hx + head.eye_x as i32 - eyes.anchor_x as i32;
            let ey = hy + head.eye_y as i32 - eyes.anchor_y as i32;
            renderer.draw_sprite(&eyes.sprite, Point::new(ex, ey), SpriteOpts::default());
        }

        let gender_icon: &[u8] = match cand.favs.pet_gender {
            PetGender::Tom => icons::TOM_ICON,
            PetGender::Queen => icons::QUEEN_ICON,
        };
        renderer.draw_sprite_raw(
            gender_icon,
            icons::GENDER_W,
            icons::GENDER_H,
            Point::new(cx + CELL_W - 12, cy + CELL_H - 12),
            SpriteOpts::default(),
        );

        renderer.draw_rect(
            Point::new(cx, cy),
            Size::new((CELL_W - 1) as u32, (CELL_H - 1) as u32),
            false,
        );
        if selected {
            renderer.draw_rect(
                Point::new(cx + 1, cy + 1),
                Size::new((CELL_W - 3) as u32, (CELL_H - 3) as u32),
                false,
            );
        }
    }

    fn draw_profile(&self, renderer: &mut Renderer) {
        self.popup.draw(renderer, true);
        renderer.draw_text(t!("[A]Adopt [B]Back"), Point::new(0, 56));
    }

    fn draw_confirm(&self, renderer: &mut Renderer) {
        // Same look as `ui::menu::draw_confirm_dialog`.
        renderer.fill_rect_off(Point::new(5, 13), Size::new(118, 38));
        renderer.draw_rect(Point::new(4, 12), Size::new(120, 40), false);
        let q = t!("Is this the cat you want to adopt?");
        wrap_and_draw_confirm(renderer, q);
        renderer.draw_text(t!("[A]Yes [B]No"), Point::new(20, 42));
    }

    fn draw_moment(&self, renderer: &mut Renderer) {
        let pos = Point::new(self.moment_x as i32, 55);
        match self.moment_phase {
            MomentPhase::Walking => {
                draw_pose(renderer, self.moment_pose.data(), &self.moment_anim, pos, true, None);
            }
            MomentPhase::Sitting => {
                draw_pose(renderer, self.moment_pose.data(), &self.moment_anim, pos, false, None);
                bubble::draw_above_char(
                    renderer,
                    BubbleIcon::Exclaim,
                    self.moment_x as i32,
                    55,
                    self.bubble_prog,
                    false,
                );
            }
        }
    }
}

impl Scene for AdoptionScene {
    fn enter(&mut self, ctx: &mut GameContext) {
        Self::build_portrait_poses(&mut self.portrait_poses);
        self.generate_candidates(ctx);
        self.state = State::Grid;
        self.selected = 0;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        match self.state {
            State::Grid => self.input_grid(buttons),
            State::Profile => self.input_profile(buttons),
            State::Confirm => self.input_confirm(buttons),
            State::Naming => self.input_naming(buttons, ctx),
            State::Moment => {
                let next = self.update_moment(dt);
                // Persist the newly-adopted pet the moment the bonding
                // sequence finishes, so a power cycle immediately after
                // adoption doesn't lose them. `update_moment` keeps
                // returning `Some(Inside)` until the transition midpoint
                // actually swaps the scene; `saved` makes this idempotent.
                if next == Some(SceneId::Inside) && !self.saved {
                    crate::save::save(ctx);
                    self.saved = true;
                }
                next
            }
        }
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        match self.state {
            State::Grid => self.draw_grid(renderer),
            State::Profile => self.draw_profile(renderer),
            State::Confirm => self.draw_confirm(renderer),
            State::Naming => self.keyboard.draw(renderer),
            State::Moment => self.draw_moment(renderer),
        }
    }
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

/// Pull a fresh 64-bit seed from the HW RNG.
fn next_seed(ctx: &mut GameContext) -> u64 {
    let mut buf = [0u8; 8];
    ctx.hw_rng.read(&mut buf);
    u64::from_be_bytes(buf)
}

fn seed_to_u32(seed: u64) -> u32 {
    ((seed ^ (seed >> 32)) as u32).max(1)
}

fn dominant_trait_index(offs: &[i32; 5]) -> usize {
    let mut best = 0usize;
    let mut best_v = offs[0];
    for (i, v) in offs.iter().enumerate().skip(1) {
        if *v > best_v {
            best_v = *v;
            best = i;
        }
    }
    best
}

fn fav_weather_label(w: FavWeather) -> &'static str {
    match w {
        FavWeather::Sunny => t!("sunny"),
        FavWeather::Rainy => t!("rainy"),
        FavWeather::Snowy => t!("snowy"),
        FavWeather::Overcast => t!("cloudy"),
    }
}

fn profile_text(cand: &Candidate) -> String<PROFILE_TEXT_CAP> {
    let possessive = match cand.favs.pet_gender {
        PetGender::Tom => t!("His"),
        PetGender::Queen => t!("Her"),
    };
    let possessive_lower: String<8> = to_lower(possessive);
    // Displayed sign uses `seed % 12`, not the stored `cand.favs.star_sign`.
    let displayed_sign = StarSign::from_index(cand.seed as u32 % 12);
    let temper = Temperament::from_dominant_index(dominant_trait_index(&cand.offsets));
    let weather = fav_weather_label(cand.favs.fav_weather);

    let temper_lower: String<24> = to_lower(temper.label());
    let sign_lower: String<24> = to_lower(displayed_sign.label());
    let meal_lower: String<24> = to_lower(food_label(cand.favs.fav_meal));
    let snack_lower: String<24> = to_lower(food_label(cand.favs.fav_snack));
    let toy_lower: String<24> = to_lower(toy_label(cand.favs.fav_toy));

    let mut s: String<PROFILE_TEXT_CAP> = String::new();
    substitute(
        &mut s,
        t!("{name} is a {temper} {sign}, who loves {weather} weather."),
        &[
            ("name", cand.name),
            ("temper", temper_lower.as_str()),
            ("sign", sign_lower.as_str()),
            ("weather", weather),
        ],
    );
    let _ = s.push('\n');
    substitute(
        &mut s,
        t!("{possessive} favorite meal is {meal}. {possessive} favorite snack is {snack}. And {possessive_lower} favorite toy is the {toy}."),
        &[
            ("possessive", possessive),
            ("meal", meal_lower.as_str()),
            ("snack", snack_lower.as_str()),
            ("possessive_lower", possessive_lower.as_str()),
            ("toy", toy_lower.as_str()),
        ],
    );
    s
}

fn food_label(item: crate::context::FoodItem) -> &'static str {
    item.label()
}

fn toy_label(toy: crate::context::ToyVariant) -> &'static str {
    toy.label()
}

/// Word-wrap a confirm prompt to ~14 chars/line and draw inside the dialog
/// frame at (8, 14..). Same metrics as `ui::menu`.
fn wrap_and_draw_confirm(renderer: &mut Renderer, text: &str) {
    const CHARS_PER_LINE: usize = 14;
    const LINE_BUF: usize = 24;
    let mut current: String<LINE_BUF> = String::new();
    let mut line_idx: i32 = 0;
    let flush =
        |line: &mut String<LINE_BUF>, line_idx: &mut i32, renderer: &mut Renderer| {
            if !line.is_empty() {
                renderer.draw_text(line.as_str(), Point::new(8, 14 + *line_idx * 8));
                *line_idx += 1;
                line.clear();
            }
        };
    for word in text.split(' ') {
        let needs_space = !current.is_empty();
        let extra = if needs_space { 1 } else { 0 } + word.len();
        if current.len() + extra <= CHARS_PER_LINE {
            if needs_space {
                let _ = current.push(' ');
            }
            let _ = current.push_str(word);
        } else {
            flush(&mut current, &mut line_idx, renderer);
            let _ = current.push_str(&word[..word.len().min(CHARS_PER_LINE)]);
        }
    }
    flush(&mut current, &mut line_idx, renderer);
}
