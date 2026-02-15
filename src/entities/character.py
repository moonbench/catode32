from entities.entity import Entity
from assets.character import POSES

class CharacterEntity(Entity):
    """The main pet character entity."""

    def __init__(self, x, y, pose="idle"):
        super().__init__(x, y)
        self.pose = pose

        self.anim_body = 0.0
        self.anim_head = 0.0
        self.anim_eyes = 0.0
        self.anim_tail = 0.0

    def set_pose(self, pose_name):
        """Change the character's pose."""
        if pose_name in POSES:
            self.pose = pose_name

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
        pose = POSES[self.pose]

        self.anim_body = (self.anim_body + dt * pose["body"].get("speed", 1)) % self._get_total_frames(pose["body"])
        self.anim_head = (self.anim_head + dt * pose["head"].get("speed", 1)) % self._get_total_frames(pose["head"])
        self.anim_eyes = (self.anim_eyes + dt * pose["eyes"].get("speed", 1)) % self._get_total_frames(pose["eyes"])
        self.anim_tail = (self.anim_tail + dt * pose["tail"].get("speed", 1)) % self._get_total_frames(pose["tail"])

    def draw(self, renderer, mirror=False, camera_offset=0):
        """Draw the character at its position.

        Args:
            renderer: the renderer to draw with
            mirror: if True, flip the character horizontally
            camera_offset: horizontal camera offset to subtract from x position
        """
        if not self.visible:
            return

        pose = POSES[self.pose]
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
        renderer.draw_sprite_obj(body, body_x, body_y, frame=body_frame, mirror_h=mirror)
        renderer.draw_sprite_obj(head, head_x, head_y, frame=head_frame, mirror_h=mirror)
        renderer.draw_sprite_obj(eyes, eye_x, eye_y, frame=eye_frame, mirror_h=mirror)

