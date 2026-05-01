"""VacationScene - base class for all vacation location scenes.

Subclasses define:
    SCENE_NAME      str   e.g. 'vacation_park'
    ENJOY_DURATION  float seconds until enjoyment cap and home-bubble trigger
    GRACE_DURATION  float seconds of grace period before overstay penalties
    STAT_ACCRUAL    dict  stat -> total gain over ENJOY_DURATION
    STAT_PENALTIES  dict  stat -> loss per second during overstay

Subclasses implement:
    setup_scene()        create self.environment + self.character
    on_enter()           add sky/custom draws
    on_exit()            teardown sky
    on_update(dt)        update sky + character + environment
"""

from scenes.main_scene import MainScene
from menu import MenuItem
from assets.icons import HOUSE_ICON


class VacationScene(MainScene):
    SCENE_NAME      = None
    IS_VACATION     = True

    ENJOY_DURATION  = 900.0    # seconds; override per destination
    GRACE_DURATION  = 120.0    # seconds grace before penalties
    STAT_ACCRUAL    = {}       # e.g. {'fulfillment': 8, 'playfulness': 8}
    STAT_PENALTIES  = {'comfort': -0.005, 'serenity': -0.003}


    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self._vac_timer     = 0.0
        self._home_wanted   = False  # True once ENJOY_DURATION reached
        self._penalty_accum = 0.0   # accumulates dt; penalties fire every 10s

        # Pre-allocate change dict so exit never creates garbage
        self._accrual_changes = {s: 0.0 for s in self.STAT_ACCRUAL}
        self._penalty_changes = {s: 0.0 for s in self.STAT_PENALTIES}

    # ------------------------------------------------------------------
    # Scene enter / exit — wrap MainScene to preserve last_main_scene
    # and set the on_vacation flag
    # ------------------------------------------------------------------

    def enter(self):
        super().enter()
        # last_main_scene is now this vacation scene, so sub-screens return here
        self.context.on_vacation = True
        self.context.wants_to_go_home = False

    def exit(self):
        self._apply_vacation_rewards()
        self.context.on_vacation = False
        self.context.wants_to_go_home = False
        super().exit()

    # ------------------------------------------------------------------
    # Timer + accrual (called by subclass on_update via super())
    # ------------------------------------------------------------------

    def _tick_vacation(self, dt):
        """Advance the vacation timer and check milestones.

        Stat rewards are applied once on exit. Only the home-wanted flag and
        overstay penalties are tracked here each frame.
        """
        prev = self._vac_timer
        self._vac_timer += dt

        # Crossed the enjoyment cap this frame
        if prev < self.ENJOY_DURATION <= self._vac_timer:
            self._home_wanted = True
            self.context.wants_to_go_home = True
            print('[Vacation] Enjoyment cap reached — cat wants to go home')

        # Overstay penalties — accumulate dt, fire every 90s to avoid spam
        if self.STAT_PENALTIES and self._vac_timer >= self.ENJOY_DURATION + self.GRACE_DURATION:
            self._penalty_accum += dt
            if self._penalty_accum >= 90.0:
                for s in self._penalty_changes:
                    self._penalty_changes[s] = self.STAT_PENALTIES[s] * self._penalty_accum
                self.context.apply_stat_changes(self._penalty_changes)
                self._penalty_accum = 0.0

    def _apply_vacation_rewards(self):
        """Apply stat rewards on exit, proportional to time spent up to the cap."""
        if not self.STAT_ACCRUAL:
            return
        proportion = min(1.0, self._vac_timer / self.ENJOY_DURATION)
        if proportion <= 0:
            return
        for s in self._accrual_changes:
            self._accrual_changes[s] = self.STAT_ACCRUAL[s] * proportion
        print('[Vacation] Applying rewards at proportion %.2f' % proportion)
        self.context.apply_stat_changes(self._accrual_changes)

    # ------------------------------------------------------------------
    # Menu — inject "Go home" at the bottom of menu2
    # ------------------------------------------------------------------

    def _build_menu_items(self):
        items = [i for i in super()._build_menu_items() if i.label != "Gardening"]
        items.insert(0, MenuItem(
            "Go home",
            icon=HOUSE_ICON,
            action=("vacation_go_home",),
            confirm="Ready to go home?",
        ))
        return items

    def _handle_menu_action(self, action):
        if action and action[0] == "vacation_go_home":
            return ('change_scene', 'inside')
        return super()._handle_menu_action(action)
