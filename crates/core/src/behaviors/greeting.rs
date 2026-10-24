use embedded_graphics::prelude::Point;

use crate::{
    assets::character::PoseId,
    behavior::{Behavior, BehaviorId, BehaviorState},
    behaviors::common,
    context::{GameContext, StatId},
    entities::character::Character,
    render::Renderer,
    ui::bubble::{self, BubbleIcon},
};

const WALK_SPEED: f32 = 20.0;
const SNIFF_DURATION: f32 = 3.5;
const REACT_DURATION: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Walking,
    Sniffing,
    Reacting,
}

pub struct GreetingBehavior {
    phase: Phase,
    phase_timer: f32,
    pose_id: PoseId,
    sniff_pose: PoseId,
    target_x: Option<i32>,
    walker_accum: f32,
    familiarity: f32,
    bubble_icon: BubbleIcon,
    bubble_progress: f32,
}

impl GreetingBehavior {
    pub fn new() -> Self {
        Self {
            phase: Phase::Sniffing,
            phase_timer: 0.0,
            pose_id: PoseId::StandingSideSniffing,
            sniff_pose: PoseId::StandingSideSniffing,
            target_x: None,
            walker_accum: 0.0,
            familiarity: 0.0,
            bubble_icon: BubbleIcon::Question,
            bubble_progress: 0.0,
        }
    }

    fn start_sniff(&mut self, _ctx: &GameContext) {
        // TODO(friendship): Once the friendship system is ported, derive
        // familiarity from `ctx.get_friendship_level(peer_id)` using the
        // current visit's peer id. See `visit_manager.py` for the Python
        // implementation. Until then familiarity stays at 0.0 and the pet
        // always shows the neutral question bubble + neutral react pose
        // (matches Python behavior for a first encounter).
        self.familiarity = 0.0;
        self.bubble_icon = if self.familiarity >= 0.5 {
            BubbleIcon::Heart
        } else {
            BubbleIcon::Question
        };
        self.phase = Phase::Sniffing;
        self.phase_timer = 0.0;
        self.bubble_progress = 0.0;
        self.pose_id = self.sniff_pose;
    }
}

impl Behavior for GreetingBehavior {
    fn id(&self) -> BehaviorId {
        BehaviorId::Greeting
    }
    fn progress(&self) -> f32 {
        match self.phase {
            Phase::Walking => 0.0,
            Phase::Sniffing => self.bubble_progress,
            Phase::Reacting => 1.0,
        }
    }
    fn pose(&self) -> PoseId {
        self.pose_id
    }

    fn enter(&mut self, ctx: &mut GameContext, character: &mut Character) {
        // Pull a one-shot target_x from the context if one's been staged
        // (visit greeting / proximity sniff). Falls back to an in-place sniff
        // when no target was provided.
        self.target_x = ctx.pending_greeting_target_x.take();
        self.sniff_pose = PoseId::StandingSideSniffing;
        self.walker_accum = 0.0;
        self.phase_timer = 0.0;
        self.bubble_progress = 0.0;

        if let Some(tx) = self.target_x {
            character.mirror_h = tx > character.pos.x;
            self.pose_id = PoseId::WalkingSideNeutral;
            self.phase = Phase::Walking;
        } else {
            self.start_sniff(ctx);
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        character: &mut Character,
        dt: f32,
    ) -> BehaviorState {
        self.phase_timer += dt;
        match self.phase {
            Phase::Walking => {
                if let Some(tx) = self.target_x {
                    let dir = if character.pos.x < tx { 1 } else { -1 };
                    common::step_walker(
                        character,
                        ctx,
                        dir,
                        WALK_SPEED,
                        dt,
                        &mut self.walker_accum,
                    );
                    if common::distance_to(character, tx) <= 1 {
                        character.pos.x = tx;
                        self.start_sniff(ctx);
                    }
                }
            }
            Phase::Sniffing => {
                self.bubble_progress = (self.phase_timer / SNIFF_DURATION).min(1.0);
                if self.phase_timer >= SNIFF_DURATION {
                    self.phase = Phase::Reacting;
                    self.phase_timer = 0.0;
                    self.pose_id = if self.familiarity >= 0.5 {
                        PoseId::SittingSideHappy
                    } else {
                        PoseId::SittingSideNeutral
                    };
                }
            }
            Phase::Reacting if self.phase_timer >= REACT_DURATION => {
                return BehaviorState::Completed;
            }
            _ => {}
        }
        BehaviorState::Running
    }

    fn apply_completion_bonus(&self, ctx: &mut GameContext, progress: f32) {
        let bonus = [
            (StatId::Sociability, 0.25 * progress),
            (StatId::Affection, 0.3 * progress),
            (StatId::Serenity, 0.1 * progress),
        ];
        ctx.apply_stat_changes(&bonus);
    }

    fn draw(&self, renderer: &mut Renderer, _ctx: &GameContext, char_screen: Point, mirror_h: bool) {
        if matches!(self.phase, Phase::Sniffing) {
            bubble::draw_above_char(
                renderer,
                self.bubble_icon,
                char_screen.x,
                char_screen.y,
                self.bubble_progress,
                mirror_h,
            );
        }
    }
}
