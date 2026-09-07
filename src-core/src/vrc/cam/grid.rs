use std::fmt;

use crate::vrc::cam::algo::Pixel;

// ---------------------------------------------------------------------------
// Layout constants. These must mirror the shader / your C# `GridConstants`.
// They were not in the pasted file, so these are the values implied by its
// arithmetic — check them before you trust a decode.
// ---------------------------------------------------------------------------

/// Width of a marker ring, in blocks.
pub const MARKER_BLOCKS: i32 = 3;
/// Block index of the marker centre line (the strips run along it).
pub const MARKER_CENTER: i32 = 1;
/// Blocks reserved on each side before data starts (marker + one quiet block).
pub const DATA_BORDER: i32 = 4;

/// A frame darker than this is treated as "shader off".
pub const MIN_PEAK: u32 = 24;

const CANDIDATE_ORDER: [i32; 5] = [0, 1, -1, 2, -2];

// ---------------------------------------------------------------------------
// Pixel access
// ---------------------------------------------------------------------------

/// Luma of one pixel. The shader writes gray, so the weights only have to
/// reject coloured bloom. Green is the middle channel in both orders, so the
/// result does not depend on the channel order.
#[inline(always)]
pub fn luma<P: Pixel>(p: &P) -> u32 {
    let (r, g, b) = p.rgb();
    (b as u32 + 2 * g as u32 + r as u32) >> 2
}

/// A borrowed, typed view of one captured frame.
#[derive(Clone, Copy)]
pub struct Frame<'a, P: Pixel> {
    px: &'a [P],
    stride: usize, // in pixels, not bytes
    width: usize,
    height: usize,
}

impl<'a, P: Pixel> Frame<'a, P> {
    pub fn new(px: &'a [P], stride: usize, width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 || width > stride {
            return None;
        }
        // Last row only needs `width` pixels, not a whole stride.
        let needed = stride.checked_mul(height - 1)?.checked_add(width)?;
        if px.len() < needed {
            return None;
        }
        Some(Self {
            px,
            stride,
            width,
            height,
        })
    }

    pub fn band(&self, x: i32, y: i32, w: i32, h: i32) -> Option<(&'a [P], usize)> {
        if x < 0 || y < 0 || w <= 0 || h <= 0 {
            return None;
        }
        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
        if x + w > self.width || y + h > self.height {
            return None;
        }
        let start = y * self.stride + x;
        let len = (h - 1) * self.stride + w;
        Some((&self.px[start..start + len], self.stride))
    }

    #[inline]
    pub fn packed(px: &'a [P], width: usize, height: usize) -> Option<Self> {
        Self::new(px, width, width, height)
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }
    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }
    /// Row pitch, in pixels.
    #[inline]
    pub fn stride(&self) -> usize {
        self.stride
    }
    #[inline]
    pub fn as_slice(&self) -> &'a [P] {
        self.px
    }

    #[inline]
    pub fn row(&self, y: usize) -> &'a [P] {
        let o = y * self.stride;
        &self.px[o..o + self.width]
    }

    #[inline]
    fn luma_at(&self, x: usize, y: usize) -> u32 {
        luma(&self.px[y * self.stride + x])
    }

    /// Returns `None` outside the frame. Used by every candidate test so a bad
    /// block-size guess cannot read out of bounds.
    #[inline]
    fn bright_checked(&self, x: i32, y: i32, threshold: u32) -> Option<bool> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        Some(self.luma_at(x as usize, y as usize) > threshold)
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridLayout {
    pub origin_x: i32,
    pub origin_y: i32,
    pub block_px: i32,
    pub cols: i32,
    pub rows: i32,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl GridLayout {
    pub fn sample_radius(&self) -> i32 {
        ((self.block_px - 1) / 2 - 1).clamp(0, 2)
    }

    /// Bit pairs per data row. Mirror your C# `GridLayout.Pairs` here — this is
    /// the one value I had to infer, and every `total_bits` check depends on it.
    pub fn pairs(&self) -> i32 {
        (self.cols - DATA_BORDER * 2) / 2
    }

    /// Centre pixel of a block, in grid coordinates (flips applied).
    pub fn block_center(&self, ix: i32, iy: i32) -> (i32, i32) {
        let bx = if self.flip_x { self.cols - 1 - ix } else { ix };
        let by = if self.flip_y { self.rows - 1 - iy } else { iy };
        let half = self.block_px / 2;
        (
            self.origin_x + bx * self.block_px + half,
            self.origin_y + by * self.block_px + half,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectError {
    /// Buffer smaller than `w * h`, or a zero dimension.
    BadFrame,
    Dark {
        peak: u32,
    },
    NoBrightPixels,
    ShortMarkerRun {
        run: u32,
    },
    NoBlockSize {
        span_x: u32,
        span_y: u32,
        run: u32,
    },
}

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DetectError::BadFrame => write!(f, "The frame buffer does not match the frame size."),
            DetectError::Dark { peak } => write!(
                f,
                "The frame is dark (peak luma {peak}). Turn the shader on and check that the sender shows the code."
            ),
            DetectError::NoBrightPixels => write!(f, "The frame holds no bright pixels."),
            DetectError::ShortMarkerRun { run } => {
                write!(f, "The marker run is {run} pixels. The detector needs at least 3.")
            }
            DetectError::NoBlockSize { span_x, span_y, run } => write!(
                f,
                "No block size fits a {span_x}x{span_y} pixel code with a {run} pixel marker run."
            ),
        }
    }
}

impl std::error::Error for DetectError {}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

pub fn detect<P: Pixel>(f: &Frame<'_, P>, total_bits: u32) -> Result<GridLayout, DetectError> {
    let peak = peak_luma(f);
    if peak < MIN_PEAK {
        return Err(DetectError::Dark { peak });
    }
    let threshold = peak / 2;

    let (x0, y0, x1, y1) = bounds(f, threshold).ok_or(DetectError::NoBrightPixels)?;
    let span_x = x1 - x0 + 1;
    let span_y = y1 - y0 + 1;

    // The outer edge of a marker ring runs for three blocks.
    let run_top = bright_run(f, x0, (y0 + 1).min(f.height - 1), threshold);
    let run_bottom = bright_run(f, x0, y1.saturating_sub(1), threshold);
    let run = run_top.min(run_bottom);
    if run < 3 {
        return Err(DetectError::ShortMarkerRun { run: run as u32 });
    }

    let guess = ((run + 1) / 3).max(1) as i32; // round(run / 3)

    for d in CANDIDATE_ORDER {
        let candidate = guess + d;
        if candidate < 1 {
            continue;
        }
        if let Some(layout) =
            try_candidate(f, x0, y0, span_x, span_y, candidate, threshold, total_bits)
        {
            return Ok(layout);
        }
    }

    Err(DetectError::NoBlockSize {
        span_x: span_x as u32,
        span_y: span_y as u32,
        run: run as u32,
    })
}

/// A wrong block size can still divide the bounding box evenly, so every
/// candidate has to pass the marker test and the strip test.
#[allow(clippy::too_many_arguments)]
fn try_candidate<P: Pixel>(
    f: &Frame<'_, P>,
    x0: usize,
    y0: usize,
    span_x: usize,
    span_y: usize,
    block_px: i32,
    threshold: u32,
    total_bits: u32,
) -> Option<GridLayout> {
    let block = block_px as i64;

    // The shader forces an odd block count on both axes.
    let cols = div_round(span_x as i64, block);
    let rows = div_round(span_y as i64, block);
    if cols & 1 == 0 || rows & 1 == 0 {
        return None;
    }
    if (cols * block - span_x as i64).abs() > 2 {
        return None;
    }
    if (rows * block - span_y as i64).abs() > 2 {
        return None;
    }
    if cols < (DATA_BORDER * 2 + 2) as i64 {
        return None;
    }
    if rows < (DATA_BORDER * 2 + 1) as i64 {
        return None;
    }

    let (cols, rows) = (cols as i32, rows as i32);
    let last = MARKER_BLOCKS; // ring occupies [i, i + 3)

    let top_left = has_marker(f, x0, y0, block_px, 0, 0, threshold);
    let top_right = has_marker(f, x0, y0, block_px, cols - last, 0, threshold);
    let bottom_left = has_marker(f, x0, y0, block_px, 0, rows - last, threshold);
    let bottom_right = has_marker(f, x0, y0, block_px, cols - last, rows - last, threshold);

    let found = top_left as u32 + top_right as u32 + bottom_left as u32 + bottom_right as u32;
    if found != 3 {
        return None;
    }

    // The shader leaves the bottom right corner empty. The empty corner in the
    // capture gives the flip.
    let (flip_x, flip_y) = if !bottom_right {
        (false, false)
    } else if !bottom_left {
        (true, false)
    } else if !top_right {
        (false, true)
    } else {
        (true, true)
    };

    let layout = GridLayout {
        origin_x: x0 as i32,
        origin_y: y0 as i32,
        block_px,
        cols,
        rows,
        flip_x,
        flip_y,
    };

    let pairs = layout.pairs();
    if pairs <= 0 {
        return None;
    }

    let data_rows = (total_bits as i32 + pairs - 1) / pairs;
    if data_rows > rows - DATA_BORDER * 2 {
        return None;
    }

    if !check_strips(f, &layout, threshold) {
        return None;
    }

    Some(layout)
}

/// Round-half-up integer division. C#'s `Math.Round` is banker's rounding; the
/// difference only shows on an exact .5, which the `± 2` span checks reject
/// anyway, and half-up is the behaviour you want for a pixel count.
#[inline]
fn div_round(n: i64, d: i64) -> i64 {
    (2 * n + d) / (2 * d)
}

fn peak_luma<P: Pixel>(f: &Frame<'_, P>) -> u32 {
    let mut peak = 0;
    for y in (0..f.height).step_by(3) {
        for p in f.row(y).iter().step_by(2) {
            let v = luma(p);
            if v > peak {
                peak = v;
                if peak >= 255 {
                    return peak; // cannot get brighter
                }
            }
        }
    }
    peak
}

/// Bounding box of the bright pixels, inclusive. Returns `None` if the frame
/// holds none.
///
/// Once `x0`/`x1` are known a row can only widen the box from outside them, so
/// the interior is scanned with a short-circuiting `any` purely to decide
/// whether the row contributes to `y1`.
fn bounds<P: Pixel>(f: &Frame<'_, P>, threshold: u32) -> Option<(usize, usize, usize, usize)> {
    let bright = |p: &P| luma(p) > threshold;

    let (mut x0, mut x1) = (usize::MAX, 0usize);
    let (mut y0, mut y1) = (usize::MAX, 0usize);

    for y in 0..f.height {
        let row = f.row(y);
        let mut hit = false;

        if x0 == usize::MAX {
            if let Some(first) = row.iter().position(bright) {
                // rposition cannot fail here: `first` is bright.
                x1 = row.iter().rposition(bright).unwrap_or(first);
                x0 = first;
                hit = true;
            }
        } else {
            if let Some(first) = row[..x0].iter().position(bright) {
                x0 = first;
                hit = true;
            }
            if let Some(last) = row[x1 + 1..].iter().rposition(bright) {
                x1 += 1 + last;
                hit = true;
            }
            if !hit {
                hit = row[x0..=x1].iter().any(bright);
            }
        }

        if hit {
            if y0 == usize::MAX {
                y0 = y;
            }
            y1 = y;
        }
    }

    if x1 >= x0 && y0 != usize::MAX {
        Some((x0, y0, x1, y1))
    } else {
        None
    }
}

/// Counts the bright pixels that follow the first bright pixel on a row.
fn bright_run<P: Pixel>(f: &Frame<'_, P>, start_x: usize, y: usize, threshold: u32) -> usize {
    let row = &f.row(y)[start_x.min(f.width)..];
    let Some(start) = row.iter().position(|p| luma(p) > threshold) else {
        return 0;
    };
    row[start..]
        .iter()
        .take_while(|p| luma(*p) > threshold)
        .count()
}

/// A marker is a bright ring of three by three blocks with a dark centre.
/// `ix`/`iy` are image block coordinates, before any flip.
fn has_marker<P: Pixel>(
    f: &Frame<'_, P>,
    x0: usize,
    y0: usize,
    block_px: i32,
    ix: i32,
    iy: i32,
    threshold: u32,
) -> bool {
    let corner = |dx: i32, dy: i32| block_bright(f, x0, y0, block_px, ix + dx, iy + dy, threshold);

    corner(0, 0) == Some(true)
        && corner(2, 0) == Some(true)
        && corner(0, 2) == Some(true)
        && corner(2, 2) == Some(true)
        && corner(1, 1) == Some(false)
}

/// Brightness at the centre of one block, addressed in image block coordinates.
#[inline]
fn block_bright<P: Pixel>(
    f: &Frame<'_, P>,
    x0: usize,
    y0: usize,
    block_px: i32,
    ix: i32,
    iy: i32,
    threshold: u32,
) -> Option<bool> {
    let half = block_px / 2;
    f.bright_checked(
        x0 as i32 + ix * block_px + half,
        y0 as i32 + iy * block_px + half,
        threshold,
    )
}

/// The strips run along the marker centre lines. The first strip block after
/// the ring is dark, then the strip alternates on every block.
fn check_strips<P: Pixel>(f: &Frame<'_, P>, layout: &GridLayout, threshold: u32) -> bool {
    for i in 0..6 {
        let want = i & 1 == 1;

        let (lx, ly) = layout.block_center(MARKER_CENTER, MARKER_BLOCKS + i);
        if f.bright_checked(lx, ly, threshold) != Some(want) {
            return false;
        }

        let (tx, ty) = layout.block_center(MARKER_BLOCKS + i, MARKER_CENTER);
        if f.bright_checked(tx, ty, threshold) != Some(want) {
            return false;
        }
    }
    true
}
