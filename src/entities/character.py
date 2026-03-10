from entities.entity import Entity
from assets.character import POSES
from sprite_transform import mirror_sprite_h


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

    def __init__(self, x, y, pose="sitting.side.neutral", context=None):
        super().__init__(x, y)
        self.pose_name = pose
        self._pose = get_pose(pose)
        self.context = context

        self.mirror = False  # True = facing left, False = facing right
        self._mirror_cache = {}    # id(sprite) -> {"frames": [...], "inv_fill_frames": [...]}
        self._inv_fill_cache = {}  # id(sprite) -> [inverted_fill_frame, ...]

        self.anim_body = 0.0
        self.anim_head = 0.0
        self.anim_eyes = 0.0
        self.anim_tail = 0.0

        # The currently active behavior instance (always set when context exists)
        self.current_behavior = None

        # Behavior manager — handles lazy loading and module lifecycle
        self.behavior_manager = None

        # Burst sparkle effects (played via play_bursts())
        self._burst_sprite = None
        self._burst_timer = 0.0
        self._bursts = []
        if context:
            from behavior_manager import BehaviorManager
            self.behavior_manager = BehaviorManager(self)
            self.behavior_manager.trigger('idle')

    def trigger(self, name, **kwargs):
        """Interrupt the current behavior and start a player-initiated one.

        The interrupted behavior's stop(completed=False) fires so scene
        callbacks (e.g., removing a food bowl) can clean up. The new
        behavior then owns the transition chain from that point.

        Args:
            name: The behavior name string (key in BehaviorManager._REGISTRY).
            **kwargs: Passed through to the behavior's start() method.
        """
        if self.behavior_manager:
            self.behavior_manager.trigger(name, **kwargs)

    def set_pose(self, pose_name):
        """Change the character's pose using dot notation (e.g., 'sitting.side.neutral')."""
        pose = get_pose(pose_name)
        if pose is not None:
            self.pose_name = pose_name
            self._pose = pose
            self._mirror_cache = {}
            self._inv_fill_cache = {}
        else:
            print(f"[character] Failed to set pose: '{pose_name}', keeping current pose")

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

    def play_bursts(self, count=5, sprite=None):
        """Trigger sparkle burst effects scattered around the pet.

        Args:
            count: Number of bursts to spawn.
            sprite: Sprite dict to use (defaults to BURST1).
        """
        import random
        from assets.effects import BURST1
        self._burst_sprite = sprite if sprite is not None else BURST1
        self._burst_timer = 0.0
        self._bursts = [
            {
                "x": random.randint(-35, 35),
                "y": random.randint(-50, -20),
                "delay": i * 0.5 + random.uniform(0.0, 0.25),
            }
            for i in range(count)
        ]

    def update(self, dt):
        """Update animation counters and the current behavior."""
        if self._pose is None:
            return

        pose = self._pose
        self.anim_body = (self.anim_body + dt * pose["body"].get("speed", 1)) % self._get_total_frames(pose["body"])
        self.anim_head = (self.anim_head + dt * pose["head"].get("speed", 1)) % self._get_total_frames(pose["head"])
        self.anim_eyes = (self.anim_eyes + dt * pose["eyes"].get("speed", 1)) % self._get_total_frames(pose["eyes"])
        self.anim_tail = (self.anim_tail + dt * pose["tail"].get("speed", 1)) % self._get_total_frames(pose["tail"])

        if self._bursts:
            self._burst_timer += dt
            sprite = self._burst_sprite
            total = len(sprite["frames"]) / sprite["speed"]
            if all(self._burst_timer - b["delay"] >= total for b in self._bursts):
                self._bursts = []
                self._burst_sprite = None
                self._burst_timer = 0.0

        if self.current_behavior and self.context:
            self.current_behavior.update(dt)

    def _ensure_mirrored(self, sprite):
        """Return cached dict of pre-mirrored (and pre-inverted fill) bytearrays."""
        sid = id(sprite)
        if sid not in self._mirror_cache:
            w, h = sprite["width"], sprite["height"]
            entry = {"frames": [mirror_sprite_h(f, w, h) for f in sprite["frames"]]}
            if "fill_frames" in sprite:
                mirrored_fills = [mirror_sprite_h(f, w, h) for f in sprite["fill_frames"]]
                entry["inv_fill_frames"] = [bytearray(b ^ 0xFF for b in f) for f in mirrored_fills]
            self._mirror_cache[sid] = entry
        return self._mirror_cache[sid]

    def _ensure_inv_fill(self, sprite):
        """Return cached list of pre-inverted fill_frame bytearrays for this sprite."""
        sid = id(sprite)
        if sid not in self._inv_fill_cache:
            self._inv_fill_cache[sid] = [bytearray(b ^ 0xFF for b in f) for f in sprite["fill_frames"]]
        return self._inv_fill_cache[sid]

    def _draw_part_mirrored(self, renderer, sprite, x, y, frame):
        """Draw a sprite part using cached pre-mirrored, pre-inverted frames."""
        cached = self._ensure_mirrored(sprite)
        if "inv_fill_frames" in cached:
            renderer.draw_sprite(cached["inv_fill_frames"][frame], sprite["width"], sprite["height"],
                                 x, y, transparent=True, transparent_color=1)
        renderer.draw_sprite(cached["frames"][frame], sprite["width"], sprite["height"], x, y)

    def _draw_part(self, renderer, sprite, x, y, frame):
        """Draw a sprite part using cached pre-inverted fill frames (no per-frame allocation)."""
        if "fill_frames" in sprite:
            inv_fills = self._ensure_inv_fill(sprite)
            renderer.draw_sprite(inv_fills[frame], sprite["width"], sprite["height"],
                                 x, y, transparent=True, transparent_color=1)
        renderer.draw_sprite(sprite["frames"][frame], sprite["width"], sprite["height"], x, y)

    def draw(self, renderer, mirror=False, camera_offset=0, eye_frame=None):
        """Draw the character at its position.

        Args:
            renderer: the renderer to draw with
            mirror: if True, flip the character horizontally
            camera_offset: horizontal camera offset to subtract from x position
            eye_frame: optional int to override the eye animation frame index.
                       If None, checks the current behavior's eye_frame_override,
                       then falls back to the normal animation frame.
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
        eye_frame_idx = self._get_frame_index(eyes, self.anim_eyes)
        if eye_frame is None and self.current_behavior is not None:
            beh_override = self.current_behavior.eye_frame_override
            if beh_override is not None:
                eye_frame_idx = beh_override
        elif eye_frame is not None:
            eye_frame_idx = eye_frame
        eye_x = head_x + self._get_point(head, "eye_x", head_frame, mirror) - self._get_anchor_x(eyes, mirror)
        eye_y = head_y + self._get_point(head, "eye_y", head_frame) - eyes["anchor_y"]

        tail = pose["tail"]
        tail_frame = self._get_frame_index(tail, self.anim_tail)
        tail_root_x = body_x + self._get_point(body, "tail_x", body_frame, mirror)
        tail_root_y = body_y + self._get_point(body, "tail_y", body_frame)
        tail_x = tail_root_x - self._get_anchor_x(tail, mirror)
        tail_y = tail_root_y - tail["anchor_y"]

        # Draw the parts — use cached mirrored frames to avoid per-frame allocation
        if mirror:
            self._draw_part_mirrored(renderer, tail, tail_x, tail_y, tail_frame)
            if pose.get("head_first"):
                self._draw_part_mirrored(renderer, head, head_x, head_y, head_frame)
                self._draw_part_mirrored(renderer, body, body_x, body_y, body_frame)
            else:
                self._draw_part_mirrored(renderer, body, body_x, body_y, body_frame)
                self._draw_part_mirrored(renderer, head, head_x, head_y, head_frame)
            self._draw_part_mirrored(renderer, eyes, eye_x, eye_y, eye_frame_idx)
        else:
            self._draw_part(renderer, tail, tail_x, tail_y, tail_frame)
            if pose.get("head_first"):
                self._draw_part(renderer, head, head_x, head_y, head_frame)
                self._draw_part(renderer, body, body_x, body_y, body_frame)
            else:
                self._draw_part(renderer, body, body_x, body_y, body_frame)
                self._draw_part(renderer, head, head_x, head_y, head_frame)
            self._draw_part(renderer, eyes, eye_x, eye_y, eye_frame_idx)

        # Draw active behavior's visual effects (bubbles, etc.)
        if self.current_behavior:
            self.current_behavior.draw(renderer, x, y, mirror)

        # Draw burst sparkle effects
        if self._bursts and self._burst_sprite:
            sprite = self._burst_sprite
            frame_dur = 1.0 / sprite["speed"]
            total = len(sprite["frames"]) * frame_dur
            hw = sprite["width"] // 2
            hh = sprite["height"] // 2
            for burst in self._bursts:
                elapsed = self._burst_timer - burst["delay"]
                if elapsed < 0 or elapsed >= total:
                    continue
                frame_idx = min(int(elapsed / frame_dur), len(sprite["frames"]) - 1)
                renderer.draw_sprite(
                    sprite["frames"][frame_idx],
                    sprite["width"], sprite["height"],
                    x + burst["x"] - hw, y + burst["y"] - hh,
                    transparent=True, transparent_color=0,
                )
