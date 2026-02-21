"""Debug scene for testing and triggering behaviors manually."""

from scene import Scene
from entities.character import CharacterEntity
from ui import Scrollbar


class DebugBehaviorsScene(Scene):
    """Debug scene for testing behavior execution."""

    LINES_VISIBLE = 6
    LINE_HEIGHT = 8

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.character = None
        self.selected_index = 0
        self.scrollbar = Scrollbar(renderer)
        self.scroll_offset = 0
        self.behaviors = []  # Built dynamically from manager

    def load(self):
        super().load()
        # Create character with context for behavior management
        self.character = CharacterEntity(90, 60, context=self.context)

        # Build behavior list from manager
        if self.character.behavior_manager:
            self.behaviors = [
                (name, name[0].upper() + name[1:])
                for name in sorted(self.character.behavior_manager._behaviors.keys())
            ]

    def unload(self):
        super().unload()

    def enter(self):
        self.selected_index = 0
        self.scroll_offset = 0

    def exit(self):
        # Stop any running behavior when leaving
        if self.character and self.character.behavior_manager:
            active = self.character.behavior_manager.active_behavior
            if active:
                active.stop(completed=False)

    def update(self, dt):
        if self.character:
            self.character.update(dt)

    def draw(self):
        self.renderer.clear()

        # Draw floor line
        self.renderer.draw_line(0, 60, 128, 60)

        # Draw behavior list on left (width ~60px)
        self._draw_behavior_list()

        # Draw current behavior status at bottom
        self._draw_status()

        # Draw character on right side
        if self.character:
            self.character.draw(self.renderer)

    def _draw_behavior_list(self):
        """Draw the list of behaviors with selection indicator."""
        y = 0
        visible_end = min(self.scroll_offset + self.LINES_VISIBLE, len(self.behaviors))

        for i in range(self.scroll_offset, visible_end):
            key, name = self.behaviors[i]
            line_y = y + (i - self.scroll_offset) * self.LINE_HEIGHT

            # Selection indicator
            prefix = ">" if i == self.selected_index else " "

            # Check if this behavior is currently active
            suffix = ""
            if self.character and self.character.behavior_manager:
                behavior = self.character.behavior_manager.get_behavior(key)
                if behavior and behavior.active:
                    suffix = "*"

            self.renderer.draw_text(f"{prefix}{name}{suffix}", 0, line_y)

        # Scrollbar if needed (will appear on right edge of screen)
        if len(self.behaviors) > self.LINES_VISIBLE:
            self.scrollbar.draw(
                len(self.behaviors),
                self.LINES_VISIBLE,
                self.scroll_offset
            )

    def _draw_status(self):
        """Draw current behavior status at bottom of screen."""
        if not self.character or not self.character.behavior_manager:
            return

        active = self.character.behavior_manager.active_behavior

        if active:
            # Show progress percentage
            self.renderer.draw_rect(0, 60, int(active.progress * 128), 4, True)

    def handle_input(self):
        # Navigation
        if self.input.was_just_pressed('up'):
            self.selected_index = max(0, self.selected_index - 1)
            # Scroll to keep selection visible
            if self.selected_index < self.scroll_offset:
                self.scroll_offset = self.selected_index

        if self.input.was_just_pressed('down'):
            self.selected_index = min(len(self.behaviors) - 1, self.selected_index + 1)
            # Scroll to keep selection visible
            if self.selected_index >= self.scroll_offset + self.LINES_VISIBLE:
                self.scroll_offset = self.selected_index - self.LINES_VISIBLE + 1

        # A button: trigger selected behavior
        if self.input.was_just_pressed('a'):
            self._trigger_selected()

        # B button: stop current behavior or exit
        if self.input.was_just_pressed('b'):
            if self.character and self.character.behavior_manager:
                active = self.character.behavior_manager.active_behavior
                if active:
                    active.stop(completed=False)
                    return None  # Stay in scene after stopping
            return ('change_scene', 'normal')

        return None

    def _trigger_selected(self):
        """Trigger the currently selected behavior."""
        if not self.character or not self.character.behavior_manager:
            return

        behavior_key = self.behaviors[self.selected_index][0]

        # Special handling for eating - it needs a bowl sprite
        if behavior_key == "eating":
            self._trigger_eating()
        else:
            self.character.behavior_manager.trigger(behavior_key)

    def _trigger_eating(self):
        """Trigger eating behavior with food bowl."""
        try:
            from assets.items import FOOD_BOWL
            self.character.behavior_manager.trigger(
                "eating", bowl_sprite=FOOD_BOWL, meal_type="chicken"
            )
        except ImportError:
            # Fall back if food bowl not available
            pass
