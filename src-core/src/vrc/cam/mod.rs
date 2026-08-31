mod algo;
mod center;
mod ffi;
mod grid;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::Result;
pub use ffi::{DrdField, DrdStatus, DrdStatusCode, DRD_LIBRARY_PATH};
#[cfg(windows)]
use rustc_hash::FxHashMap;
use spout2::dx;
use tokio::runtime::Builder;
use tokio::sync::Notify;
use tokio::time::{self, Interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use triple_buffer::{Input, Output};

use crate::api::CamConfig;
#[cfg(windows)]
use crate::log_err;
#[cfg(windows)]
use crate::mapping::{
    BatchUpdateMsg, InputEventMessage, MapHandle, NodeField, NodeFieldKind, SlotField,
    SlotFieldKind,
};
use crate::vrc::cam::algo::{CamValue, Decoder};
#[cfg(windows)]
use crate::vrc::{AddrInfo, WatchedAddr};

/// starts sending updates from a spout stream to the global map
#[cfg(windows)]
pub fn start_cam(
    conf: CamConfig,
    map: MapHandle,
    watched: FxHashMap<String, WatchedAddr>,
) -> Result<CamHandle> {
    let (input, output) = triple_buffer::triple_buffer(&Vec::with_capacity(conf.fields.len()));

    log::trace!("Creating cam with config: {conf:?}");

    // start polling
    let cancel = CancellationToken::new();
    let notify = Arc::new(Notify::new());
    let active = Arc::new(AtomicBool::new(false));
    let handle = spawn_spout_worker(
        Arc::clone(&active),
        cancel.clone(),
        conf.clone(),
        input,
        Arc::clone(&notify),
    );

    // push batches to the map directly
    let notf = Arc::clone(&notify);
    tokio::spawn(cancel.clone().run_until_cancelled_owned(async move {
        let mut last_size = conf.fields.len(); // assume one field per on first go around
        loop {
            let mut batch = Box::new(Vec::with_capacity(last_size));
            notf.notified().await;
            let vals = output.output_buffer();

            for (field, val) in conf.fields.iter().zip(vals) {
                if let Some(info) = watched.get(&field.osc) {
                    for element in &info.infos {
                        match element {
                            AddrInfo::Slot(s, kind) => {
                                let msg = match kind {
                                    SlotFieldKind::Weight => BatchUpdateMsg::SlotField {
                                        key: s.clone(),
                                        field: SlotField::Weight(val.into_f32().unwrap_or(0.)),
                                    },
                                    SlotFieldKind::Value => BatchUpdateMsg::SlotField {
                                        key: s.clone(),
                                        field: SlotField::Value(val.into_f32().unwrap_or(0.)),
                                    },
                                    SlotFieldKind::Muted => BatchUpdateMsg::SlotField {
                                        key: s.clone(),
                                        field: SlotField::Muted(val.into_bool()),
                                    },
                                };
                                batch.push(msg);
                            }
                            AddrInfo::Node(n, kind) => {
                                let msg = match kind {
                                    NodeFieldKind::Location => {
                                        log::error!(
                                            "Vectors VIA cam's have not been implemented yet"
                                        );
                                        continue;
                                    }
                                    NodeFieldKind::Radius => BatchUpdateMsg::NodeField {
                                        key: n.clone(),
                                        field: NodeField::Radius(val.into_f32().unwrap_or(0.)),
                                    },
                                    NodeFieldKind::Muted => BatchUpdateMsg::NodeField {
                                        key: n.clone(),
                                        field: NodeField::Muted(val.into_bool()),
                                    },
                                };
                                batch.push(msg);
                            }
                        }
                    }
                } else {
                    log::error!("Unable to get key for: {}", field.osc);
                };
            }
            last_size = batch.len();
            log_err!(map.send_event(InputEventMessage::BatchUpdate(batch)));
        }
    }));

    // construct/return handle
    Ok(CamHandle {
        stop: cancel,
        handle,
        check: notify,
        active: active,
    })
}

pub enum State {
    Hot,
    Cold,
}

pub fn spawn_spout_worker(
    active: Arc<AtomicBool>,
    cancel: CancellationToken,
    conf: CamConfig,
    input: Input<Vec<CamValue>>,
    notif: Arc<Notify>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("spout-rx".into())
        .spawn(move || {
            let rt = Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("current-thread runtime");

            // block_on does not require Send, so the !Send Receiver is fine here.
            rt.block_on(cancel.run_until_cancelled_owned(spout_loop(active, conf, notif, input)));
        })
        .expect("spawn spout-rx thread")
}

async fn spout_loop(
    active: Arc<AtomicBool>,
    conf: CamConfig,
    notif: Arc<Notify>,
    mut input: Input<Vec<CamValue>>,
) {
    let mut state = State::Cold;

    let mut hot = time::interval(Duration::from_millis(2));
    hot.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut cool = time::interval(Duration::from_secs(1));
    cool.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut rx = match dx::Receiver::new(Some("VRCSender1")) {
        Ok(r) => r,
        Err(e) => {
            log::error!("Spout receiver init failed: {e}");
            return;
        }
    };

    // source config from vrc
    let mut decoder = Decoder::new(conf);
    let mut last_state = false;

    loop {
        let ticker: &mut Interval = match state {
            State::Cold => &mut cool,
            State::Hot => &mut hot,
        };
        let _ = ticker.tick().await;

        let connected = match rx.receive_image(
            &mut decoder.img,
            decoder.w,
            decoder.h,
            decoder.use_rgb,
            false,
        ) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Spout error: {e}");
                active.store(false, Ordering::Release);
                state = State::Cold;
                continue;
            }
        };

        if !last_state && connected {
            log::trace!("Spout Camera Connected");
            last_state = true;
        }

        if !connected {
            state = State::Cold;
            continue;
        }

        if rx.is_updated() {
            let (w, h) = rx.sender_size();
            if w == 0 || h == 0 {
                state = State::Cold;
                active.store(false, Ordering::Release);
                continue;
            }
            decoder.resize(w, h);
            continue;
        }

        if !rx.is_frame_new() {
            continue;
        }

        let out = input.input_buffer_mut();
        let has_values = decoder.process_frame(out);
        if has_values {
            state = State::Hot;
            let was_read = input.publish();
            active.store(true, Ordering::Release);
            notif.notify_one();

            if !was_read {
                log::trace!("buffer was not read before republishing");
            }
        }
    }
}

/// Dropping cancels tasks, not guaranteed to have finished before Drop returns.
pub struct CamHandle {
    pub active: Arc<AtomicBool>,
    pub stop: CancellationToken,
    handle: JoinHandle<()>,
    pub check: Arc<Notify>,
}

impl CamHandle {
    pub fn info(&self) -> CameraInfo {
        CameraInfo::default()
    }
}

impl Drop for CamHandle {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

pub enum CamMsg {}

pub struct CamStatus {
    width: u32,
    height: u32,
    rgb: bool,
}

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
        self.poll_hz
            .store(rates.poll_hz.to_bits(), Ordering::Relaxed);
        self.capture_hz
            .store(rates.capture_hz.to_bits(), Ordering::Relaxed);
        self.change_hz
            .store(rates.change_hz.to_bits(), Ordering::Relaxed);
        self.value_hz
            .store(rates.value_hz.to_bits(), Ordering::Relaxed);
        self.library_hz
            .store(rates.library_hz.to_bits(), Ordering::Relaxed);
    }

    /// Copies the state of the handle in. The worker calls this once a second.
    fn store_status(&self, status: &DrdStatus) {
        self.connected
            .store(status.connected != 0, Ordering::Relaxed);
        self.detected.store(status.detected != 0, Ordering::Relaxed);
        let cols = status.cols.clamp(0, u16::MAX as i32) as u32;
        let rows = status.rows.clamp(0, u16::MAX as i32) as u32;
        self.grid.store((cols << 16) | rows, Ordering::Relaxed);
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
