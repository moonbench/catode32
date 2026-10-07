use embedded_graphics::prelude::Point;

use crate::{
    assets::{
        character::PoseId,
        items::{
            CHEW_STICKS, FOOD_BOWL, FOOD_BOWL_ALT, FOOD_BOWL_KIBBLE, MOUSE_TOY, SNACK_PUREE,
            TREAT_PILE,
        },
    },
    behavior::{Behavior, BehaviorId, BehaviorState, EatingSource, NextBehavior},
    context::{FoodItem, GameContext, MealEntry, StatId},
    entities::character::Character,
    rand,
    render::{Renderer, Sprite, SpriteOpts},
};

const FOOD_OFFSET_X: i32 = 34;
const REJECTION_LOOK_DURATION: f32 = 4.5;
const LOWER_DURATION: f32 = 1.0;
const PAUSE_DURATION: f32 = 1.5;

const REJECTION_POSES: &[PoseId] = &[
    PoseId::StandingSideNeutralLookingDown,
    PoseId::SittingSideLookingDown,
    PoseId::LayingSideNeutral2,
    PoseId::LayingSideBored,
    PoseId::SittingSillySideNeutral,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Lowering,
    PreEating,
    Eating,
    PostEating,
    Rejecting,
}

pub struct EatingBehavior {
    source: EatingSource,
    sprite: &'static Sprite,
    eating_speed: f32,
    is_snack: bool,
    phase: Phase,
    phase_timer: f32,
    food_y_progress: f32,
    food_frame: f32,
    pose_id: PoseId,
    rejecting: bool,
}

impl EatingBehavior {
    pub fn new(source: EatingSource) -> Self {
        let cfg = food_cfg(source);
        Self {
            source,
            sprite: cfg.sprite,
            eating_speed: cfg.eating_speed,
            is_snack: cfg.is_snack,
            phase: Phase::Lowering,
            phase_timer: 0.0,
            food_y_progress: 0.0,
            food_frame: 0.0,
            pose_id: PoseId::StandingSideHappy,
            rejecting: false,
        }
    }

    /// Linear ramp from 0% at `start` to 100% at fullness=100. Midpoint of the
    /// ramp is offset by appeal: 76 + appeal*8 for meals, 86 + appeal*8 for
    /// snacks, so meals are refused more readily than treats at the same
    /// fullness level.
    fn rejection_chance(fullness: f32, appeal: f32, is_snack: bool) -> f32 {
        let base = if is_snack { 86.0 } else { 76.0 };
        let midpoint = base + appeal * 8.0;
        let start = 2.0 * midpoint - 100.0;
        if fullness <= start {
            0.0
        } else {
            ((fullness - start) / (100.0 - start)).min(1.0)
        }
    }
}

impl Behavior for EatingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Eating
    }
    fn progress(&self) -> f32 {
        let n = self.sprite.frames.len().max(1) as f32;
        (self.food_frame / n).clamp(0.0, 1.0)
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::Lowering;
        self.phase_timer = 0.0;
        self.food_y_progress = 0.0;
        self.food_frame = 0.0;

        let cfg = food_cfg(self.source);
        let appeal = cfg.appeal;
        let chance = Self::rejection_chance(ctx.fullness, appeal, self.is_snack);
        self.rejecting = rand::rand_f32(&mut ctx.rng) < chance;

        self.pose_id = PoseId::StandingSideHappy;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Lowering => {
                self.food_y_progress = (self.phase_timer / LOWER_DURATION).min(1.0);
                if self.phase_timer >= LOWER_DURATION {
                    self.phase_timer = 0.0;
                    if self.rejecting {
                        self.phase = Phase::Rejecting;
                        let i = rand::rand_range_u32(
                            &mut ctx.rng,
                            0,
                            (REJECTION_POSES.len() as u32) - 1,
                        ) as usize;
                        self.pose_id = REJECTION_POSES[i];
                    } else {
                        self.phase = Phase::PreEating;
                        self.pose_id = PoseId::LeaningForwardSideNeutral;
                    }
                }
            }
            Phase::PreEating => {
                if self.phase_timer >= PAUSE_DURATION {
                    self.phase = Phase::Eating;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::LeaningForwardSideEating;
                }
            }
            Phase::Eating => {
                let n = self.sprite.frames.len() as f32;
                self.food_frame += dt * self.eating_speed;
                if self.food_frame >= n {
                    self.food_frame = n;
                    self.phase = Phase::PostEating;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::LeaningForwardSideNeutral;
                }
            }
            Phase::Rejecting => {
                if self.phase_timer >= REJECTION_LOOK_DURATION {
                    return BehaviorState::Completed;
                }
            }
            Phase::PostEating => {
                if self.phase_timer >= PAUSE_DURATION {
                    // Kitchen-location burst (Python apply_location_bonus).
                    if !self.rejecting
                        && matches!(ctx.last_main_scene, crate::scene::SceneId::Kitchen)
                    {
                        let is_fav = if let EatingSource::Item(item) = self.source {
                            ctx.fav_meal == Some(item) || ctx.fav_snack == Some(item)
                        } else {
                            false
                        };
                        let count = if is_fav { 8 } else { 5 };
                        character.play_bursts(&mut ctx.rng, count);
                    }
                    return BehaviorState::Completed;
                }
            }
        }
        BehaviorState::Running
    }

    fn next(&self, _ctx: &GameContext) -> Option<NextBehavior> {
        if self.rejecting {
            Some(NextBehavior::Meandering)
        } else {
            None
        }
    }

    fn exit(&mut self, ctx: &mut GameContext, completed: bool) {
        if completed && !self.rejecting {
            ctx.milestone_fed = true;
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        if self.rejecting {
            return;
        }

        let cfg = food_cfg(self.source);
        let mut bonus = (cfg.bonus_table)();

        // Variety penalty: scale all non-exempt stats by the repeat multiplier.
        let entry = source_to_meal_entry(self.source);
        let repeat_count = ctx
            .recent_meals
            .iter()
            .filter(|m| **m == entry)
            .count() as f32;
        if repeat_count > 0.0 {
            let multiplier = (1.0 - repeat_count * 0.18 * (1.0 - cfg.appeal)).max(0.25);
            for (stat, val) in bonus.iter_mut() {
                if !matches!(stat, StatId::Fullness | StatId::Energy) {
                    *val *= multiplier;
                }
            }
        }

        // Meals (not snacks, not caught) award loyalty.
        if !self.is_snack && !matches!(self.source, EatingSource::CaughtSnack) {
            let loyalty = if repeat_count == 0.0 { 0.5 } else { 0.15 };
            add_to(&mut bonus, StatId::Loyalty, loyalty);
        }

        // Being fed by the owner is a small social interaction.
        if !matches!(self.source, EatingSource::CaughtSnack) {
            add_to(&mut bonus, StatId::Sociability, 0.1);
        }

        ctx.record_meal(entry);

        // Snack streak adds sickness once you've eaten 4+ snacks in the
        // recent-meals window.
        if self.is_snack {
            let recent_snacks: heapless::Vec<MealEntry, 8> = ctx
                .recent_meals
                .iter()
                .copied()
                .filter(|m| matches!(m, MealEntry::Item(it) if it.is_snack()))
                .collect();
            let n = recent_snacks.len();
            if n >= 4 {
                let mut sum = 0.0;
                for m in &recent_snacks {
                    if let MealEntry::Item(it) = m {
                        sum += unhealthiness(*it);
                    }
                }
                let avg = sum / n as f32;
                let base = if n >= 5 { 2.0 } else { 1.0 };
                ctx.sickness = (ctx.sickness + base * avg).min(10.0);
            }
        }

        // Kitchen location bonus: 1.2x to fullness/energy.
        if matches!(ctx.last_main_scene, crate::scene::SceneId::Kitchen) {
            scale(&mut bonus, StatId::Fullness, 1.2);
            scale(&mut bonus, StatId::Energy, 1.2);
        }

        // Favorite / least-favorite affection multiplier.
        if let EatingSource::Item(item) = self.source {
            let (fav, dislike) = if self.is_snack {
                (ctx.fav_snack, ctx.least_fav_snack)
            } else {
                (ctx.fav_meal, ctx.least_fav_meal)
            };
            if fav == Some(item) {
                scale(&mut bonus, StatId::Affection, 1.2);
            } else if dislike == Some(item) {
                scale(&mut bonus, StatId::Affection, 0.85);
            }
        }

        for (_, val) in bonus.iter_mut() {
            *val *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _ctx: &GameContext,
        char_screen: Point,
        mirror_h: bool,
    ) {
        let food_w = self.sprite.width as i32;
        let food_h = self.sprite.height as i32;

        let ground_y = char_screen.y - food_h;
        let start_y = ground_y - 40;
        let food_y =
            (start_y as f32 + (ground_y - start_y) as f32 * self.food_y_progress) as i32;

        let food_x = if mirror_h {
            char_screen.x + FOOD_OFFSET_X - food_w / 2
        } else {
            char_screen.x - FOOD_OFFSET_X - food_w / 2
        };

        let frame =
            (self.food_frame as usize).min(self.sprite.frames.len().saturating_sub(1));
        renderer.draw_sprite(
            self.sprite,
            Point::new(food_x, food_y),
            SpriteOpts {
                frame,
                ..Default::default()
            },
        );
    }
}

// --- per-item config ----------------------------------------------------

struct EatCfg {
    sprite: &'static Sprite,
    eating_speed: f32,
    appeal: f32,
    is_snack: bool,
    bonus_table: fn() -> Bonus,
}

type Bonus = heapless::Vec<(StatId, f32), 8>;

fn food_cfg(source: EatingSource) -> EatCfg {
    match source {
        EatingSource::CaughtSnack => EatCfg {
            sprite: &MOUSE_TOY,
            eating_speed: 0.5,
            appeal: 1.0,
            is_snack: false,
            bonus_table: || {
                let mut b: Bonus = heapless::Vec::new();
                let _ = b.push((StatId::Fullness, 20.0));
                b
            },
        },
        EatingSource::Item(item) => match item {
            FoodItem::Kibble => EatCfg {
                sprite: &FOOD_BOWL_KIBBLE,
                eating_speed: 0.45,
                appeal: 0.2,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 45.0));
                    let _ = b.push((StatId::Energy, 2.0));
                    let _ = b.push((StatId::Fitness, 2.0));
                    b
                },
            },
            FoodItem::Cod => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.4,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 45.0));
                    let _ = b.push((StatId::Energy, 2.0));
                    let _ = b.push((StatId::Affection, 2.0));
                    let _ = b.push((StatId::Curiosity, 1.0));
                    b
                },
            },
            FoodItem::Haddock => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.5,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 45.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Affection, 2.0));
                    b
                },
            },
            FoodItem::Trout => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.6,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 45.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Affection, 3.0));
                    b
                },
            },
            FoodItem::Shrimp => EatCfg {
                sprite: &FOOD_BOWL_KIBBLE,
                eating_speed: 0.5,
                appeal: 0.65,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 25.0));
                    let _ = b.push((StatId::Energy, 2.0));
                    let _ = b.push((StatId::Affection, 4.0));
                    let _ = b.push((StatId::Playfulness, 3.0));
                    let _ = b.push((StatId::Curiosity, 2.0));
                    b
                },
            },
            FoodItem::Herring => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.5,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 45.0));
                    let _ = b.push((StatId::Energy, 4.0));
                    let _ = b.push((StatId::Fitness, 2.0));
                    let _ = b.push((StatId::Affection, 2.0));
                    b
                },
            },
            FoodItem::Turkey => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.4,
                appeal: 0.6,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 50.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Fitness, 3.0));
                    let _ = b.push((StatId::Serenity, 3.0));
                    b
                },
            },
            FoodItem::Tuna => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.9,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 40.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Affection, 7.0));
                    let _ = b.push((StatId::Playfulness, 2.0));
                    let _ = b.push((StatId::Mischievousness, 1.0));
                    b
                },
            },
            FoodItem::Salmon => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.8,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 50.0));
                    let _ = b.push((StatId::Energy, 5.0));
                    let _ = b.push((StatId::Fitness, 4.0));
                    let _ = b.push((StatId::Affection, 2.0));
                    b
                },
            },
            FoodItem::Chicken => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.5,
                appeal: 0.7,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 55.0));
                    let _ = b.push((StatId::Energy, 6.0));
                    let _ = b.push((StatId::Affection, 4.0));
                    b
                },
            },
            FoodItem::Liver => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.65,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 55.0));
                    let _ = b.push((StatId::Energy, 5.0));
                    let _ = b.push((StatId::Fitness, 5.0));
                    let _ = b.push((StatId::Affection, 3.0));
                    b
                },
            },
            FoodItem::Beef => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.7,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 55.0));
                    let _ = b.push((StatId::Energy, 4.0));
                    let _ = b.push((StatId::Fitness, 2.0));
                    let _ = b.push((StatId::Affection, 4.0));
                    let _ = b.push((StatId::Comfort, 2.0));
                    b
                },
            },
            FoodItem::Lamb => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.45,
                appeal: 0.85,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 55.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Affection, 4.0));
                    let _ = b.push((StatId::Comfort, 4.0));
                    let _ = b.push((StatId::Serenity, 2.0));
                    b
                },
            },
            // Mackerel uses the default config: fullness 8, eating_speed 0.4, appeal 0.5.
            FoodItem::Mackerel => EatCfg {
                sprite: &FOOD_BOWL,
                eating_speed: 0.4,
                appeal: 0.5,
                is_snack: false,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 8.0));
                    b
                },
            },
            FoodItem::Carrots => EatCfg {
                sprite: &CHEW_STICKS,
                eating_speed: 1.0,
                appeal: 0.15,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 2.0));
                    let _ = b.push((StatId::Affection, 1.0));
                    b
                },
            },
            FoodItem::Pumpkin => EatCfg {
                sprite: &TREAT_PILE,
                eating_speed: 1.0,
                appeal: 0.25,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 3.0));
                    let _ = b.push((StatId::Fitness, 2.0));
                    b
                },
            },
            FoodItem::Treats => EatCfg {
                sprite: &TREAT_PILE,
                eating_speed: 1.25,
                appeal: 0.75,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 2.0));
                    let _ = b.push((StatId::Affection, 3.0));
                    let _ = b.push((StatId::Playfulness, 1.0));
                    b
                },
            },
            FoodItem::FishBite => EatCfg {
                sprite: &TREAT_PILE,
                eating_speed: 1.25,
                appeal: 0.8,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 4.0));
                    let _ = b.push((StatId::Affection, 2.0));
                    let _ = b.push((StatId::Playfulness, 1.0));
                    let _ = b.push((StatId::Curiosity, 2.0));
                    b
                },
            },
            FoodItem::Eggs => EatCfg {
                sprite: &TREAT_PILE,
                eating_speed: 1.25,
                appeal: 0.5,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 8.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Fitness, 3.0));
                    b
                },
            },
            FoodItem::Nugget => EatCfg {
                sprite: &TREAT_PILE,
                eating_speed: 1.25,
                appeal: 0.55,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 12.0));
                    let _ = b.push((StatId::Energy, 3.0));
                    let _ = b.push((StatId::Affection, 2.0));
                    b
                },
            },
            FoodItem::Milk => EatCfg {
                sprite: &FOOD_BOWL_ALT,
                eating_speed: 0.25,
                appeal: 0.7,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 8.0));
                    let _ = b.push((StatId::Affection, 3.0));
                    let _ = b.push((StatId::Comfort, 5.0));
                    let _ = b.push((StatId::Mischievousness, 1.0));
                    b
                },
            },
            FoodItem::ChewStick => EatCfg {
                sprite: &CHEW_STICKS,
                eating_speed: 1.0,
                appeal: 0.4,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 6.0));
                    let _ = b.push((StatId::Fitness, 1.0));
                    let _ = b.push((StatId::Playfulness, 2.0));
                    let _ = b.push((StatId::Comfort, 3.0));
                    b
                },
            },
            FoodItem::Puree => EatCfg {
                sprite: &SNACK_PUREE,
                eating_speed: 1.5,
                appeal: 0.85,
                is_snack: true,
                bonus_table: || {
                    let mut b: Bonus = heapless::Vec::new();
                    let _ = b.push((StatId::Fullness, 8.0));
                    let _ = b.push((StatId::Affection, 4.0));
                    let _ = b.push((StatId::Comfort, 4.0));
                    let _ = b.push((StatId::Fulfillment, 2.0));
                    b
                },
            },
        },
    }
}

fn unhealthiness(item: FoodItem) -> f32 {
    match item {
        FoodItem::Carrots => 0.15,
        FoodItem::Pumpkin => 0.05,
        FoodItem::Treats => 0.55,
        FoodItem::FishBite => 0.2,
        FoodItem::Eggs => 0.1,
        FoodItem::Nugget => 0.7,
        FoodItem::Milk => 0.65,
        FoodItem::ChewStick => 0.25,
        FoodItem::Puree => 0.35,
        _ => 0.5,
    }
}

fn source_to_meal_entry(source: EatingSource) -> MealEntry {
    match source {
        EatingSource::Item(item) => MealEntry::Item(item),
        EatingSource::CaughtSnack => MealEntry::CaughtSnack,
    }
}

fn add_to(bonus: &mut Bonus, stat: StatId, delta: f32) {
    if let Some(entry) = bonus.iter_mut().find(|e| e.0 == stat) {
        entry.1 += delta;
    } else {
        let _ = bonus.push((stat, delta));
    }
}

fn scale(bonus: &mut Bonus, stat: StatId, factor: f32) {
    if let Some(entry) = bonus.iter_mut().find(|e| e.0 == stat) {
        entry.1 *= factor;
    }
}
