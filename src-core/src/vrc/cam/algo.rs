use bytemuck::{Pod, Zeroable};
use std::mem::size_of;
use std::ops::{Deref, DerefMut};

use crate::api::CamConfig;
use crate::vrc::cam::center::{total_bits, CodeReader};
use crate::vrc::cam::grid::{detect, DetectError, Frame, GridLayout};

/// Assumed to be brighter than this value.
pub const MIN_PEAK: u32 = 24;

pub const CANDIDATE_ORDER: [i32; 5] = [0, 1, -1, 2, -2];

pub struct Decoder {
    pub w: u32,
    pub h: u32,
    pub img: Img,
    pub use_rgb: bool,
    pub schema: CamConfig,
    pub grid_layout: Option<GridLayout>,
    total_bits: u32,
    reader: Option<CodeReader>,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Bgr {
    pub b: u8,
    pub g: u8,
    pub r: u8,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Bgra {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

pub struct Img(pub Vec<u8>);

impl Img {
    pub fn pixels<P: Pod>(&self) -> &[P] {
        let n = self.0.len() - self.0.len() % size_of::<P>();
        bytemuck::cast_slice(&self.0[..n])
    }

    pub fn pixels_mut<P: Pod>(&mut self) -> &mut [P] {
        let n = self.0.len() - self.0.len() % size_of::<P>();
        bytemuck::cast_slice_mut(&mut self.0[..n])
    }

    /// P must be how the frame was captured.
    pub fn frame<P: Pixel>(&self, w: u32, h: u32) -> Option<Frame<'_, P>> {
        Frame::packed(self.pixels::<P>(), w as usize, h as usize)
    }
}

impl Deref for Img {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Img {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// never care about `A` channel so a trait that downcasts is acceptable.
pub trait Pixel: Pod {
    fn rgb(&self) -> (u8, u8, u8);
}

impl Pixel for Bgr {
    fn rgb(&self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }
}

impl Pixel for Bgra {
    fn rgb(&self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }
}

impl Decoder {
    pub fn new(conf: CamConfig) -> Self {
        Self {
            w: 0,
            h: 0,
            img: Img(Vec::new()),
            use_rgb: true,
            total_bits: total_bits(&conf),
            schema: conf,
            grid_layout: None,
            reader: None,
        }
    }

    pub fn detect_grid(&self, total_bits: u32) -> Result<GridLayout, DetectError> {
        if self.use_rgb {
            let f = self
                .img
                .frame::<Bgr>(self.w, self.h)
                .ok_or(DetectError::BadFrame)?;
            detect(&f, total_bits)
        } else {
            let f = self
                .img
                .frame::<Bgra>(self.w, self.h)
                .ok_or(DetectError::BadFrame)?;
            detect(&f, total_bits)
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        let px = (w as usize) * (h as usize);
        let bytes = px
            * if self.use_rgb {
                size_of::<Bgr>()
            } else {
                size_of::<Bgra>()
            };
        self.img.0.clear();
        self.img.0.resize(bytes, 0);
        self.invalidate();
    }

    pub fn invalidate(&mut self) {
        self.grid_layout = None;
        self.reader = None;
    }

    pub fn reader(&self) -> Option<&CodeReader> {
        self.reader.as_ref()
    }

    pub fn process_frame(&mut self, out: &mut [CamValue]) -> bool {
        if self.use_rgb {
            self.run::<Bgr>(out)
        } else {
            self.run::<Bgra>(out)
        }
    }

    fn run<P: Pixel>(&mut self, out: &mut [CamValue]) -> bool {
        let Decoder {
            w,
            h,
            img,
            schema,
            grid_layout,
            reader,
            total_bits,
            ..
        } = self;

        let Some(frame) = img.frame::<P>(*w, *h) else {
            log::error!("Cam frame does not hold a {}x{} image", *w, *h);
            return false;
        };

        // Detect once. Your caller controls the retry rate.
        if reader.is_none() {
            let layout = match detect(&frame, *total_bits) {
                Ok(v) => v,
                Err(e) => {
                    // Most frames are game screenshots and hold no code.
                    log::trace!("Cam grid not found: {e}");
                    return false;
                }
            };

            let Some(r) = CodeReader::new(schema, *total_bits, layout) else {
                log::error!("Cam layout does not fit the schema: {layout:?}");
                return false;
            };

            log::debug!("Grid detected: {layout:?}");
            *grid_layout = Some(layout);
            *reader = Some(r);
        }

        let r = reader.as_mut().unwrap();

        let Some((band, stride)) = frame.band(r.band_x, r.band_y, r.band_width, r.band_height)
        else {
            // The frame size changed without a call to resize.
            log::error!("Cam band left the frame");
            return false;
        };

        r.read(band, stride, out)
    }
}

#[derive(Clone, Copy)]
pub enum CamValue {
    Float(f32),
    Int(i32),
    Uint(u32),
    Bool(bool),
}

impl CamValue {
    pub fn into_f32(self) -> Option<f32> {
        match self {
            CamValue::Float(f) => Some(f),
            CamValue::Bool(b) => Some(b as u32 as f32),
            CamValue::Int(i) => Some(i as f32),
            CamValue::Uint(u) => Some(u as f32),
        }
    }

    pub fn into_bool(self) -> bool {
        match self {
            CamValue::Bool(b) => b,
            CamValue::Float(f) => f > 0.5,
            CamValue::Int(i) => i > 0,
            CamValue::Uint(u) => u > 0,
        }
    }
}
