//! Reads one frame of the Custom/DataReference code into `[CamValue]`.
//!
//! Everything is built once, when the grid layout is first detected. A decoding
//! frame then does: one `fill(0)` over `total_bits / 64` words, two block
//! samples per bit, one `extract` + `decode_number` per field, and no
//! allocation, no copy, and no clone at all.
//!
//! Two things the C# does that are not worth their cost here:
//!   * No sample-offset table. The sample points are a 2-D arithmetic
//!     progression, so the loop walks it with adds. That removes
//!     `(total_bits * 2 + 2) * 4` bytes of table traffic per frame and the
//!     rebuild pass that went with it.
//!   * No per-bit confidence array. `out` has no confidence slot, so only the
//!     frame aggregates are kept, and they are accumulated as integers —
//!     the bit loop does no floating-point work at all.

use std::fmt;

use crate::api::{CamConfig, CamField};
use crate::vrc::cam::algo::Pixel;
use crate::vrc::cam::grid::{luma, GridLayout, DATA_BORDER, MARKER_BLOCKS, MARKER_CENTER};
use crate::vrc::cam::CamValue;

/// A frame whose calibration blocks are closer together than this is not
/// showing the code.
const MIN_CONTRAST: i32 = 12;

// ---------------------------------------------------------------------------
// Field plan
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ClampedFloat,
    Int,
    UInt,
    Bool,
}

#[derive(Clone, Copy, Debug)]
struct FieldPlan {
    offset: u32,
    bits: u32,
    kind: Kind,
}

/// The only place that knows what `CamField` looks like.
///
/// Two assumptions, both from `CamConfig`: fields are packed in declaration
/// order with no gaps (which is what "0 means the sum of the field widths"
/// implies), and `CamField` carries its width and its kind. If `CamField`
/// already stores an explicit offset, drop the running total and read it.
fn plan_fields(schema: &CamConfig) -> Box<[FieldPlan]> {
    let mut offset = 0u32;
    schema
        .fields
        .iter()
        .map(|f| {
            let plan = FieldPlan {
                offset,
                bits: f.bits,
                kind: kind_of(f),
            };
            offset += f.bits;
            plan
        })
        .collect()
}

fn kind_of(_field: &CamField) -> Kind {
    // Map your `CamField::kind` variants onto these four.
    Kind::ClampedFloat
}

/// Width of the code. `CamConfig::total_bits` of 0 means "sum of the fields",
/// so this is what must be handed to `detect`, not the raw field.
pub fn total_bits(schema: &CamConfig) -> u32 {
    if schema.total_bits != 0 {
        schema.total_bits
    } else {
        schema.fields.iter().map(|f| f.bits).sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// `out` is shorter than the schema. A caller bug — the layout is fine and
    /// re-detecting will not help.
    OutTooShort { need: usize, got: usize },
    /// The sample points do not fit inside the band. The layout is wrong.
    BandTooSmall,
    /// The calibration blocks are closer than `MIN_CONTRAST`. Either the sender
    /// stopped painting the code, or the layout is stale. A negative gap means
    /// the two strip blocks are swapped, i.e. the flip is wrong.
    NoContrast { high: i32, low: i32 },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ReadError::OutTooShort { need, got } => {
                write!(f, "out holds {got} values, the schema needs {need}")
            }
            ReadError::BandTooSmall => write!(f, "the sample points fall outside the band"),
            ReadError::NoContrast { high, low } => {
                write!(
                    f,
                    "calibration gap is {} (high {high}, low {low})",
                    high - low
                )
            }
        }
    }
}

impl std::error::Error for ReadError {}

/// One line of geometry, for the log after a detection.
impl fmt::Display for CodeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "band {}x{} at ({},{}), {} pairs x {} rows, {} bits, r{}, first {:?}, high {:?}, low {:?}",
            self.band_width, self.band_height, self.band_x, self.band_y,
            self.pairs, self.data_rows, self.total_bits, self.sample_radius,
            self.first, self.high, self.low
        )
    }
}

pub struct CodeReader {
    plan: Box<[FieldPlan]>,
    total_bits: usize,
    pairs: usize,
    data_rows: usize,
    bits: Box<[u64]>,

    // Sample geometry, band-relative pixels. The deltas already carry the
    // flip signs, so the read loop never branches on orientation.
    first: (i32, i32),
    high: (i32, i32),
    low: (i32, i32),
    sub_dx: i32,  // sample a -> sample b of the same bit
    pair_dx: i32, // bit -> next bit in the row
    row_dy: i32,  // data row -> next data row

    // Resolved against the current stride.
    cached_stride: usize,
    fits: bool,
    base: isize,
    sub: isize,
    pair: isize,
    row: isize,
    high_off: isize,
    low_off: isize,

    sample_radius: i32,
    sample_count: i32,

    pub band_x: i32,
    pub band_y: i32,
    pub band_width: i32,
    pub band_height: i32,
    pub layout: GridLayout,

    // Last frame, all scalars.
    pub contrast: i32,
    pub blank_bits: u32,
    pub min_confidence: f32,
    pub mean_confidence: f32,
    pub is_id_frame: bool,
    pub id_bits: u32,
    /// Only touched on an ID frame. The buffer is reused.
    pub id_text: String,
}

impl CodeReader {
    pub fn new(schema: &CamConfig, total_bits: u32, layout: GridLayout) -> Option<Self> {
        let total_bits = total_bits as usize;
        let pairs = layout.pairs();
        if pairs <= 0 || total_bits == 0 || schema.fields.is_empty() {
            return None;
        }
        let pairs = pairs as usize;
        let data_rows = total_bits.div_ceil(pairs);
        let border = DATA_BORDER;

        // The band covers the data rows plus the marker centre row above them.
        // That row carries the calibration strip, which gives one known high
        // block and one known low block inside the same copy.
        let min_block_y = MARKER_BLOCKS;
        let max_block_y = border + data_rows as i32 - 1;
        let min_block_x = MARKER_CENTER;
        let max_block_x = border + pairs as i32 * 2 - 1;
        if max_block_x >= layout.cols || max_block_y >= layout.rows {
            return None;
        }

        let (ix_min, ix_max) = if layout.flip_x {
            (layout.cols - 1 - max_block_x, layout.cols - 1 - min_block_x)
        } else {
            (min_block_x, max_block_x)
        };
        let (iy_min, iy_max) = if layout.flip_y {
            (layout.rows - 1 - max_block_y, layout.rows - 1 - min_block_y)
        } else {
            (min_block_y, max_block_y)
        };

        let band_x = layout.origin_x + ix_min * layout.block_px;
        let band_y = layout.origin_y + iy_min * layout.block_px;
        let band_width = (ix_max - ix_min + 1) * layout.block_px;
        let band_height = (iy_max - iy_min + 1) * layout.block_px;

        let rel = |bx: i32, by: i32| {
            let (x, y) = layout.block_center(bx, by);
            (x - band_x, y - band_y)
        };

        // `block_center` mirrors the index under a flip, so one step in block
        // index is one negative step in pixels.
        let block = layout.block_px;
        let sub_dx = if layout.flip_x { -block } else { block };
        let row_dy = if layout.flip_y { -block } else { block };

        let sample_radius = layout.sample_radius();
        let side = sample_radius * 2 + 1;

        Some(Self {
            plan: plan_fields(schema),
            total_bits,
            pairs,
            data_rows,
            // One spare word so `extract` can always read `word + 1`.
            bits: vec![0u64; total_bits.div_ceil(64) + 1].into_boxed_slice(),

            first: rel(border, border),
            // Strip block 4 is high and strip block 3 is low: the first strip
            // block after the ring is low.
            high: rel(MARKER_CENTER, border),
            low: rel(MARKER_CENTER, MARKER_BLOCKS),
            sub_dx,
            pair_dx: sub_dx * 2,
            row_dy,

            cached_stride: usize::MAX,
            fits: false,
            base: 0,
            sub: 0,
            pair: 0,
            row: 0,
            high_off: 0,
            low_off: 0,

            sample_radius,
            sample_count: side * side,

            band_x,
            band_y,
            band_width,
            band_height,
            layout,

            contrast: 0,
            blank_bits: 0,
            min_confidence: 0.0,
            mean_confidence: 0.0,
            is_id_frame: false,
            id_bits: 0,
            id_text: String::new(),
        })
    }

    #[inline]
    pub fn field_count(&self) -> usize {
        self.plan.len()
    }

    #[inline]
    pub fn raw_bits(&self) -> &[u64] {
        &self.bits
    }

    /// Reads one band and writes every field into `out`.
    ///
    /// Returns true only when this call writes all fields into `out`. A false
    /// result means the frame holds no values for you. The layout stays valid.
    pub fn read<P: Pixel>(&mut self, band: &[P], stride: usize, out: &mut [CamValue]) -> bool {
        if out.len() < self.plan.len() {
            return false;
        }
        if stride != self.cached_stride {
            self.cached_stride = stride;
            self.resolve(band.len(), stride);
        }
        if !self.fits {
            return false;
        }

        let high = self.sample(band, self.high_off);
        let low = self.sample(band, self.low_off);
        let contrast = high - low;
        if contrast < MIN_CONTRAST {
            // A game screenshot has no calibration blocks. This is the normal
            // result for most frames. It is not an error, and the layout is
            // still good.
            return false;
        }

        let blank_limit = contrast / 4;
        self.bits.fill(0);

        // Integer-only accumulators: the confidences are derived once, after.
        let mut min_mag = i32::MAX;
        let mut sum_mag = 0u64;
        let mut blanks = 0u32;
        let mut first_blank = -1i32;

        let mut bit = 0usize;
        let mut row_off = self.base;

        'rows: for _ in 0..self.data_rows {
            let mut off = row_off;

            for _ in 0..self.pairs {
                if bit == self.total_bits {
                    break 'rows;
                }

                let a = self.sample(band, off);
                let b = self.sample(band, off + self.sub);
                let delta = a - b;
                let magnitude = delta.abs();

                // High then low is a one. Low then high is a zero.
                if delta > 0 {
                    self.bits[bit >> 6] |= 1u64 << (bit & 63);
                }

                if magnitude < min_mag {
                    min_mag = magnitude;
                }
                sum_mag += magnitude.min(contrast) as u64;

                if magnitude < blank_limit {
                    blanks += 1;
                    if first_blank < 0 {
                        first_blank = bit as i32;
                    }
                }

                off += self.pair;
                bit += 1;
            }

            row_off += self.row;
        }

        let scale = 1.0 / contrast as f32;
        self.contrast = contrast;
        self.blank_bits = blanks;
        self.min_confidence = (min_mag.min(contrast) as f32 * scale).min(1.0);
        self.mean_confidence = sum_mag as f32 * scale / self.total_bits as f32;
        self.is_id_frame = false;
        self.id_bits = 0;

        // The shader can paint the ID string in place of the values. It paints
        // whole bytes and leaves the rest of the area black. This call writes
        // nothing into `out`, so it returns false. Read `id_text` for the
        // string.
        if blanks > 0
            && first_blank > 0
            && first_blank & 7 == 0
            && blanks as usize == self.total_bits - first_blank as usize
        {
            self.is_id_frame = true;
            self.id_bits = first_blank as u32;
            self.build_id(first_blank as usize / 8);
            return false;
        }

        for (slot, field) in out.iter_mut().zip(self.plan.iter()) {
            let raw = extract(&self.bits, field.offset as usize, field.bits as usize);
            *slot = decode_value(raw, field.bits, field.kind);
        }

        true
    }

    /// Turns the band-relative points into pixel offsets and checks that every
    /// sample square lands inside the band. Runs once per stride change, so the
    /// read loop needs no bounds logic of its own.
    fn resolve(&mut self, band_len: usize, stride: usize) {
        self.fits = false;

        let r = self.sample_radius;
        let last_x = self.first.0 + self.pair_dx * (self.pairs as i32 - 1) + self.sub_dx;
        let last_y = self.first.1 + self.row_dy * (self.data_rows as i32 - 1);

        let xs = [self.first.0, last_x, self.high.0, self.low.0];
        let ys = [self.first.1, last_y, self.high.1, self.low.1];

        for &x in &xs {
            if x - r < 0 || x + r >= self.band_width {
                return;
            }
        }
        for &y in &ys {
            if y - r < 0 || y + r >= self.band_height {
                return;
            }
        }
        if band_len < (self.band_height as usize - 1) * stride + self.band_width as usize {
            return;
        }

        let at = |p: (i32, i32)| p.1 as isize * stride as isize + p.0 as isize;
        self.base = at(self.first);
        self.high_off = at(self.high);
        self.low_off = at(self.low);
        self.sub = self.sub_dx as isize;
        self.pair = self.pair_dx as isize;
        self.row = self.row_dy as isize * stride as isize;
        self.fits = true;
    }

    /// `paintId` writes the high bit of each byte first. Trims trailing spaces.
    fn build_id(&mut self, chars: usize) {
        let Self { bits, id_text, .. } = self;
        id_text.clear();
        id_text.reserve(chars);

        for i in 0..chars {
            let mut value = 0u32;
            for b in 0..8 {
                let index = i * 8 + b;
                value = (value << 1) | ((bits[index >> 6] >> (index & 63)) & 1) as u32;
            }
            id_text.push(if value == 0 { ' ' } else { value as u8 as char });
        }

        let end = id_text.trim_end().len();
        id_text.truncate(end);
    }

    /// Averages a small square at the centre of a block. The square stays away
    /// from the block edge, where bloom from the neighbour block lands.
    #[inline(always)]
    fn sample<P: Pixel>(&self, band: &[P], offset: isize) -> i32 {
        let start = offset as usize;
        if self.sample_radius == 0 {
            return luma(&band[start]) as i32;
        }

        let stride = self.cached_stride;
        let radius = self.sample_radius as usize;
        let side = radius * 2 + 1;
        let start = start - radius * stride - radius;

        let mut sum = 0u32;
        for y in 0..side {
            let row = start + y * stride;
            for p in &band[row..row + side] {
                sum += luma(p);
            }
        }

        sum as i32 / self.sample_count
    }
}

/// The shader packs each field with the low bit first.
#[inline]
fn extract(words: &[u64], offset: usize, count: usize) -> u32 {
    debug_assert!(count > 0 && count <= 32);
    let word = offset >> 6;
    let shift = offset & 63;
    let mut value = words[word] >> shift;
    // `count <= 32`, so this branch implies `shift > 32` and the shift below
    // cannot reach 64.
    if shift + count > 64 {
        value |= words[word + 1] << (64 - shift);
    }
    if count >= 32 {
        value as u32
    } else {
        (value & ((1u64 << count) - 1)) as u32
    }
}

/// Reverses `EncodeParam` from the shader.
#[inline]
pub fn decode_value(raw: u32, bits: u32, kind: Kind) -> CamValue {
    match kind {
        Kind::ClampedFloat => {
            // The shader caps the float scale at 24 bits so the product stays
            // exact. Longer fields repeat the top bits, so the top 24 bits
            // carry the value.
            let safe_len = bits.min(24);
            let code = raw >> (bits - safe_len);
            let mask = if safe_len >= 32 {
                u32::MAX
            } else {
                (1u32 << safe_len) - 1
            };
            CamValue::Float(if mask == 0 {
                0.0
            } else {
                code as f32 / mask as f32
            })
        }
        Kind::Int => {
            let shift = 32u32.saturating_sub(bits);
            CamValue::Int(((raw << shift) as i32) >> shift)
        }
        Kind::UInt => CamValue::Uint(raw),
        Kind::Bool => CamValue::Bool(raw & 1 != 0),
    }
}
