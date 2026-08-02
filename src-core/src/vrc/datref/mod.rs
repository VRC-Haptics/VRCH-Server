//! Video side parameter decode.
//!
//! VRChat draws a packed bit grid into a camera. `datarefdecode.dll` reads the
//! Spout sender and returns the fields of that grid. This module runs the
//! decoder on one thread and pushes every changed field into the input map.
//!
//! The path exists next to OSC, not in place of it. OSC still carries the
//! avatar description and the slow parameters. The camera carries the fast
//! ones. Both write the same slots, so the newer value wins.
//!
//! # Cost
//!
//! The decode thread allocates at bind time only. A frame costs one compare
//! per field and one channel send per changed target. The thread takes no lock
//! on the frame path. The control path uses an atomic pointer swap.

mod ffi;
mod session;

use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::slice;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use arc_swap::ArcSwapOption;
use tokio::sync::mpsc::UnboundedSender;

use crate::mapping::InputEventMessage;

use super::push_scalar;

pub use ffi::{DrdField, DrdStatus, DrdStatusCode, DRD_LIBRARY_PATH};
pub use session::{CameraSession, FieldRoute, FieldSpec, SessionError};

use ffi::{Drd, DrdConfig, DrdFrame, DRD_CONFIG_ON_CHANGE, DRD_FRAME_ID};

/// The wait between polls while frames arrive.
const PACE_HOT: Duration = Duration::from_millis(4);
/// The wait after a short run of empty polls.
const PACE_WARM: Duration = Duration::from_millis(5);
/// The wait when no sender exists.
const PACE_COLD: Duration = Duration::from_millis(100);
/// The count of empty polls before the warm wait.
const IDLE_WARM: u32 = 16;
/// The count of empty polls before the cold wait.
const IDLE_COLD: u32 = 32;
/// The wait before a second open after a failure.
const REOPEN_DELAY: Duration = Duration::from_secs(2);
/// The wait between two reads of the handle state.
const STATUS_PERIOD: Duration = Duration::from_secs(1);

/// The state that the UI shows for the camera path.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraState {
    /// No avatar bound a camera.
    #[default]
    Off,
    /// The decoder is open. No frame arrived yet.
    Waiting,
    /// Frames arrive and drive the input map.
    Running,
    /// The library or the sender failed. The thread retries.
    Failed,
}

impl CameraState {
    fn code(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Waiting => 1,
            Self::Running => 2,
            Self::Failed => 3,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Waiting,
            2 => Self::Running,
            3 => Self::Failed,
            _ => Self::Off,
        }
    }
}

/// A snapshot of the camera path for the UI.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct CameraInfo {
    /// The camera ID of the avatar.
    pub id: Option<String>,
    /// The schema version in use.
    pub version: u32,
    /// The count of fields that drive a watched parameter.
    pub mapped_fields: u32,
    /// The count of fields in the schema.
    pub total_fields: u32,
    pub state: CameraState,
    pub frames: u64,
    pub values_sent: u64,
    pub dropped_frames: u64,
    pub errors: u64,
    /// The decode time of the last frame.
    pub decode_micros: u64,
    pub mean_confidence: f32,
    /// True when the frame ID does not match the schema ID.
    pub id_mismatch: bool,
    /// True when the Spout sender exists.
    pub connected: bool,
    /// True when the detector found the grid.
    pub detected: bool,
    pub grid_cols: u32,
    pub grid_rows: u32,
    /// Calls to `drd_poll` per second. This shows the pace of the thread, not
    /// the pace of the camera.
    pub poll_hz: f32,
    /// Frames per second that the poll loop took from the library.
    pub capture_hz: f32,
    /// Frames per second that carried new bits.
    pub change_hz: f32,
    /// Map updates per second.
    pub value_hz: f32,
    /// Frames per second that the library counted for the handle.
    pub library_hz: f32,
    /// The running total of frames that the library counted but the poll loop
    /// never took.
    pub missed_frames: u64,
}

/// The counters that the decode thread writes and the UI reads.
///
/// Every field uses relaxed order. The values report progress. They order
/// nothing.
#[derive(Debug, Default)]
pub struct CameraStats {
    frames: AtomicU64,
    values_sent: AtomicU64,
    dropped_frames: AtomicU64,
    errors: AtomicU64,
    decode_nanos: AtomicU64,
    mean_confidence: AtomicU32,
    state: AtomicU8,
    id_mismatch: AtomicBool,
    connected: AtomicBool,
    detected: AtomicBool,
    /// The grid size, columns in the high half and rows in the low half.
    grid: AtomicU32,
    /// The rates of the last second, held as the bits of an f32.
    poll_hz: AtomicU32,
    capture_hz: AtomicU32,
    change_hz: AtomicU32,
    value_hz: AtomicU32,
    library_hz: AtomicU32,
    /// Frames that the library counted but never handed to the poll loop.
    missed_frames: AtomicU64,
}

impl CameraStats {
    /// The count of frames that ran the callback.
    #[inline]
    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    /// A count that moves whenever the sender delivers a frame.
    ///
    /// The change gate holds back a frame with the same bits, so the frame
    /// count alone does not show a live sender. This count adds the gated
    /// frames. The VRC loop compares it against the last tick.
    #[inline]
    pub fn activity(&self) -> u64 {
        self.frames.load(Ordering::Relaxed) + self.dropped_frames.load(Ordering::Relaxed)
    }

    #[inline]
    fn set_state(&self, state: CameraState) {
        self.state.store(state.code(), Ordering::Relaxed);
    }

    fn state(&self) -> CameraState {
        CameraState::from_code(self.state.load(Ordering::Relaxed))
    }

    fn reset(&self) {
        self.frames.store(0, Ordering::Relaxed);
        self.values_sent.store(0, Ordering::Relaxed);
        self.dropped_frames.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.decode_nanos.store(0, Ordering::Relaxed);
        self.mean_confidence.store(0, Ordering::Relaxed);
        self.id_mismatch.store(false, Ordering::Relaxed);
        self.connected.store(false, Ordering::Relaxed);
        self.detected.store(false, Ordering::Relaxed);
        self.grid.store(0, Ordering::Relaxed);
        self.missed_frames.store(0, Ordering::Relaxed);
        self.clear_rates();
    }

    /// Zeroes the rates. A closed decoder delivers nothing.
    fn clear_rates(&self) {
        for cell in [
            &self.poll_hz,
            &self.capture_hz,
            &self.change_hz,
            &self.value_hz,
            &self.library_hz,
        ] {
            cell.store(0, Ordering::Relaxed);
        }
    }

    fn store_rates(&self, rates: &CameraRates) {
        self.poll_hz.store(rates.poll_hz.to_bits(), Ordering::Relaxed);
        self.capture_hz
            .store(rates.capture_hz.to_bits(), Ordering::Relaxed);
        self.change_hz
            .store(rates.change_hz.to_bits(), Ordering::Relaxed);
        self.value_hz.store(rates.value_hz.to_bits(), Ordering::Relaxed);
        self.library_hz
            .store(rates.library_hz.to_bits(), Ordering::Relaxed);
    }

    /// Copies the state of the handle in. The worker calls this once a second.
    fn store_status(&self, status: &DrdStatus) {
        self.connected.store(status.connected != 0, Ordering::Relaxed);
        self.detected.store(status.detected != 0, Ordering::Relaxed);
        let cols = status.cols.clamp(0, u16::MAX as i32) as u32;
        let rows = status.rows.clamp(0, u16::MAX as i32) as u32;
        self.grid.store((cols << 16) | rows, Ordering::Relaxed);
    }

    /// Fills the UI view. The session supplies the names.
    pub fn snapshot(&self, session: Option<&CameraSession>) -> CameraInfo {
        let grid = self.grid.load(Ordering::Relaxed);
        CameraInfo {
            id: session.map(|s| s.id.to_string()),
            version: session.map_or(0, |s| s.version),
            mapped_fields: session.map_or(0, |s| s.mapped_fields),
            total_fields: session.map_or(0, |s| s.field_count()),
            state: self.state(),
            frames: self.frames.load(Ordering::Relaxed),
            values_sent: self.values_sent.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            decode_micros: self.decode_nanos.load(Ordering::Relaxed) / 1_000,
            mean_confidence: f32::from_bits(self.mean_confidence.load(Ordering::Relaxed)),
            id_mismatch: self.id_mismatch.load(Ordering::Relaxed),
            connected: self.connected.load(Ordering::Relaxed),
            detected: self.detected.load(Ordering::Relaxed),
            grid_cols: grid >> 16,
            grid_rows: grid & 0xFFFF,
            poll_hz: f32::from_bits(self.poll_hz.load(Ordering::Relaxed)),
            capture_hz: f32::from_bits(self.capture_hz.load(Ordering::Relaxed)),
            change_hz: f32::from_bits(self.change_hz.load(Ordering::Relaxed)),
            value_hz: f32::from_bits(self.value_hz.load(Ordering::Relaxed)),
            library_hz: f32::from_bits(self.library_hz.load(Ordering::Relaxed)),
            missed_frames: self.missed_frames.load(Ordering::Relaxed),
        }
    }
}

/// The handle that the VRC loop keeps.
///
/// The handle is cheap to hold and safe to drop. A drop stops the thread.
pub struct CameraDecoder {
    desired: Arc<ArcSwapOption<CameraSession>>,
    stats: Arc<CameraStats>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CameraDecoder {
    /// Starts the decode thread. The thread idles until a bind arrives.
    pub fn start(tx: UnboundedSender<InputEventMessage>) -> CameraDecoder {
        Self::start_with_library(tx, Path::new(DRD_LIBRARY_PATH).to_path_buf())
    }

    /// Starts the decode thread against a named library file.
    pub fn start_with_library(
        tx: UnboundedSender<InputEventMessage>,
        library: std::path::PathBuf,
    ) -> CameraDecoder {
        let desired = Arc::new(ArcSwapOption::empty());
        let stats = Arc::new(CameraStats::default());
        let running = Arc::new(AtomicBool::new(true));

        // The worker holds the pointer that the library calls back on. Build it
        // on the decode thread so that the pointer never crosses a thread.
        let (worker_desired, worker_stats, worker_running) =
            (Arc::clone(&desired), Arc::clone(&stats), Arc::clone(&running));

        let thread = thread::Builder::new()
            .name("datref-decode".to_string())
            .spawn(move || {
                Worker {
                    tx,
                    library,
                    desired: worker_desired,
                    stats: worker_stats,
                    running: worker_running,
                    current: None,
                    pending: None,
                    retry_at: Instant::now(),
                }
                .run()
            })
            .map_err(|err| log::error!("Unable to start the camera decode thread: {}", err))
            .ok();

        CameraDecoder {
            desired,
            stats,
            running,
            thread,
        }
    }

    /// Binds a session. The thread opens the decoder on the next poll.
    pub fn bind(&self, session: Arc<CameraSession>) {
        log::info!(
            "Camera bound: id={} version={} fields={}/{} bits={}",
            session.id,
            session.version,
            session.mapped_fields,
            session.field_count(),
            session.total_bits
        );
        self.stats.reset();
        self.stats.set_state(CameraState::Waiting);
        self.desired.store(Some(session));
    }

    /// Drops the session. The thread closes the decoder on the next poll.
    pub fn clear(&self) {
        if self.desired.swap(None).is_some() {
            log::debug!("Camera unbound");
        }
        self.stats.set_state(CameraState::Off);
    }

    /// The live counters.
    #[inline]
    pub fn stats(&self) -> &Arc<CameraStats> {
        &self.stats
    }
}

impl Drop for CameraDecoder {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        self.desired.store(None);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The per field state that the callback owns.
///
/// The pointer that `drd_open` receives points at this struct. Only the decode
/// thread reads it, and only inside `drd_poll`.
struct CallbackState {
    tx: UnboundedSender<InputEventMessage>,
    session: Arc<CameraSession>,
    /// The last raw bits of each field. The callback drops a field that did
    /// not change.
    last: Box<[u32]>,
    /// False until the first frame fills `last`.
    primed: bool,
    stats: Arc<CameraStats>,
}

impl CallbackState {
    /// Routes one frame. The function allocates nothing.
    fn dispatch(&mut self, frame: &DrdFrame) {
        let Self {
            tx,
            session,
            last,
            primed,
            stats,
        } = self;

        stats.frames.fetch_add(1, Ordering::Relaxed);
        stats.decode_nanos.store(frame.decode_nanos, Ordering::Relaxed);
        stats
            .mean_confidence
            .store(frame.mean_confidence.to_bits(), Ordering::Relaxed);
        stats.set_state(CameraState::Running);

        // An ID frame carries a name, not field data. Compare it against the
        // schema ID so that a wrong camera shows in the UI.
        if frame.flags & DRD_FRAME_ID != 0 {
            let matches = if frame.id_text.is_null() {
                false
            } else {
                let text = unsafe { slice::from_raw_parts(frame.id_text, frame.id_len as usize) };
                text == session.id.as_bytes()
            };
            stats.id_mismatch.store(!matches, Ordering::Relaxed);
            return;
        }

        if frame.values.is_null() {
            return;
        }

        let count = (frame.value_count as usize)
            .min(session.specs.len())
            .min(last.len());
        let values = unsafe { slice::from_raw_parts(frame.values, count) };
        let gate = session.min_confidence;
        let mut sent = 0u64;

        for (index, value) in values.iter().enumerate() {
            // A field below the gate keeps the last accepted bits. The next
            // clean frame still reports a change.
            if value.confidence < gate {
                continue;
            }
            if *primed && last[index] == value.raw {
                continue;
            }
            last[index] = value.raw;

            let route = &session.routes[index];
            if route.is_empty() {
                continue;
            }

            let scalar = session.specs[index].to_scalar(value.raw, value.number);
            for target in route.iter() {
                if push_scalar(tx, target, scalar) {
                    sent += 1;
                }
            }
        }

        *primed = true;
        if sent != 0 {
            stats.values_sent.fetch_add(sent, Ordering::Relaxed);
        }
    }
}

/// The frame callback that the library calls inside `drd_poll`.
///
/// # Safety
///
/// `user` must point at a live `CallbackState`. The library holds that pointer
/// from `drd_open` to `drd_close`.
unsafe extern "C" fn on_frame(user: *mut c_void, frame: *const DrdFrame) {
    if user.is_null() || frame.is_null() {
        return;
    }
    let state = unsafe { &mut *(user as *mut CallbackState) };
    let frame = unsafe { &*frame };

    // Rule 3 of the ABI: no panic crosses the callback.
    if panic::catch_unwind(AssertUnwindSafe(|| state.dispatch(frame))).is_err() {
        state.stats.errors.fetch_add(1, Ordering::Relaxed);
        log::error!("The camera frame callback panicked");
    }
}

/// The rates of one measure window.
#[derive(Copy, Clone, Debug, Default)]
struct CameraRates {
    poll_hz: f32,
    capture_hz: f32,
    change_hz: f32,
    value_hz: f32,
    library_hz: f32,
}

/// The counter values at the start of a measure window.
///
/// The worker reads the counters once a second and turns the deltas into
/// rates. The frame path counts. It never divides.
#[derive(Copy, Clone, Debug)]
struct RateWindow {
    at: Instant,
    polls: u64,
    frames: u64,
    dropped: u64,
    values: u64,
    library_frames: u64,
}

impl RateWindow {
    fn start(at: Instant) -> Self {
        Self {
            at,
            polls: 0,
            frames: 0,
            dropped: 0,
            values: 0,
            library_frames: 0,
        }
    }

    /// Turns the counter deltas into rates and starts the next window.
    ///
    /// The second return value is the count of frames that the library counted
    /// but the poll loop never took.
    fn measure(
        &mut self,
        now: Instant,
        polls: u64,
        stats: &CameraStats,
        library_frames: u64,
    ) -> Option<(CameraRates, u64)> {
        let elapsed = now.duration_since(self.at).as_secs_f32();
        if elapsed <= 0.0 {
            return None;
        }

        let frames = stats.frames.load(Ordering::Relaxed);
        let dropped = stats.dropped_frames.load(Ordering::Relaxed);
        let values = stats.values_sent.load(Ordering::Relaxed);

        let taken = frames.saturating_sub(self.frames) + dropped.saturating_sub(self.dropped);
        let counted = library_frames.saturating_sub(self.library_frames);
        // The library counts a frame that the poll loop was too slow to take.
        // A first window after an open sees a large count, so hold it at the
        // frames that the loop took.
        let missed = counted.saturating_sub(taken);

        let per_second = |count: u64| count as f32 / elapsed;
        let rates = CameraRates {
            poll_hz: per_second(polls.saturating_sub(self.polls)),
            capture_hz: per_second(taken),
            change_hz: per_second(frames.saturating_sub(self.frames)),
            value_hz: per_second(values.saturating_sub(self.values)),
            library_hz: per_second(counted),
        };

        *self = RateWindow {
            at: now,
            polls,
            frames,
            dropped,
            values,
            library_frames,
        };
        Some((rates, missed))
    }
}

/// One open decoder handle and the state that the library points at.
struct OpenDecoder {
    drd: &'static Drd,
    handle: isize,
    session: Arc<CameraSession>,
    /// The library holds this pointer. The box outlives the handle.
    state: *mut CallbackState,
    _timer: TimerResolution,
}

impl Drop for OpenDecoder {
    fn drop(&mut self) {
        // Close first. The library must not call back after this point.
        unsafe { (self.drd.close)(self.handle) };
        drop(unsafe { Box::from_raw(self.state) });
    }
}

/// The decode thread.
struct Worker {
    tx: UnboundedSender<InputEventMessage>,
    library: std::path::PathBuf,
    desired: Arc<ArcSwapOption<CameraSession>>,
    stats: Arc<CameraStats>,
    running: Arc<AtomicBool>,
    current: Option<OpenDecoder>,
    pending: Option<Arc<CameraSession>>,
    retry_at: Instant,
}

impl Worker {
    fn run(mut self) {
        let mut idle: u32 = 0;
        let mut scratch = [0u8; 512];
        let mut next_status = Instant::now();
        // A plain counter. The poll path writes no atomic.
        let mut polls: u64 = 0;
        let mut window = RateWindow::start(next_status);
        let mut measured_handle: isize = 0;

        while self.running.load(Ordering::Acquire) {
            self.follow_desired();

            // Copy the handle out. The poll path then holds no borrow of the
            // worker, so an error can close the decoder in place.
            let (drd, handle) = match self.current.as_ref() {
                Some(open) => (open.drd, open.handle),
                None => {
                    thread::sleep(PACE_COLD);
                    continue;
                }
            };

            // A new handle carries its own counters, and the bind reset the
            // stats. Start a new measure window with it.
            if handle != measured_handle {
                measured_handle = handle;
                polls = 0;
                let now = Instant::now();
                window = RateWindow::start(now);
                next_status = now + STATUS_PERIOD;
            }

            polls += 1;
            let code = unsafe { (drd.poll)(handle) };
            match DrdStatusCode::from_raw(code) {
                DrdStatusCode::Ok => idle = 0,
                DrdStatusCode::Skipped => {
                    idle = 0;
                    self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
                }
                DrdStatusCode::Waiting => {
                    idle = idle.saturating_add(1);
                    if idle == IDLE_COLD {
                        self.stats.set_state(CameraState::Waiting);
                    }
                }
                status => {
                    let text = unsafe { drd.error_text(handle, &mut scratch) };
                    log::error!("Camera poll failed: {:?}: {}", status, text);
                    self.stats.errors.fetch_add(1, Ordering::Relaxed);
                    self.stats.set_state(CameraState::Failed);
                    if status.is_fatal() {
                        self.close();
                        self.retry_at = Instant::now() + REOPEN_DELAY;
                        idle = 0;
                        continue;
                    }
                }
            }

            // The state of the handle costs one call, so read it once a
            // second. The frame path never reads it.
            let now = Instant::now();
            if now >= next_status {
                next_status = now + STATUS_PERIOD;

                let library_frames = match unsafe { drd.read_status(handle) } {
                    Ok(status) => {
                        self.stats.store_status(&status);
                        status.frames
                    }
                    Err(code) => {
                        log::debug!("Camera status read failed: {:?}", code);
                        window.library_frames
                    }
                };

                if let Some((rates, missed)) =
                    window.measure(now, polls, &self.stats, library_frames)
                {
                    self.stats.store_rates(&rates);
                    if missed != 0 {
                        self.stats
                            .missed_frames
                            .fetch_add(missed, Ordering::Relaxed);
                    }
                    if rates.capture_hz > 0.0 || rates.library_hz > 0.0 {
                        log::debug!(
                            "Camera: capture {:.1} Hz, change {:.1} Hz, values {:.0}/s, library {:.1} Hz, poll {:.0} Hz, missed {}",
                            rates.capture_hz,
                            rates.change_hz,
                            rates.value_hz,
                            rates.library_hz,
                            rates.poll_hz,
                            missed
                        );
                    }
                }
            }

            thread::sleep(if idle < IDLE_WARM {
                PACE_HOT
            } else if idle < IDLE_COLD {
                PACE_WARM
            } else {
                PACE_COLD
            });
        }

        self.close();
    }

    /// Opens, closes, or reopens the decoder to match the published session.
    ///
    /// The published session is the only source of truth. A poll error clears
    /// the open decoder but keeps the session, so the loop reopens it after the
    /// retry delay.
    fn follow_desired(&mut self) {
        let target = self.desired.load_full();

        // An open decoder that runs the wrong session must go.
        let current_matches = match (&target, &self.current) {
            (Some(next), Some(open)) => Arc::ptr_eq(next, &open.session),
            (None, None) => true,
            _ => false,
        };
        if !current_matches {
            self.close();
        }

        // A new session resets the retry clock. A repeat of the same session
        // keeps it, so a failed open backs off.
        let session_changed = match (&target, &self.pending) {
            (Some(next), Some(pending)) => !Arc::ptr_eq(next, pending),
            (None, None) => false,
            _ => true,
        };
        if session_changed {
            self.pending = target;
            self.retry_at = Instant::now();
        }

        if self.current.is_some() || Instant::now() < self.retry_at {
            return;
        }
        let Some(session) = self.pending.clone() else {
            return;
        };

        match self.open(session) {
            Ok(()) => self.stats.set_state(CameraState::Waiting),
            Err(message) => {
                log::error!("Unable to open the camera decoder: {}", message);
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                self.stats.set_state(CameraState::Failed);
                self.retry_at = Instant::now() + REOPEN_DELAY;
            }
        }
    }

    fn open(&mut self, session: Arc<CameraSession>) -> Result<(), String> {
        let drd = ffi::library(&self.library).map_err(|err| err.to_string())?;

        let state = Box::into_raw(Box::new(CallbackState {
            tx: self.tx.clone(),
            last: vec![0u32; session.fields.len()].into_boxed_slice(),
            primed: false,
            stats: Arc::clone(&self.stats),
            session: Arc::clone(&session),
        }));

        let mut flags = 0u32;
        if session.only_on_change {
            flags |= DRD_CONFIG_ON_CHANGE;
        }
        let (name_ptr, name_len) = match &session.sender_name {
            Some(name) => (name.as_ptr(), name.len() as u32),
            None => (std::ptr::null(), 0),
        };

        let config = DrdConfig {
            version: ffi::DRD_ABI_VERSION,
            total_bits: session.total_bits,
            fields: session.fields.as_ptr(),
            field_count: session.fields.len() as u32,
            min_confidence: session.min_confidence,
            sender_name: name_ptr,
            sender_name_len: name_len,
            flags,
        };

        let mut handle: isize = 0;
        let code = unsafe {
            (drd.open)(
                &config,
                on_frame,
                state as *mut c_void,
                &mut handle as *mut isize,
            )
        };

        let status = DrdStatusCode::from_raw(code);
        if status.is_error() || handle == 0 {
            let mut scratch = [0u8; 512];
            // A failed open leaves the reason under the zero handle, so ask
            // with the handle that came back.
            let text = unsafe { drd.error_text(handle, &mut scratch) }.to_string();
            // The library never saw the pointer, or it dropped the handle.
            drop(unsafe { Box::from_raw(state) });
            return Err(format!("{:?}: {}", status, text));
        }

        self.current = Some(OpenDecoder {
            drd,
            handle,
            session,
            state,
            _timer: TimerResolution::acquire(),
        });
        Ok(())
    }

    /// Closes the open decoder. The state stays with the caller.
    fn close(&mut self) {
        if self.current.take().is_some() {
            self.stats.clear_rates();
        }
    }
}

/// Raises the Windows timer resolution while a decoder runs.
///
/// The decode loop sleeps for a quarter of a millisecond between polls. The
/// default Windows timer rounds that up to about 16 milliseconds, which caps
/// the parameter rate near 60 Hz. The guard asks for 1 millisecond and gives it
/// back at the drop.
struct TimerResolution {
    #[cfg(windows)]
    end: Option<unsafe extern "system" fn(u32) -> u32>,
}

impl TimerResolution {
    #[cfg(windows)]
    fn acquire() -> Self {
        use std::sync::OnceLock;
        type PeriodFn = unsafe extern "system" fn(u32) -> u32;
        static WINMM: OnceLock<Option<(PeriodFn, PeriodFn)>> = OnceLock::new();

        let calls = WINMM.get_or_init(|| unsafe {
            let lib = libloading::Library::new("winmm.dll").ok()?;
            let lib: &'static libloading::Library = Box::leak(Box::new(lib));
            let begin = *lib.get::<PeriodFn>(b"timeBeginPeriod\0").ok()?;
            let end = *lib.get::<PeriodFn>(b"timeEndPeriod\0").ok()?;
            Some((begin, end))
        });

        // A function pointer is Copy, so take it out of the cache by value.
        match *calls {
            Some((begin, end)) => {
                unsafe { begin(1) };
                Self { end: Some(end) }
            }
            None => {
                log::warn!("Unable to raise the timer resolution. Expect a lower frame rate.");
                Self { end: None }
            }
        }
    }

    #[cfg(not(windows))]
    fn acquire() -> Self {
        Self {}
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(end) = self.end {
            unsafe { end(1) };
        }
    }
}