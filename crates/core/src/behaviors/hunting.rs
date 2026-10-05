use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, EatingSource, GiftKind, NextBehavior},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Stalking,
    Darting,
    Pouncing,
    Missing,
    Catching,
}

pub struct HuntingBehavior {
    phase: Phase,
    phase_timer: f32,
    stalk_duration: f32,
    pounce_slide_duration: f32,
    miss_pause_duration: f32,
    catch_duration: f32,
    dart_speed: f32,
    pounce_slide_speed: f32,
    pose_id: PoseId,
    prey_x: f32,
    dart_target_x: f32,
    dart_direction: i32,
    darts_remaining: u32,
    miss_count: u32,
    max_misses: u32,
    walker_accum: f32,
}

impl HuntingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Stalking,
            phase_timer: 0.0,
            stalk_duration: 5.0,
            pounce_slide_duration: 0.8,
            miss_pause_duration: 1.5,
            catch_duration: 3.0,
            dart_speed: 55.0,
            pounce_slide_speed: 25.0,
            pose_id: PoseId::SittingSideAloof,
            prey_x: 0.0,
            dart_target_x: 0.0,
            dart_direction: 1,
            darts_remaining: 1,
            miss_count: 0,
            max_misses: 0,
            walker_accum: 0.0,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        if ctx.fullness < 15.0 && ctx.energy > 20.0 {
            return true;
        }
        let outdoor = common::is_outdoor(ctx.last_main_scene);
        let threshold = if outdoor { 15.0 } else { 20.0 };
        ctx.energy > threshold && ctx.playfulness > threshold
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let hunger_pull = 100.0 - ctx.fullness;
        let play_pull = ctx.playfulness;
        let ceiling = (85.0 - play_pull * 0.5 - hunger_pull * 0.3).max(25.0);
        let floor = (25.0 + hunger_pull * 0.15 - play_pull * 0.1).max(10.0);
        let mut base = rand::rand_range_f32(rng, floor, (floor + 5.0).max(ceiling));
        if common::is_outdoor(ctx.last_main_scene) {
            base *= 0.75;
        }
        if ctx.fullness < 5.0 {
            base = base.min(15.0 + ctx.fullness * 0.5);
        }
        base.max(0.0) as u32
    }

    /// Scene bounds with the Python 15px margin (slightly tighter than the
    /// 20px margin used by pacing / meandering / zoomies).
    fn scene_bounds(ctx: &GameContext) -> (f32, f32) {
        ((ctx.scene_x_min + 15) as f32, (ctx.scene_x_max - 15) as f32)
    }

    fn pick_prey_location(&mut self, ctx: &mut GameContext) {
        let (x_min, x_max) = Self::scene_bounds(ctx);
        self.prey_x = rand::rand_range_f32(&mut ctx.rng, x_min, x_max);
    }

    /// Pick the next dart segment's destination.
    ///
    /// While more than one dart is queued, the first is a feinting dash to
    /// the opposite half of the screen from the prey (to build momentum).
    /// The final dart always runs straight at the prey.
    fn set_next_dart_target(&mut self, ctx: &mut GameContext, character: &Character) {
        let (x_min, x_max) = Self::scene_bounds(ctx);
        if self.darts_remaining > 1 {
            let midpoint = (x_min + x_max) / 2.0;
            self.dart_target_x = if self.prey_x >= midpoint {
                rand::rand_range_f32(&mut ctx.rng, x_min, midpoint)
            } else {
                rand::rand_range_f32(&mut ctx.rng, midpoint, x_max)
            };
        } else {
            self.dart_target_x = self.prey_x.clamp(x_min, x_max);
        }
        self.dart_direction = if self.dart_target_x > character.pos.x as f32 {
            1
        } else {
            -1
        };
    }

    fn begin_dart_phase(&mut self, ctx: &mut GameContext, character: &mut Character) {
        self.darts_remaining = rand::rand_range_u32(&mut ctx.rng, 1, 2);
        self.set_next_dart_target(ctx, character);
        character.mirror_h = self.dart_direction > 0;
        self.phase = Phase::Darting;
        self.phase_timer = 0.0;
        self.walker_accum = 0.0;
        self.pose_id = PoseId::RunningSideAngry;
    }

    /// Step `character.pos.x` toward `dart_target_x` at `speed` and return
    /// whether the target was reached this tick.
    fn step_dart(&mut self, character: &mut Character, speed: f32, dt: f32) -> bool {
        self.walker_accum += speed * dt;
        let whole = self.walker_accum as i32;
        if whole != 0 {
            self.walker_accum -= whole as f32;
            character.pos.x += whole * self.dart_direction.signum();
        }
        if self.dart_direction > 0 {
            character.pos.x as f32 >= self.dart_target_x
        } else {
            character.pos.x as f32 <= self.dart_target_x
        }
    }
}

impl Behavior for HuntingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Hunting
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Stalking => (self.phase_timer / self.stalk_duration).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        // Per-phase Python ranges:
        //   stalk_duration       = uniform(3.0, 8.0)
        //   pounce_slide_duration = uniform(0.5, 1.1)
        //   miss_pause_duration   = uniform(1.0, 2.0)
        //   catch_duration        = uniform(2.0, 4.0)
        //   dart_speed            = randint(50, 60)
        //   pounce_slide_speed    = randint(20, 32)
        self.stalk_duration = rand::rand_range_f32(&mut ctx.rng, 3.0, 8.0);
        self.pounce_slide_duration = rand::rand_range_f32(&mut ctx.rng, 0.5, 1.1);
        self.miss_pause_duration = rand::rand_range_f32(&mut ctx.rng, 1.0, 2.0);
        self.catch_duration = rand::rand_range_f32(&mut ctx.rng, 2.0, 4.0);
        self.dart_speed = rand::rand_range_u32(&mut ctx.rng, 50, 60) as f32;
        self.pounce_slide_speed = rand::rand_range_u32(&mut ctx.rng, 20, 32) as f32;

        self.miss_count = 0;
        // Weighted max_misses: 50% none, 35% one, 15% two (10/7/3 of 20).
        let roll = rand::rand_range_u32(&mut ctx.rng, 0, 19);
        self.max_misses = if roll < 10 {
            0
        } else if roll < 17 {
            1
        } else {
            2
        };
        self.pick_prey_location(ctx);

        self.phase = Phase::Stalking;
        self.phase_timer = 0.0;
        self.walker_accum = 0.0;
        self.darts_remaining = 1;
        self.dart_direction = 1;
        self.dart_target_x = 0.0;
        self.pose_id = PoseId::SittingSideAloof;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Stalking => {
                if self.phase_timer >= self.stalk_duration {
                    self.begin_dart_phase(ctx, character);
                }
            }
            Phase::Darting => {
                let arrived = self.step_dart(character, self.dart_speed, dt);
                if arrived {
                    character.pos.x = self.dart_target_x as i32;
                    self.darts_remaining = self.darts_remaining.saturating_sub(1);
                    if self.darts_remaining > 0 {
                        self.set_next_dart_target(ctx, character);
                        character.mirror_h = self.dart_direction > 0;
                        self.walker_accum = 0.0;
                    } else {
                        self.phase = Phase::Pouncing;
                        self.phase_timer = 0.0;
                        self.walker_accum = 0.0;
                        self.pose_id = PoseId::LeaningForwardSidePounce;
                    }
                }
            }
            Phase::Pouncing => {
                // Slide forward while the leap is in flight.
                self.walker_accum += self.pounce_slide_speed * dt;
                let whole = self.walker_accum as i32;
                if whole != 0 {
                    self.walker_accum -= whole as f32;
                    character.pos.x += whole * self.dart_direction.signum();
                }
                if self.phase_timer >= self.pounce_slide_duration {
                    let (x_min, x_max) = Self::scene_bounds(ctx);
                    character.pos.x = (character.pos.x as f32).clamp(x_min, x_max) as i32;

                    if self.miss_count < self.max_misses {
                        self.miss_count += 1;
                        self.pick_prey_location(ctx);
                        self.phase = Phase::Missing;
                        self.phase_timer = 0.0;
                        self.pose_id = PoseId::SittingSideAloof;
                    } else {
                        self.phase = Phase::Catching;
                        self.phase_timer = 0.0;
                        self.pose_id = PoseId::SittingSillySideHappy;
                    }
                }
            }
            Phase::Missing => {
                if self.phase_timer >= self.miss_pause_duration {
                    self.begin_dart_phase(ctx, character);
                }
            }
            Phase::Catching => {
                if self.phase_timer >= self.catch_duration {
                    // Outdoor-scene burst (Python apply_location_bonus).
                    if common::is_outdoor(ctx.last_main_scene) {
                        character.play_bursts(&mut ctx.rng, 5);
                    }
                    return BehaviorState::Completed;
                }
            }
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        #[cfg(not(feature = "desktop"))]
        use micromath::F32Ext;
        // Sigmoid centred at fullness=30 — at full belly the cat almost
        // always shares the catch instead of eating it.
        let eating_chance = 0.95 / (1.0 + (0.2 * (ctx.fullness - 30.0)).exp());
        let mut rng = ctx.rng;
        let roll = rand::rand_f32(&mut rng);
        if roll < eating_chance {
            return Some(NextBehavior::Eating(EatingSource::CaughtSnack));
        }
        if roll < eating_chance + 0.25 {
            // Python gifts a fish (FISH1), not the mouse it presumably hunted.
            return Some(NextBehavior::GiftBringing(GiftKind::Fish));
        }
        None
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let coins = rand::rand_range_u32(&mut ctx.rng, 1, 3) as i32;
        ctx.coins = (ctx.coins + coins).min(9999);

        let mut bonus: heapless::Vec<(StatId, f32), 14> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Fullness, -0.5);
        common::bonus_add(&mut bonus, StatId::Energy, -2.0);
        common::bonus_add(&mut bonus, StatId::Comfort, -0.6);
        common::bonus_add(&mut bonus, StatId::Playfulness, -0.25);
        common::bonus_add(&mut bonus, StatId::Fulfillment, 0.05);
        common::bonus_add(&mut bonus, StatId::Intelligence, 0.015);
        common::bonus_add(&mut bonus, StatId::Cleanliness, -0.3);
        common::bonus_add(&mut bonus, StatId::Fitness, 0.02);
        common::bonus_add(&mut bonus, StatId::Serenity, -0.05);
        common::bonus_add(&mut bonus, StatId::Mischievousness, 0.02);
        common::bonus_add(&mut bonus, StatId::Courage, 0.0075);

        if common::is_outdoor(ctx.last_main_scene) {
            common::bonus_scale(&mut bonus, StatId::Fitness, 1.5);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 0.05);
        }

        for entry in bonus.iter_mut() {
            entry.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }
}
