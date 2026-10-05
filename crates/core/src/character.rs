use embedded_graphics::prelude::Point;

use crate::render::{Renderer, Sprite, SpriteOpts};

pub struct CharBody {
    pub sprite: Sprite,
    pub anchor_x: i16,
    pub anchor_y: i16,
    pub head_x: i16,
    pub head_y: i16,
    pub tail_x: i16,
    pub tail_y: i16,
    pub speed: f32,
    pub extra_frames: u8,
}

pub struct CharHead {
    pub sprite: Sprite,
    pub anchor_x: i16,
    pub anchor_y: i16,
    pub eye_x: i16,
    pub eye_y: i16,
    pub speed: f32,
    pub extra_frames: u8,
}

pub struct CharPart {
    pub sprite: Sprite,
    pub anchor_x: i16,
    pub anchor_y: i16,
    pub speed: f32,
    pub extra_frames: u8,
}

pub struct Pose {
    pub body: &'static CharBody,
    pub head: &'static CharHead,
    pub tail: &'static CharPart,
    pub eyes: Option<&'static CharPart>,
    pub head_first: bool,
    pub tail_last: bool,
    pub head_offset: Option<(i16, i16)>,
}

pub struct PoseAnim {
    pub body: f32,
    pub head: f32,
    pub eyes: f32,
    pub tail: f32,
    rng: u32,
}

impl PoseAnim {
    pub fn new(seed: u32) -> Self {
        Self {
            body: 0.0,
            head: 0.0,
            eyes: 0.0,
            tail: 0.0,
            rng: seed.max(1),
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn random_offset(&mut self, total: usize) -> f32 {
        if total == 0 {
            return 0.0;
        }
        let r = (self.next_u32() & 0xFFFFFF) as f32 / (1u32 << 24) as f32;
        r * total as f32
    }

    pub fn reseed_for(&mut self, pose: &Pose) {
        let body_total = total_frames(pose.body.sprite.frames.len(), pose.body.extra_frames);
        let head_total = total_frames(pose.head.sprite.frames.len(), pose.head.extra_frames);
        let tail_total = total_frames(pose.tail.sprite.frames.len(), pose.tail.extra_frames);
        self.body = self.random_offset(body_total);
        self.head = self.random_offset(head_total);
        self.tail = self.random_offset(tail_total);
        self.eyes = match pose.eyes {
            Some(e) => {
                let t = total_frames(e.sprite.frames.len(), e.extra_frames);
                self.random_offset(t)
            }
            None => 0.0,
        };
    }

    pub fn update(&mut self, pose: &Pose, dt: f32) {
        self.body = advance(self.body, pose.body.speed, dt, pose.body.sprite.frames.len(), pose.body.extra_frames);
        self.head = advance(self.head, pose.head.speed, dt, pose.head.sprite.frames.len(), pose.head.extra_frames);
        self.tail = advance(self.tail, pose.tail.speed, dt, pose.tail.sprite.frames.len(), pose.tail.extra_frames);
        if let Some(eyes) = pose.eyes {
            self.eyes = advance(self.eyes, eyes.speed, dt, eyes.sprite.frames.len(), eyes.extra_frames);
        }
    }
}

fn advance(timer: f32, speed: f32, dt: f32, frames_len: usize, extra: u8) -> f32 {
    let total = total_frames(frames_len, extra) as f32;
    let mut next = (timer + dt * speed) % total;
    if next < 0.0 {
        next += total;
    }
    next
}

fn total_frames(frames_len: usize, extra: u8) -> usize {
    (frames_len + extra as usize).max(1)
}

fn frame_index(timer: f32, frames_len: usize, extra: u8) -> usize {
    let total = total_frames(frames_len, extra);
    let idx = (timer as usize) % total;
    if idx < frames_len {
        idx
    } else {
        0
    }
}

fn mirror_x(value: i16, width: u16, mirror: bool) -> i16 {
    if mirror {
        width as i16 - value
    } else {
        value
    }
}

pub struct PoseLayout {
    pub body: (i16, i16),
    pub head: (i16, i16),
    pub tail: (i16, i16),
    pub eyes: Option<(i16, i16)>,
    pub head_attach: (i16, i16),
    pub tail_attach: (i16, i16),
    pub eye_attach: Option<(i16, i16)>,
}

pub fn pose_layout(pose: &Pose, pos: Point, mirror_h: bool) -> PoseLayout {
    let body = pose.body;
    let head = pose.head;
    let tail = pose.tail;

    let body_anchor_x = mirror_x(body.anchor_x, body.sprite.width, mirror_h);
    let body_x = pos.x as i16 - body_anchor_x;
    let body_y = pos.y as i16 - body.anchor_y;

    let head_attach_x = body_x + mirror_x(body.head_x, body.sprite.width, mirror_h);
    let head_attach_y = body_y + body.head_y;
    let head_anchor_x = mirror_x(head.anchor_x, head.sprite.width, mirror_h);
    let mut head_x = head_attach_x - head_anchor_x;
    let mut head_y = head_attach_y - head.anchor_y;
    if let Some((ox, oy)) = pose.head_offset {
        head_x += if mirror_h { -ox } else { ox };
        head_y += oy;
    }

    let tail_attach_x = body_x + mirror_x(body.tail_x, body.sprite.width, mirror_h);
    let tail_attach_y = body_y + body.tail_y;
    let tail_anchor_x = mirror_x(tail.anchor_x, tail.sprite.width, mirror_h);
    let tail_x = tail_attach_x - tail_anchor_x;
    let tail_y = tail_attach_y - tail.anchor_y;

    let (eyes, eye_attach) = if let Some(e) = pose.eyes {
        let attach_x = head_x + mirror_x(head.eye_x, head.sprite.width, mirror_h);
        let attach_y = head_y + head.eye_y;
        let anchor_x = mirror_x(e.anchor_x, e.sprite.width, mirror_h);
        let ex = attach_x - anchor_x;
        let ey = attach_y - e.anchor_y;
        (Some((ex, ey)), Some((attach_x, attach_y)))
    } else {
        (None, None)
    };

    PoseLayout {
        body: (body_x, body_y),
        head: (head_x, head_y),
        tail: (tail_x, tail_y),
        eyes,
        head_attach: (head_attach_x, head_attach_y),
        tail_attach: (tail_attach_x, tail_attach_y),
        eye_attach,
    }
}

/// Rows from the top of a head sprite down to the top of the skull. The ears
/// occupy roughly this many rows above it on the head sprites.
const HEAD_EAR_HEIGHT: i16 = 6;

/// Screen y of the top of the cat's head (below the ear tips) for `pose`
/// drawn at `pos`. Used to rest hand / brush sprites on the head.
pub fn head_top_y(pose: &Pose, pos: Point, mirror_h: bool) -> i32 {
    let layout = pose_layout(pose, pos, mirror_h);
    (layout.head.1 + HEAD_EAR_HEIGHT) as i32
}

pub fn draw_pose(
    renderer: &mut Renderer,
    pose: &Pose,
    anim: &PoseAnim,
    pos: Point,
    mirror_h: bool,
    eye_override: Option<usize>,
) {
    let layout = pose_layout(pose, pos, mirror_h);
    let body_frame = frame_index(anim.body, pose.body.sprite.frames.len(), pose.body.extra_frames);
    let head_frame = frame_index(anim.head, pose.head.sprite.frames.len(), pose.head.extra_frames);
    let tail_frame = frame_index(anim.tail, pose.tail.sprite.frames.len(), pose.tail.extra_frames);
    let eye_frame = pose
        .eyes
        .map(|e| match eye_override {
            Some(ov) => ov.min(e.sprite.frames.len().saturating_sub(1)),
            None => frame_index(anim.eyes, e.sprite.frames.len(), e.extra_frames),
        });

    let opts = |frame: usize| SpriteOpts {
        frame,
        transparent: true,
        invert: false,
        mirror_h,
        mirror_v: false,
        transparent_color: false,
    };

    if !pose.tail_last {
        renderer.draw_sprite(
            &pose.tail.sprite,
            Point::new(layout.tail.0 as i32, layout.tail.1 as i32),
            opts(tail_frame),
        );
    }
    if pose.head_first {
        renderer.draw_sprite(
            &pose.head.sprite,
            Point::new(layout.head.0 as i32, layout.head.1 as i32),
            opts(head_frame),
        );
        renderer.draw_sprite(
            &pose.body.sprite,
            Point::new(layout.body.0 as i32, layout.body.1 as i32),
            opts(body_frame),
        );
    } else {
        renderer.draw_sprite(
            &pose.body.sprite,
            Point::new(layout.body.0 as i32, layout.body.1 as i32),
            opts(body_frame),
        );
        renderer.draw_sprite(
            &pose.head.sprite,
            Point::new(layout.head.0 as i32, layout.head.1 as i32),
            opts(head_frame),
        );
    }
    if let (Some(eyes), Some(epos), Some(efr)) = (pose.eyes, layout.eyes, eye_frame) {
        renderer.draw_sprite(
            &eyes.sprite,
            Point::new(epos.0 as i32, epos.1 as i32),
            opts(efr),
        );
    }
    if pose.tail_last {
        renderer.draw_sprite(
            &pose.tail.sprite,
            Point::new(layout.tail.0 as i32, layout.tail.1 as i32),
            opts(tail_frame),
        );
    }
}
