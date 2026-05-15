from lang import t
"""Debug scene for testing and triggering behaviors manually."""

from scene import Scene
from entities.character import CharacterEntity
from ui import Scrollbar


# All triggerable behaviors: (entry_key, display_name, trigger_name, start_kwargs)
def get_behavior_entries():
    return [
    ("idle",         t("Idle"),           "idle",          {}),
    ("sleeping",     t("Sleeping"),       "sleeping",      {}),
    ("napping",      t("Napping"),        "napping",       {}),
    ("stretching",   t("Stretching"),     "stretching",    {}),
    ("kneading",     t("Kneading"),       "kneading",      {}),
    ("lounging",     t("Lounging"),       "lounging",      {}),
    ("investigating",t("Investigating"),  "investigating",  {}),
    ("startled",     t("Startled"),       "startled",      {}),
    ("observing",    t("Observing"),      "observing",     {}),
    ("chattering",   t("Chattering"),     "chattering",    {}),
    ("zoomies",      t("Zoomies"),        "zoomies",       {}),
    ("vocalizing",   t("Vocalizing"),     "vocalizing",    {}),
    ("self_grooming",t("Self Grooming"),  "self_grooming", {}),
    ("being_groomed",t("Being Groomed"),  "being_groomed", {}),
    ("hunting",      t("Hunting"),        "hunting",       {}),
    ("gift_bringing",t("Gift Bringing"),  "gift_bringing", {}),
    ("pacing",       t("Pacing"),         "pacing",        {}),
    ("meandering",   t("Meandering"),     "meandering",    {}),
    ("sulking",      t("Sulking"),        "sulking",       {}),
    ("mischief",     t("Mischief"),       "mischief",      {}),
    ("hiding",       t("Hiding"),         "hiding",        {}),
    ("training",     t("Training"),       "training",      {}),
    ("playing",      t("Playing"),        "playing",       {}),
    ("playing_ball", t("Playing (ball)"), "playing",       {"variant": "ball"}),
    ("affection",    t("Affection"),      "affection",     {"variant": "pets"}),
    ("attention",      t("Attention"),        "attention", {"variant": "psst"}),
    ("hearing_exclaim",t("Hearing (exclaim)"),"hearing",   {"icon": "exclaim"}),
    ("hearing_heart",  t("Hearing (heart)"),  "hearing",   {"icon": "heart"}),
    ("hearing_note",   t("Hearing (note)"),   "hearing",   {"icon": "note"}),
    ("eating",         t("Eating"),           "eating",    None),  # special case
    ("eating_treat",   t("Eating (treat)"),   "eating",    None),  # special case
]


BEHAVIOR_ENTRIES = get_behavior_entries

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
        self.context.scene_x_min = 10
        self.context.scene_x_max = 118
        self.character = CharacterEntity(100, 60, context=self.context)

    def unload(self):
        super().unload()

    def enter(self):
        self.selected_index = 0
        self.scroll_offset = 0

    def exit(self):
        if self.character:
            self.character.behavior_manager.stop_current()

    def update(self, dt):
        if self.character:
            self.character.update(dt)

    def draw(self):

        self.renderer.draw_line(0, 60, 128, 60)

        self._draw_behavior_list()
        self._draw_status()

        if self.character:
            self.character.draw(self.renderer, mirror=self.character.mirror)

    def _draw_behavior_list(self):
        """Draw the list of behaviors with selection indicator."""
        y = 0
        visible_end = min(self.scroll_offset + self.LINES_VISIBLE, len(BEHAVIOR_ENTRIES))

        for i in range(self.scroll_offset, visible_end):
            key, name, trigger_name, _ = BEHAVIOR_ENTRIES[i]
            line_y = y + (i - self.scroll_offset) * self.LINE_HEIGHT
            is_selected = i == self.selected_index

            current = self.character.current_behavior if self.character else None
            suffix = "*" if (current and current.NAME == trigger_name) else ""

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
            return ('change_scene', 'last_main')

        return None

    def _trigger_selected(self):
        """Trigger the currently selected behavior."""
        if not self.character:
            return

        key, name, trigger_name, kwargs = BEHAVIOR_ENTRIES[self.selected_index]

        if key in ("eating", "eating_treat"):
            self._trigger_eating(key)
        else:
            self.character.trigger(trigger_name, **(kwargs or {}))

    def _trigger_eating(self, key="eating"):
        """Trigger eating behavior with the appropriate food."""
        from assets.items import FOOD_BOWL, TREAT1
        if key == "eating_treat":
            self.character.trigger('eating', food_sprite=TREAT1, food_type="treat")
        else:
            self.character.trigger('eating', food_sprite=FOOD_BOWL, food_type="chicken")
