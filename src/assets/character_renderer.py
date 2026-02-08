from assets.poses import POSES


def get_point(sprite, key, frame=0):
    """Get a point value from a sprite, handling both static (int) and animated (list) values."""
    value = sprite[key]
    return value[frame] if isinstance(value, list) else value


class CharacterRenderer():
    def __init__(self, renderer):
        self.renderer = renderer

    def _get_total_frames(self, sprite):
        """Get total frame count including extra_frames for pause at end of cycle."""
        return len(sprite["frames"]) + sprite.get("extra_frames", 0)

    def _get_frame_index(self, sprite, counter):
        """Get actual frame index, clamping to first frame during extra_frames period."""
        frame_count = len(sprite["frames"])
        index = int(counter) % self._get_total_frames(sprite)
        return index if index < frame_count else 0

    def update_animation(self, context, dt):
        """Update and wrap animation counters based on current pose's frame counts."""
        pose_name = context.get("pose", "idle")
        pose = POSES[pose_name]

        body_total = self._get_total_frames(pose["body"])
        head_total = self._get_total_frames(pose["head"])
        eyes_total = self._get_total_frames(pose["eyes"])
        tail_total = self._get_total_frames(pose["tail"])

        context["body"] = (context["body"] + dt * pose["body"].get("speed", 1)) % body_total
        context["head"] = (context["head"] + dt * pose["head"].get("speed", 1)) % head_total
        context["eyes"] = (context["eyes"] + dt * pose["eyes"].get("speed", 1)) % eyes_total
        context["tail"] = (context["tail"] + dt * pose["tail"].get("speed", 1)) % tail_total

    def draw_character(self, context, x, y):
        pose_name = context.get("pose", "idle")
        pose = POSES[pose_name]

        # Body
        body = pose["body"]
        body_frame = self._get_frame_index(body, context.get("body", 0))
        body_x = x - body["anchor_x"]
        body_y = y - body["anchor_y"]
        self.renderer.draw_sprite_obj(body, body_x, body_y, frame=body_frame)

        # Head
        head = pose["head"]
        head_frame = self._get_frame_index(head, context.get("head", 0))
        head_root_x = body_x + get_point(body, "head_x", body_frame)
        head_root_y = body_y + get_point(body, "head_y", body_frame)

        head_x = head_root_x - head["anchor_x"]
        head_y = head_root_y - head["anchor_y"]
        self.renderer.draw_sprite_obj(head, head_x, head_y, frame=head_frame)

        # Eyes
        eyes = pose["eyes"]
        eye_frame = self._get_frame_index(eyes, context.get("eyes", 0))
        eye_x = head_x + get_point(head, "eye_x", head_frame) - eyes["anchor_x"]
        eye_y = head_y + get_point(head, "eye_y", head_frame) - eyes["anchor_y"]
        self.renderer.draw_sprite_obj(eyes, eye_x, eye_y, frame=eye_frame)

        # Tail
        tail = pose["tail"]
        tail_frame = self._get_frame_index(tail, context.get("tail", 0))
        tail_root_x = body_x + get_point(body, "tail_x", body_frame)
        tail_root_y = body_y + get_point(body, "tail_y", body_frame)
        tail_x = tail_root_x - tail["anchor_x"]
        tail_y = tail_root_y - tail["anchor_y"]
        self.renderer.draw_sprite_obj(tail, tail_x, tail_y, frame=tail_frame)
