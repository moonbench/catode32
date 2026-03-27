"""Behavior manager - lazy-loads and manages behavior module lifecycle."""

import math
import random
import gc
import sys


class BehaviorManager:
    """Central registry for behavior loading, selection, and module lifecycle.

    Behaviors are lazy-loaded on demand and unloaded after completion to free
    memory. The manager holds all can_trigger and priority logic so behavior
    modules don't need cross-imports just for eligibility checks.
    """

    # Registry: name -> (module_path, class_name)
    _REGISTRY = {
        'idle':          ('entities.behaviors.idle',          'IdleBehavior'),
        'sleeping':      ('entities.behaviors.sleeping',      'SleepingBehavior'),
        'napping':       ('entities.behaviors.napping',       'NappingBehavior'),
        'stretching':    ('entities.behaviors.stretching',    'StretchingBehavior'),
        'kneading':      ('entities.behaviors.kneading',      'KneadingBehavior'),
        'lounging':      ('entities.behaviors.lounging',      'LoungeingBehavior'),
        'investigating': ('entities.behaviors.investigating', 'InvestigatingBehavior'),
        'observing':     ('entities.behaviors.observing',     'ObservingBehavior'),
        'chattering':    ('entities.behaviors.chattering',    'ChatteringBehavior'),
        'zoomies':       ('entities.behaviors.zoomies',       'ZoomiesBehavior'),
        'vocalizing':    ('entities.behaviors.vocalizing',    'VocalizingBehavior'),
        'self_grooming': ('entities.behaviors.self_grooming', 'SelfGroomingBehavior'),
        'being_groomed': ('entities.behaviors.being_groomed', 'BeingGroomedBehavior'),
        'hunting':       ('entities.behaviors.hunting',       'HuntingBehavior'),
        'gift_bringing': ('entities.behaviors.gift_bringing', 'GiftBringingBehavior'),
        'pacing':        ('entities.behaviors.pacing',        'PacingBehavior'),
        'sulking':       ('entities.behaviors.sulking',       'SulkingBehavior'),
        'mischief':      ('entities.behaviors.mischief',      'MischiefBehavior'),
        'hiding':        ('entities.behaviors.hiding',        'HidingBehavior'),
        'training':      ('entities.behaviors.training',      'TrainingBehavior'),
        'playing':       ('entities.behaviors.playing',       'PlayingBehavior'),
        'affection':     ('entities.behaviors.affection',     'AffectionBehavior'),
        'attention':     ('entities.behaviors.attention',     'AttentionBehavior'),
        'eating':        ('entities.behaviors.eating',        'EatingBehavior'),
        'startled':      ('entities.behaviors.startled',      'StartledBehavior'),
        'meandering':    ('entities.behaviors.meandering',    'MeanderingBehavior'),
        'go_to':         ('entities.behaviors.go_to',         'GoToBehavior'),
        'hearing':       ('entities.behaviors.hearing',       'HearingBehavior'),
    }

    # Ordered tuple of auto-selectable behavior names. Built once at class definition;
    # can_trigger_<name> and priority_<name> are looked up via getattr at runtime.
    _AUTO_SELECT_NAMES = (
        'sleeping', 'napping', 'zoomies', 'vocalizing', 'hunting', 'playing',
        'investigating', 'observing', 'self_grooming', 'stretching', 'pacing',
        'sulking', 'mischief', 'hiding', 'lounging', 'startled',
    )

    def __init__(self, character):
        self._character = character

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def stop_current(self):
        """Stop the active behavior and unload its module.

        Called when the scene is exited so the cached scene's character
        doesn't keep a behavior module pinned in sys.modules.
        """
        if self._character.current_behavior and self._character.current_behavior.active:
            old_module = type(self._character.current_behavior).__module__
            self._character.current_behavior.stop(completed=False)
            self._unload_module(old_module)

    def trigger(self, name, **kwargs):
        """Player-initiated behavior trigger — interrupts the current behavior."""
        if self._character.current_behavior and self._character.current_behavior.active:
            old_module = type(self._character.current_behavior).__module__
            self._character.current_behavior.stop(completed=False)
            # Unload old module before loading new one: stop(completed=False) returns
            # to behavior_manager.py, so the old module has no live stack frames.
            # Running gc.collect() here gives the new module the best heap conditions.
            new_module_path = self._REGISTRY.get(name, (old_module,))[0]
            if old_module != new_module_path:
                self._unload_module(old_module)

        self._load_and_start(name, **kwargs)

    def advance(self, name, kwargs, context):
        """Chain to the next behavior after natural completion.

        Called from base.stop(completed=True). If name is None, auto-selects
        based on context stats.
        """
        if context:
            cb = self._character.current_behavior
            if cb and hasattr(cb, '_behavior_name'):
                context.record_behavior(cb._behavior_name)

        if name is None:
            name, kwargs = self._auto_select(context)
        if name is None:
            name = 'idle'
            kwargs = {}
        self._load_and_start(name, **kwargs)

    # ------------------------------------------------------------------
    # Module lifecycle
    # ------------------------------------------------------------------

    def resume_prior_behavior(self):
        """Restart the prior behavior on scene re-entry, or idle if it was an interaction.

        Interaction behaviors (triggered by the player, not autonomous) should not
        be resumed when re-entering a scene — fall back to idle instead.
        """
        _INTERACTION_BEHAVIORS = frozenset((
            'affection', 'attention', 'being_groomed', 'eating',
            'playing', 'gift_bringing', 'chattering', 'go_to', 'hearing',
        ))
        ctx = self._character.context
        prior = ctx.current_behavior_name if ctx else None
        if prior and prior not in _INTERACTION_BEHAVIORS:
            self.trigger(prior)
        else:
            self.trigger('idle')

    def _load_and_start(self, name, **kwargs):
        """Load a behavior module, instantiate it, and start it."""
        if name not in self._REGISTRY:
            print(f"\033[31mUnknown behavior: {name}, falling back to idle\033[0m")
            name = 'idle'
            kwargs = {}
        module_path, class_name = self._REGISTRY[name]

        ctx = self._character.context
        if ctx:
            ctx.current_behavior_name = name

        gc.collect()
        try:
            mod = __import__(module_path, None, None, [class_name])
        except MemoryError:
            if name != 'idle':
                print(f"\033[31mOOM loading {name}, falling back to idle\033[0m")
                name = 'idle'
                kwargs = {}
                module_path, class_name = self._REGISTRY[name]
                if ctx:
                    ctx.current_behavior_name = name
                gc.collect()
                mod = __import__(module_path, None, None, [class_name])
            else:
                raise

        cls = getattr(mod, class_name)
        behavior = cls(self._character)
        behavior._behavior_name = name
        self._character.current_behavior = behavior
        behavior.start(**kwargs)

    # Modules kept permanently in sys.modules to avoid repeated import spikes.
    # idle cycles on every behavior transition so pinning it eliminates the most
    # frequent source of fragmentation.
    _PINNED_MODULES = frozenset((
        'entities.behaviors.idle',
    ))

    def _unload_module(self, module_path):
        """Remove a behavior module from sys.modules and trigger GC.

        Safe to call while still executing code from that module — the call
        stack's frame reference keeps the code object alive until return.
        """
        if module_path in self._PINNED_MODULES:
            return
        if module_path in sys.modules:
            del sys.modules[module_path]
            gc.collect()

    # ------------------------------------------------------------------
    # Auto-selection (inlined from IdleBehavior.next)
    # ------------------------------------------------------------------

    def _auto_select(self, context):
        """Scan auto-triggerable behaviors and return (name, kwargs) for the best one.

        Returns (None, {}) to restart idle.
        """
        if not context:
            return None, {}

        # Random meander (special case — checked before main selection)
        if self.can_trigger_meandering(context) and random.random() <= 0.2:
            print("\033[32mRandomly meandering....\033[0m")
            return 'meandering', {}

        # Scene exit (special case — pet may decide to walk to a different location)
        exit_result = self._auto_select_scene_exit(context)
        if exit_result:
            name, kwargs = exit_result
            print("\033[32mScene exit -> %s\033[0m" % kwargs.get('pending_scene', '?'))
            return name, kwargs

        # High serenity makes the pet content to keep resting
        if context.serenity > 25 and random.random() < (context.serenity - 25) / 150:
            print(f"\033[32mStaying idle (serenity: {context.serenity:.1f})\033[0m")
            return None, {}

        print("--------------------------------------------------------------------------------")
        context.debug_print_stats()

        candidates = []
        for name in self._AUTO_SELECT_NAMES:
            if getattr(self, 'can_trigger_' + name)(context):
                candidates.append(name)

        if not candidates:
            return None, {}

        priorities = {}
        for name in candidates:
            priorities[name] = max(0, getattr(self, 'priority_' + name)(context))

        # Penalize recently completed behaviors to prevent loops.
        # Most recent (index 0) gets +50, next +40, down to +10 at index 4.
        for i, recent in enumerate(context.recent_behaviors):
            if recent in priorities:
                priorities[recent] += 50 - i * 10

        for name in sorted(candidates, key=lambda n: priorities[n]):
            recent_marker = ""
            if name in context.recent_behaviors:
                idx = context.recent_behaviors.index(name)
                recent_marker = f" (+{50 - idx * 10} recency)"
            print(f">> {name}: priority= {priorities[name]}{recent_marker}")
        print("--------------------------------------------------------------------------------")

        binned = {name: math.ceil(p / 10) * 10 for name, p in priorities.items()}
        best_bin = min(binned.values())
        top = [name for name, b in binned.items() if b == best_bin]
        chosen = random.choice(top)
        if len(top) > 1:
            print(f">> Selected: {chosen} (from bin tied at {best_bin}: {top})")
        return chosen, {}

    # ------------------------------------------------------------------
    # Scene exit selection
    # ------------------------------------------------------------------

    # Valid destinations from each main scene.
    _SCENE_TRANSITIONS = {
        'inside':    ('bedroom', 'kitchen', 'outside'),
        'bedroom':   ('inside',  'kitchen'),
        'kitchen':   ('inside',  'bedroom'),
        'outside':   ('inside',  'treehouse'),
        'treehouse': ('outside',),
    }

    def _auto_select_scene_exit(self, ctx):
        """Maybe walk to a new location. Returns ('go_to', kwargs) or None.

        Base probability ~8%, boosted when needs align with a destination
        (low fullness → kitchen, low comfort/energy → bedroom, bad weather → away
        from outdoor destinations). Never totally deterministic.
        """
        current = getattr(ctx, 'last_main_scene', None)
        options = self._SCENE_TRANSITIONS.get(current)
        if not options:
            return None

        if ctx.energy < 15:
            return None  # Too exhausted to go anywhere

        weather = ctx.environment.get('weather', 'Clear')
        outdoor_bad = weather in ('Rain', 'Storm', 'Snow')

        # Compute per-destination weights
        weights = []
        for dest in options:
            w = 1.0
            if dest == 'kitchen':
                # Hungry pet more likely to head for food
                w += max(0.0, 50.0 - ctx.fullness) * 0.04
            if dest == 'bedroom':
                # Tired or uncomfortable pet more likely to rest
                w += max(0.0, 50.0 - ctx.comfort) * 0.02
                w += max(0.0, 50.0 - ctx.energy) * 0.02
            if dest in ('outside', 'treehouse') and outdoor_bad:
                # Bad weather discourages outdoor trips
                w *= 0.2
            weights.append(w)

        # Boost effective probability when the strongest pull is high
        max_w = max(weights)
        effective_p = min(0.4, 0.08 + (max_w - 1.0) * 0.05)

        roll = random.random()
        print("Scene exit p=%.3f roll=%.3f (%s) weights: %s" % (
            effective_p, roll, current,
            ", ".join("%s=%.2f" % (d, w) for d, w in zip(options, weights))
        ))

        if roll > effective_p:
            return None

        # Weighted random destination pick
        total = sum(weights)
        r = random.uniform(0.0, total)
        chosen = options[-1]
        for dest, w in zip(options, weights):
            r -= w
            if r <= 0.0:
                chosen = dest
                break

        target_x = getattr(ctx, 'scene_x_min', 10)
        return 'go_to', {'target_x': target_x, 'speed': 12, 'pending_scene': chosen}

    # ------------------------------------------------------------------
    # can_trigger methods
    # ------------------------------------------------------------------

    def can_trigger_sleeping(self, ctx):
        h = ctx.environment.get('time_hours', 12)
        in_bedroom = getattr(ctx, 'last_main_scene', None) == 'bedroom'
        threshold = 70 if (h >= 21 or h < 6) else 40
        if in_bedroom:
            threshold += 20
        trigger = ctx.energy < threshold
        if not trigger:
            print("Skipping sleeping. Energy: %6.4f" % ctx.energy)
        return trigger

    def can_trigger_napping(self, ctx):
        h = ctx.environment.get('time_hours', 12)
        in_bedroom = getattr(ctx, 'last_main_scene', None) == 'bedroom'
        threshold = 85 if (h >= 21 or h < 6) else 60
        if in_bedroom:
            threshold += 20
        trigger = ctx.energy < threshold
        if not trigger:
            print("Skipping napping. Energy: %6.4f" % ctx.energy)
        return trigger

    def can_trigger_zoomies(self, ctx):
        trigger = ctx.energy > 40 and ctx.playfulness > 40
        if not trigger:
            failures = []
            if ctx.energy <= 40:
                failures.append("Energy: %6.4f" % ctx.energy)
            if ctx.playfulness <= 40:
                failures.append("Playfulness: %6.4f" % ctx.playfulness)
            print("Skipping zoomies. " + ", ".join(failures))
        return trigger

    def can_trigger_vocalizing(self, ctx):
        _NEED = 60
        happy = ctx.energy > 35 and ctx.playfulness > 40
        needs_unmet = (ctx.fullness < _NEED or ctx.comfort < _NEED
                       or ctx.fulfillment < _NEED or ctx.affection < _NEED
                       or ctx.sociability < _NEED)
        trigger = happy or needs_unmet
        if not trigger:
            failures = []
            if ctx.energy <= 35:
                failures.append("Energy: %6.4f" % ctx.energy)
            if ctx.playfulness <= 40:
                failures.append("Playfulness: %6.4f" % ctx.playfulness)
            print("Skipping vocalizing. " + ", ".join(failures))
        return trigger

    def can_trigger_hunting(self, ctx):
        if ctx.fullness < 15 and ctx.energy > 20:
            return True
        outdoors = getattr(ctx, 'last_main_scene', None) in ('outside', 'treehouse')
        threshold = 15 if outdoors else 20
        trigger = ctx.energy > threshold and ctx.playfulness > threshold
        if not trigger:
            failures = []
            if ctx.energy <= threshold:
                failures.append("Energy: %6.4f" % ctx.energy)
            if ctx.playfulness <= threshold:
                failures.append("Playfulness: %6.4f" % ctx.playfulness)
            print("Skipping hunting. " + ", ".join(failures))
        return trigger

    def can_trigger_playing(self, ctx):
        return ctx.playfulness >= 40

    def can_trigger_investigating(self, ctx):
        trigger = ctx.curiosity >= 40
        if not trigger:
            print("Skipping investigating. Curiosity: %6.4f" % ctx.curiosity)
        return trigger

    def can_trigger_observing(self, ctx):
        trigger = ctx.curiosity >= 30
        if not trigger:
            print("Skipping observing. Curiosity: %6.4f" % ctx.curiosity)
        return trigger

    def can_trigger_self_grooming(self, ctx):
        trigger = ctx.cleanliness < 57 and ctx.energy > 30
        if not trigger:
            failures = []
            if ctx.cleanliness >= 57:
                failures.append("Cleanliness: %6.4f" % ctx.cleanliness)
            if ctx.energy <= 30:
                failures.append("Energy: %6.4f" % ctx.energy)
            print("Skipping self grooming. " + ", ".join(failures))
        return trigger

    def can_trigger_stretching(self, ctx):
        trigger = ctx.comfort < 55
        if not trigger:
            print("Skipping stretching. Comfort: %6.2f" % ctx.comfort)
        return trigger

    def can_trigger_pacing(self, ctx):
        trigger = ctx.comfort < 70 and ctx.serenity < 65
        if not trigger:
            failures = []
            if ctx.comfort >= 70:
                failures.append("Comfort: %6.4f" % ctx.comfort)
            if ctx.serenity >= 65:
                failures.append("Serenity: %6.4f" % ctx.serenity)
            print("Skipping pacing. " + ", ".join(failures))
        return trigger

    def can_trigger_sulking(self, ctx):
        trigger = ctx.fulfillment < 50 or ctx.affection < 50
        if not trigger:
            print("Skipping sulking. Fulfillment: %6.4f, Affection: %6.4f" % (ctx.fulfillment, ctx.affection))
        return trigger

    def can_trigger_mischief(self, ctx):
        trigger = (ctx.mischievousness > 25 and ctx.maturity < 55
                   and ctx.playfulness > 50 and ctx.energy > 40)
        if not trigger:
            print("Skipping mischief. Mischievousness: %6.4f, Maturity: %6.4f" % (ctx.mischievousness, ctx.maturity))
        return trigger

    def can_trigger_hiding(self, ctx):
        trigger = ctx.courage < 65 and (ctx.affection < 55 or ctx.energy < 55)
        if not trigger:
            print("Skipping hiding. Courage: %6.4f" % ctx.courage)
        return trigger

    def can_trigger_lounging(self, ctx):
        trigger = ctx.focus > 30 and ctx.serenity > 30
        if not trigger:
            failures = []
            if ctx.focus <= 30:
                failures.append("Focus: %6.2f" % ctx.focus)
            if ctx.serenity <= 30:
                failures.append("Serenity: %6.2f" % ctx.serenity)
            print("Skipping lounging. " + ", ".join(failures))
        return trigger

    def can_trigger_startled(self, ctx):
        p = 0.45 * (1 - ctx.courage / 100)
        trigger = random.random() < p
        if not trigger:
            print("Skipping startled. p=%.3f, Courage %6.4f" % (p, ctx.courage))
        return trigger

    def can_trigger_meandering(self, ctx):
        trigger = ctx.energy > 20
        if not trigger:
            print("Skipping meandering. Energy: %6.4f" % ctx.energy)
        return trigger

    # ------------------------------------------------------------------
    # Priority methods
    # ------------------------------------------------------------------

    def priority_sleeping(self, ctx):
        base = random.uniform(ctx.energy * 0.25, max(ctx.energy * 0.25, ctx.energy * 2))
        h = ctx.environment.get('time_hours', 12)
        if h >= 19 or h < 6:
            base *= 0.4
        if getattr(ctx, 'last_main_scene', None) == 'bedroom':
            base *= 0.55
        return base

    def priority_napping(self, ctx):
        base = random.uniform(ctx.energy * 0.3, max(ctx.energy * 0.5, ctx.energy * 2.5))
        h = ctx.environment.get('time_hours', 12)
        if h >= 19 or h < 6:
            base *= 0.5
        if getattr(ctx, 'last_main_scene', None) == 'bedroom':
            base *= 0.55
        return base

    def priority_zoomies(self, ctx):
        base = random.uniform(100 - ctx.playfulness * 1.5, ctx.playfulness * 1.5)
        if getattr(ctx, 'in_familiar_location', False):
            base *= 0.85  # more likely to zoom in a safe, familiar place
        return base

    def priority_vocalizing(self, ctx):
        # Outdoors and not recently vocalized: cats are chatty outside —
        # push priority low enough to win selection roughly every ~5 behaviors.
        # Checked first so mild urgency can't bypass it.
        espnow = getattr(ctx, 'espnow', None)
        if espnow and espnow.active and 'vocalizing' not in ctx.recent_behaviors:
            return random.uniform(5, 15)

        # How urgently each need demands communication
        _NEED = 40
        hunger_deficit    = max(0, _NEED - ctx.fullness)    # hungry → vocalize before hunting
        loneliness        = max(0, _NEED - ctx.sociability)  # lonely
        affection_deficit = max(0, _NEED - ctx.affection)    # wants affection
        comfort_deficit   = max(0, _NEED - ctx.comfort)
        play_deficit      = max(0, _NEED - ctx.playfulness)  # wants to play

        urgency = max(hunger_deficit, loneliness, affection_deficit,
                      comfort_deficit, play_deficit)

        if urgency > 0:
            # urgency=40 (stat=0) → priority 5; urgency=17 (fullness=23) → priority 14
            return max(5, 65 - urgency * 3)

        # Happy and energetic: chatty but not urgent
        return random.uniform(25, max(25, (200 - ctx.energy - ctx.playfulness) * 0.5))

    def priority_hunting(self, ctx):
        hunger_pull = 100 - ctx.fullness   # hungry → high pull (eat-motivated)
        play_pull = ctx.playfulness         # playful → high pull (fun/gift-motivated)
        # Both pulls lower the ceiling (more competitive hunting); hunger additionally
        # raises the floor so vocalizing can still win first when needs are unmet.
        ceiling = max(25, 85 - play_pull * 0.5 - hunger_pull * 0.3)
        floor = max(10, 25 + hunger_pull * 0.15 - play_pull * 0.1)
        base = random.uniform(floor, max(floor + 5, ceiling))
        if getattr(ctx, 'last_main_scene', None) in ('outside', 'treehouse'):
            base *= 0.75
        return base

    def priority_playing(self, ctx):
        base = random.uniform(100 - ctx.playfulness * 1.5, ctx.playfulness * 1.5)
        if getattr(ctx, 'in_familiar_location', False):
            base *= 0.85  # more playful in familiar surroundings
        return base

    def priority_investigating(self, ctx):
        base = random.uniform(10, max(10, 100 - ctx.curiosity))
        if not getattr(ctx, 'in_familiar_location', False):
            base *= 0.8  # more curious/exploratory in unfamiliar places
        return base

    def priority_observing(self, ctx):
        return random.uniform(10, max(10, 100 - ctx.curiosity))

    def priority_self_grooming(self, ctx):
        return random.uniform(ctx.cleanliness * 0.5, ctx.cleanliness * 1.5) + random.uniform(0, max(10, ctx.energy * 0.25))

    def priority_stretching(self, ctx):
        return random.uniform(ctx.comfort * 0.4, max(10, ctx.comfort))

    def priority_pacing(self, ctx):
        worst = min(ctx.comfort, ctx.serenity)
        base = random.uniform(10, max(10, 100 - (100 - worst) * 0.8))
        if not getattr(ctx, 'in_familiar_location', False):
            base *= 0.8  # more restless in unfamiliar places
        return base

    def priority_sulking(self, ctx):
        base = random.uniform(10, max(20, (ctx.fulfillment + ctx.affection) * 0.45))
        if not getattr(ctx, 'in_familiar_location', False):
            base *= 0.85  # slight mood dampening away from home
        return base

    def priority_mischief(self, ctx):
        return random.uniform(20, max(20, (200 - ctx.mischievousness - ctx.playfulness) * 0.5))

    def priority_hiding(self, ctx):
        base = random.uniform(15, max(15, ctx.courage))
        if getattr(ctx, 'in_familiar_location', False):
            base *= 1.4   # unlikely to hide somewhere safe and familiar
        else:
            base *= 0.65  # anxious in unknown territory
        return base

    def priority_lounging(self, ctx):
        base = 100 - random.uniform(ctx.serenity * 0.5, min(90, ctx.serenity * 1.5))
        if getattr(ctx, 'in_familiar_location', False):
            base *= 0.8  # more likely to just relax at home
        return max(5, base)

    def priority_startled(self, ctx):
        base = random.uniform(20, max(20, ctx.courage * 1.2))
        if not getattr(ctx, 'in_familiar_location', False):
            base *= 0.75  # on-edge in unknown territory
        return base
