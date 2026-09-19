use embedded_graphics::prelude::Point;
use crate::t;

use crate::{
    assets::character::PoseId,
    behavior::{BehaviorId, BehaviorManager, EatingSource, NextBehavior},
    context::GameContext,
    entities::{character::Character, visitor_cat::VisitorCat},
    environment::{Environment, Layer},
    espnow_msg,
    gardening_ui::{
        PlacementKind, PlacementMode, PlacementResult, PlantBursts, PlantSelectionMode,
        PlantSurface, SelectionFilter, SelectionResult,
    },
    input::{Button, Buttons},
    plant_renderer::draw_plants_layer,
    plant_system::{
        self, fertilize_plant, get_plant, get_plant_mut, move_plant, place_empty_pot,
        plant_in_ground, plant_seed_into_pot, remove_plant, repot_plant, scene_plant_health_score,
        tick_plants, water_plant, PlantLayer,
    },
    render::Renderer,
    scene::SceneId,
    sky::SkyRenderer,
    ui::{
        bubble::{self, BubbleIcon, Corner},
        burst::HEAL_STYLE,
        location_menu::{GardeningAction, LocationAction, LocationMenu, LocationMenuResult},
        popup::Popup,
    },
};

const PAN_SPEED: i32 = 4;
const DISPLAY_WIDTH: i32 = 128;
const FOLLOW_MARGIN: i32 = 32;

/// How long a heard-bubble overlay stays on screen after a `vocalize`
/// frame arrives (5.5 s).
const HEARD_FLASH_DURATION: f32 = 5.5;

/// Cap on consecutive Hearing-triggered hearings before we stop
/// interrupting whatever the cat is doing. Crowd control: if the last
/// 3 entries in `recent_behaviors` are all Hearing, the next vocalize
/// only shows the corner bubble.
const HEARING_CROWD_CAP: usize = 3;

/// How often we re-broadcast the local cat's state during a visit
/// even when nothing changed.
const VST_HEARTBEAT: f32 = 3.0;

/// Minimum on-screen x drift that triggers an out-of-cycle `vst `
/// broadcast. Picks up walking motion smoothly without spamming the
/// radio for every per-frame pixel nudge.
const VST_POS_DELTA: i32 = 4;

/// Shared "entry" anchor used when placing the two pets at the start
/// of a visit. Both devices use the same constant so the absolute x
/// each pet broadcasts in its first `vst ` lands at the position the
/// other device expects.
const VISIT_ANCHOR_X: i32 = 64;

/// Distance from the anchor to each pet at the start of a visit.
/// Inviter is placed at `ANCHOR - GAP`, invitee at `ANCHOR + GAP`;
/// the visitor (the other device's pet) sits at the mirror position.
const VISIT_HALF_GAP: i32 = 20;

/// How long to wait for any `vst ` (or other visit traffic) from the
/// peer before declaring the playdate dead and tearing it down. The
/// peer is sending heartbeats every 3 s, so 10 s is comfortably
/// past three missed beats (a real silence).
const VISIT_SILENCE_TIMEOUT: f32 = 10.0;

/// World-pixel distance at which the inviter triggers a proximity
/// sniff exchange (`vprx`) with the visitor.
const SNIFF_PROXIMITY: i32 = 15;

/// Minimum seconds between consecutive proximity sniffs so the pets
/// don't endlessly head-butt each other.
const SNIFF_COOLDOWN: f32 = 30.0;

/// Walking step offset from the peer's x. The local cat stops a few
/// pixels short of the visitor when greeting so they don't overlap.
const GREETING_APPROACH_GAP: i32 = 8;

/// Inviter to invitee environment-sync broadcast interval.
const VENV_INTERVAL: f32 = 1.0;

#[derive(Clone, Copy)]
struct HeardFlash {
    icon: BubbleIcon,
    corner: Corner,
    remaining: f32,
    /// Time since the flash began, used to drive the y-bob animation.
    elapsed: f32,
}

pub struct LocationScene {
    pub environment: Environment,
    pub character: Character,
    pub sky: SkyRenderer,
    pub behaviors: BehaviorManager,
    menu: LocationMenu,
    menu_active: bool,
    placement: PlacementMode,
    selection: PlantSelectionMode,
    plant_bursts: PlantBursts,
    scene_id: SceneId,
    plant_surfaces: &'static [PlantSurface],
    /// True while the player is chaining tend actions (Water/Fertilize). The
    /// menu re-opens the selection cursor after each one so multiple plants
    /// can be tended in succession without re-navigating from Menu2.
    in_tend_mode: bool,
    last_tended_plant_id: Option<u32>,
    /// Pending seed kind for an in-ground placement flow. Set when the player
    /// picks "Plant Seed -> In Ground -> Rose" before placement starts.
    pending_ground_seed: Option<crate::context::SeedKind>,
    popup: Popup,
    popup_active: bool,
    /// Bubble shown in a screen corner when another nearby device
    /// broadcasts a `vocalize` (`voc `) frame. `None` when no sound is
    /// currently being "heard". Only updated while ESP-NOW is acquired
    /// for this scene.
    heard_flash: Option<HeardFlash>,
    /// The other pet, while a playdate is active. Created on `enter`
    /// when `ctx.visit.is_some()`; destroyed on `exit`. Position and
    /// pose come from inbound `vst ` frames.
    visitor: Option<VisitorCat>,
    /// Last broadcast `vst ` state, used to throttle outbound traffic
    /// to only when something material changed.
    last_vst_x: i32,
    last_vst_pose: PoseId,
    last_vst_vx: f32,
    vst_heartbeat: f32,
    /// X position of the local cat at the start of this frame's
    /// behavior update. `update` uses it to compute the local cat's
    /// instantaneous velocity for outbound `vst ` frames.
    prev_x_for_vx: i32,
    /// Seconds since the last *anything* arrived from the peer
    /// during a visit. Past [`VISIT_SILENCE_TIMEOUT`] we declare the
    /// playdate dead and tear it down without notifying.
    visit_silence_timer: f32,
    /// Last broadcast behavior id. `tick_espnow` compares against
    /// `behaviors.current_id()` and sends `vbeh` on change. Sentinel
    /// `None` means "no broadcast yet this visit".
    last_vbeh: Option<BehaviorId>,
    /// Seconds since the last inviter-to-invitee proximity sniff; the
    /// inviter waits this much before firing another one.
    sniff_cooldown: f32,
    /// Inviter-only: seconds since the last `venv` broadcast.
    venv_timer: f32,
}

impl LocationScene {
    pub fn new(world_width: i32, character_pos: Point) -> Self {
        Self {
            environment: Environment::new(world_width),
            character: Character::new(character_pos),
            sky: SkyRenderer::new(world_width),
            behaviors: BehaviorManager::new(),
            menu: LocationMenu::new(),
            menu_active: false,
            placement: PlacementMode::new(),
            selection: PlantSelectionMode::new(),
            plant_bursts: PlantBursts::new(),
            scene_id: SceneId::Inside,
            plant_surfaces: &[],
            in_tend_mode: false,
            last_tended_plant_id: None,
            pending_ground_seed: None,
            popup: Popup::new(14, 12, 100, 40),
            popup_active: false,
            heard_flash: None,
            visitor: None,
            last_vst_x: i32::MIN,
            last_vst_pose: PoseId::SittingSideNeutral,
            last_vst_vx: 0.0,
            vst_heartbeat: 0.0,
            prev_x_for_vx: 0,
            visit_silence_timer: 0.0,
            last_vbeh: None,
            sniff_cooldown: 0.0,
            venv_timer: 0.0,
        }
    }

    /// Per-frame ESP-NOW dispatch hook. Called from [`Self::update`].
    /// Drains the manager's inbox, processes any `voc ` frames into a
    /// heard-bubble overlay + maybe-Hearing trigger, ticks the
    /// existing flash timer, and broadcasts a `voc ` of our own if
    /// the Vocalizing behavior asked us to.
    ///
    /// Whole method is a no-op when ESP-NOW has not been activated for
    /// this scene (the inbox stays empty and the broadcast no-ops).
    fn tick_espnow(&mut self, ctx: &mut GameContext, dt: f32) {
        // Outgoing: behaviors set `pending_vocalize_broadcast` when
        // they want a sound to travel over the air; we read-and-clear
        // it here so the same vocalization only goes out once.
        if let Some(name) = ctx.pending_vocalize_broadcast.take() {
            if let Some(icon) = BubbleIcon::from_name(name) {
                if let Some(espnow) = ctx.espnow.as_mut() {
                    let frame = espnow_msg::encode_vocalize(icon);
                    espnow.send_broadcast(&frame);
                }
            }
        }

        // Visit silence timeout, checked before broadcast so a dead
        // peer can't keep the heartbeat going forever.
        if ctx.visit.is_some() {
            self.visit_silence_timer += dt;
            if self.visit_silence_timer >= VISIT_SILENCE_TIMEOUT {
                // Peer hasn't been heard in a while. End the visit
                // locally without notifying; they're probably gone.
                crate::espnow_manager::end_visit(ctx, false);
            }
        }

        // Outgoing: visit-state heartbeat. Only relevant while a
        // visit is active. Compute the local cat's instantaneous
        // velocity from the per-frame delta, then broadcast if pose
        // changed, position drifted past the delta threshold, or the
        // heartbeat fired.
        if let Some(visit) = ctx.visit.as_ref() {
            let peer_mac = visit.peer_mac;
            let role = visit.role;
            self.broadcast_vst(ctx, peer_mac, dt);
            self.broadcast_vbeh_on_change(ctx, peer_mac);
            self.maybe_proximity_sniff(ctx, peer_mac, role, dt);
            // Inviter is the authority on time-of-day during a
            // visit. Sending hour/minute/weather/season every ~1 s
            // keeps both screens visually in sync.
            if matches!(role, crate::context::VisitRole::Inviter) {
                self.venv_timer += dt;
                if self.venv_timer >= VENV_INTERVAL {
                    self.venv_timer = 0.0;
                    if let Some(espnow) = ctx.espnow.as_mut() {
                        let frame = espnow_msg::encode_venv(
                            ctx.time_hours,
                            ctx.time_minutes,
                            espnow_msg::weather_to_wire(ctx.weather),
                            espnow_msg::season_to_wire(ctx.season),
                        );
                        espnow.send_to(peer_mac, &frame);
                    }
                }
                // Sky event sync: any locally-spawned shooting star
                // gets relayed to the invitee so both screens see the
                // same streak. The `take_*` API ensures we only
                // broadcast each star once.
                if let Some((sx_pos, sy_pos, max_len, vx, vy)) =
                    self.sky.take_just_spawned_shooting_star()
                {
                    if let Some(espnow) = ctx.espnow.as_mut() {
                        let frame = espnow_msg::encode_vss(
                            sx_pos as i32,
                            sy_pos as i32,
                            max_len as i32,
                            vx as i32,
                            vy as i32,
                        );
                        espnow.send_to(peer_mac, &frame);
                    }
                }
                if let Some((idx, going_right, y, speed)) =
                    self.sky.take_just_spawned_sky_event()
                {
                    if let Some(espnow) = ctx.espnow.as_mut() {
                        let frame = espnow_msg::encode_vse(idx, going_right, y, speed);
                        espnow.send_to(peer_mac, &frame);
                    }
                }
            }
        } else {
            // Reset the vbeh memo so the next visit resends right
            // away rather than skipping if the cat happens to be in
            // the same behavior as the last visit.
            self.last_vbeh = None;
        }

        // Incoming: process every queued frame. `drain` returns an
        // owned vec so the manager borrow ends before we touch other
        // ctx fields (Hearing trigger reads stats, recent_behaviors,
        // etc.).
        let inbox = match ctx.espnow.as_mut() {
            Some(m) => m.drain(),
            None => Default::default(),
        };
        let peer_mac = ctx.visit.as_ref().map(|v| v.peer_mac);
        for item in inbox.iter() {
            if let Some(icon) = espnow_msg::decode_vocalize(&item.data) {
                self.handle_heard_vocalize(ctx, icon);
                continue;
            }
            if let Some(vst) = espnow_msg::decode_vst(&item.data) {
                if Some(item.src) == peer_mac {
                    self.visit_silence_timer = 0.0;
                }
                if let Some(visitor) = self.visitor.as_mut() {
                    visitor.apply_state(vst.x, vst.pose, vst.mirror, vst.vx);
                }
                continue;
            }
            if espnow_msg::matches_tag(espnow_msg::TAG_VBYE, &item.data) {
                if Some(item.src) == peer_mac {
                    // Peer is leaving. End locally without re-notifying.
                    crate::espnow_manager::end_visit(ctx, false);
                }
                continue;
            }
            if Some(item.src) == peer_mac {
                if let Some(peer_id) = espnow_msg::decode_vbeh(&item.data) {
                    self.visit_silence_timer = 0.0;
                    self.handle_peer_vbeh(ctx, peer_id);
                    continue;
                }
                if espnow_msg::matches_tag(espnow_msg::TAG_VGREET, &item.data) {
                    self.visit_silence_timer = 0.0;
                    self.handle_peer_vgreet(ctx);
                    continue;
                }
                if espnow_msg::matches_tag(espnow_msg::TAG_VPROX, &item.data) {
                    self.visit_silence_timer = 0.0;
                    self.handle_peer_vprox(ctx);
                    continue;
                }
                if let Some(env) = espnow_msg::decode_venv(&item.data) {
                    self.visit_silence_timer = 0.0;
                    // Invitee mirrors the inviter's clock + weather
                    // for the duration of the visit. We don't touch
                    // day_number or season_offset, those are local-only.
                    ctx.time_hours = env.hour.min(23);
                    ctx.time_minutes = env.minute.min(59);
                    ctx.weather = espnow_msg::weather_from_wire(env.weather);
                    ctx.season = espnow_msg::season_from_wire(env.season);
                    continue;
                }
                if let Some(ss) = espnow_msg::decode_vss(&item.data) {
                    self.visit_silence_timer = 0.0;
                    self.sky.spawn_shooting_star_from_packet(
                        ss.x as f32,
                        ss.y as f32,
                        ss.max_life as f32,
                        ss.sx as f32,
                        ss.sy as f32,
                    );
                    continue;
                }
                if let Some(se) = espnow_msg::decode_vse(&item.data) {
                    self.visit_silence_timer = 0.0;
                    self.sky.spawn_sky_event_from_packet(
                        se.event_idx,
                        se.going_right,
                        se.y,
                        se.speed,
                    );
                    continue;
                }
                if let Some(target_scene) = espnow_msg::decode_vloc(&item.data) {
                    self.visit_silence_timer = 0.0;
                    if target_scene != self.scene_id {
                        // Stage a scene swap. `update` checks
                        // `ctx.pending_scene` at the bottom of the
                        // frame and returns it; the scene manager
                        // then handles enter/exit. Our own follow-up
                        // `vloc` after enter doesn't cause a ping-pong
                        // because the peer will be in the same scene
                        // by then and the comparison short-circuits.
                        ctx.pending_scene = Some(target_scene);
                    }
                    continue;
                }
            }
        }

        // Drop the visitor cat after any end-visit path (vbye,
        // timeout, or someone else flipped `ctx.visit` to None).
        if ctx.visit.is_none() && self.visitor.is_some() {
            self.visitor = None;
        }

        // Extrapolate the visitor between packets + tick its
        // animation.
        if let Some(visitor) = self.visitor.as_mut() {
            visitor.update(dt);
        }

        // Tick the active flash.
        if let Some(flash) = self.heard_flash.as_mut() {
            flash.remaining -= dt;
            flash.elapsed += dt;
            if flash.remaining <= 0.0 {
                self.heard_flash = None;
            }
        }
    }

    fn broadcast_vst(&mut self, ctx: &mut GameContext, peer_mac: [u8; 6], dt: f32) {
        let cur_x = self.character.pos.x;
        let cur_pose = self.character.pose_id;
        let cur_mirror = self.character.mirror_h;
        // Pixels per second over the last frame. Capped to keep
        // teleporting pets (scene swap, behavior snap) from generating
        // huge velocity spikes the visitor would extrapolate from.

        let raw_vx = if dt > 0.0 {
            (cur_x - self.prev_x_for_vx) as f32 / dt
        } else {
            0.0
        };
        let vx = raw_vx.clamp(-20.0, 20.0);
        // Update the per-frame baseline regardless of whether we
        // ended up broadcasting this tick.
        self.prev_x_for_vx = cur_x;

        self.vst_heartbeat += dt;
        let pose_changed = cur_pose != self.last_vst_pose;
        let pos_changed = self.last_vst_x == i32::MIN
            || (cur_x - self.last_vst_x).abs() >= VST_POS_DELTA;
        let stop_edge = vx == 0.0 && self.last_vst_vx != 0.0;
        let heartbeat = self.vst_heartbeat >= VST_HEARTBEAT;
        if !(pose_changed || pos_changed || stop_edge || heartbeat) {
            return;
        }
        let Some(espnow) = ctx.espnow.as_mut() else {
            return;
        };
        // Ensure the peer is registered for unicast; `start_session`
        // only adds the broadcast peer. Idempotent on repeat calls.
        espnow.add_peer(peer_mac);
        let frame = espnow_msg::encode_vst(cur_x, cur_pose, cur_mirror, vx);
        espnow.send_to(peer_mac, &frame);
        self.last_vst_x = cur_x;
        self.last_vst_pose = cur_pose;
        self.last_vst_vx = vx;
        self.vst_heartbeat = 0.0;
    }

    fn broadcast_vbeh_on_change(&mut self, ctx: &mut GameContext, peer_mac: [u8; 6]) {
        let cur = self.behaviors.current_id();
        if Some(cur) == self.last_vbeh {
            return;
        }
        self.last_vbeh = Some(cur);
        if let Some(espnow) = ctx.espnow.as_mut() {
            let frame = espnow_msg::encode_vbeh(cur);
            espnow.send_to(peer_mac, &frame);
        }
    }

    /// Inviter-only proximity check. When the local cat drifts close
    /// to the visitor and the per-pair cooldown has elapsed, trigger
    /// a sniff locally and tell the invitee to do the same.
    fn maybe_proximity_sniff(
        &mut self,
        ctx: &mut GameContext,
        peer_mac: [u8; 6],
        role: crate::context::VisitRole,
        dt: f32,
    ) {
        if self.sniff_cooldown > 0.0 {
            self.sniff_cooldown -= dt;
        }
        if !matches!(role, crate::context::VisitRole::Inviter) {
            return;
        }
        let Some(visitor) = self.visitor.as_ref() else {
            return;
        };
        if self.sniff_cooldown > 0.0 {
            return;
        }
        let dist = (self.character.pos.x - visitor.pos.x).abs();
        if dist > SNIFF_PROXIMITY {
            return;
        }
        // Don't interrupt a sniff already in progress.
        if self.behaviors.current_id() == BehaviorId::Greeting {
            self.sniff_cooldown = SNIFF_COOLDOWN;
            return;
        }
        // Trigger local sniff in place (no target_x, we're already
        // close), and tell the peer to do the same.
        ctx.pending_greeting_target_x = None;
        self.behaviors
            .trigger(NextBehavior::Greeting, ctx, &mut self.character);
        if let Some(espnow) = ctx.espnow.as_mut() {
            let frame = espnow_msg::encode_empty(espnow_msg::TAG_VPROX);
            espnow.send_to(peer_mac, &frame);
        }
        self.sniff_cooldown = SNIFF_COOLDOWN;
    }

    fn handle_peer_vbeh(&mut self, ctx: &mut GameContext, peer: BehaviorId) {
        if self.behaviors.current_id() == BehaviorId::Greeting {
            // Don't break a greeting/sniff with a mirrored idle.
            return;
        }
        if let Some(next) = mirror_for_peer(peer, ctx) {
            self.behaviors.trigger(next, ctx, &mut self.character);
        }
    }

    fn handle_peer_vgreet(&mut self, ctx: &mut GameContext) {
        // Invitee receives the inviter's greeting kickoff. Walk
        // toward the inviter's spawn position.
        let target_x = self
            .visitor
            .as_ref()
            .map(|v| v.pos.x + GREETING_APPROACH_GAP)
            .unwrap_or(self.character.pos.x);
        if self.behaviors.current_id() != BehaviorId::Greeting {
            ctx.pending_greeting_target_x = Some(target_x);
            self.behaviors
                .trigger(NextBehavior::Greeting, ctx, &mut self.character);
        }
    }

    fn handle_peer_vprox(&mut self, ctx: &mut GameContext) {
        if self.behaviors.current_id() == BehaviorId::Greeting {
            return;
        }
        // Sniff in place.
        ctx.pending_greeting_target_x = None;
        self.behaviors
            .trigger(NextBehavior::Greeting, ctx, &mut self.character);
    }

    fn handle_heard_vocalize(&mut self, ctx: &mut GameContext, icon: BubbleIcon) {
        // Corner is opposite the cat so the bubble doesn't overlap
        // the sprite. World-to-screen accuracy isn't important; the
        // scene midpoint is good enough.
        let mid = (ctx.scene_x_min + ctx.scene_x_max) / 2;
        let corner = if self.character.pos.x < mid {
            Corner::Right
        } else {
            Corner::Left
        };
        self.heard_flash = Some(HeardFlash {
            icon,
            corner,
            remaining: HEARD_FLASH_DURATION,
            elapsed: 0.0,
        });

        // Trigger Hearing behavior unless: cat is already hearing, or
        // the last few behaviors have been mostly Hearing (so we don't
        // pin the pet into an endless reaction loop).
        if self.behaviors.current_id() == BehaviorId::Hearing {
            return;
        }
        let recent_hearing_count = ctx
            .recent_behaviors
            .iter()
            .filter(|b| **b == BehaviorId::Hearing)
            .count();
        if recent_hearing_count >= HEARING_CROWD_CAP {
            return;
        }
        let icon_name = icon_to_name(icon);
        self.behaviors
            .trigger(NextBehavior::Hearing(icon_name), ctx, &mut self.character);
    }

    fn show_popup(&mut self, text: &str) {
        self.popup.set_text(text, true, true);
        self.popup_active = true;
    }

    pub fn menu_active(&self) -> bool {
        self.menu_active
    }

    pub fn plant_overlay_active(&self) -> bool {
        self.placement.active() || self.selection.active()
    }

    /// Per-scene entry point. `plant_surfaces` is the slice of placement
    /// surfaces defined as a const on the scene type (empty slice for scenes
    /// that don't support gardening).
    pub fn enter(
        &mut self,
        ctx: &mut GameContext,
        scene_id: SceneId,
        plant_surfaces: &'static [PlantSurface],
    ) {
        ctx.last_main_scene = scene_id;
        self.scene_id = scene_id;
        self.plant_surfaces = plant_surfaces;
        self.sky.reseed_stars(ctx.pet_seed);
        // Default walkable strip. Scenes that need tighter bounds (e.g.
        // treehouse's tree trunks, bedroom's bed) overwrite these after
        // calling `base.enter`.
        ctx.scene_x_min = 10;
        ctx.scene_x_max = (self.environment.world_width - 10).max(10);
        self.character.reseed_anim();
        self.character.randomize_facing(&mut ctx.rng);
        self.environment
            .set_camera(self.character.pos.x - DISPLAY_WIDTH / 2);
        self.behaviors.start(ctx, &mut self.character);
        self.placement.cancel();
        self.selection.cancel();
        self.in_tend_mode = false;
        self.pending_ground_seed = None;

        // Deterministic placement during a visit: the inviter's cat
        // is always on the left at `ANCHOR - GAP`, the invitee's is
        // always on the right at `ANCHOR + GAP`. Both devices use the
        // same constants so the first `vst ` from the peer lands at
        // the position we already drew them at. No jump, no
        // randomness. mirror_h has them facing inward toward each
        // other (cats face left by default; `mirror_h = true` flips
        // to face right).
        if let Some(visit) = ctx.visit.as_ref() {
            const INVITER_X: i32 = VISIT_ANCHOR_X - VISIT_HALF_GAP;
            const INVITEE_X: i32 = VISIT_ANCHOR_X + VISIT_HALF_GAP;
            let (local_x, visitor_x, local_face_right, visitor_face_right) = match visit.role {
                crate::context::VisitRole::Inviter => (INVITER_X, INVITEE_X, true, false),
                crate::context::VisitRole::Invitee => (INVITEE_X, INVITER_X, false, true),
            };
            self.character.pos.x = local_x;
            self.character.mirror_h = local_face_right;
            let pos = Point::new(visitor_x, self.character.pos.y);
            let mut v = VisitorCat::new(pos);
            v.mirror_h = visitor_face_right;
            self.visitor = Some(v);
            // Camera was set from the (pre-visit) character x above.
            // Re-center on the new x.
            self.environment
                .set_camera(self.character.pos.x - DISPLAY_WIDTH / 2);
        } else {
            self.visitor = None;
        }
        // Reset the vst broadcast throttles so the very first frame
        // after enter is guaranteed to broadcast our pose+position
        // (sentinel `i32::MIN` triggers `pos_changed`).
        self.last_vst_x = i32::MIN;
        self.last_vst_pose = self.character.pose_id;
        self.last_vst_vx = 0.0;
        self.vst_heartbeat = VST_HEARTBEAT; // force first-frame broadcast
        self.prev_x_for_vx = self.character.pos.x;
        self.visit_silence_timer = 0.0;
        self.last_vbeh = None;
        self.sniff_cooldown = 0.0;

        // Broadcast our current scene to the peer so they can follow.
        // Only valid for `LocationScene`-backed scenes; non-encodable
        // scene ids (minigames etc.) just skip the broadcast.
        if let Some(visit) = ctx.visit.as_ref() {
            let peer_mac = visit.peer_mac;
            if let Some(frame) = espnow_msg::encode_vloc(scene_id) {
                if let Some(espnow) = ctx.espnow.as_mut() {
                    espnow.send_to(peer_mac, &frame);
                }
            }
        }

        // Greeting ritual on the very first scene entry of a visit
        // (`greeted == false`). Both cats walk toward the anchor; the
        // inviter (on the left) sends `vgrt` so the invitee mirrors
        // the trigger.
        if let Some(visit) = ctx.visit.as_ref() {
            if !visit.greeted {
                let target_x = match visit.role {
                    crate::context::VisitRole::Inviter => {
                        VISIT_ANCHOR_X - GREETING_APPROACH_GAP
                    }
                    crate::context::VisitRole::Invitee => {
                        VISIT_ANCHOR_X + GREETING_APPROACH_GAP
                    }
                };
                let peer_mac = visit.peer_mac;
                let role = visit.role;
                ctx.pending_greeting_target_x = Some(target_x);
                self.behaviors.trigger(
                    NextBehavior::Greeting,
                    ctx,
                    &mut self.character,
                );
                if matches!(role, crate::context::VisitRole::Inviter) {
                    if let Some(espnow) = ctx.espnow.as_mut() {
                        let frame = espnow_msg::encode_empty(espnow_msg::TAG_VGREET);
                        espnow.send_to(peer_mac, &frame);
                    }
                }
                if let Some(visit) = ctx.visit.as_mut() {
                    visit.greeted = true;
                }
            }
        }

        // Honor a pending cross-scene plant move arriving at this scene.
        if let Some(pending) = ctx.pending_gardening_move {
            if pending.dest_scene == scene_id && !plant_surfaces.is_empty() {
                if let Some(plant) = get_plant(ctx, pending.plant_id) {
                    let pot = plant.pot;
                    let id = plant.id;
                    if pot != crate::assets::plants::PotKind::Ground {
                        self.placement.enter(
                            PlacementKind::Move { pot, plant_id: id },
                            plant_surfaces,
                            &self.environment,
                        );
                    }
                }
                ctx.pending_gardening_move = None;
            }
        }
    }

    /// World-tick used by callers (e.g. the big-menu overlay) that want the
    /// scene to keep advancing while input is being consumed elsewhere. Runs
    /// Tell the active behavior to wind down quickly (wake from device sleep).
    /// Forwarded by the `Scene` trait method of the same name.
    pub fn mark_behavior_almost_done(&mut self, ctx: &mut GameContext) {
        self.behaviors.mark_almost_done(ctx);
    }

    /// the same world updates as `update` but without reading the player's
    /// buttons, dispatching menu/placement input, or returning a scene swap.
    pub fn tick_background(&mut self, ctx: &mut GameContext, dt: f32) {
        ctx.scene_camera_x = self.environment.camera_offset(Layer::Foreground);
        ctx.input.left = false;
        ctx.input.right = false;
        ctx.input.up = false;
        ctx.input.down = false;
        ctx.input.a = false;
        ctx.input.b = false;
        ctx.input.a_just_pressed = false;
        ctx.input.b_just_pressed = false;

        let score = scene_plant_health_score(ctx, self.scene_id).clamp(-100, 100);
        ctx.scene_plant_health = score as i8;

        self.sky.update(ctx, dt);
        tick_plants(ctx);
        let prev_x = self.character.pos.x;
        self.behaviors.update(ctx, &mut self.character, dt);
        let pose = self.behaviors.current_pose();
        self.character.set_pose(pose);
        self.character.animate(dt);
        self.character.eye_override = self.behaviors.current_eye_frame_override();
        self.plant_bursts.update(dt);
        if self.placement.active() {
            self.placement.update(dt);
        }
        if self.selection.active() {
            self.selection.update(dt);
        }

        // Auto-follow: player isn't panning (they're in the overlay), so the
        // camera tracks the cat normally if a behavior nudged it.
        let cursor_active = self.placement.active() || self.selection.active();
        if !cursor_active && self.character.pos.x != prev_x {
            let screen_x = self.character.pos.x - self.environment.camera_x;
            if screen_x < FOLLOW_MARGIN {
                self.environment
                    .set_camera(self.character.pos.x - FOLLOW_MARGIN);
            } else if screen_x > DISPLAY_WIDTH - FOLLOW_MARGIN {
                self.environment
                    .set_camera(self.character.pos.x - (DISPLAY_WIDTH - FOLLOW_MARGIN));
            }
        }
    }

    pub fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        ctx.scene_camera_x = self.environment.camera_offset(Layer::Foreground);
        ctx.input.left = buttons.is_pressed(Button::Left);
        ctx.input.right = buttons.is_pressed(Button::Right);
        ctx.input.up = buttons.is_pressed(Button::Up);
        ctx.input.down = buttons.is_pressed(Button::Down);
        ctx.input.a = buttons.is_pressed(Button::A);
        ctx.input.b = buttons.is_pressed(Button::B);
        // `was_just_pressed` is consume-once: calling it here marks the edge
        // as seen and prevents downstream readers (menu, placement, selection)
        // from observing the same press. Only sample for behaviors when there
        // is no overlay competing for the press.
        let overlay_active = self.menu_active
            || self.placement.active()
            || self.selection.active()
            || self.popup_active;
        ctx.input.a_just_pressed = !overlay_active && buttons.was_just_pressed(Button::A);
        ctx.input.b_just_pressed = !overlay_active && buttons.was_just_pressed(Button::B);

        // Refresh the aggregate plant-health score behaviors read from ctx.
        let score = scene_plant_health_score(ctx, self.scene_id).clamp(-100, 100);
        ctx.scene_plant_health = score as i8;

        // World ticks regardless of menu state.
        self.sky.update(ctx, dt);
        // Advance plants once per in-game hour (catches up if many hours elapsed).
        tick_plants(ctx);
        // ESP-NOW: drain any inbound vocalizations from nearby devices
        // and emit our own pending broadcast. No-op when the scene has
        // not acquired the radio (see outside/treehouse `enter`).
        self.tick_espnow(ctx, dt);
        let prev_x = self.character.pos.x;
        self.behaviors.update(ctx, &mut self.character, dt);
        let pose = self.behaviors.current_pose();
        self.character.set_pose(pose);
        self.character.animate(dt);
        self.character.eye_override = self.behaviors.current_eye_frame_override();
        self.plant_bursts.update(dt);
        if self.placement.active() {
            self.placement.update(dt);
        }
        if self.selection.active() {
            self.selection.update(dt);
        }

        // Auto-follow: when the cat moves on its own, nudge the camera so it
        // stays within FOLLOW_MARGIN of the screen edges. Suppressed while the
        // player is actively panning or steering plant cursors.
        let panning = buttons.is_pressed(Button::Left) || buttons.is_pressed(Button::Right);
        let cursor_active = self.placement.active() || self.selection.active();
        if !panning && !cursor_active && self.character.pos.x != prev_x {
            let screen_x = self.character.pos.x - self.environment.camera_x;
            if screen_x < FOLLOW_MARGIN {
                self.environment
                    .set_camera(self.character.pos.x - FOLLOW_MARGIN);
            } else if screen_x > DISPLAY_WIDTH - FOLLOW_MARGIN {
                self.environment
                    .set_camera(self.character.pos.x - (DISPLAY_WIDTH - FOLLOW_MARGIN));
            }
        }

        // Popup messages are modal: A or B dismisses, and if the player was
        // tending we re-enter the selection cursor for the next action.
        if self.popup_active {
            if buttons.was_just_pressed(Button::A)
                || buttons.was_just_pressed(Button::B)
                || buttons.was_just_pressed(Button::Menu1)
                || buttons.was_just_pressed(Button::Menu2)
            {
                self.popup_active = false;
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
            } else if buttons.was_just_pressed(Button::Down) {
                self.popup.scroll_down();
            } else if buttons.was_just_pressed(Button::Up) {
                self.popup.scroll_up();
            }
            return None;
        }

        // Cursor modes take precedence over the menu so the player can confirm
        // placement / selection without first dismissing the menu.
        if self.placement.active() {
            return self.handle_placement_input(ctx, buttons);
        }
        if self.selection.active() {
            return self.handle_selection_input(ctx, buttons);
        }

        if self.menu_active {
            return self.handle_menu_input(ctx, buttons);
        }

        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }
        if buttons.was_just_pressed(Button::Menu2) {
            self.menu.open(
                ctx,
                self.scene_id,
                !self.plant_surfaces.is_empty(),
                ctx.on_vacation,
            );
            self.menu_active = true;
            return None;
        }

        if !self.behaviors.current_captures_dpad() {
            if buttons.is_pressed(Button::Left) {
                self.environment.pan(-PAN_SPEED);
            }
            if buttons.is_pressed(Button::Right) {
                self.environment.pan(PAN_SPEED);
            }
        }

        if let Some(next) = ctx.pending_scene.take() {
            return Some(next);
        }
        None
    }

    fn handle_menu_input(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
    ) -> Option<SceneId> {
        match self.menu.handle_input(ctx, buttons) {
            LocationMenuResult::Continue => None,
            LocationMenuResult::Closed => {
                self.menu_active = false;
                self.in_tend_mode = false;
                None
            }
            LocationMenuResult::Action(action) => {
                self.menu_active = false;
                self.apply_menu_action(ctx, action)
            }
        }
    }

    fn handle_placement_input(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
    ) -> Option<SceneId> {
        let kind = self.placement.current_kind();
        match self.placement.handle_input(buttons, &mut self.environment) {
            PlacementResult::Continue => None,
            PlacementResult::Cancelled => {
                self.pending_ground_seed = None;
                // Re-enter tend selection if we cancelled out of a within-scene
                // move; otherwise just drop back to play.
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
                None
            }
            PlacementResult::Confirm { layer, x, y_snap } => {
                self.commit_placement(ctx, kind, layer, x, y_snap);
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
                None
            }
        }
    }

    fn commit_placement(
        &mut self,
        ctx: &mut GameContext,
        kind: PlacementKind,
        layer: PlantLayer,
        x: i32,
        y_snap: i32,
    ) {
        match kind {
            PlacementKind::Pot(pot) => {
                // Cap to 16 plants per scene.
                let scene_count = ctx.plants.iter().filter(|p| p.scene == self.scene_id).count();
                if scene_count < 16 {
                    let _ = place_empty_pot(ctx, self.scene_id, layer, x, y_snap, pot);
                }
            }
            PlacementKind::Ground => {
                if let Some(seed) = self.pending_ground_seed.take() {
                    let scene_count =
                        ctx.plants.iter().filter(|p| p.scene == self.scene_id).count();
                    if scene_count < 16 {
                        let _ = plant_in_ground(ctx, self.scene_id, layer, x, y_snap, seed);
                    }
                }
            }
            PlacementKind::GroundSeed => {
                if let Some(seed) = self.pending_ground_seed.take() {
                    let scene_count =
                        ctx.plants.iter().filter(|p| p.scene == self.scene_id).count();
                    if scene_count < 16 {
                        let _ = plant_in_ground(ctx, self.scene_id, layer, x, y_snap, seed);
                    }
                }
            }
            PlacementKind::Move { plant_id, .. } => {
                move_plant(ctx, plant_id, self.scene_id, layer, x, y_snap);
            }
        }
    }

    fn handle_selection_input(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
    ) -> Option<SceneId> {
        match self.selection.handle_input(ctx, buttons, &mut self.environment) {
            SelectionResult::Continue => None,
            SelectionResult::Cancelled => {
                self.in_tend_mode = false;
                self.pending_ground_seed = None;
                None
            }
            SelectionResult::Confirm(id) => {
                self.handle_selection_confirm(ctx, id);
                None
            }
        }
    }

    fn handle_selection_confirm(&mut self, ctx: &mut GameContext, id: u32) {
        // Selection is reused for two flows: tending an existing plant, and
        // dropping a seed into an empty pot. Inspect which mode we're in by
        // looking at whether a pending seed was queued.
        if let Some(seed) = self.pending_ground_seed.take() {
            // We were planting a seed; this id is the empty pot.
            let _ = plant_seed_into_pot(ctx, id, seed);
            return;
        }
        // Otherwise: open the dynamic Tend menu for this plant.
        self.last_tended_plant_id = Some(id);
        self.in_tend_mode = true;
        self.menu.open_tend(ctx, self.scene_id, id);
        self.menu_active = true;
    }

    /// Re-open the plant-selection cursor for the next tend action. Drops out
    /// of tend mode if no plants remain in the scene.
    fn reenter_tend_selection(&mut self, ctx: &GameContext) {
        let found = self.selection.enter(
            ctx,
            self.scene_id,
            SelectionFilter::All,
            self.last_tended_plant_id,
        );
        if !found {
            self.in_tend_mode = false;
        }
    }

    fn apply_menu_action(
        &mut self,
        ctx: &mut GameContext,
        action: LocationAction,
    ) -> Option<SceneId> {
        match action {
            LocationAction::Affection(variant) => {
                self.behaviors.trigger(
                    NextBehavior::Affection(variant),
                    ctx,
                    &mut self.character,
                );
            }
            LocationAction::Attention(variant) => {
                self.behaviors.trigger(
                    NextBehavior::Attention(variant),
                    ctx,
                    &mut self.character,
                );
            }
            LocationAction::Eat(item) => {
                let idx = item as usize;
                if ctx.food_stock[idx] > 0 {
                    ctx.food_stock[idx] -= 1;
                }
                let screen_x = self.character.pos.x - self.environment.camera_x;
                self.character.mirror_h = screen_x < 64;
                self.behaviors.trigger(
                    NextBehavior::Eating(EatingSource::Item(item)),
                    ctx,
                    &mut self.character,
                );
            }
            LocationAction::Groom => {
                self.behaviors
                    .trigger(NextBehavior::BeingGroomed, ctx, &mut self.character);
            }
            LocationAction::Train(kind) => {
                self.behaviors.trigger(
                    NextBehavior::Training(kind),
                    ctx,
                    &mut self.character,
                );
            }
            LocationAction::Play(variant) => {
                self.behaviors.trigger(
                    NextBehavior::Playing(variant),
                    ctx,
                    &mut self.character,
                );
            }
            LocationAction::Medicine => {
                if ctx.medicine > 0 {
                    let first_dose = !ctx.medicine_pending;
                    ctx.medicine -= 1;
                    ctx.medicine_pending = true;
                    if first_dose {
                        self.character.play_bursts_styled(&mut ctx.rng, 10, HEAL_STYLE);
                    }
                }
            }
            LocationAction::GoToStore => return Some(SceneId::Store),
            LocationAction::Gardening(action) => return self.apply_gardening_action(ctx, action),
            LocationAction::GoHome => return Some(SceneId::Inside),
        }
        None
    }

    fn apply_gardening_action(
        &mut self,
        ctx: &mut GameContext,
        action: GardeningAction,
    ) -> Option<SceneId> {
        match action {
            GardeningAction::PlacePot(pot) => {
                self.placement.enter(
                    PlacementKind::Pot(pot),
                    self.plant_surfaces,
                    &self.environment,
                );
            }
            GardeningAction::PlantSeedInPot(seed) => {
                if !ctx.owns_tool(crate::context::ToolKind::Spade) {
                    self.show_popup(t!("You need the Spade to plant seeds. Buy one at the store!"));
                    return None;
                }
                self.pending_ground_seed = Some(seed);
                let found =
                    self.selection
                        .enter(ctx, self.scene_id, SelectionFilter::EmptyPot, None);
                if !found {
                    self.pending_ground_seed = None;
                    self.show_popup(t!("No empty pots in this location"));
                }
            }
            GardeningAction::PlantSeedInGround(seed) => {
                if !ctx.owns_tool(crate::context::ToolKind::Spade) {
                    self.show_popup(t!("You need the Spade to plant seeds. Buy one at the store!"));
                    return None;
                }
                self.pending_ground_seed = Some(seed);
                self.placement.enter(
                    PlacementKind::Ground,
                    self.plant_surfaces,
                    &self.environment,
                );
            }
            GardeningAction::StartTend => {
                self.in_tend_mode = true;
                let found = self.selection.enter(
                    ctx,
                    self.scene_id,
                    SelectionFilter::All,
                    self.last_tended_plant_id,
                );
                if !found {
                    self.in_tend_mode = false;
                }
            }
            GardeningAction::Water(id) => {
                if !ctx.owns_tool(crate::context::ToolKind::WateringCan) {
                    self.show_popup(
                        t!("You need the Watering Can to water plants. Buy one at the store!"),
                    );
                    return None;
                }
                if let Some(p) = get_plant_mut(ctx, id) {
                    water_plant(p);
                    let mut rng = ctx.rng;
                    self.plant_bursts.trigger(id, &mut rng, 4);
                    ctx.rng = rng;
                }
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
            }
            GardeningAction::Fertilize(id) => {
                if ctx.fertilizer > 0 {
                    if let Some(p) = get_plant_mut(ctx, id) {
                        fertilize_plant(p);
                        ctx.fertilizer = ctx.fertilizer.saturating_sub(1);
                        let mut rng = ctx.rng;
                        self.plant_bursts.trigger(id, &mut rng, 4);
                        ctx.rng = rng;
                    }
                }
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
            }
            GardeningAction::Pluck(id) => {
                remove_plant(ctx, id);
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
            }
            GardeningAction::Repot(id, target) => {
                repot_plant(ctx, id, target);
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
            }
            GardeningAction::MoveHere(id) => {
                if let Some(plant) = get_plant(ctx, id) {
                    let pot = plant.pot;
                    if pot != crate::assets::plants::PotKind::Ground {
                        self.placement.enter(
                            PlacementKind::Move { pot, plant_id: id },
                            self.plant_surfaces,
                            &self.environment,
                        );
                    }
                }
            }
            GardeningAction::MoveTo(id, dest) => {
                ctx.pending_gardening_move = Some(crate::context::PendingGardeningMove {
                    plant_id: id,
                    dest_scene: dest,
                });
                self.in_tend_mode = false;
                return Some(dest);
            }
            GardeningAction::InspectDismiss => {
                if self.in_tend_mode {
                    self.reenter_tend_selection(ctx);
                }
            }
        }
        None
    }

    pub fn draw_sky(&self, renderer: &mut Renderer, ctx: &GameContext) {
        renderer.set_invert(self.sky.lightning_invert());
        self.sky.draw(renderer, ctx, self.environment.camera_x);
    }

    pub fn draw_layers(&self, renderer: &mut Renderer) {
        self.environment.draw_all_layers(renderer);
    }

    pub fn draw_plants(&self, ctx: &GameContext, renderer: &mut Renderer, layer: PlantLayer) {
        if self.plant_surfaces.is_empty() {
            return;
        }
        draw_plants_layer(ctx, renderer, &self.environment, self.scene_id, layer);
    }

    pub fn draw_character(&self, renderer: &mut Renderer, ctx: &GameContext) {
        let camera_offset = self.environment.camera_offset(Layer::Foreground);
        self.character.draw(renderer, camera_offset);
        // Visitor pet sits behind the local cat's overlays but on top
        // of the base sprite. Same z-order as having two siblings on
        // the same layer.
        if let Some(visitor) = self.visitor.as_ref() {
            visitor.draw(renderer, camera_offset);
        }
        let screen = Point::new(
            self.character.pos.x - camera_offset,
            self.character.pos.y,
        );
        self.behaviors
            .draw_overlay(renderer, ctx, screen, self.character.mirror_h);
        self.character.draw_sick_overlay(renderer, camera_offset, ctx);
        self.character.draw_bursts(renderer, screen);
        self.plant_bursts.draw(ctx, renderer, &self.environment);
    }

    pub fn draw_menu(&self, renderer: &mut Renderer) {
        self.menu.draw(renderer);
    }

    pub fn draw_overlay(&self, ctx: &GameContext, renderer: &mut Renderer) {
        // Cursor overlays (placement reticle, selection marker). Drawn last so
        // they sit above sprites and plants.
        self.placement.draw(renderer, &self.environment);
        self.selection.draw(ctx, renderer, &self.environment);
        if let Some(flash) = self.heard_flash {
            use micromath::F32Ext;
            // Sinusoidal bob using `sin(elapsed * 9.42) * 3`.
            let y_offset = ((flash.elapsed * 9.42).sin() * 3.0) as i32;
            bubble::draw_heard(renderer, flash.icon, flash.corner, y_offset);
        }
        if self.popup_active {
            self.popup.draw(renderer, true);
        }
    }
}

// Reuse plant_system helpers in other modules via re-export.
#[allow(unused_imports)]
pub use plant_system::scene_plant_health_score as plant_health;

/// "Mirror" table: given the peer's freshly-broadcast behavior id,
/// returns a [`NextBehavior`] we should consider triggering locally,
/// or `None` if the peer's behavior shouldn't propagate. The
/// returned behavior is gated by a probability + stat check, both
/// rolled here using `ctx.rng`. Entries and weights are tuned for
/// social-feel.
fn mirror_for_peer(peer: BehaviorId, ctx: &mut GameContext) -> Option<NextBehavior> {
    use crate::rand::rand_f32;
    let roll = rand_f32(&mut ctx.rng);
    match peer {
        BehaviorId::Zoomies if ctx.playfulness > 50.0 && roll < 0.40 => {
            Some(NextBehavior::Zoomies)
        }
        BehaviorId::Sleeping if ctx.energy < 60.0 && roll < 0.50 => {
            Some(NextBehavior::Napping)
        }
        BehaviorId::Napping if ctx.energy < 60.0 && roll < 0.40 => {
            Some(NextBehavior::Napping)
        }
        BehaviorId::Eating if ctx.fullness < 70.0 && roll < 0.60 => {
            Some(NextBehavior::Vocalizing)
        }
        BehaviorId::Vocalizing if roll < 0.30 => Some(NextBehavior::Vocalizing),
        BehaviorId::Stretching if roll < 0.30 => Some(NextBehavior::Stretching),
        BehaviorId::SelfGrooming if roll < 0.40 => Some(NextBehavior::SelfGrooming),
        BehaviorId::Lounging if roll < 0.40 => Some(NextBehavior::Lounging),
        BehaviorId::Investigating if roll < 0.40 => Some(NextBehavior::Investigating),
        BehaviorId::Observing if roll < 0.35 => Some(NextBehavior::Observing),
        BehaviorId::Chattering if roll < 0.35 => Some(NextBehavior::Chattering),
        _ => None,
    }
}

/// Map a [`BubbleIcon`] to the `&'static str` the Hearing behavior
/// expects. Inverse of [`BubbleIcon::from_name`]. Used when a received
/// `voc ` frame needs to be handed off to the behavior layer.
fn icon_to_name(icon: BubbleIcon) -> Option<&'static str> {
    Some(match icon {
        BubbleIcon::Heart => "heart",
        BubbleIcon::Question => "question",
        BubbleIcon::Exclaim => "exclaim",
        BubbleIcon::Note => "note",
        BubbleIcon::Star => "star",
        BubbleIcon::Hunger => "hunger",
        BubbleIcon::Discomfort => "discomfort",
        BubbleIcon::Bored => "bored",
        BubbleIcon::Lonely => "lonely",
        BubbleIcon::Home => "home",
        BubbleIcon::Hot => "hot",
        BubbleIcon::Wet => "wet",
        BubbleIcon::Cold => "cold",
    })
}
