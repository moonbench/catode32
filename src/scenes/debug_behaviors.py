"""Debug scene for testing and triggering behaviors manually."""

from scene import Scene
from entities.character import CharacterEntity
from entities.behaviors.idle import IdleBehavior
from entities.behaviors.sleeping import SleepingBehavior
from entities.behaviors.napping import NappingBehavior
from entities.behaviors.stretching import StretchingBehavior
from entities.behaviors.kneading import KneadingBehavior
from entities.behaviors.lounging import LoungeingBehavior
from entities.behaviors.investigating import InvestigatingBehavior
from entities.behaviors.observing import ObservingBehavior
from entities.behaviors.chattering import ChatteringBehavior
from entities.behaviors.zoomies import ZoomiesBehavior
from entities.behaviors.vocalizing import VocalizingBehavior
from entities.behaviors.self_grooming import SelfGroomingBehavior
from entities.behaviors.being_groomed import BeingGroomedBehavior
from entities.behaviors.hunting import HuntingBehavior
from entities.behaviors.gift_bringing import GiftBringingBehavior
from entities.behaviors.pacing import PacingBehavior
from entities.behaviors.sulking import SulkingBehavior
from entities.behaviors.mischief import MischiefBehavior
from entities.behaviors.hiding import HidingBehavior
from entities.behaviors.training import TrainingBehavior
from entities.behaviors.playing import PlayingBehavior
from entities.behaviors.affection import AffectionBehavior
from entities.behaviors.attention import AttentionBehavior
from entities.behaviors.snacking import SnackingBehavior
from entities.behaviors.eating import EatingBehavior
from ui import Scrollbar


# All triggerable behaviors with their display name and any start() kwargs
BEHAVIOR_ENTRIES = [
    ("idle",         "Idle",         IdleBehavior,         {}),
    ("sleeping",     "Sleeping",     SleepingBehavior,     {}),
    ("napping",      "Napping",      NappingBehavior,      {}),
    ("stretching",   "Stretching",   StretchingBehavior,   {}),
    ("kneading",     "Kneading",     KneadingBehavior,     {}),
    ("lounging",     "Lounging",     LoungeingBehavior,    {}),
    ("investigating","Investigating", InvestigatingBehavior,{}),
    ("observing",    "Observing",    ObservingBehavior,    {}),
    ("chattering",   "Chattering",   ChatteringBehavior,   {}),
    ("zoomies",      "Zoomies",      ZoomiesBehavior,      {}),
    ("vocalizing",   "Vocalizing",   VocalizingBehavior,   {}),
    ("self_grooming","Self Grooming",SelfGroomingBehavior, {}),
    ("being_groomed","Being Groomed",BeingGroomedBehavior, {}),
    ("hunting",      "Hunting",      HuntingBehavior,      {}),
    ("gift_bringing","Gift Bringing",GiftBringingBehavior, {}),
    ("pacing",       "Pacing",       PacingBehavior,       {}),
    ("sulking",      "Sulking",      SulkingBehavior,      {}),
    ("mischief",     "Mischief",     MischiefBehavior,     {}),
    ("hiding",       "Hiding",       HidingBehavior,       {}),
    ("training",     "Training",     TrainingBehavior,     {}),
    ("playing",      "Playing",      PlayingBehavior,      {}),
    ("affection",    "Affection",    AffectionBehavior,    {"variant": "pets"}),
    ("attention",    "Attention",    AttentionBehavior,    {"variant": "psst"}),
    ("snacking",     "Snacking",     SnackingBehavior,     {"variant": "snack"}),
    ("eating",       "Eating",       EatingBehavior,       None),  # None = special case
]


class DebugBehaviorsScene(Scene):
    """Debug scene for testing behavior execution."""

    LINES_VISIBLE = 7
    LINE_HEIGHT = 8

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.character = None
        self.selected_index = 0
        self.scrollbar = Scrollbar(renderer)
        self.scroll_offset = 0

    def load(self):
        super().load()
        self.character = CharacterEntity(100, 60, context=self.context)

    def unload(self):
        super().unload()

    def enter(self):
        self.selected_index = 0
        self.scroll_offset = 0

    def exit(self):
        if self.character and self.character.current_behavior:
            self.character.current_behavior.stop(completed=False)

    def update(self, dt):
        if self.character:
            self.character.update(dt)

    def draw(self):
        self.renderer.clear()

        self.renderer.draw_line(0, 60, 128, 60)

        self._draw_behavior_list()
        self._draw_status()

        if self.character:
            self.character.draw(self.renderer)

    def _draw_behavior_list(self):
        """Draw the list of behaviors with selection indicator."""
        y = 0
        visible_end = min(self.scroll_offset + self.LINES_VISIBLE, len(BEHAVIOR_ENTRIES))

        for i in range(self.scroll_offset, visible_end):
            key, name, cls, _ = BEHAVIOR_ENTRIES[i]
            line_y = y + (i - self.scroll_offset) * self.LINE_HEIGHT
            is_selected = i == self.selected_index

            suffix = "*" if (self.character and isinstance(self.character.current_behavior, cls)) else ""

            if is_selected:
                self.renderer.draw_rect(0, line_y, 128, self.LINE_HEIGHT, filled=True, color=1)

            text_color = 0 if is_selected else 1
            self.renderer.draw_text(f"{name}{suffix}", 1, line_y, text_color)

        if len(BEHAVIOR_ENTRIES) > self.LINES_VISIBLE:
            self.scrollbar.draw(len(BEHAVIOR_ENTRIES), self.LINES_VISIBLE, self.scroll_offset)

    def _draw_status(self):
        """Draw current behavior progress bar at bottom of screen."""
        if not self.character or not self.character.current_behavior:
            return

        active = self.character.current_behavior
        if active.active:
            self.renderer.draw_rect(0, 60, int(active.progress * 128), 4, True)

    def handle_input(self):
        if self.input.was_just_pressed('up'):
            self.selected_index = max(0, self.selected_index - 1)
            if self.selected_index < self.scroll_offset:
                self.scroll_offset = self.selected_index

        if self.input.was_just_pressed('down'):
            self.selected_index = min(len(BEHAVIOR_ENTRIES) - 1, self.selected_index + 1)
            if self.selected_index >= self.scroll_offset + self.LINES_VISIBLE:
                self.scroll_offset = self.selected_index - self.LINES_VISIBLE + 1

        if self.input.was_just_pressed('a'):
            self._trigger_selected()

        if self.input.was_just_pressed('b'):
            if self.character and self.character.current_behavior:
                self.character.current_behavior.stop(completed=False)
                return None
            return ('change_scene', 'normal')

        return None

    def _trigger_selected(self):
        """Trigger the currently selected behavior."""
        if not self.character:
            return

        key, name, cls, kwargs = BEHAVIOR_ENTRIES[self.selected_index]

        if key == "eating":
            self._trigger_eating()
        else:
            self.character.trigger(cls, **(kwargs or {}))

    def _trigger_eating(self):
        """Trigger eating behavior with food bowl."""
        try:
            from assets.items import FOOD_BOWL
            self.character.trigger(EatingBehavior, FOOD_BOWL, "chicken")
        except ImportError:
            pass
