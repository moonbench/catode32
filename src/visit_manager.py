"""visit_manager.py - Runtime visit interaction logic.

Owned by the Game object (parallel to EspNowHandler). Manages:
  - VisitorCatEntity lifecycle (creation, position, animation)
  - Visitor speech bubble (vocalization, greeting, proximity sniff)
  - State broadcasts: vst (pose/position), venv (time-of-day), vbeh (behavior)
  - Proximity-sniff detection (inviter-only, to avoid both sides firing at once)
  - Vocalization reply timer
  - Behavior mirroring (via inbound vbeh messages)
  - Greeting and proximity-sniff rituals (via inbound vgreet/vprox messages)

Call from main_scene (thin hooks): on_scene_enter(scene), on_scene_exit()
Call from EspNowHandler:           handle_msg(mac, msg_type, payload)
Call from game loop (after scene_manager.update): update(dt)
"""

import random

# Seconds between vst heartbeats (pose/position change always triggers immediately)
_VST_HEARTBEAT = 3.0

# Inviter sends time-of-day sync once per second; weather/season sent on scene entry
_VENV_INTERVAL = 1.0

# Used to size the visitor bubble during greeting/sniff phases (seconds)
_SNIFF_DURATION = 3.5

# Minimum gap between proximity-sniff triggers (seconds)
_SNIFF_COOLDOWN_SECS = 30.0

# Proximity threshold in world pixels to trigger a sniff
_SNIFF_PROXIMITY = 15


class VisitManager:
    """Manages all runtime visit interaction state and logic."""

    # Behaviors worth broadcasting so the peer can decide to mirror
    _BROADCASTABLE_BEHAVIORS = frozenset((
        'zoomies', 'sleeping', 'napping', 'lounging', 'eating',
        'being_groomed', 'sulking', 'self_grooming', 'vocalizing',
    ))

    # Behaviors that block proximity sniff triggering
    _NO_SNIFF_BEHAVIORS = frozenset((
        'sleeping', 'eating', 'being_groomed', 'training', 'playing',
        'greeting', 'go_to',
    ))

    # Behavior mirroring: peer_behavior -> (local_behavior, probability, stat_check_fn)
    # stat_check_fn receives context and returns True if eligible; None means always eligible.
    _MIRROR_TABLE = {
        'zoomies':       ('zoomies',       0.40, lambda ctx: ctx.playfulness > 60),
        'sleeping':      ('napping',       0.50, lambda ctx: ctx.energy < 50),
        'napping':       ('napping',       0.40, lambda ctx: ctx.energy < 60),
        'lounging':      ('lounging',      0.35, None),
        'eating':        ('vocalizing',    0.60, lambda ctx: ctx.fullness < 65),
        'being_groomed': ('self_grooming', 0.30, None),
        'sulking':       ('vocalizing',    0.40, lambda ctx: ctx.sociability > 50),
        'self_grooming': ('self_grooming', 0.25, None),
        'vocalizing':    ('vocalizing',    0.30, None),
    }

    # Behaviors that block mirroring (don't interrupt these)
    _NO_MIRROR_BEHAVIORS = frozenset((
        'sleeping', 'eating', 'being_groomed', 'training', 'playing',
        'greeting', 'sniffing',
    ))

    def __init__(self, context, scene_manager):
        self._context = context
        self._scene_manager = scene_manager
        self._scene = None          # set by on_scene_enter; cleared by on_scene_exit

        # Public: read by main_scene.draw() for render integration
        self.visitor_cat = None     # VisitorCatEntity or None
        self.visitor_bubble = None  # (icon, remaining_secs, max_secs) or None

        # Broadcast state
        self._last_bcast_behavior = None
        self._last_broadcast_pose = None
        self._last_broadcast_x = None
        self._last_broadcast_vx = 0
        self._prev_char_x = None
        self._char_vx = 0.0
        self._vst_timer = 0.0
        self._venv_timer = 0.0

        # Interaction state
        self._sniff_cooldown = 0.0
        self._voc_reply_timer = 0.0

    # ------------------------------------------------------------------
    # Scene hooks (called from main_scene.enter / main_scene.exit)
    # ------------------------------------------------------------------

    def on_scene_enter(self, scene):
        """Set up visitor cat, send vloc/venv, and trigger the greeting ritual."""
        ctx = self._context
        self._scene = scene

        # Reset per-scene state
        self._last_bcast_behavior = None
        self.visitor_bubble = None
        self._sniff_cooldown = 0.0
        self._voc_reply_timer = 0.0
        self._last_broadcast_pose = None
        self._last_broadcast_x = None
        self._last_broadcast_vx = 0
        self._prev_char_x = None
        self._char_vx = 0.0
        self._vst_timer = 0.0
        self._venv_timer = 0.0

        if ctx.visit is None:
            self.visitor_cat = None
            return

        # Create visitor cat entity on the opposite side of the local cat
        from entities.visitor_cat import VisitorCatEntity
        y = int(scene.character.y) if scene.character else 64
        entry_x = getattr(scene, 'ENTRY_X', 64)
        if ctx.visit.get('role') == 'inviter':
            visitor_x = entry_x + 20
        else:
            visitor_x = entry_x - 20
        self.visitor_cat = VisitorCatEntity(visitor_x, y)

        # Tell the peer which scene we just entered
        scene_name = getattr(scene, 'SCENE_NAME', None)
        if scene_name and ctx.espnow:
            ctx.espnow.send_to(ctx.visit['peer_mac'], 'vloc', {'s': scene_name})

            # Inviter is authoritative for sky — push full environment on every entry
            # so weather/time changes (which re-trigger enter) propagate to the peer
            if ctx.visit.get('role') == 'inviter':
                env = ctx.environment
                ctx.espnow.send_to(ctx.visit['peer_mac'], 'venv', {
                    'h':  env.get('time_hours', 12),
                    'mn': env.get('time_minutes', 0),
                    'w':  env.get('weather', 'Clear'),
                    's':  env.get('season', 'Spring'),
                    'mp': env.get('moon_phase', 'Full'),
                })

        # Greeting ritual: only on the very first scene entry of this visit
        if not ctx.visit.get('greeted'):
            ctx.visit['greeted'] = True
            role = ctx.visit.get('role')
            if role == 'inviter':
                # Inviter walks left toward the center (started at ENTRY_X-20)
                target_x = entry_x - 8
                scene.character.trigger('greeting',
                                        target_x=target_x,
                                        sniff_pose='leaning_forward.side.stretch')
                if ctx.espnow:
                    ctx.espnow.send_to(ctx.visit['peer_mac'], 'vgreet', {})

    def on_scene_exit(self):
        """Tear down visitor cat and reset all broadcast state."""
        self.visitor_cat = None
        self.visitor_bubble = None
        self._scene = None
        self._last_broadcast_pose = None
        self._last_broadcast_x = None
        self._last_broadcast_vx = 0
        self._prev_char_x = None
        self._char_vx = 0.0
        self._vst_timer = 0.0
        self._venv_timer = 0.0
        self._last_bcast_behavior = None
        self._sniff_cooldown = 0.0
        self._voc_reply_timer = 0.0

    # ------------------------------------------------------------------
    # Game-loop update (called after scene_manager.update)
    # ------------------------------------------------------------------

    def update(self, dt):
        """Tick visitor cat, sync to peer, and drive playdate interactions."""
        ctx = self._context
        scene = self._scene
        if scene is None or not hasattr(scene, 'character'):
            return

        # Compute character velocity from position delta (character has already moved this frame)
        cur_x = scene.character.x
        if self._prev_char_x is not None and dt > 0:
            self._char_vx = (cur_x - self._prev_char_x) / dt
        else:
            self._char_vx = 0.0
        self._prev_char_x = cur_x

        sky = getattr(scene, 'sky', None)

        if ctx.visit is not None:
            # Lazily create visitor cat if missing (e.g. visit started mid-scene)
            if self.visitor_cat is None:
                from entities.visitor_cat import VisitorCatEntity
                y = int(scene.character.y) if scene.character else 64
                self.visitor_cat = VisitorCatEntity(96, y)

            self.visitor_cat.update(dt)
            self._broadcast_vst(dt, scene)
            self._broadcast_venv(dt)
            self._update_playdate(dt, scene)
            self._tick_voc_reply(dt, scene)

            if sky:
                is_inviter = ctx.visit.get('role') == 'inviter'
                sky.suppress_auto_spawns = not is_inviter
                if sky.pending_events:
                    if is_inviter and ctx.espnow:
                        for evt_type, params in sky.pending_events:
                            msg = 'vss' if evt_type == 'ss' else 'vse'
                            ctx.espnow.send_to(ctx.visit['peer_mac'], msg, params)
                    sky.pending_events.clear()
        else:
            if self.visitor_cat is not None:
                self.visitor_cat = None
            if sky:
                sky.suppress_auto_spawns = False
                sky.pending_events.clear()

    # ------------------------------------------------------------------
    # ESP-NOW message handler (called from EspNowHandler.dispatch)
    # ------------------------------------------------------------------

    def handle_msg(self, mac, msg_type, payload):
        """Dispatch a visit-related inbound ESP-NOW message."""
        if msg_type == 'vst':
            self._handle_vst(mac, payload)
        elif msg_type == 'vbeh':
            self._handle_vbeh(mac, payload)
        elif msg_type == 'vgreet':
            self._handle_vgreet(mac, payload)
        elif msg_type == 'vprox':
            self._handle_vprox(mac, payload)
        elif msg_type == 'vocalize':
            self._handle_vocalize(mac, payload)

    # ------------------------------------------------------------------
    # Private: inbound handlers
    # ------------------------------------------------------------------

    def _handle_vst(self, mac, payload):
        """Apply remote cat state to the visitor entity."""
        ctx = self._context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        if self.visitor_cat is not None:
            self.visitor_cat.apply_state(
                payload.get('x', 64),
                payload.get('p', 'sitting.side.neutral'),
                payload.get('m', 0),
                payload.get('vx', 0),
            )

    def _handle_vbeh(self, mac, payload):
        """Peer started a behavior; maybe mirror it on our cat."""
        ctx = self._context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        peer_behavior = payload.get('b')
        if not peer_behavior:
            return
        entry = self._MIRROR_TABLE.get(peer_behavior)
        if not entry:
            return
        local_behavior, probability, check_fn = entry

        scene = self._scene_manager.current_scene
        if not hasattr(scene, 'character'):
            return
        character = scene.character
        cb = character.current_behavior
        if cb and cb.NAME in self._NO_MIRROR_BEHAVIORS:
            return
        if check_fn is not None and not check_fn(ctx):
            return
        if random.random() < probability:
            print('[Visit] Mirroring %s -> %s' % (peer_behavior, local_behavior))
            character.behavior_manager.trigger(local_behavior)

    def _handle_vgreet(self, mac, payload):
        """Inviter started the greeting ritual; invitee mirrors it."""
        ctx = self._context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        scene = self._scene_manager.current_scene
        if not hasattr(scene, 'character'):
            return

        # Show a bubble above the visitor (inviter) during their greeting
        if self.visitor_cat is not None:
            mac_hex = ':'.join('%02x' % b for b in mac)
            familiarity = ctx.get_friendship_level(mac_hex)
            icon = 'heart' if familiarity >= 0.5 else 'question'
            self.visitor_bubble = (icon, _SNIFF_DURATION + 0.5, _SNIFF_DURATION + 0.5)

        # Invitee walks right toward ENTRY_X+8 (started at ENTRY_X+20)
        entry_x = getattr(scene, 'ENTRY_X', 64)
        target_x = entry_x + 8
        scene.character.trigger('greeting',
                                target_x=target_x,
                                sniff_pose='standing.side.sniffing')

    def _handle_vprox(self, mac, payload):
        """Inviter detected proximity; invitee does a proximity sniff."""
        ctx = self._context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        scene = self._scene_manager.current_scene
        if not hasattr(scene, 'character'):
            return
        cb = scene.character.current_behavior
        if cb and cb.NAME in self._NO_MIRROR_BEHAVIORS:
            return

        # Show visitor bubble for the peer's sniff reaction
        if self.visitor_cat is not None:
            mac_hex = ':'.join('%02x' % b for b in mac)
            familiarity = ctx.get_friendship_level(mac_hex)
            icon = 'heart' if familiarity >= 0.5 else 'question'
            self.visitor_bubble = (icon, _SNIFF_DURATION + 0.5, _SNIFF_DURATION + 0.5)

        # Invitee skips walk, goes straight to sniff pose
        scene.character.trigger('greeting', sniff_pose='standing.side.sniffing')

    def _handle_vocalize(self, mac, payload):
        """Peer vocalized during a visit; show visitor bubble and maybe schedule a reply."""
        ctx = self._context
        if ctx.visit is None or mac != ctx.visit['peer_mac']:
            return
        icon = payload.get('i', 'exclaim')
        self.visitor_bubble = (icon, 4.0, 4.0)
        if self._voc_reply_timer <= 0 and random.random() < 0.5:
            self._voc_reply_timer = random.uniform(1.0, 4.0)

    # ------------------------------------------------------------------
    # Private: outbound broadcasts
    # ------------------------------------------------------------------

    def _broadcast_vst(self, dt, scene):
        """Send our cat's state on pose/position change, velocity stop, or heartbeat."""
        self._vst_timer -= dt
        current_x = int(scene.character.x)
        current_pose = scene.character.pose_name
        current_vx = int(self._char_vx)
        pos_changed = (self._last_broadcast_x is None or
                       abs(current_x - self._last_broadcast_x) >= 4)
        just_stopped = (current_vx == 0 and self._last_broadcast_vx != 0)
        if (current_pose != self._last_broadcast_pose or self._vst_timer <= 0
                or pos_changed or just_stopped):
            ctx = self._context
            if ctx.espnow and ctx.visit:
                payload = {'x': current_x, 'p': current_pose,
                           'm': 1 if scene.character.mirror else 0}
                if current_vx != 0:
                    payload['vx'] = current_vx
                ctx.espnow.send_to(ctx.visit['peer_mac'], 'vst', payload)
            self._last_broadcast_pose = current_pose
            self._last_broadcast_x = current_x
            self._last_broadcast_vx = current_vx
            self._vst_timer = _VST_HEARTBEAT

    def _broadcast_venv(self, dt):
        """Inviter sends time-of-day once per second to keep peer's sky in sync."""
        ctx = self._context
        if ctx.visit is None or ctx.visit.get('role') != 'inviter':
            return
        self._venv_timer -= dt
        if self._venv_timer > 0:
            return
        self._venv_timer = _VENV_INTERVAL
        if ctx.espnow:
            env = ctx.environment
            ctx.espnow.send_to(ctx.visit['peer_mac'], 'venv', {
                'h':  env.get('time_hours', 12),
                'mn': env.get('time_minutes', 0),
            })

    def _update_playdate(self, dt, scene):
        """Accumulate playtime, broadcast behavior changes, detect sniff proximity."""
        ctx = self._context
        visit = ctx.visit

        # Accumulate total visit time
        visit['play_time'] = visit.get('play_time', 0.0) + dt

        # Broadcast behavior changes
        current_beh = ctx.current_behavior_name
        if (current_beh and current_beh != self._last_bcast_behavior
                and current_beh in self._BROADCASTABLE_BEHAVIORS):
            if ctx.espnow:
                ctx.espnow.send_to(visit['peer_mac'], 'vbeh', {'b': current_beh})
            self._last_bcast_behavior = current_beh

        # Tick visitor bubble timer
        if self.visitor_bubble is not None:
            icon, remaining, max_secs = self.visitor_bubble
            remaining -= dt
            if remaining <= 0:
                self.visitor_bubble = None
            else:
                self.visitor_bubble = (icon, remaining, max_secs)

        # Tick sniff cooldown
        if self._sniff_cooldown > 0:
            self._sniff_cooldown -= dt

        # Proximity sniff — inviter only, to avoid both sides triggering simultaneously
        if (visit.get('role') == 'inviter' and self._sniff_cooldown <= 0
                and self.visitor_cat is not None):
            dist = abs(scene.character.x - self.visitor_cat.x)
            if dist < _SNIFF_PROXIMITY:
                cb = scene.character.current_behavior
                if not cb or cb.NAME not in self._NO_SNIFF_BEHAVIORS:
                    self._sniff_cooldown = _SNIFF_COOLDOWN_SECS
                    mac_hex = ':'.join('%02x' % b for b in visit['peer_mac'])
                    familiarity = ctx.get_friendship_level(mac_hex)
                    scene.character.trigger('greeting',
                                            sniff_pose='leaning_forward.side.stretch')
                    if ctx.espnow:
                        ctx.espnow.send_to(visit['peer_mac'], 'vprox', {})
                    icon = 'heart' if familiarity >= 0.5 else 'question'
                    self.visitor_bubble = (icon, 4.0, 4.0)

    def _tick_voc_reply(self, dt, scene):
        """Count down the vocalization reply timer and trigger vocalizing when it fires."""
        if self._voc_reply_timer <= 0:
            return
        self._voc_reply_timer -= dt
        if self._voc_reply_timer <= 0:
            self._voc_reply_timer = 0.0
            cb = scene.character.current_behavior
            if cb and cb.NAME not in ('sleeping', 'eating', 'being_groomed'):
                scene.character.trigger('vocalizing')
