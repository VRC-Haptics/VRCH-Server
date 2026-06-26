[use std::ptr::null;

use anyhow::{Result, anyhow};
use memchr::memchr;

pub const BUNDLE_TAG: &'static str = "#bundle";
/// "/" + null + ",F" {no bytes allocated}
pub const MIN_PACKET_BYTES: i32 = 1 + 1 + 2;
/// "#bundle\0" + timetag + 1 -> 4 bytes (i32)
/// 8 + 8 + 1 = 17
pub const BUNDLE_FIRST_SIZE: std::ops::Range<usize> = 17..(17 + 5);
pub const EMPTY: &'static [u8] = &[0; 1];

/// Returns the first singular message of a buffer containing an OSC packet.
pub fn first_message(bytes: &[u8], size: u16) -> Result<RefMessage<'_>> {
    // Size must index into bytes.
    debug_assert!(bytes.len() <= size as usize);

    // check for bundle tag
    if let Some(first) = bytes.get(0..BUNDLE_TAG.len()) {
        if first == BUNDLE_TAG.as_bytes() {
            let content_size = bytes.get(BUNDLE_FIRST_SIZE).unwrap_or(&[0]);
            let (int_bytes, rest) = content_size.split_at(size_of::<u32>());
            debug_assert!(rest.len() != 0);

            // must point to one after the timestamp.
            let contents = (21)..bytes.len()-1;
            let bundle = bytes.get(contents).ok_or(anyhow!("Contents indexed wrong"))?;
            let size = i32::from_be_bytes(int_bytes.try_into()?);
            first_message(bundle, size as u16);
        }
    }

    let mut addr  = "";
    let mut tag_idx = 0;
    for (idx, bytes) in bytes.chunks_exact(4).enumerate() {
        // on null byte
        if *bytes.last().unwrap() == 0 {
            addr = str::from_utf8(bytes.get(0..idx).unwrap())?;
            // skip this null and 
            tag_idx = (idx + 1) * 4;
            break;
        }
    }

    let (osc_type, null_byte) = get_type(bytes, tag_idx)?;

    let len = osc_type.size();
    let contents = if len > 0 {
        let start = null_byte + 1;
        bytes.get(start..(start + len  as usize)).ok_or(anyhow!("Unable to get sized output"))?
    } else {
        &EMPTY
    };

    return Ok(RefMessage { addr, t: osc_type, contents });
}

/// returns type and the null byte index of the tag string
pub fn get_type(bytes: &[u8], start: usize) -> Result<(MsgType, usize)> {
    let char = *bytes.get(start).unwrap() as char;
    match char {
        'f' => Ok((MsgType::F32, start+1)),
        'i' => Ok((MsgType::I32, start+1)),
        's' => Ok((MsgType::String, start+1)),
        'T' => Ok((MsgType::BoolTrue, start+1)),
        'F' => Ok((MsgType::BoolFalse, start+1)),
        'N' => Ok((MsgType::Nill, start+1)),
        'I' => Ok((MsgType::Infinitum, start+1)),
        '[' => array_type(bytes, start+1),
        _ => Err(anyhow!("Unsupported Osc Message type: `{char}`"))
    }
}

pub fn array_type(bytes: &[u8], start: usize) -> Result<(MsgType, usize)> {
    debug_assert!(bytes.len() > start);
    // unwrap should be protected by assert?
    let iter =  bytes.get(start..bytes.len()).ok_or(anyhow!("Unable to get idx"))?.iter();
    let mut current = MsgType::Nill;
    let mut idx = 0;
    for (rel_idx, b) in iter.enumerate() {
        let char = *b as char;
        current = match (current, char) {
            (MsgType::Nill, 'f') => MsgType::F32,
            (MsgType::F32, 'f') => MsgType::ArrayF32X2,
            (MsgType::ArrayF32X2, 'f') => MsgType::ArrayF32X3,
            (MsgType::ArrayF32X3, 'f') => MsgType::ArrayF32X4,
            (MsgType::ArrayF32X4, 'f') => MsgType::ArrayF32X5,
            (MsgType::ArrayF32X5, 'f') => MsgType::ArrayF32X6,
            (MsgType::Nill, ']') => return Err(anyhow!("Expected a type, found closing")),
            (current, ']') => return Ok((current, memchr(b'\0', bytes.get((start + rel_idx)..bytes.len()).ok_or(anyhow!("Unable to get idx"))?).ok_or(anyhow!("Unable to get idx"))?)),
            (current, char) => return Err(anyhow!("Unhandled pairing of array types: {current:?}: {char}")),
        }
    }
    
    if idx == 0 {
        return Err(anyhow!("No type tags"));
    }

    return Ok((current, idx));
}

pub struct RefMessage<'a> {
    /// Has some number of trailing null values.
    pub addr: &'a str,
    pub t: MsgType,
    pub contents: &'a [u8],
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

impl MsgType {
    pub const fn try_f32(&self, buff: &[u8], scale_ints: bool) -> Result<f32> {
        // non allocated constants
        match self {
            Self::BoolFalse => return Ok(0.0),
            Self::BoolTrue => return Ok(1.0),
            Self::Nill => return Err(anyhow!("Null type is not f32 coercible")),
            Self::String => return Err(anyhow!("String type isn't coercible")),
            Self::Infinitum => return Err(anyhow!("Infinitum is considered an error value")),
            _ => {},
        }

        if buff.len() < 4 {
            return Err(anyhow!("Sized types must have at least 4 byte buffers. Len: `{}`", buff.len()));
        }

        let first_chunk = buff.get(0..5)?;

        // native f32 types
        match self {
            Self::F32 |
            Self::ArrayF32X2 |
            Self::ArrayF32X3 |
            Self::ArrayF32X4 |
            Self::ArrayF32X5 |
            Self::ArrayF32X6 => {
                return Ok(Self::to_f32(first_chunk.into()));
            }
            _ => {},
        }

        // converted types
        match self {
            Self::I32 => {
                let val = Self::to_i32(first_chunk.into());
                if use_range {
                    return Ok(f32::clamp(val / 255.0, 0., 1.));
                } else {
                    return Ok(f32::clamp(val as f32, 0., 1.));
                }
            },
            _ => {},
        }

        Err(anyhow!("Unhandled message type: {self}"))
    }

    pub const fn to_f32(buff: [u8; 4]) -> f32 {
        f32::from_be_bytes(buff)
    }

    pub const fn to_i32(buff: [u8; 4]) -> i32 {
        i32::from_be_bytes(buff)
    }

    pub const fn parse_bool(&self, buff: [u8], scale_ints: bool) -> Option<bool> {
        match self {
            Self::BoolTrue => Some(true),
            Self::BoolFalse => Some(false),
            Self::F32 |
            Self::ArrayF32X2 |
            Self::ArrayF32X3 |
            Self::ArrayF32X4 |
            Self::ArrayF32X5 | 
            Self::ArrayF32X6 => {
                if buff.len() >= 4 {
                    return Some(Self::to_f32(buff.get(0..5).unwrap().into()) > 0.5 );
                } else {
                    return None;
                }
            },
            Self::I32 => {
                if buff.len() >= 4 {
                    if scale_ints {
                        return Some(Self::to_i32(buff.get(0..5).unwrap().into()) > 128 );
                    }
                    return Some(Self::to_i32(buff.get(0..5).unwrap().into()) >= 1 );
                } else {
                    return None;
                }
            }
            Self::Infinitum => None,
            Self::Nill => None,
            Self::String => {
                let str = str::from_utf8(*buff)?.to_lowercase();
                if str.contains("true") {
                    Some(true)
                } else if (str.contains("false")) {
                    Some(false)
                } else {
                    None
                }
            },
        }
    }

    pub const fn size(&self) -> u16 {
        match self {
            MsgType::BoolFalse |
            MsgType::BoolTrue  |
            MsgType::Infinitum |
            MsgType::Nill      
                => 0,
            MsgType::F32 |
            MsgType::I32 => 4,
            MsgType::ArrayF32X2 => 8,
            MsgType::ArrayF32X3 => 12,
            MsgType::ArrayF32X4 => 16,
            MsgType::ArrayF32X5 => 20,
            MsgType::ArrayF32X6 => 24,
            MsgType::String => 0,
        }
    }
}