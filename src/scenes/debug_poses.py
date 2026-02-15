from scene import Scene
from entities.character import CharacterEntity, get_all_pose_names


class DebugPosesScene(Scene):
    """Debug scene for previewing and debugging character poses"""

    def __init__(self, context, renderer, input):
        super().__init__(context, renderer, input)
        self.character = None
        self.pose_names = []
        self.pose_index = 0
        self.show_anchors = False
        self.show_grid = False
        self.grid_offset = 0.0

    def load(self):
        super().load()
        self.pose_names = get_all_pose_names()
        self.character = CharacterEntity(64, 60)
        if self.pose_names:
            self.character.set_pose(self.pose_names[0])

    def unload(self):
        super().unload()

    def enter(self):
        self.pose_index = 0
        self.show_anchors = False
        self.show_grid = False
        self.grid_offset = 0.0
        if self.pose_names:
            self.character.set_pose(self.pose_names[0])

    def exit(self):
        pass

    def update(self, dt):
        self.character.update(dt)
        if self.show_grid:
            self.grid_offset = (self.grid_offset + dt * 16) % 8

    def draw(self):
        self.renderer.clear()

        # Draw moving grid background if enabled
        if self.show_grid:
            self._draw_moving_grid()

        # Draw floor
        self.renderer.draw_line(0, 60, 128, 60)

        # Draw character
        self.character.draw(self.renderer)

        # Draw pose name at top (split into position, direction, emotion)
        if self.pose_names:
            pose_name = self.pose_names[self.pose_index]
            parts = pose_name.split(".")
            for i, part in enumerate(parts):
                self.renderer.draw_text(part, 0, i * 8)

        # Draw anchor/attachment point markers if enabled
        if self.show_anchors:
            self._draw_debug_markers()
        
    def _draw_debug_markers(self):
        """Draw anchor points and attachment points for debugging"""
        pose = self.character._pose
        x, y = int(self.character.x), int(self.character.y)

        body = pose["body"]
        head = pose["head"]
        eyes = pose["eyes"]
        tail = pose["tail"]

        # Calculate body position
        body_x = x - body["anchor_x"]
        body_y = y - body["anchor_y"]

        # Calculate head position
        head_root_x = body_x + body["head_x"]
        head_root_y = body_y + body["head_y"]
        head_x = head_root_x - head["anchor_x"]
        head_y = head_root_y - head["anchor_y"]

        # Calculate eye position
        eye_x = head_x + head["eye_x"] - eyes["anchor_x"]
        eye_y = head_y + head["eye_y"] - eyes["anchor_y"]

        # Calculate tail position
        tail_root_x = body_x + body["tail_x"]
        tail_root_y = body_y + body["tail_y"]
        tail_x = tail_root_x - tail["anchor_x"]
        tail_y = tail_root_y - tail["anchor_y"]

        # Draw anchor points as 5x5 rectangles (black with white outline)
        # Body anchor (the character's x,y position)
        self._draw_anchor_rect(x, y)
        # Head anchor
        self._draw_anchor_rect(head_x + head["anchor_x"], head_y + head["anchor_y"])
        # Eye anchor
        self._draw_anchor_rect(eye_x + eyes["anchor_x"], eye_y + eyes["anchor_y"])
        # Tail anchor
        self._draw_anchor_rect(tail_x + tail["anchor_x"], tail_y + tail["anchor_y"])

        # Draw attachment points as 3x3 X marks
        # head_x/y - where head attaches to body
        self._draw_x_marker(body_x + body["head_x"], body_y + body["head_y"])
        # tail_x/y - where tail attaches to body
        self._draw_x_marker(body_x + body["tail_x"], body_y + body["tail_y"])
        # eye_x/y - where eyes attach to head
        self._draw_x_marker(head_x + head["eye_x"], head_y + head["eye_y"])

    def _draw_anchor_rect(self, cx, cy):
        """Draw a 5x5 anchor rectangle centered at (cx, cy).

        Black rectangle with white outline, 3x3 black inside.
        """
        # White outline (5x5)
        for dx in range(-2, 3):
            for dy in range(-2, 3):
                self.renderer.draw_pixel(cx + dx, cy + dy, color=1)
        # Black inner (3x3)
        for dx in range(-1, 2):
            for dy in range(-1, 2):
                self.renderer.draw_pixel(cx + dx, cy + dy, color=0)

    def _draw_x_marker(self, cx, cy):
        """Draw a 3x3 X marker centered at (cx, cy)."""
        # Draw the X pattern (diagonals)
        self.renderer.draw_pixel(cx - 1, cy - 1, color=1)
        self.renderer.draw_pixel(cx, cy, color=1)
        self.renderer.draw_pixel(cx + 1, cy + 1, color=1)
        self.renderer.draw_pixel(cx + 1, cy - 1, color=1)
        self.renderer.draw_pixel(cx - 1, cy + 1, color=1)

    def _draw_moving_grid(self):
        """Draw a moving grid background for testing sprite fill gaps."""
        offset = int(self.grid_offset)
        # Draw vertical lines (moving right)
        for x in range(-8 + offset, 128 + 8, 8):
            self.renderer.draw_line(x, 0, x, 128)
        # Draw horizontal lines (moving down)
        for y in range(-8 + offset, 128 + 8, 8):
            self.renderer.draw_line(0, y, 128, y)

    def handle_input(self):
        # Left/right to cycle poses
        if self.input.was_just_pressed('left'):
            self.pose_index = (self.pose_index - 1) % len(self.pose_names)
            self.character.set_pose(self.pose_names[self.pose_index])

        if self.input.was_just_pressed('right'):
            self.pose_index = (self.pose_index + 1) % len(self.pose_names)
            self.character.set_pose(self.pose_names[self.pose_index])

        # Up to toggle anchor display
        if self.input.was_just_pressed('up'):
            self.show_anchors = not self.show_anchors

        # Down to toggle moving grid background
        if self.input.was_just_pressed('down'):
            self.show_grid = not self.show_grid

        # B to go back to normal scene
        if self.input.was_just_pressed('b'):
            from scenes.normal import NormalScene
            return ('change_scene', NormalScene)

        return None
