"""Greeting behavior - cats approach each other, sniff, and react.

Used both for the visit-start ritual (with a walk preamble) and for
proximity-triggered mid-visit sniffs (target_x=None, skip the walk).

start() kwargs:
    target_x   - walk destination before sniffing; None skips the walk phase.
    sniff_pose - pose name to use during the sniff ('leaning_forward.side.stretch'
                 or 'standing.side.sniffing').  Defaults to 'standing.side.sniffing'.

Familiarity is derived from context.get_friendship_level() at sniff time.
Familiarity >= 0.5 shows a heart bubble and a happy post-sniff pose.
Familiarity <  0.5 shows a question bubble and a neutral post-sniff pose.
"""

import random
from entities.behaviors.base import BaseBehavior
from ui import draw_bubble

_WALK_SPEED = 20   # px/s — slightly faster than normal walking
_SNIFF_DURATION = 3.5
_REACT_DURATION = 2.0

_SNIFF_POSE_DEFAULT = 'standing.side.sniffing'
_REACT_FRIENDLY = 'sitting.side.happy'
_REACT_NEUTRAL  = 'sitting.side.neutral'


class GreetingBehavior(BaseBehavior):
    NAME = 'greeting'

    COMPLETION_BONUS = {
        'sociability': 0.5,
        'affection':   0.3,
        'serenity':    0.1,
    }

    def __init__(self, character):
        super().__init__(character)
        self._target_x = None
        self._direction = 0
        self._sniff_pose = _SNIFF_POSE_DEFAULT
        self._familiarity = 0.0
        self._bubble_icon = 'question'
        self._bubble_progress = 0.0

    def start(self, target_x=None, sniff_pose=_SNIFF_POSE_DEFAULT, on_complete=None):
        if self._active:
            return
        super().start(on_complete)

        self._sniff_pose = sniff_pose
        self._bubble_progress = 0.0

        if target_x is not None:
            self._target_x = float(target_x)
            self._direction = 1 if self._target_x > self._character.x else -1
            self._character.mirror = self._direction > 0
            self._character.set_pose('walking.side.neutral')
            self._phase = 'walking'
        else:
            self._start_sniff()

    def _start_sniff(self):
        ctx = self._character.context
        self._familiarity = 0.0
        if ctx and ctx.visit:
            mac = ctx.visit.get('peer_mac')
            if mac:
                mac_hex = ':'.join('%02x' % b for b in mac)
                self._familiarity = ctx.get_friendship_level(mac_hex)

        self._bubble_icon = 'heart' if self._familiarity >= 0.5 else 'question'
        self._phase = 'sniffing'
        self._phase_timer = 0.0
        self._bubble_progress = 0.0
        self._character.set_pose(self._sniff_pose)

    def _start_react(self):
        friendly = self._familiarity >= 0.5
        self._character.set_pose(_REACT_FRIENDLY if friendly else _REACT_NEUTRAL)
        self._phase = 'reacting'
        self._phase_timer = 0.0

    def update(self, dt):
        if not self._active:
            return

        self._phase_timer += dt

        if self._phase == 'walking':
            self._character.x += self._direction * _WALK_SPEED * dt
            arrived = (
                (self._direction < 0 and self._character.x <= self._target_x) or
                (self._direction > 0 and self._character.x >= self._target_x)
            )
            if arrived:
                self._character.x = self._target_x
                self._start_sniff()

        elif self._phase == 'sniffing':
            self._bubble_progress = min(1.0, self._phase_timer / _SNIFF_DURATION)
            if self._phase_timer >= _SNIFF_DURATION:
                self._start_react()

        elif self._phase == 'reacting':
            if self._phase_timer >= _REACT_DURATION:
                self.stop(completed=True)

    def draw(self, renderer, char_x, char_y, mirror=False):
        if self._active and self._phase == 'sniffing':
            draw_bubble(renderer, self._bubble_icon, char_x, char_y,
                        self._bubble_progress, mirror)

    def next(self, context):
        return None  # fall through to idle / auto-select
