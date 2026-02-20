from entities.entity import Entity
from assets.character import POSES


def get_pose(pose_name):
    """Get a pose by dot-notation name (e.g., 'sitting.side.neutral').

    Returns the pose dict or None if not found.
    """
    parts = pose_name.split(".")
    if len(parts) != 3:
        print(f"[character] Invalid pose format: '{pose_name}' (expected 'position.direction.emotion')")
        return None
    position, direction, emotion = parts
    try:
        return POSES[position][direction][emotion]
    except KeyError:
        print(f"[character] Pose not found: '{pose_name}'")
        return None


def get_all_pose_names():
    """Get a flat list of all pose names in dot notation."""
    names = []
    for position, directions in POSES.items():
        for direction, emotions in directions.items():
            for emotion in emotions.keys():
                names.append(f"{position}.{direction}.{emotion}")
    return names


class CharacterEntity(Entity):
    """The main pet character entity."""

    def __init__(self, x, y, pose="sitting.side.neutral"):
        super().__init__(x, y)
        self.pose_name = pose
        self._pose = get_pose(pose)

        self.anim_body = 0.0
        self.anim_head = 0.0
        self.anim_eyes = 0.0
        self.anim_tail = 0.0

        # Eating state
        self._eating = False
        self._eating_bowl_sprite = None
        self._eating_bowl_frame = 0.0
        self._eating_speed = 0.8  # Frames per second
        self._eating_pose_before = None
        self._eating_on_complete = None

    def set_pose(self, pose_name):
        """Change the character's pose using dot notation (e.g., 'sitting.side.neutral')."""
        # If eating and pose is changing to something else, cancel eating
        if self._eating and pose_name != "leaning_forward.side.eating":
            self.stop_eating(completed=False)

        pose = get_pose(pose_name)
        if pose is not None:
            self.pose_name = pose_name
            self._pose = pose
        else:
            print(f"[character] Failed to set pose: '{pose_name}', keeping current pose")

    def start_eating(self, bowl_sprite, on_complete=None):
        """Begin the eating animation sequence.

        Args:
            bowl_sprite: The food bowl sprite dict (with frames)
            on_complete: Optional callback function to call when eating finishes
        """
        if self._eating:
            return
        self._eating = True
        self._eating_bowl_sprite = bowl_sprite
        self._eating_bowl_frame = 0.0
        self._eating_pose_before = self.pose_name
        self._eating_on_complete = on_complete
        self.set_pose("leaning_forward.side.eating")

    def stop_eating(self, completed=True):
        """End the eating state.

        Args:
            completed: If True, eating finished naturally. If False, it was
                       interrupted (e.g., by another action changing the pose).
        """
        if not self._eating:
            return
        self._eating = False
        # Only restore previous pose if eating completed naturally
        if self._eating_pose_before and completed:
            self.set_pose(self._eating_pose_before)
        self._eating_pose_before = None
        self._eating_bowl_sprite = None
        callback = self._eating_on_complete
        self._eating_on_complete = None
        if callback:
            callback(completed)

    @property
    def is_eating(self):
        """Return True if character is currently eating."""
        return self._eating

    def get_bowl_frame(self):
        """Get the current bowl animation frame index (0-5)."""
        return min(int(self._eating_bowl_frame), 5)

    def get_bowl_position(self, mirror=False):
        """Get the world position where the food bowl should be drawn.

        Args:
            mirror: If True, position bowl on right side of character.

        Returns:
            (x, y) tuple for bowl position in world coordinates.
        """
        bowl_offset_x = 30
        bowl_width = self._eating_bowl_sprite["width"] if self._eating_bowl_sprite else 22
        bowl_height = self._eating_bowl_sprite["height"] if self._eating_bowl_sprite else 8
        bowl_y = int(self.y) - bowl_height

        if mirror:
            bowl_x = int(self.x) + bowl_offset_x - bowl_width // 2
        else:
            bowl_x = int(self.x) - bowl_offset_x - bowl_width // 2

        return bowl_x, bowl_y

    def _get_point(self, sprite, key, frame=0, mirror=False):
        """Get a point value from a sprite, handling both static (int) and animated (list) values.

        When mirror=True and key ends with '_x', the value is mirrored within the sprite width.
        """
        value = sprite[key]
        result = value[frame] if isinstance(value, list) else value
        if mirror and key.endswith('_x'):
            return sprite["width"] - result
        return result

    def _get_anchor_x(self, sprite, mirror=False):
        """Get anchor_x, mirrored within sprite width if needed."""
        anchor_x = sprite["anchor_x"]
        return sprite["width"] - anchor_x if mirror else anchor_x

    def _get_total_frames(self, sprite):
        """Get total frame count including extra_frames for pause at end of cycle."""
        return len(sprite["frames"]) + sprite.get("extra_frames", 0)

    def _get_frame_index(self, sprite, counter):
        """Get actual frame index, clamping to first frame during extra_frames period."""
        frame_count = len(sprite["frames"])
        index = int(counter) % self._get_total_frames(sprite)
        return index if index < frame_count else 0

    def update(self, dt):
        """Update animation counters."""
        if self._pose is None:
            return

        pose = self._pose
        self.anim_body = (self.anim_body + dt * pose["body"].get("speed", 1)) % self._get_total_frames(pose["body"])
        self.anim_head = (self.anim_head + dt * pose["head"].get("speed", 1)) % self._get_total_frames(pose["head"])
        self.anim_eyes = (self.anim_eyes + dt * pose["eyes"].get("speed", 1)) % self._get_total_frames(pose["eyes"])
        self.anim_tail = (self.anim_tail + dt * pose["tail"].get("speed", 1)) % self._get_total_frames(pose["tail"])

        # Update eating animation
        if self._eating and self._eating_bowl_sprite:
            num_frames = len(self._eating_bowl_sprite["frames"])
            self._eating_bowl_frame += dt * self._eating_speed
            if self._eating_bowl_frame >= num_frames:
                self.stop_eating()

    def draw(self, renderer, mirror=False, camera_offset=0):
        """Draw the character at its position.

        Args:
            renderer: the renderer to draw with
            mirror: if True, flip the character horizontally
            camera_offset: horizontal camera offset to subtract from x position
        """
        if not self.visible or self._pose is None:
            return

        pose = self._pose
        x, y = int(self.x) - camera_offset, int(self.y)

        # Get the positions for the parts
        body = pose["body"]
        body_frame = self._get_frame_index(body, self.anim_body)
        body_x = x - self._get_anchor_x(body, mirror)
        body_y = y - body["anchor_y"]

        head = pose["head"]
        head_frame = self._get_frame_index(head, self.anim_head)
        head_root_x = body_x + self._get_point(body, "head_x", body_frame, mirror)
        head_root_y = body_y + self._get_point(body, "head_y", body_frame)
        head_x = head_root_x - self._get_anchor_x(head, mirror)
        head_y = head_root_y - head["anchor_y"]

        eyes = pose["eyes"]
        eye_frame = self._get_frame_index(eyes, self.anim_eyes)
        eye_x = head_x + self._get_point(head, "eye_x", head_frame, mirror) - self._get_anchor_x(eyes, mirror)
        eye_y = head_y + self._get_point(head, "eye_y", head_frame) - eyes["anchor_y"]

        tail = pose["tail"]
        tail_frame = self._get_frame_index(tail, self.anim_tail)
        tail_root_x = body_x + self._get_point(body, "tail_x", body_frame, mirror)
        tail_root_y = body_y + self._get_point(body, "tail_y", body_frame)
        tail_x = tail_root_x - self._get_anchor_x(tail, mirror)
        tail_y = tail_root_y - tail["anchor_y"]


        # Draw the parts
        renderer.draw_sprite_obj(tail, tail_x, tail_y, frame=tail_frame, mirror_h=mirror)

        if pose.get("head_first") == True:
            renderer.draw_sprite_obj(head, head_x, head_y, frame=head_frame, mirror_h=mirror)
            renderer.draw_sprite_obj(body, body_x, body_y, frame=body_frame, mirror_h=mirror)
        else:
            renderer.draw_sprite_obj(body, body_x, body_y, frame=body_frame, mirror_h=mirror)
            renderer.draw_sprite_obj(head, head_x, head_y, frame=head_frame, mirror_h=mirror)

        renderer.draw_sprite_obj(eyes, eye_x, eye_y, frame=eye_frame, mirror_h=mirror)

