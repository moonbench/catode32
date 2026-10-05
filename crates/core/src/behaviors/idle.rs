use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    rand,
    scene::SceneId,
    time_system::Weather,
};

const NEUTRAL: &[PoseId] = &[
    PoseId::SittingSideNeutral,
    PoseId::SittingSideLookingDown,
    PoseId::SittingForwardNeutral,
    PoseId::SittingForwardSleepy,
    PoseId::SittingForwardContent,
    PoseId::SittingSillySideNeutral,
    PoseId::StandingSideNeutral,
    PoseId::StandingSideNeutralLookingDown,
];

// Python: HAPPY_POSES = NEUTRAL_POSES + (extras...)
const HAPPY: &[PoseId] = &[
    // NEUTRAL
    PoseId::SittingSideNeutral,
    PoseId::SittingSideLookingDown,
    PoseId::SittingForwardNeutral,
    PoseId::SittingForwardSleepy,
    PoseId::SittingForwardContent,
    PoseId::SittingSillySideNeutral,
    PoseId::StandingSideNeutral,
    PoseId::StandingSideNeutralLookingDown,
    // extras
    PoseId::StandingSideHappy,
    PoseId::SittingForwardAloof,
    PoseId::SittingForwardHappy,
    PoseId::SittingSideHappy,
    PoseId::SittingSideAloof,
    PoseId::SittingSillySideHappy,
    PoseId::SittingSillySideAloof,
];

// Python: UPSET_POSES = NEUTRAL_POSES + (extras...)
const UPSET: &[PoseId] = &[
    // NEUTRAL
    PoseId::SittingSideNeutral,
    PoseId::SittingSideLookingDown,
    PoseId::SittingForwardNeutral,
    PoseId::SittingForwardSleepy,
    PoseId::SittingForwardContent,
    PoseId::SittingSillySideNeutral,
    PoseId::StandingSideNeutral,
    PoseId::StandingSideNeutralLookingDown,
    // extras
    PoseId::StandingSideAngry,
    PoseId::SittingSideAngry,
    PoseId::SittingSideAnnoyed,
    PoseId::SittingSillySideAnnoyed,
    PoseId::SittingSillySideAngry,
];

const SICK_POSE: PoseId = PoseId::LayingSideSick;
const LOOK_AWAY_POSES: &[PoseId] = &[
    PoseId::SittingBackBackNeutral,
    PoseId::SittingBackSideNeutral,
];

const MIN_POSE_DURATION: f32 = 15.0;
const MAX_POSE_DURATION: f32 = 60.0;

pub struct IdleBehavior {
    pose_id: PoseId,
    elapsed: f32,
    duration: f32,
    pose_timer: f32,
    pose_change_in: f32,
}

impl IdleBehavior {
    pub fn new() -> Self {
        Self {
            pose_id: PoseId::SittingSideNeutral,
            elapsed: 0.0,
            duration: 30.0,
            pose_timer: 0.0,
            pose_change_in: 30.0,
        }
    }

    fn pose_pool(ctx: &GameContext) -> &'static [PoseId] {
        if ctx.sickness >= 2.0 {
            return &[SICK_POSE];
        }
        if ctx.fullness >= 50.0
            && ctx.comfort >= 50.0
            && ctx.affection >= 60.0
            && ctx.serenity >= 60.0
        {
            HAPPY
        } else if ctx.fullness <= 25.0
            || ctx.comfort <= 20.0
            || ctx.affection <= 25.0
            || ctx.serenity <= 15.0
        {
            UPSET
        } else {
            NEUTRAL
        }
    }

    fn look_away_weight(scene: SceneId) -> u32 {
        match scene {
            SceneId::VacationAquarium => 10,
            SceneId::VacationBeach => 6,
            SceneId::Outside | SceneId::Treehouse => 3,
            _ => 0,
        }
    }

    fn pick_new_pose(&mut self, ctx: &mut GameContext) {
        let pool = Self::pose_pool(ctx);
        let weight = Self::look_away_weight(ctx.last_main_scene) as usize;
        // Python excludes the current pose from the candidate list when it's
        // in the pool and the pool has at least 2 entries.
        let current = self.pose_id;
        let exclude = pool.len() > 1 && pool.iter().any(|p| *p == current);
        let pool_slots = if exclude { pool.len() - 1 } else { pool.len() };
        // Each weight unit appends both LOOK_AWAY poses.
        let look_away_slots = weight * LOOK_AWAY_POSES.len();
        let total = pool_slots + look_away_slots;
        let roll = if total == 0 {
            0
        } else {
            rand::rand_range_u32(&mut ctx.rng, 0, total as u32 - 1) as usize
        };
        self.pose_id = if roll >= pool_slots {
            LOOK_AWAY_POSES[(roll - pool_slots) % LOOK_AWAY_POSES.len()]
        } else if exclude {
            // Walk the pool skipping `current` and pick by index.
            let mut i = 0;
            let mut chosen = pool[0];
            for &p in pool {
                if p == current {
                    continue;
                }
                if i == roll {
                    chosen = p;
                    break;
                }
                i += 1;
            }
            chosen
        } else {
            pool[roll]
        };
        println!(
            "[idle] scene={:?} weight={} picked={:?}",
            ctx.last_main_scene, weight, self.pose_id
        );
    }
}

impl Behavior for IdleBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Idle
    }

    fn progress(&self) -> f32 {
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, _character: &mut Character) {
        self.elapsed = 0.0;
        self.pose_timer = 0.0;
        self.duration =
            rand::rand_range_f32(&mut ctx.rng, MIN_POSE_DURATION, MAX_POSE_DURATION * 2.0);
        self.pose_change_in =
            rand::rand_range_f32(&mut ctx.rng, MIN_POSE_DURATION, MAX_POSE_DURATION);
        self.pick_new_pose(ctx);
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        _character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.elapsed += dt;
        self.pose_timer += dt;
        if self.pose_timer >= self.pose_change_in {
            self.pose_timer = 0.0;
            self.pose_change_in =
                rand::rand_range_f32(&mut ctx.rng, MIN_POSE_DURATION, MAX_POSE_DURATION);
            self.pick_new_pose(ctx);
        }
        if self.elapsed >= self.duration {
            BehaviorState::Completed
        } else {
            BehaviorState::Running
        }
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let mut bonus: heapless::Vec<(StatId, f32), 14> = heapless::Vec::new();
        common::bonus_add(&mut bonus, StatId::Fullness, -0.05);
        common::bonus_add(&mut bonus, StatId::Energy, -0.1);
        common::bonus_add(&mut bonus, StatId::Comfort, -0.4);
        common::bonus_add(&mut bonus, StatId::Playfulness, -0.05);
        common::bonus_add(&mut bonus, StatId::Focus, -0.05);
        common::bonus_add(&mut bonus, StatId::Fulfillment, -0.02);
        common::bonus_add(&mut bonus, StatId::Curiosity, 0.02);
        common::bonus_add(&mut bonus, StatId::Cleanliness, -0.06);
        common::bonus_add(&mut bonus, StatId::Intelligence, -0.005);
        common::bonus_add(&mut bonus, StatId::Fitness, -0.015);
        common::bonus_add(&mut bonus, StatId::Serenity, 0.0075);

        let outdoor = matches!(
            ctx.last_main_scene,
            SceneId::Outside | SceneId::Treehouse
        );
        if outdoor
            && matches!(
                ctx.weather,
                Weather::Rain | Weather::Storm | Weather::Snow
            )
        {
            common::bonus_add(&mut bonus, StatId::Comfort, -5.0);
        }
        if ctx.meteor_shower_happening() {
            common::bonus_add(&mut bonus, StatId::Serenity, 0.5);
            common::bonus_add(&mut bonus, StatId::Fulfillment, 0.3);
            common::bonus_add(&mut bonus, StatId::Comfort, 0.5);
        }
        if ctx.scene_plant_health != 0 {
            let ph = ctx.scene_plant_health as f32;
            common::bonus_add(&mut bonus, StatId::Serenity, ph * 0.15);
            common::bonus_add(&mut bonus, StatId::Comfort, ph * 0.1);
        }
        let (c, s) = common::fav_weather_bonus(ctx);
        if c != 0.0 {
            common::bonus_add(&mut bonus, StatId::Comfort, c);
        }
        if s != 0.0 {
            common::bonus_add(&mut bonus, StatId::Serenity, s);
        }
        for entry in bonus.iter_mut() {
            entry.1 *= progress;
        }
        ctx.apply_stat_changes(&bonus);
    }
}
