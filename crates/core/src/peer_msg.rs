//! Wire format for peer messages (ESP-NOW and BLE) between Rust devices.
//!
//! Every message is a 4-byte ASCII tag followed by a small binary body.
//! The tag identifies the message kind; the body shape is per-tag and
//! documented next to each encode/decode helper.
//!
//! Tags are padded to 4 bytes with spaces (so they sort and grep cleanly
//! in the debug log). Unknown tags are dropped silently by the
//! receiver.
//!
//! Currently encoded:
//! - `b"voc "` is vocalize: u8 icon index (see [`BubbleIcon::to_wire`]).
//!
//! Phase 4/5 will add `b"hi  "`, `b"vreq"`, `b"vok "`, `b"vno "`,
//! `b"vst "`, `b"venv"`, `b"vbeh"`, `b"vss "`, `b"vse "`, `b"vbye"`.

#![allow(dead_code)]

use heapless::String;

use crate::{
    assets::character::{PoseId, ALL_POSES},
    behavior::BehaviorId,
    context::PET_NAME_MAX,
    scene::SceneId,
    time_system::{Season, Weather},
    ui::bubble::BubbleIcon,
};

pub const TAG_VOCALIZE: [u8; 4] = *b"voc ";
/// Discovery hello. Body: u8 name_len, then name_len UTF-8 bytes
/// (truncated to [`PET_NAME_MAX`]).
pub const TAG_HELLO: [u8; 4] = *b"hi  ";
/// Visit request. Same body shape as hello (sender's pet name).
pub const TAG_VREQ: [u8; 4] = *b"vreq";
/// Visit accepted. Same body shape (acceptor's pet name).
pub const TAG_VOK: [u8; 4] = *b"vok ";
/// Visit declined. Empty body.
pub const TAG_VNO: [u8; 4] = *b"vno ";
/// End visit. Empty body. Either side may send when leaving the visit.
pub const TAG_VBYE: [u8; 4] = *b"vbye";
/// Visitor state heartbeat. Body shape:
///   `[x_lo, x_hi, pose_idx, mirror, vx_q4]`
/// where `x` is a little-endian i16, `pose_idx` is the index into
/// [`ALL_POSES`], `mirror` is 0/1, and `vx_q4` is an i8 storing
/// `vx * 4.0` (so range +/-31.75 px/s, plenty for our walking cats).
pub const TAG_VST: [u8; 4] = *b"vst ";

/// Fixed wire size of a `vst ` body, after the 4-byte tag.
pub const VST_BODY_LEN: usize = 5;

/// Behavior id heartbeat. Body: u8 [`BehaviorId`] discriminant. Sent
/// from one peer to the other whenever the local cat's behavior
/// changes during a visit. Receiver may mirror via the lookup table
/// in [`mirror_behavior`].
pub const TAG_VBEH: [u8; 4] = *b"vbeh";

/// Environment sync (inviter -> invitee, ~1 Hz during a visit). Body:
/// u8 hour, u8 minute, u8 weather index, u8 season index. Receiver
/// applies to its own context so both screens show the inviter's
/// time-of-day / weather. Other env fields (moon phase) can be added
/// later without breaking forward-compat thanks to the embedded
/// length check on receive.
pub const TAG_VENV: [u8; 4] = *b"venv";
pub const VENV_BODY_LEN: usize = 4;

/// Greeting trigger (inviter -> invitee on first scene entry of a
/// visit). Empty body; the invitee responds by triggering its own
/// greeting toward the inviter.
pub const TAG_VGREET: [u8; 4] = *b"vgrt";

/// Proximity sniff (inviter -> invitee when the local cat gets close
/// enough to the visitor entity). Empty body; invitee responds by
/// also triggering a sniff in place.
pub const TAG_VPROX: [u8; 4] = *b"vprx";

/// Shooting star (inviter -> invitee). Body shape:
/// `[x_lo,x_hi, y_lo,y_hi, max_life_lo,max_life_hi, sx, sy]` (same
/// numbers the local sky uses to spawn the streak). Receiver applies
/// directly to its sky.
pub const TAG_VSS: [u8; 4] = *b"vss ";
pub const VSS_BODY_LEN: usize = 8;

/// Other sky event (inviter -> invitee). Body shape:
/// `[event_idx, dir, y_lo,y_hi, speed_q4]`. `event_idx` selects
/// which kind (balloon / plane / etc.), `dir` is -1/0/1 stored as
/// i8, `y` is i16 LE, `speed_q4` is i8 = `speed * 4.0`.
pub const TAG_VSE: [u8; 4] = *b"vse ";
pub const VSE_BODY_LEN: usize = 5;

/// Scene change during a visit. Sent from the broadcaster when they
/// switch to a different LocationScene; the peer follows. Body: u8
/// scene index (see [`scene_id_to_wire`]). Only `LocationScene`-
/// backed scenes are encodable. Going into a minigame leaves the
/// peer in the prior scene until the user returns.
pub const TAG_VLOC: [u8; 4] = *b"vloc";

/// Generate paired `pub fn $to(_: $Enum) -> u8` and
/// `pub fn $from(_: u8) -> $Enum` from a `Variant => byte` table.
///
/// The decode arm falls back to `$default` for unknown bytes so the receiver
/// always gets a sensible value rather than dropping the whole message.
/// Variant bytes are stable across versions; the table is append-only.
macro_rules! wire_enum_default {
    (
        $Enum:ident, $to:ident, $from:ident, $default:expr;
        $($Variant:ident => $byte:literal),+ $(,)?
    ) => {
        pub fn $to(e: $Enum) -> u8 {
            match e {
                $($Enum::$Variant => $byte,)+
            }
        }
        pub fn $from(b: u8) -> $Enum {
            match b {
                $($byte => $Enum::$Variant,)+
                _ => $default,
            }
        }
    };
}

/// Variant of [`wire_enum_default`] whose decode returns `None` on an
/// unrecognized byte (used where forward-compat would silently mask a real
/// protocol mismatch - e.g. behavior ids).
macro_rules! wire_enum_option {
    (
        $Enum:ident, $to:ident, $from:ident;
        $($Variant:ident => $byte:literal),+ $(,)?
    ) => {
        pub fn $to(e: $Enum) -> u8 {
            match e {
                $($Enum::$Variant => $byte,)+
            }
        }
        pub fn $from(b: u8) -> Option<$Enum> {
            Some(match b {
                $($byte => $Enum::$Variant,)+
                _ => return None,
            })
        }
    };
}

/// Variant of [`wire_enum_option`] where only a *subset* of the enum's
/// variants are encodable — `to_wire` itself returns `Option<u8>` and yields
/// `None` for any variant not listed. Used by [`scene_id_to_wire`] since
/// only `LocationScene`-backed scenes mirror across a visit.
macro_rules! wire_enum_subset_option {
    (
        $Enum:ident, $to:ident, $from:ident;
        $($Variant:ident => $byte:literal),+ $(,)?
    ) => {
        pub fn $to(e: $Enum) -> Option<u8> {
            Some(match e {
                $($Enum::$Variant => $byte,)+
                _ => return None,
            })
        }
        pub fn $from(b: u8) -> Option<$Enum> {
            Some(match b {
                $($byte => $Enum::$Variant,)+
                _ => return None,
            })
        }
    };
}

wire_enum_default! {
    BubbleIcon, icon_to_wire, icon_from_wire, BubbleIcon::Exclaim;
    Heart      => 0,
    Question   => 1,
    Exclaim    => 2,
    Note       => 3,
    Star       => 4,
    Hunger     => 5,
    Discomfort => 6,
    Bored      => 7,
    Lonely     => 8,
    Home       => 9,
    Hot        => 10,
    Wet        => 11,
    Cold       => 12,
}

/// Encode a vocalize frame: `[v, o, c, ' ', icon_byte]`.
pub fn encode_vocalize(icon: BubbleIcon) -> [u8; 5] {
    let mut buf = [0u8; 5];
    buf[0..4].copy_from_slice(&TAG_VOCALIZE);
    buf[4] = icon_to_wire(icon);
    buf
}

/// If `data` is a `voc ` frame, return the decoded icon. Returns
/// `None` if the tag does not match or the body is malformed.
pub fn decode_vocalize(data: &[u8]) -> Option<BubbleIcon> {
    if data.len() < 5 {
        return None;
    }
    if data[0..4] != TAG_VOCALIZE {
        return None;
    }
    Some(icon_from_wire(data[4]))
}

/// Type alias matching the on-context pet-name buffer.
pub type PetName = String<PET_NAME_MAX>;

/// Buffer big enough to encode any of the named frames (`hi`, `vreq`,
/// `vok`): 4-byte tag + 1-byte length + up to [`PET_NAME_MAX`] bytes.
pub const NAMED_FRAME_MAX: usize = 4 + 1 + PET_NAME_MAX;

/// Encode `[tag, name_len, name_bytes...]` into a stack buffer. The
/// returned slice length is `5 + name.len().min(PET_NAME_MAX)`. Same
/// shape used by hello / vreq / vok.
pub fn encode_named(tag: [u8; 4], name: &str, buf: &mut [u8; NAMED_FRAME_MAX]) -> usize {
    buf[0..4].copy_from_slice(&tag);
    let bytes = name.as_bytes();
    let n = bytes.len().min(PET_NAME_MAX);
    buf[4] = n as u8;
    buf[5..5 + n].copy_from_slice(&bytes[..n]);
    5 + n
}

/// Decode `[tag, name_len, name_bytes...]`. Returns the embedded name
/// if `data` starts with `tag` and the length byte is consistent; else
/// `None`. Invalid UTF-8 bytes are dropped from the returned name (the
/// pet-name surface is restricted to ASCII anyway).
pub fn decode_named(tag: [u8; 4], data: &[u8]) -> Option<PetName> {
    if data.len() < 5 || data[0..4] != tag {
        return None;
    }
    let n = data[4] as usize;
    if data.len() < 5 + n || n > PET_NAME_MAX {
        return None;
    }
    let mut out = PetName::new();
    if let Ok(s) = core::str::from_utf8(&data[5..5 + n]) {
        for c in s.chars() {
            if out.push(c).is_err() {
                break;
            }
        }
    }
    Some(out)
}

/// Encode an empty-body frame (vno, vbye). Always 4 bytes.
pub fn encode_empty(tag: [u8; 4]) -> [u8; 4] {
    tag
}

/// Match an empty-body frame. Returns true if `data` begins with
/// `tag`; ignores any trailing bytes for forward-compatibility.
pub fn matches_tag(tag: [u8; 4], data: &[u8]) -> bool {
    data.len() >= 4 && data[0..4] == tag
}

/// Map a [`PoseId`] to the byte we put on the wire (the index into
/// [`ALL_POSES`]). Returns 0 (which is `SittingSideNeutral`) if the
/// pose somehow isn't in the array; that shouldn't happen but a
/// sane default beats panicking on a broadcast hot path.
pub fn pose_to_wire(pose: PoseId) -> u8 {
    ALL_POSES
        .iter()
        .position(|p| *p == pose)
        .unwrap_or(0) as u8
}

/// Inverse of [`pose_to_wire`]. Out-of-range bytes fall back to
/// [`PoseId::SittingSideNeutral`].
pub fn pose_from_wire(b: u8) -> PoseId {
    *ALL_POSES.get(b as usize).unwrap_or(&PoseId::SittingSideNeutral)
}

/// Encode a `vst ` frame: tag + i16-LE x + u8 pose index + u8 mirror
/// + i8 quantized vx.
pub fn encode_vst(x: i32, pose: PoseId, mirror: bool, vx: f32) -> [u8; 4 + VST_BODY_LEN] {
    let mut buf = [0u8; 4 + VST_BODY_LEN];
    buf[0..4].copy_from_slice(&TAG_VST);
    let x_i16 = x.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    buf[4..6].copy_from_slice(&x_i16.to_le_bytes());
    buf[6] = pose_to_wire(pose);
    buf[7] = if mirror { 1 } else { 0 };
    // Quantize vx to i8 with 0.25 px/s precision. Saturating cast
    // means very fast cats just clamp to the rails, which is fine.
    let vx_q4 = (vx * 4.0).clamp(i8::MIN as f32, i8::MAX as f32) as i8;
    buf[8] = vx_q4 as u8;
    buf
}

/// Decoded `vst ` frame.
pub struct DecodedVst {
    pub x: i32,
    pub pose: PoseId,
    pub mirror: bool,
    pub vx: f32,
}

/// Decode a `vst ` frame. Returns `None` on a tag mismatch or short
/// buffer.
pub fn decode_vst(data: &[u8]) -> Option<DecodedVst> {
    if data.len() < 4 + VST_BODY_LEN || data[0..4] != TAG_VST {
        return None;
    }
    let x = i16::from_le_bytes([data[4], data[5]]) as i32;
    let pose = pose_from_wire(data[6]);
    let mirror = data[7] != 0;
    let vx_q4 = data[8] as i8;
    let vx = vx_q4 as f32 / 4.0;
    Some(DecodedVst {
        x,
        pose,
        mirror,
        vx,
    })
}

/// Decode a `vbeh` frame into its [`BehaviorId`]. `None` on tag
/// mismatch, short buffer, or out-of-range discriminant.
pub fn decode_vbeh(data: &[u8]) -> Option<BehaviorId> {
    if data.len() < 5 || data[0..4] != TAG_VBEH {
        return None;
    }
    behavior_from_wire(data[4])
}

/// Encode a `vbeh` frame.
pub fn encode_vbeh(id: BehaviorId) -> [u8; 5] {
    let mut buf = [0u8; 5];
    buf[0..4].copy_from_slice(&TAG_VBEH);
    buf[4] = behavior_to_wire(id);
    buf
}

// Wire bytes mirror the source-order discriminants `as u8` used historically;
// generating both directions from a single table keeps them in lockstep if the
// enum ever has variants inserted out of order.
wire_enum_option! {
    BehaviorId, behavior_to_wire, behavior_from_wire;
    Idle          => 0,
    Sleeping      => 1,
    Napping       => 2,
    Stretching    => 3,
    Kneading      => 4,
    Lounging      => 5,
    Investigating => 6,
    Observing     => 7,
    Chattering    => 8,
    Zoomies       => 9,
    Vocalizing    => 10,
    SelfGrooming  => 11,
    BeingGroomed  => 12,
    Hunting       => 13,
    GiftBringing  => 14,
    Pacing        => 15,
    Sulking       => 16,
    Mischief      => 17,
    Hiding        => 18,
    Training      => 19,
    Playing       => 20,
    Affection     => 21,
    Attention     => 22,
    Eating        => 23,
    Startled      => 24,
    Meandering    => 25,
    GoTo          => 26,
    Hearing       => 27,
    Greeting      => 28,
}

/// Encode a `venv` frame. Weather/season pass through as `as u8`;
/// the receiver maps back through [`weather_from_wire`] /
/// [`season_from_wire`].
pub fn encode_venv(hour: u8, minute: u8, weather: u8, season: u8) -> [u8; 4 + VENV_BODY_LEN] {
    let mut buf = [0u8; 4 + VENV_BODY_LEN];
    buf[0..4].copy_from_slice(&TAG_VENV);
    buf[4] = hour;
    buf[5] = minute;
    buf[6] = weather;
    buf[7] = season;
    buf
}

pub struct DecodedVenv {
    pub hour: u8,
    pub minute: u8,
    pub weather: u8,
    pub season: u8,
}

pub fn decode_venv(data: &[u8]) -> Option<DecodedVenv> {
    if data.len() < 4 + VENV_BODY_LEN || data[0..4] != TAG_VENV {
        return None;
    }
    Some(DecodedVenv {
        hour: data[4],
        minute: data[5],
        weather: data[6],
        season: data[7],
    })
}

pub fn encode_vss(x: i32, y: i32, max_life: i32, sx: i32, sy: i32) -> [u8; 4 + VSS_BODY_LEN] {
    let mut buf = [0u8; 4 + VSS_BODY_LEN];
    buf[0..4].copy_from_slice(&TAG_VSS);
    let xb = (x.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_le_bytes();
    let yb = (y.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_le_bytes();
    let ml = (max_life.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_le_bytes();
    buf[4..6].copy_from_slice(&xb);
    buf[6..8].copy_from_slice(&yb);
    buf[8..10].copy_from_slice(&ml);
    buf[10] = sx.clamp(i8::MIN as i32, i8::MAX as i32) as i8 as u8;
    buf[11] = sy.clamp(i8::MIN as i32, i8::MAX as i32) as i8 as u8;
    buf
}

pub struct DecodedVss {
    pub x: i32,
    pub y: i32,
    pub max_life: i32,
    pub sx: i32,
    pub sy: i32,
}

wire_enum_default! {
    Weather, weather_to_wire, weather_from_wire, Weather::Clear;
    Clear    => 0,
    Cloudy   => 1,
    Overcast => 2,
    Windy    => 3,
    Rain     => 4,
    Storm    => 5,
    Snow     => 6,
}

wire_enum_default! {
    Season, season_to_wire, season_from_wire, Season::Spring;
    Winter => 0,
    Spring => 1,
    Summer => 2,
    Fall   => 3,
}

pub fn decode_vss(data: &[u8]) -> Option<DecodedVss> {
    if data.len() < 4 + VSS_BODY_LEN || data[0..4] != TAG_VSS {
        return None;
    }
    Some(DecodedVss {
        x: i16::from_le_bytes([data[4], data[5]]) as i32,
        y: i16::from_le_bytes([data[6], data[7]]) as i32,
        max_life: i16::from_le_bytes([data[8], data[9]]) as i32,
        sx: (data[10] as i8) as i32,
        sy: (data[11] as i8) as i32,
    })
}

/// Encode a `vse ` frame (other sky events: balloons / planes).
pub fn encode_vse(event_idx: u8, going_right: bool, y: i16, speed: f32) -> [u8; 4 + VSE_BODY_LEN] {
    let mut buf = [0u8; 4 + VSE_BODY_LEN];
    buf[0..4].copy_from_slice(&TAG_VSE);
    buf[4] = event_idx;
    buf[5] = if going_right { 1 } else { 0 };
    buf[6..8].copy_from_slice(&y.to_le_bytes());
    // Quantize speed to i8 with 0.25 px/s precision (range +/-31.75).
    let speed_q4 = (speed * 4.0).clamp(i8::MIN as f32, i8::MAX as f32) as i8;
    buf[8] = speed_q4 as u8;
    buf
}

pub struct DecodedVse {
    pub event_idx: u8,
    pub going_right: bool,
    pub y: i16,
    pub speed: f32,
}

// `scene_id_to_wire` likewise returns `Option<u8>` rather than `u8` because
// only `LocationScene`-backed scenes are encodable; minigames, menus, debug
// screens, and vacations are intentionally unrepresentable.
wire_enum_subset_option! {
    SceneId, scene_id_to_wire, scene_id_from_wire;
    Inside    => 0,
    Outside   => 1,
    Treehouse => 2,
    Bedroom   => 3,
    Kitchen   => 4,
}

/// Encode a `vloc` frame. Returns `None` if the scene isn't
/// representable on the wire.
pub fn encode_vloc(id: SceneId) -> Option<[u8; 5]> {
    let idx = scene_id_to_wire(id)?;
    let mut buf = [0u8; 5];
    buf[0..4].copy_from_slice(&TAG_VLOC);
    buf[4] = idx;
    Some(buf)
}

/// Decode a `vloc` frame.
pub fn decode_vloc(data: &[u8]) -> Option<SceneId> {
    if data.len() < 5 || data[0..4] != TAG_VLOC {
        return None;
    }
    scene_id_from_wire(data[4])
}

pub fn decode_vse(data: &[u8]) -> Option<DecodedVse> {
    if data.len() < 4 + VSE_BODY_LEN || data[0..4] != TAG_VSE {
        return None;
    }
    Some(DecodedVse {
        event_idx: data[4],
        going_right: data[5] != 0,
        y: i16::from_le_bytes([data[6], data[7]]),
        speed: (data[8] as i8) as f32 / 4.0,
    })
}
