use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState, NextBehavior},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
};

const PLOT_DURATION: f32 = 1.5;
const MISCHIEF_DURATION: f32 = 8.0;
const SATISFY_DURATION: f32 = 1.5;
const ZOOM_SPEED: f32 = 50.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Plotting,
    Mischief,
    Satisfied,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    Running,
    Kneading,
}

pub struct MischiefBehavior {
    phase: Phase,
    phase_timer: f32,
    pose_id: PoseId,
    dir: i32,
    dir_change_timer: f32,
    dir_change_interval: f32,
    walker_accum: f32,
    sub: Sub,
    sub_timer: f32,
    sub_duration: f32,
}

impl MischiefBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Plotting,
            phase_timer: 0.0,
            pose_id: PoseId::LeaningForwardSidePounce,
            dir: 1,
            dir_change_timer: 0.0,
            dir_change_interval: 1.5,
            walker_accum: 0.0,
            sub: Sub::Running,
            sub_timer: 0.0,
            sub_duration: 0.0,
        }
    }

    pub fn can_trigger(ctx: &GameContext) -> bool {
        ctx.mischievousness > 25.0
            && ctx.maturity < 55.0
            && ctx.playfulness > 50.0
            && ctx.energy > 40.0
    }

    pub fn priority(ctx: &GameContext, rng: &mut u32) -> u32 {
        let hi = ((200.0 - ctx.mischievousness - ctx.playfulness) * 0.5).max(20.0);
        rand::rand_range_f32(rng, 20.0, hi).max(0.0) as u32
    }
}

impl Behavior for MischiefBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Mischief
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Plotting => 0.0,
            Phase::Mischief => (self.phase_timer / MISCHIEF_DURATION).clamp(0.0, 1.0),
            Phase::Satisfied => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _: &mut Character) {
        self.phase = Phase::Plotting;
        self.phase_timer = 0.0;
        self.dir = if rand::rand_bool(&mut ctx.rng, 0.5) { 1 } else { -1 };
        self.dir_change_timer = 0.0;
        self.dir_change_interval = rand::rand_range_f32(&mut ctx.rng, 1.0, 2.5);
        self.walker_accum = 0.0;
        self.sub = Sub::Running;
        self.sub_timer = 0.0;
        self.sub_duration = 0.0;
        self.pose_id = PoseId::LeaningForwardSidePounce;
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Plotting if self.phase_timer >= PLOT_DURATION => {
                self.phase = Phase::Mischief;
                self.phase_timer = 0.0;
                self.sub = Sub::Running;
                self.sub_timer = 0.0;
                self.sub_duration = rand::rand_range_f32(&mut ctx.rng, 1.5, 3.0);
                self.pose_id = PoseId::RunningSideAngry;
                character.mirror_h = self.dir > 0;
            }
            Phase::Mischief => {
                let x_min = ctx.scene_x_min + 20;
                let x_max = ctx.scene_x_max - 20;
                self.sub_timer += dt;

                match self.sub {
                    Sub::Running => {
                        self.walker_accum += ZOOM_SPEED * dt;
                        let whole = self.walker_accum as i32;
                        if whole != 0 {
                            self.walker_accum -= whole as f32;
                            character.pos.x += whole * self.dir.signum();
                        }
                        character.mirror_h = self.dir > 0;

                        if character.pos.x <= x_min {
                            character.pos.x = x_min;
                            self.dir = 1;
                            self.dir_change_timer = 0.0;
                            self.dir_change_interval =
                                rand::rand_range_f32(&mut ctx.rng, 1.0, 2.5);
                            character.mirror_h = true;
                        } else if character.pos.x >= x_max {
                            character.pos.x = x_max;
                            self.dir = -1;
                            self.dir_change_timer = 0.0;
                            self.dir_change_interval =
                                rand::rand_range_f32(&mut ctx.rng, 1.0, 2.5);
                            character.mirror_h = false;
                        }

                        self.dir_change_timer += dt;
                        if self.dir_change_timer >= self.dir_change_interval {
                            self.dir = -self.dir;
                            self.dir_change_timer = 0.0;
                            self.dir_change_interval =
                                rand::rand_range_f32(&mut ctx.rng, 1.0, 2.5);
                            character.mirror_h = self.dir > 0;
                        }

                        if self.sub_timer >= self.sub_duration {
                            self.sub = Sub::Kneading;
                            self.sub_timer = 0.0;
                            self.sub_duration = rand::rand_range_f32(&mut ctx.rng, 0.4, 0.8);
                            self.pose_id = PoseId::KneadingSideAngry;
                        }
                    }
                    Sub::Kneading => {
                        if self.sub_timer >= self.sub_duration {
                            self.sub = Sub::Running;
                            self.sub_timer = 0.0;
                            self.sub_duration = rand::rand_range_f32(&mut ctx.rng, 1.5, 3.0);
                            self.pose_id = PoseId::RunningSideAngry;
                            character.mirror_h = self.dir > 0;
                        }
                    }
                }

                if self.phase_timer >= MISCHIEF_DURATION {
                    self.phase = Phase::Satisfied;
                    self.phase_timer = 0.0;
                    self.pose_id = PoseId::SittingSillySideAnnoyed;
                }
            }
            Phase::Satisfied if self.phase_timer >= SATISFY_DURATION => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn next(&self, ctx: &GameContext) -> Option<NextBehavior> {
        // Retreat if the pet's nerve broke and it's now depleted.
        if ctx.courage < 60.0 && ctx.affection < 40.0 && ctx.energy < 40.0 {
            let mut rng = ctx.rng;
            if rand::rand_f32(&mut rng) < 0.4 {
                return Some(NextBehavior::Hiding);
            }
        }
        Some(NextBehavior::Pacing)
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 10> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Energy, -1.0);
        common::bonus_add(&mut bonus, StatId::Focus, -1.0);
        common::bonus_add(&mut bonus, StatId::Playfulness, -0.25);
        common::bonus_add(&mut bonus, StatId::Maturity, -0.1);
        common::bonus_add(&mut bonus, StatId::Sociability, -0.2);
        common::bonus_add(&mut bonus, StatId::Affection, -0.02);
        common::bonus_add(&mut bonus, StatId::Mischievousness, 0.03);
        common::bonus_add(&mut bonus, StatId::Loyalty, -0.1);

        let hf = common::hungry_factor(ctx);
        if hf > 0.0 {
            common::bonus_add(&mut bonus, StatId::Loyalty, -0.15 * hf);
            common::bonus_add(&mut bonus, StatId::Focus, -0.5 * hf);
            common::bonus_add(&mut bonus, StatId::Affection, -0.03 * hf);
            common::bonus_add(&mut bonus, StatId::Mischievousness, 0.04 * hf);
        }

        for e in bonus.iter_mut() {
            e.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }
}
