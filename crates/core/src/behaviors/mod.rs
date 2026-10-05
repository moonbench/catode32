pub mod affection;
pub mod attention;
pub mod being_groomed;
pub mod chattering;
pub mod common;
pub mod eating;
pub mod gift_bringing;
pub mod go_to;
pub mod greeting;
pub mod hearing;
pub mod hiding;
pub mod hunting;
pub mod idle;
pub mod investigating;
pub mod kneading;
pub mod lounging;
pub mod meandering;
pub mod mischief;
pub mod napping;
pub mod observing;
pub mod pacing;
pub mod playing;
pub mod self_grooming;
pub mod sleeping;
pub mod startled;
pub mod stretching;
pub mod sulking;
pub mod training;
pub mod vocalizing;
pub mod zoomies;

use crate::{
    behavior::{Behavior, BehaviorId, NextBehavior},
    context::GameContext,
    rand,
};

pub use affection::AffectionBehavior;
pub use attention::AttentionBehavior;
pub use being_groomed::BeingGroomedBehavior;
pub use chattering::ChatteringBehavior;
pub use eating::EatingBehavior;
pub use gift_bringing::GiftBringingBehavior;
pub use go_to::GoToBehavior;
pub use greeting::GreetingBehavior;
pub use hearing::HearingBehavior;
pub use hiding::HidingBehavior;
pub use hunting::HuntingBehavior;
pub use idle::IdleBehavior;
pub use investigating::InvestigatingBehavior;
pub use kneading::KneadingBehavior;
pub use lounging::LoungingBehavior;
pub use meandering::MeanderingBehavior;
pub use mischief::MischiefBehavior;
pub use napping::NappingBehavior;
pub use observing::ObservingBehavior;
pub use pacing::PacingBehavior;
pub use playing::PlayingBehavior;
pub use self_grooming::SelfGroomingBehavior;
pub use sleeping::SleepingBehavior;
pub use startled::StartledBehavior;
pub use stretching::StretchingBehavior;
pub use sulking::SulkingBehavior;
pub use training::TrainingBehavior;
pub use vocalizing::VocalizingBehavior;
pub use zoomies::ZoomiesBehavior;

// Enum-dispatched union of every concrete behavior implementation. Eight
// variants (Hearing, Playing, Eating, GiftBringing, Training, GoTo,
// Affection, Attention) carry construction payload from the matching
// `NextBehavior` variant; the rest construct with `::new()`.
crate::dispatch_enum! {
    #[allow(dead_code)]
    pub enum ActiveBehavior from NextBehavior via from_next,
    as dyn Behavior via as_dyn / as_dyn_mut
    {
        Idle(IdleBehavior)                              = IdleBehavior::new(),
        Sleeping(SleepingBehavior)                      = SleepingBehavior::new(),
        Napping(NappingBehavior)                        = NappingBehavior::new(),
        Stretching(StretchingBehavior)                  = StretchingBehavior::new(),
        Kneading(KneadingBehavior)                      = KneadingBehavior::new(),
        Lounging(LoungingBehavior)                      = LoungingBehavior::new(),
        Investigating(InvestigatingBehavior)            = InvestigatingBehavior::new(),
        Observing(ObservingBehavior)                    = ObservingBehavior::new(),
        Chattering(ChatteringBehavior)                  = ChatteringBehavior::new(),
        Zoomies(ZoomiesBehavior)                        = ZoomiesBehavior::new(),
        Vocalizing(VocalizingBehavior)                  = VocalizingBehavior::new(),
        SelfGrooming(SelfGroomingBehavior)              = SelfGroomingBehavior::new(),
        BeingGroomed(BeingGroomedBehavior)              = BeingGroomedBehavior::new(),
        Hunting(HuntingBehavior)                        = HuntingBehavior::new(),
        GiftBringing(GiftBringingBehavior)(gift)        = GiftBringingBehavior::new(gift),
        Pacing(PacingBehavior)                          = PacingBehavior::new(),
        Sulking(SulkingBehavior)                        = SulkingBehavior::new(),
        Mischief(MischiefBehavior)                      = MischiefBehavior::new(),
        Hiding(HidingBehavior)                          = HidingBehavior::new(),
        Training(TrainingBehavior)(kind)                = TrainingBehavior::new(kind),
        Playing(PlayingBehavior)(variant)               = PlayingBehavior::new(variant),
        Affection(AffectionBehavior)(variant)           = AffectionBehavior::new(variant),
        Attention(AttentionBehavior)(variant)           = AttentionBehavior::new(variant),
        Eating(EatingBehavior)(source)                  = EatingBehavior::new(source),
        Startled(StartledBehavior)                      = StartledBehavior::new(),
        Meandering(MeanderingBehavior)                  = MeanderingBehavior::new(),
        GoTo(GoToBehavior)(params)                      = GoToBehavior::new(params),
        Hearing(HearingBehavior)(icon)                  = HearingBehavior::new(icon),
        Greeting(GreetingBehavior)                      = GreetingBehavior::new(),
    }
}

// ---------------------------------------------------------------------------
// Auto-selection
// ---------------------------------------------------------------------------

const AUTO_SELECT_NAMES: &[BehaviorId] = &[
    BehaviorId::Sleeping,
    BehaviorId::Napping,
    BehaviorId::Zoomies,
    BehaviorId::Vocalizing,
    BehaviorId::Hunting,
    BehaviorId::Playing,
    BehaviorId::Investigating,
    BehaviorId::Observing,
    BehaviorId::SelfGrooming,
    BehaviorId::Stretching,
    BehaviorId::Pacing,
    BehaviorId::Sulking,
    BehaviorId::Mischief,
    BehaviorId::Hiding,
    BehaviorId::Lounging,
    BehaviorId::Startled,
];

pub fn auto_select(ctx: &mut GameContext) -> NextBehavior {
    // Getting out of the rain beats everything else.
    if let Some(next) = common::shelter_exit(ctx) {
        return next;
    }

    // Random meander gate, scaled by sickness.
    let meander_p = if ctx.sickness >= 8.0 {
        0.02
    } else if ctx.sickness >= 5.0 {
        0.06
    } else if ctx.sickness >= 2.0 {
        0.12
    } else {
        0.2
    };
    if ctx.fullness >= 5.0
        && can_trigger(BehaviorId::Meandering, ctx)
        && rand::rand_f32(&mut ctx.rng) <= meander_p
    {
        println!("\x1b[32mRandomly meandering....\x1b[0m");
        return NextBehavior::Meandering;
    }

    // Scene-exit detour.
    if let Some(next) = common::auto_select_scene_exit(ctx) {
        if let NextBehavior::GoTo(p) = &next {
            if let Some(dest) = p.pending_scene {
                println!("\x1b[32mScene exit -> {:?}\x1b[0m", dest);
            }
        }
        return next;
    }

    // High serenity stay-idle gate.
    if ctx.fullness >= 5.0
        && ctx.serenity > 25.0
        && rand::rand_f32(&mut ctx.rng) < (ctx.serenity - 25.0) / 150.0
    {
        println!(
            "\x1b[32mStaying idle (serenity: {:.1})\x1b[0m",
            ctx.serenity
        );
        return NextBehavior::Idle;
    }

    println!(
        "--------------------------------------------------------------------------------"
    );

    // Gather eligible candidates.
    let mut candidates: heapless::Vec<(BehaviorId, u32), 16> = heapless::Vec::new();
    for &id in AUTO_SELECT_NAMES {
        if common::sick_blocks(id, ctx) {
            println!(
                "[Sickness] Blocking '{}' (sickness={:.2})",
                id.name(),
                ctx.sickness
            );
            continue;
        }
        if !can_trigger(id, ctx) {
            print_skip_reason(id, ctx);
            continue;
        }
        let mut p = priority(id, ctx);
        // Recency penalty.
        if let Some(idx) = ctx.recent_index(id) {
            let mut penalty = 50_i32 - (idx as i32) * 10;
            if ctx.sickness >= 2.0
                && matches!(id, BehaviorId::Sleeping | BehaviorId::Napping)
            {
                penalty /= 2;
            }
            p = p.saturating_add(penalty.max(0) as u32);
        }
        let _ = candidates.push((id, p));
    }

    if candidates.is_empty() {
        println!(
            "--------------------------------------------------------------------------------"
        );
        return NextBehavior::Idle;
    }

    // Per-candidate priority dump, ascending so the lowest (most likely) is first.
    let mut sorted = candidates.clone();
    sorted.sort_unstable_by_key(|e| e.1);
    for (id, p) in sorted.iter() {
        if let Some(idx) = ctx.recent_index(*id) {
            println!(
                ">> {}: priority= {} (+{} recency)",
                id.name(),
                p,
                50 - (idx as i32) * 10
            );
        } else {
            println!(">> {}: priority= {}", id.name(), p);
        }
    }
    println!(
        "--------------------------------------------------------------------------------"
    );

    // Bin priorities (ceil to nearest 10) and randomly pick within the lowest bin.
    let mut best_bin = u32::MAX;
    for &(_, p) in &candidates {
        let b = ((p + 9) / 10) * 10;
        if b < best_bin {
            best_bin = b;
        }
    }
    let mut tied: heapless::Vec<BehaviorId, 16> = heapless::Vec::new();
    for &(id, p) in &candidates {
        if ((p + 9) / 10) * 10 == best_bin {
            let _ = tied.push(id);
        }
    }
    let pick = rand::rand_range_u32(&mut ctx.rng, 0, (tied.len() - 1) as u32) as usize;
    let chosen = tied[pick];
    if tied.len() > 1 {
        let mut names: heapless::Vec<&'static str, 16> = heapless::Vec::new();
        for id in tied.iter() {
            let _ = names.push(id.name());
        }
        println!(
            ">> Selected: {} (from bin tied at {}: {:?})",
            chosen.name(),
            best_bin,
            &names[..]
        );
    }
    if chosen == BehaviorId::Playing {
        let solo = playing::solo_toys_in_inventory(ctx);
        if !solo.is_empty() {
            let i = rand::rand_range_u32(&mut ctx.rng, 0, (solo.len() - 1) as u32) as usize;
            let variant = solo[i].to_play_variant();
            return NextBehavior::Playing(variant);
        }
    }
    to_next_default(chosen)
}

fn print_skip_reason(id: BehaviorId, ctx: &GameContext) {
    match id {
        BehaviorId::Sleeping => println!("Skipping sleeping. Energy: {:6.4}", ctx.energy),
        BehaviorId::Napping => println!("Skipping napping. Energy: {:6.4}", ctx.energy),
        BehaviorId::Zoomies => println!(
            "Skipping zoomies. Energy: {:6.4}, Playfulness: {:6.4}",
            ctx.energy, ctx.playfulness
        ),
        BehaviorId::Vocalizing => println!(
            "Skipping vocalizing. Energy: {:6.4}, Playfulness: {:6.4}, Fullness: {:6.4}, Comfort: {:6.4}, Fulfillment: {:6.4}, Affection: {:6.4}, Sociability: {:6.4}",
            ctx.energy, ctx.playfulness, ctx.fullness, ctx.comfort, ctx.fulfillment, ctx.affection, ctx.sociability
        ),
        BehaviorId::Hunting => println!(
            "Skipping hunting. Energy: {:6.4}, Playfulness: {:6.4}, Fullness: {:6.4}",
            ctx.energy, ctx.playfulness, ctx.fullness
        ),
        BehaviorId::Playing => println!("Skipping playing. Playfulness: {:6.4} or no solo toys.", ctx.playfulness),
        BehaviorId::Investigating => println!("Skipping investigating. Curiosity: {:6.4}", ctx.curiosity),
        BehaviorId::Observing => println!("Skipping observing. Curiosity: {:6.4}", ctx.curiosity),
        BehaviorId::SelfGrooming => println!(
            "Skipping self grooming. Cleanliness: {:6.4}, Energy: {:6.4}",
            ctx.cleanliness, ctx.energy
        ),
        BehaviorId::Stretching => println!("Skipping stretching. Comfort: {:6.2}", ctx.comfort),
        BehaviorId::Pacing => println!(
            "Skipping pacing. Comfort: {:6.4}, Serenity: {:6.4}",
            ctx.comfort, ctx.serenity
        ),
        BehaviorId::Sulking => println!(
            "Skipping sulking. Fulfillment: {:6.4}, Affection: {:6.4}, Fullness: {:6.4}, Comfort: {:6.4}",
            ctx.fulfillment, ctx.affection, ctx.fullness, ctx.comfort
        ),
        BehaviorId::Mischief => println!(
            "Skipping mischief. Mischievousness: {:6.4}, Maturity: {:6.4}, Playfulness: {:6.4}, Energy: {:6.4}",
            ctx.mischievousness, ctx.maturity, ctx.playfulness, ctx.energy
        ),
        BehaviorId::Hiding => println!(
            "Skipping hiding. Courage: {:6.4}, Affection: {:6.4}, Energy: {:6.4}",
            ctx.courage, ctx.affection, ctx.energy
        ),
        BehaviorId::Lounging => println!(
            "Skipping lounging. Focus: {:6.4}, Serenity: {:6.4}",
            ctx.focus, ctx.serenity
        ),
        BehaviorId::Startled => println!("Skipping startled. Courage: {:6.4}", ctx.courage),
        BehaviorId::Meandering => println!("Skipping meandering. Energy: {:6.4}", ctx.energy),
        _ => {}
    }
}

fn to_next_default(id: BehaviorId) -> NextBehavior {
    match id {
        BehaviorId::Sleeping => NextBehavior::Sleeping,
        BehaviorId::Napping => NextBehavior::Napping,
        BehaviorId::Zoomies => NextBehavior::Zoomies,
        BehaviorId::Vocalizing => NextBehavior::Vocalizing,
        BehaviorId::Hunting => NextBehavior::Hunting,
        BehaviorId::Investigating => NextBehavior::Investigating,
        BehaviorId::Observing => NextBehavior::Observing,
        BehaviorId::SelfGrooming => NextBehavior::SelfGrooming,
        BehaviorId::Stretching => NextBehavior::Stretching,
        BehaviorId::Pacing => NextBehavior::Pacing,
        BehaviorId::Sulking => NextBehavior::Sulking,
        BehaviorId::Mischief => NextBehavior::Mischief,
        BehaviorId::Hiding => NextBehavior::Hiding,
        BehaviorId::Lounging => NextBehavior::Lounging,
        BehaviorId::Startled => NextBehavior::Startled,
        BehaviorId::Meandering => NextBehavior::Meandering,
        BehaviorId::Idle => NextBehavior::Idle,
        BehaviorId::Chattering => NextBehavior::Chattering,
        _ => NextBehavior::Idle,
    }
}

// can_trigger dispatch, keeps the gate logic centralized.
fn can_trigger(id: BehaviorId, ctx: &GameContext) -> bool {
    match id {
        BehaviorId::Sleeping => SleepingBehavior::can_trigger(ctx),
        BehaviorId::Napping => NappingBehavior::can_trigger(ctx),
        BehaviorId::Zoomies => ZoomiesBehavior::can_trigger(ctx),
        BehaviorId::Vocalizing => VocalizingBehavior::can_trigger(ctx),
        BehaviorId::Hunting => HuntingBehavior::can_trigger(ctx),
        BehaviorId::Investigating => InvestigatingBehavior::can_trigger(ctx),
        BehaviorId::Observing => ObservingBehavior::can_trigger(ctx),
        BehaviorId::SelfGrooming => SelfGroomingBehavior::can_trigger(ctx),
        BehaviorId::Stretching => StretchingBehavior::can_trigger(ctx),
        BehaviorId::Pacing => PacingBehavior::can_trigger(ctx),
        BehaviorId::Sulking => SulkingBehavior::can_trigger(ctx),
        BehaviorId::Mischief => MischiefBehavior::can_trigger(ctx),
        BehaviorId::Hiding => HidingBehavior::can_trigger(ctx),
        BehaviorId::Lounging => LoungingBehavior::can_trigger(ctx),
        BehaviorId::Startled => StartledBehavior::can_trigger(ctx),
        BehaviorId::Meandering => MeanderingBehavior::can_trigger(ctx),
        BehaviorId::Playing => PlayingBehavior::can_trigger(ctx),
        _ => false,
    }
}

fn priority(id: BehaviorId, ctx: &GameContext) -> u32 {
    // The same `ctx.rng` is borrowed mutably by each priority fn via interior
    // copy. They take `&GameContext` and roll using a local RNG view. We use
    // the global rng field via a tiny mutation helper to keep priorities
    // deterministically advanced.
    let mut rng = ctx.rng.wrapping_mul(2654435769).wrapping_add(id as u32);
    match id {
        BehaviorId::Sleeping => SleepingBehavior::priority(ctx, &mut rng),
        BehaviorId::Napping => NappingBehavior::priority(ctx, &mut rng),
        BehaviorId::Zoomies => ZoomiesBehavior::priority(ctx, &mut rng),
        BehaviorId::Vocalizing => VocalizingBehavior::priority(ctx, &mut rng),
        BehaviorId::Hunting => HuntingBehavior::priority(ctx, &mut rng),
        BehaviorId::Investigating => InvestigatingBehavior::priority(ctx, &mut rng),
        BehaviorId::Observing => ObservingBehavior::priority(ctx, &mut rng),
        BehaviorId::SelfGrooming => SelfGroomingBehavior::priority(ctx, &mut rng),
        BehaviorId::Stretching => StretchingBehavior::priority(ctx, &mut rng),
        BehaviorId::Pacing => PacingBehavior::priority(ctx, &mut rng),
        BehaviorId::Sulking => SulkingBehavior::priority(ctx, &mut rng),
        BehaviorId::Mischief => MischiefBehavior::priority(ctx, &mut rng),
        BehaviorId::Hiding => HidingBehavior::priority(ctx, &mut rng),
        BehaviorId::Lounging => LoungingBehavior::priority(ctx, &mut rng),
        BehaviorId::Startled => StartledBehavior::priority(ctx, &mut rng),
        BehaviorId::Playing => PlayingBehavior::priority(ctx, &mut rng),
        _ => 100,
    }
}

