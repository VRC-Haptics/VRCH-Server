use anyhow::{Result, anyhow};
use glam::Vec3;
use memchr::memchr;

pub const BUNDLE_TAG: &[u8] = b"#bundle";
/// "/" + null + ",F" {no bytes allocated}
pub const MIN_PACKET_BYTES: i32 = 1 + 1 + 2;
/// "#bundle\0" + timetag + 1 -> 4 bytes (i32)
/// 8 + 8 + 1 = 17
pub const BUNDLE_FIRST_SIZE: std::ops::Range<usize> = 17..(17 + 5);
pub const EMPTY: &[u8] = &[];


/// Pad a null-terminated string.
/// Rust -> C style OscStrings
#[inline]
const fn padded_len(content_len: usize) -> usize {
    content_len + (4 - content_len % 4)
}

/// Returns the first singular message of a buffer containing an OSC packet.
pub fn first_message(bytes: &[u8]) -> Result<RefMessage<'_>> {
    if bytes.get(0..BUNDLE_TAG.len()) == Some(BUNDLE_TAG) {
        let size = bytes.get(16..20).ok_or_else(|| anyhow!("truncated bundle header"))?;
        let size = u32::from_be_bytes(size.try_into()?) as usize;
        let element = bytes.get(20..20 + size).ok_or_else(|| anyhow!("bundle element out of range"))?;
        return first_message(element);
    }

    // finds address locations
    let addr_null = memchr(b'\0', bytes).ok_or_else(|| anyhow!("address not terminated"))?;
    let addr = str::from_utf8(&bytes[..addr_null])?;
    let tag_start = padded_len(addr_null);

    let (osc_type, args_start) = get_type(bytes, tag_start)?;
    let len = osc_type.size() as usize;
    let contents = if len > 0 {
        bytes.get(args_start..(args_start + len)).ok_or_else(|| anyhow!("arguments out of range"))?
    } else {
        EMPTY
    };

    Ok(RefMessage { addr, t: osc_type, contents })
}

/// returns type and the null byte index of the tag string
pub fn get_type(bytes: &[u8], tag_start: usize) -> Result<(MsgType, usize)> {
    if bytes.get(tag_start) != Some(&b',') {
        return Err(anyhow!("type-tag string must begin with ','"));
    }
    let tail = bytes.get(tag_start..).ok_or_else(|| anyhow!("tag start out of range"))?;
    let rel_null = memchr(b'\0', tail).ok_or_else(|| anyhow!("type tag not terminated"))?;
    let tag_null = tag_start + rel_null;
    let args_start = tag_start + padded_len(tag_null - tag_start);
 

    let tags = &bytes[tag_start + 1..tag_null];
    let t = match tags.first() {
        None => return Err(anyhow!("empty type tag")),
        Some(b'[') => array_type(tags)?,
        Some(&c) if tags.len() == 1 => scalar(c)?,
        Some(_) => return Err(anyhow!("unsupported multi-argument tag: {tags:?}")),
    };
    Ok((t, args_start))
}

fn scalar(c: u8) -> Result<MsgType> {
    Ok(match c {
        b'f' => MsgType::F32,
        b'i' => MsgType::I32,
        b's' => MsgType::String,
        b'T' => MsgType::BoolTrue,
        b'F' => MsgType::BoolFalse,
        b'N' => MsgType::Nill,
        b'I' => MsgType::Infinitum,
        other => return Err(anyhow!("unsupported OSC type: '{}'", other as char)),
    })
}

/// comma stripped tag slices
fn array_type(tags: &[u8]) -> Result<MsgType> {
    debug_assert_eq!(tags.first(), Some(&b'['));
    let mut count = 0usize;
    for &b in &tags[1..] {
        match b {
            b'f' => count += 1,
            b']' => return count_to_type(count),
            other => return Err(anyhow!("unsupported array element '{}'", other as char)),
        }
    }
    Err(anyhow!("array not closed"))
}

fn count_to_type(count: usize) -> Result<MsgType> {
    Ok(match count {
        1 => MsgType::F32,
        2 => MsgType::ArrayF32X2,
        3 => MsgType::ArrayF32X3,
        4 => MsgType::ArrayF32X4,
        5 => MsgType::ArrayF32X5,
        6 => MsgType::ArrayF32X6,
        0 => return Err(anyhow!("empty array")),
        n => return Err(anyhow!("unsupported array length {n}")),
    })
}

pub struct RefMessage<'a> {
    /// Has some number of trailing null values.
    pub addr: &'a str,
    pub t: MsgType,
    pub contents: &'a [u8],
}

impl RefMessage<'_> {
    pub fn into_vec3(&self) -> Result<Vec3> {
        self.t.try_vec3(self.contents, true)
    }

    pub fn into_f32(&self) -> Result<f32> {
        self.t.try_f32(self.contents, true)
    }

    pub fn into_bool(&self) -> Option<bool> {
        self.t.parse_bool(self.contents, true)
    }
}

#[derive(Debug, PartialEq)]
pub enum MsgType {
    BoolTrue,
    BoolFalse,
    F32,
    I32,
    ArrayF32X2,
    ArrayF32X3,
    ArrayF32X4,
    ArrayF32X5,
    ArrayF32X6,
    String,
    Infinitum,
    Nill,
}


#[inline]
fn word(buff: &[u8], off: usize) -> Option<[u8; 4]> {
    buff.get(off..off + 4)?.try_into().ok()
}

impl MsgType {
    pub fn try_vec3(&self, buff: &[u8], _scale_ints: bool) -> Result<Vec3> {
        match self {
            Self::ArrayF32X3 | Self::ArrayF32X4 | Self::ArrayF32X5 | Self::ArrayF32X6 => {
                let x = word(buff, 0).ok_or_else(|| anyhow!("buffer too small for x"))?;
                let y = word(buff, 4).ok_or_else(|| anyhow!("buffer too small for y"))?;
                let z = word(buff, 8).ok_or_else(|| anyhow!("buffer too small for z"))?;
                Ok(Vec3::new(Self::to_f32(x), Self::to_f32(y), Self::to_f32(z)))
            }
            _ => Err(anyhow!("{self:?} isn't coercible into a vector")),
        }
    }

    pub fn try_f32(&self, buff: &[u8], scale_ints: bool) -> Result<f32> {
        // non allocated constants
        match self {
            Self::BoolFalse => return Ok(0.0),
            Self::BoolTrue => return Ok(1.0),
            Self::Nill => return Err(anyhow!("Null type is not f32 coercible")),
            Self::String => return Err(anyhow!("String type isn't coercible")),
            Self::Infinitum => return Err(anyhow!("Infinitum is considered an error value")),
            _ => {},
        }

        let chunk = word(buff, 0)
            .ok_or_else(|| anyhow!("sized types need >= 4 bytes, got {}", buff.len()))?;


        // native f32 types
        match self {
            Self::F32 | Self::ArrayF32X2 | Self::ArrayF32X3
            | Self::ArrayF32X4 | Self::ArrayF32X5 | Self::ArrayF32X6 => Ok(Self::to_f32(chunk)),
            Self::I32 => {
                let val = Self::to_i32(chunk);
                if scale_ints {
                    Ok((val as f32 / 255.0).clamp(0.0, 1.0))
                } else {
                    Ok(val as f32) // unclamped; raw value
                }
            }
            _ => Err(anyhow!("Unhandled message type: {self:?}")),
        }
    }

    pub const fn to_f32(buff: [u8; 4]) -> f32 { f32::from_be_bytes(buff) }
    pub const fn to_i32(buff: [u8; 4]) -> i32 { i32::from_be_bytes(buff) }

    pub fn parse_bool(&self, buff: &[u8], scale_ints: bool) -> Option<bool> {
        match self {
            Self::BoolTrue => Some(true),
            Self::BoolFalse => Some(false),
            Self::F32 | 
            Self::ArrayF32X2 | 
            Self::ArrayF32X3| 
            Self::ArrayF32X4 | 
            Self::ArrayF32X5 | 
            Self::ArrayF32X6 => {
                Some(Self::to_f32(word(buff, 0)?) > 0.5)
            }
            Self::I32 => {
                let val = Self::to_i32(word(buff, 0)?);
                Some(if scale_ints { val > 128 } else { val >= 1 })
            }
            Self::Infinitum | Self::Nill => None,
            Self::String => {
                // alloc-free: exact, ASCII-case-insensitive match (nulls trimmed)
                let s = str::from_utf8(buff).ok()?.trim_end_matches('\0').trim();
                if s.eq_ignore_ascii_case("true") { Some(true) }
                else if s.eq_ignore_ascii_case("false") { Some(false) }
                else { None }
            }
        }
    }

    pub const fn size(&self) -> u16 {
        match self {
            MsgType::BoolFalse | MsgType::BoolTrue | MsgType::Infinitum
            | MsgType::Nill | MsgType::String => 0,
            MsgType::F32 | MsgType::I32 => 4,
            MsgType::ArrayF32X2 => 8,
            MsgType::ArrayF32X3 => 12,
            MsgType::ArrayF32X4 => 16,
            MsgType::ArrayF32X5 => 20,
            MsgType::ArrayF32X6 => 24,
        }
    }
}