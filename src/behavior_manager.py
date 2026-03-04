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
    }

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
        old_module = None
        if self._character.current_behavior and self._character.current_behavior.active:
            old_module = type(self._character.current_behavior).__module__
            self._character.current_behavior.stop(completed=False)

        self._load_and_start(name, **kwargs)

        if old_module:
            new_module = type(self._character.current_behavior).__module__
            if old_module != new_module:
                self._unload_module(old_module)

    def advance(self, name, kwargs, context):
        """Chain to the next behavior after natural completion.

        Called from base.stop(completed=True). If name is None, auto-selects
        based on context stats.
        """
        if name is None:
            name, kwargs = self._auto_select(context)
        if name is None:
            name = 'idle'
            kwargs = {}
        self._load_and_start(name, **kwargs)

    # ------------------------------------------------------------------
    # Module lifecycle
    # ------------------------------------------------------------------

    def _load_and_start(self, name, **kwargs):
        """Load a behavior module, instantiate it, and start it."""
        if name not in self._REGISTRY:
            print(f"[BehaviorManager] Unknown behavior: {name}, falling back to idle")
            name = 'idle'
            kwargs = {}
        module_path, class_name = self._REGISTRY[name]
        mod = __import__(module_path, None, None, [class_name])
        cls = getattr(mod, class_name)
        behavior = cls(self._character)
        self._character.current_behavior = behavior
        behavior.start(**kwargs)

    def _unload_module(self, module_path):
        """Remove a behavior module from sys.modules and trigger GC.

        Safe to call while still executing code from that module — the call
        stack's frame reference keeps the code object alive until return.
        """
        if module_path in sys.modules:
            print(f"[BehaviorManager] Unloading {module_path}")
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
            print("Randomly meandering....")
            return 'meandering', {}

        # High serenity makes the pet content to keep resting
        if context.serenity > 25 and random.random() < (context.serenity - 25) / 150:
            print(f"Staying idle (serenity: {context.serenity:.1f})")
            return None, {}

        print("--------------------------------------------------------------------------------")
        context.debug_print_stats()

        check_fns = [
            ('sleeping',      self.can_trigger_sleeping,      self.priority_sleeping),
            ('napping',       self.can_trigger_napping,       self.priority_napping),
            ('zoomies',       self.can_trigger_zoomies,       self.priority_zoomies),
            ('vocalizing',    self.can_trigger_vocalizing,    self.priority_vocalizing),
            ('hunting',       self.can_trigger_hunting,       self.priority_hunting),
            ('playing',       self.can_trigger_playing,       self.priority_playing),
            ('investigating', self.can_trigger_investigating, self.priority_investigating),
            ('observing',     self.can_trigger_observing,     self.priority_observing),
            ('self_grooming', self.can_trigger_self_grooming, self.priority_self_grooming),
            ('stretching',    self.can_trigger_stretching,    self.priority_stretching),
            ('pacing',        self.can_trigger_pacing,        self.priority_pacing),
            ('lounging',      self.can_trigger_lounging,      self.priority_lounging),
            ('startled',      self.can_trigger_startled,      self.priority_startled),
        ]

        candidates = [name for name, can_fn, _ in check_fns if can_fn(context)]

        if not candidates:
            return None, {}

        priority_fns = {name: p_fn for name, _, p_fn in check_fns}
        priorities = {name: priority_fns[name](context) for name in candidates}

        for name in sorted(candidates, key=lambda n: priorities[n]):
            print(f">> {name}: priority={priorities[name]}")
        print("--------------------------------------------------------------------------------")

        binned = {name: math.ceil(p / 10) * 10 for name, p in priorities.items()}
        best_bin = min(binned.values())
        top = [name for name, b in binned.items() if b == best_bin]
        chosen = random.choice(top)
        if len(top) > 1:
            print(f">> Selected: {chosen} (from bin tied at {best_bin}: {top})")
        return chosen, {}

    # ------------------------------------------------------------------
    # can_trigger methods
    # ------------------------------------------------------------------

    def can_trigger_sleeping(self, ctx):
        trigger = ctx.energy < 40
        if not trigger:
            print("Skipping sleeping. Energy: %6.4f" % ctx.energy)
        return trigger

    def can_trigger_napping(self, ctx):
        trigger = ctx.energy < 60
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
        _NEED = 35
        happy = ctx.energy > 35 and ctx.playfulness > 35
        needs_unmet = (ctx.fullness < _NEED or ctx.comfort < _NEED
                       or ctx.fulfillment < _NEED or ctx.affection < _NEED)
        trigger = happy or needs_unmet
        if not trigger:
            failures = []
            if ctx.energy <= 35:
                failures.append("Energy: %6.4f" % ctx.energy)
            if ctx.playfulness <= 35:
                failures.append("Playfulness: %6.4f" % ctx.playfulness)
            print("Skipping vocalizing. " + ", ".join(failures))
        return trigger

    def can_trigger_hunting(self, ctx):
        if ctx.fullness < 15 and ctx.energy > 20:
            return True
        trigger = ctx.energy > 20 and ctx.playfulness > 20
        if not trigger:
            failures = []
            if ctx.energy <= 20:
                failures.append("Energy: %6.4f" % ctx.energy)
            if ctx.playfulness <= 20:
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
        trigger = ctx.cleanliness < 70 and ctx.energy > 30
        if not trigger:
            failures = []
            if ctx.cleanliness >= 70:
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
        trigger = ctx.comfort < 65 and (ctx.patience < 60 or ctx.serenity < 60)
        if not trigger:
            failures = []
            if ctx.comfort >= 65:
                failures.append("Comfort: %6.4f" % ctx.comfort)
            if ctx.patience >= 60 and ctx.serenity >= 60:
                failures.append("Patience: %6.4f" % ctx.patience)
                failures.append("Serenity: %6.4f" % ctx.serenity)
            print("Skipping pacing. " + ", ".join(failures))
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
        p = 0.15 * (1 - ctx.courage / 200) * (1 - ctx.resilience / 200)
        trigger = random.random() < p
        if not trigger:
            print("Skipping startled. p=%.3f, Courage %6.4f, Resilience %6.4f" % (p, ctx.courage, ctx.resilience))
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
        return random.uniform(ctx.energy * 0.25, max(ctx.energy * 0.25, ctx.energy * 2))

    def priority_napping(self, ctx):
        return random.uniform(ctx.energy * 0.3, max(ctx.energy * 0.5, ctx.energy * 2.5))

    def priority_zoomies(self, ctx):
        return random.uniform(100 - ctx.playfulness * 1.5, ctx.playfulness * 1.5)

    def priority_vocalizing(self, ctx):
        _NEED = 35
        urgency = max(
            _NEED - ctx.fullness,
            _NEED - ctx.comfort,
            _NEED - ctx.fulfillment,
            _NEED - ctx.affection,
            0,
        )
        if urgency > 0:
            return max(10, 65 - urgency * 2)
        return random.uniform(10, max(10, (200 - ctx.energy - ctx.playfulness) * 0.6))

    def priority_hunting(self, ctx):
        if ctx.fullness < 15 and ctx.energy > 20:
            return random.uniform(5, 15)
        upper = max(20, (100 - ctx.energy) * 0.7 + (100 - ctx.playfulness) * 0.5)
        hunger_bonus = (100 - ctx.fullness) * 0.3
        return random.uniform(10, max(10, upper - hunger_bonus * 0.5))

    def priority_playing(self, ctx):
        return random.uniform(100 - ctx.playfulness * 1.5, ctx.playfulness * 1.5)

    def priority_investigating(self, ctx):
        return random.uniform(10, max(10, 100 - ctx.curiosity))

    def priority_observing(self, ctx):
        return random.uniform(10, max(10, 100 - ctx.curiosity))

    def priority_self_grooming(self, ctx):
        return random.uniform(0, ctx.cleanliness * 1.5) + random.uniform(0, max(10, ctx.energy * 0.25))

    def priority_stretching(self, ctx):
        return random.uniform(10, max(10, ctx.comfort))

    def priority_pacing(self, ctx):
        worst = min(ctx.comfort, ctx.patience, ctx.serenity)
        return random.uniform(10, max(10, 100 - (100 - worst) * 0.8))

    def priority_lounging(self, ctx):
        return 100 - random.uniform(ctx.serenity, ctx.serenity * 2.0)

    def priority_startled(self, ctx):
        return random.uniform(20, max(20, (ctx.courage + ctx.resilience) * 0.6))
