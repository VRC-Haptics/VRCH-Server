//! Raw bindings for `sidecars/datarefdecode.dll`.
//!
//! The layout of every struct comes from `NATIVE_API.md`, ABI version 1. The
//! module holds no state other than the loaded library. All calls stay unsafe.
//! The safe wrapper lives in [`super`].
//!
//! Rules that the caller must keep (see the ABI document):
//!
//! 1. Read the frame inside the callback. The pointers die at the return.
//! 2. Do not free a pointer in the frame. The library owns that memory.
//! 3. Do not let a panic cross the callback.
//! 4. Use one handle from one thread.
//! 5. Call `drd_close` one time per handle.

#![allow(dead_code)]

use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;
use std::sync::OnceLock;

use libloading::Library;

/// The ABI version that this module reads.
pub const DRD_ABI_VERSION: u32 = 1;

/// The default path of the decoder library.
pub const DRD_LIBRARY_PATH: &str = "./sidecars/datarefdecode.dll";

// Field kind codes. `DrdField::kind` and `DrdValue::kind` share them.
pub const DRD_KIND_FLOAT: u32 = 0;
pub const DRD_KIND_INT: u32 = 1;
pub const DRD_KIND_UINT: u32 = 2;
pub const DRD_KIND_BOOL: u32 = 3;

/// `DrdConfig::flags` bit 0. The library calls back only on a bit change.
pub const DRD_CONFIG_ON_CHANGE: u32 = 1 << 0;

/// `DrdFrame::flags` bit 0. The frame carries an ID string, not field data.
pub const DRD_FRAME_ID: u32 = 1 << 0;
/// `DrdFrame::flags` bit 1. The bits match the last frame.
pub const DRD_FRAME_SAME: u32 = 1 << 1;

/// The status code that every entry point returns.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrdStatusCode {
    /// The callback ran.
    Ok,
    /// No sender, no grid, or no contrast. Sleep and call again.
    Waiting,
    /// The frame decoded. A gate dropped it.
    Skipped,
    /// A pointer or a size is out of range.
    InvalidArgument,
    /// The config failed the schema rules.
    BadConfig,
    /// The handle is dead.
    ClosedHandle,
    /// Direct3D or the sender failed.
    DeviceError,
    /// The library caught an exception.
    Internal,
    /// The library returned a code that this build does not know.
    Unknown(i32),
}

impl DrdStatusCode {
    #[inline]
    pub fn from_raw(code: i32) -> Self {
        match code {
            0 => Self::Ok,
            1 => Self::Waiting,
            2 => Self::Skipped,
            -1 => Self::InvalidArgument,
            -2 => Self::BadConfig,
            -3 => Self::ClosedHandle,
            -4 => Self::DeviceError,
            -5 => Self::Internal,
            other => Self::Unknown(other),
        }
    }

    /// True when the code means a failure, not a frame outcome.
    #[inline]
    pub fn is_error(self) -> bool {
        !matches!(self, Self::Ok | Self::Waiting | Self::Skipped)
    }

    /// True when the handle cannot serve more frames.
    #[inline]
    pub fn is_fatal(self) -> bool {
        matches!(
            self,
            Self::ClosedHandle | Self::DeviceError | Self::Internal | Self::BadConfig
        )
    }
}

/// One field of the packed code. The order gives the bit offsets.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DrdField {
    pub bits: u32,
    pub kind: u32,
}

/// The open time configuration. The library copies the data at `drd_open`.
#[repr(C)]
#[derive(Debug)]
pub struct DrdConfig {
    /// Always 1 for this ABI.
    pub version: u32,
    /// 0 means the sum of the field bits.
    pub total_bits: u32,
    pub fields: *const DrdField,
    pub field_count: u32,
    pub min_confidence: f32,
    /// UTF-16, null for the active sender.
    pub sender_name: *const u16,
    /// Code units, not bytes.
    pub sender_name_len: u32,
    /// Bit 0: call back only on a change.
    pub flags: u32,
}

/// One decoded field.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DrdValue {
    /// 0 float, 1 int, 2 uint, 3 bool.
    pub kind: u32,
    /// The packed bits of the field.
    pub raw: u32,
    /// The decoded value.
    pub number: f64,
    pub confidence: f32,
    pub _pad: u32,
}

/// One decoded frame. The library owns every pointer. They die at the return
/// of the callback.
#[repr(C)]
#[derive(Debug)]
pub struct DrdFrame {
    pub frame_index: u64,
    /// `value_count` entries, in config order.
    pub values: *const DrdValue,
    pub value_count: u32,
    /// Bit 0 ID frame, bit 1 bits match the last frame.
    pub flags: u32,
    pub min_confidence: f32,
    pub mean_confidence: f32,
    pub contrast: i32,
    pub blank_bits: i32,
    /// ASCII, valid when bit 0 of `flags` is set.
    pub id_text: *const u8,
    pub id_len: u32,
    pub reserved: u32,
    /// The raw bits, low bit of a field first.
    pub bit_words: *const u64,
    pub bit_word_count: u32,
    pub total_bits: u32,
    pub decode_nanos: u64,
}

/// The frame callback. The library calls it inside `drd_poll`.
pub type DrdFrameFn = unsafe extern "C" fn(user: *mut c_void, frame: *const DrdFrame);

/// The state of one handle. `drd_status` fills it.
///
/// The library reports a flag as an `int`, not a bool, because the ABI carries
/// blittable types only.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct DrdStatus {
    /// Non zero when the sender exists.
    pub connected: i32,
    /// Non zero when the detector found the grid.
    pub detected: i32,
    pub frame_width: i32,
    pub frame_height: i32,
    pub band_x: i32,
    pub band_y: i32,
    pub band_width: i32,
    pub band_height: i32,
    /// The side of one cell in pixels.
    pub block_px: i32,
    pub cols: i32,
    pub rows: i32,
    pub flip_x: i32,
    pub flip_y: i32,
    pub reserved: u32,
    pub frames: u64,
}

// The document states the size of each struct. A wrong size means a wrong
// build of the library or a wrong target. Fail the compile instead.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<DrdField>() == 8);
    assert!(size_of::<DrdConfig>() == 40);
    assert!(size_of::<DrdValue>() == 24);
    assert!(size_of::<DrdFrame>() == 80);
    assert!(size_of::<DrdStatus>() == 64);
};

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type OpenFn =
    unsafe extern "C" fn(*const DrdConfig, DrdFrameFn, *mut c_void, *mut isize) -> i32;
type PollFn = unsafe extern "C" fn(isize) -> i32;
type DecodeBgraFn = unsafe extern "C" fn(isize, *const u8, i32, i32, i32) -> i32;
type ResetFn = unsafe extern "C" fn(isize) -> i32;
type StatusFn = unsafe extern "C" fn(isize, *mut DrdStatus) -> i32;
type LastErrorFn = unsafe extern "C" fn(isize, *mut u8, i32) -> i32;
type CloseFn = unsafe extern "C" fn(isize);

/// The resolved entry points of the decoder library.
pub struct Drd {
    /// The library stays loaded for the life of the process.
    _lib: &'static Library,
    pub abi_version: AbiVersionFn,
    pub open: OpenFn,
    pub poll: PollFn,
    pub decode_bgra: DecodeBgraFn,
    pub reset: ResetFn,
    pub status: StatusFn,
    pub last_error: LastErrorFn,
    pub close: CloseFn,
}

// Every entry point is a plain function pointer into a library that never
// unloads. The struct holds no interior state.
unsafe impl Send for Drd {}
unsafe impl Sync for Drd {}

static LIBRARY: OnceLock<Result<Drd, String>> = OnceLock::new();

/// Loads the decoder library one time and returns the entry points.
///
/// The first call resolves the symbols and checks the ABI version. Later calls
/// return the same result. A failed load stays failed for the process.
pub fn library(path: &Path) -> Result<&'static Drd, &'static str> {
    let slot = LIBRARY.get_or_init(|| unsafe { Drd::load(path) });
    match slot {
        Ok(drd) => Ok(drd),
        Err(message) => Err(message.as_str()),
    }
}

impl Drd {
    /// # Safety
    ///
    /// The path must point at a build of `datarefdecode.dll` that matches
    /// `NATIVE_API.md`. The loader runs the DLL entry point of that file.
    unsafe fn load(path: &Path) -> Result<Drd, String> {
        let lib = unsafe { Library::new(path) }
            .map_err(|err| format!("unable to load {}: {}", path.display(), err))?;
        // The symbols must outlive the borrow of the library. The library
        // stays for the life of the process, so leak it once.
        let lib: &'static Library = Box::leak(Box::new(lib));

        let drd = Drd {
            _lib: lib,
            abi_version: unsafe { symbol(lib, b"drd_abi_version\0")? },
            open: unsafe { symbol(lib, b"drd_open\0")? },
            poll: unsafe { symbol(lib, b"drd_poll\0")? },
            decode_bgra: unsafe { symbol(lib, b"drd_decode_bgra\0")? },
            reset: unsafe { symbol(lib, b"drd_reset\0")? },
            status: unsafe { symbol(lib, b"drd_status\0")? },
            last_error: unsafe { symbol(lib, b"drd_last_error\0")? },
            close: unsafe { symbol(lib, b"drd_close\0")? },
        };

        let found = unsafe { (drd.abi_version)() };
        if found != DRD_ABI_VERSION {
            return Err(format!(
                "datarefdecode ABI {} does not match the expected {}",
                found, DRD_ABI_VERSION
            ));
        }

        Ok(drd)
    }

    /// Reads the state of one handle.
    ///
    /// # Safety
    ///
    /// The handle must come from `drd_open` and must stay open.
    pub unsafe fn read_status(&self, handle: isize) -> Result<DrdStatus, DrdStatusCode> {
        let mut status = DrdStatus::default();
        let code = unsafe { (self.status)(handle, &mut status as *mut DrdStatus) };
        match DrdStatusCode::from_raw(code) {
            DrdStatusCode::Ok => Ok(status),
            other => Err(other),
        }
    }

    /// Reads the last error of a handle into `scratch` and returns it as text.
    ///
    /// The call allocates nothing. A handle without an error returns an empty
    /// string. A zero handle returns the error of the last failed `drd_open`,
    /// so a failed open must still ask with the handle that it got.
    ///
    /// # Safety
    ///
    /// The handle must come from `drd_open` and must stay open.
    pub unsafe fn error_text<'a>(&self, handle: isize, scratch: &'a mut [u8]) -> &'a str {
        if scratch.is_empty() {
            return "";
        }
        let written =
            unsafe { (self.last_error)(handle, scratch.as_mut_ptr(), scratch.len() as i32) };
        if written <= 0 {
            return "";
        }
        let end = (written as usize).min(scratch.len());
        std::str::from_utf8(&scratch[..end]).unwrap_or("<invalid utf-8>")
    }
}

/// # Safety
///
/// `name` must name a symbol with the type `T`.
unsafe fn symbol<T: Copy>(lib: &'static Library, name: &[u8]) -> Result<T, String> {
    let symbol = unsafe { lib.get::<T>(name) }.map_err(|err| {
        format!(
            "missing symbol {}: {}",
            String::from_utf8_lossy(&name[..name.len() - 1]),
            err
        )
    })?;
    Ok(*symbol)
}