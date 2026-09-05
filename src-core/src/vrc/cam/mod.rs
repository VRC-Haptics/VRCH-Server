mod algo;
mod center;
mod grid;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::Result;
#[cfg(windows)]
use rustc_hash::FxHashMap;
#[cfg(windows)]
use spout2::dx;
use tokio::runtime::Builder;
use tokio::sync::Notify;
use tokio::time::{self, Interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use triple_buffer::Input;

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

    // block_on in the spawned worker requires a seperate thread to do the forwarding work?
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

#[cfg(windows)]
/// the dx receiver is !Send so it can't be used in normal thread pool?
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
            rt.block_on(cancel.run_until_cancelled_owned(spout_loop(active, conf, notif, input)));
        })
        .expect("spawn spout-rx thread")
}

#[cfg(windows)]
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

/// A snapshot of the camera path for the UI.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct CameraInfo {}

/// The counters that the decode thread writes and the UI reads.
///
/// Every field uses relaxed order. The values report progress. They order
/// nothing.
#[derive(Debug, Default)]
pub struct CameraStats {}

impl CameraStats {}
