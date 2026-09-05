pub mod event;
//pub mod global_map;
pub mod groups;
pub mod haptic_node;
pub mod input_node;
pub mod interp;

use crate::{
    log_err,
    mapping::{
        event::{Frames, Steps},
        input_node::{InterpolationLayer, SlotKey},
    },
    state::{get_config, VrcSettings},
    vrc::config::{InputLayer, InputType},
};
use arc_swap::{ArcSwap, Cache, Guard};
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slotmap::SlotMap;
use std::{sync::Arc, time::Duration};
use strum::EnumDiscriminants;
use tokio::{
    sync::{
        mpsc::{self, error::SendError},
        oneshot,
    },
    time::{interval, Instant},
};

use event::Event;
use glam::Vec3;
use haptic_node::HapticNode;
use input_node::InputNode;

use crate::{
    devices::{Device, DeviceHandle, DeviceId, DeviceInfo, DeviceOutEvents},
    mapping::{
        groups::NodeGroup,
        input_node::{History, Slot},
    },
    state::{self, PerDevice},
};

pub type EventInstant = u64;
pub type EventDuration = u64;

/// Implements cheap clone, can be shared between threads safely.
pub struct MapHandle {
    pub snapshot: Arc<ArcSwap<Snapshot>>,
    pub event_sender: mpsc::UnboundedSender<InputEventMessage>,
}

impl MapHandle {
    /// marks that some change has been made and should be propagated to devices
    pub fn mark_dirty(&self) {
        log_err!(self.event_sender.send(InputEventMessage::MapDirty));
    }

    /// Gets new snapshot if it is dirty, returns
    pub async fn get_state(&self) -> Guard<Arc<Snapshot>> {
        let (tx, rx) = oneshot::channel::<()>();

        log_err!(self
            .event_sender
            .send(InputEventMessage::RequestInfo { reply: tx }));
        log_err!(rx.await); // wait till refreshed
        self.snapshot.load()
    }

    pub fn send_event(&self, msg: InputEventMessage) -> Result<(), SendError<InputEventMessage>> {
        self.event_sender.send(msg)
    }
}

impl Clone for MapHandle {
    /// Cheap clone, ideally not every iteration but not expensive either.
    fn clone(&self) -> Self {
        Self {
            snapshot: self.snapshot.clone(),
            event_sender: self.event_sender.clone(),
        }
    }
}

/// Information that the global map needs to calculate haptics for a device.
pub struct MappingDevice {
    pub id: DeviceId,
    /// keep in mind locking this also locks the associated devices access to the buffer.
    outputs: Arc<RwLock<Vec<f32>>>,
    nodes: Vec<HapticNode>,
}

impl MappingDevice {
    pub fn update_nodes(&mut self, nodes: Vec<HapticNode>) {
        self.nodes = nodes;
        let out = self.outputs.read();
        if out.len() != self.nodes.len() {
            log::error!(
                "Lenght of HapticNodes and output buffer not equal for device with id: {:?}",
                self.id
            );
        }
    }

    /// updates the buffer based on the referenced input nodes.
    ///
    /// NOTE: This does not update the remote device, to force an update remember to use the `crate::devices::Device` trait as specified
    ///
    pub fn update_buffer(&self, in_nodes: &Nodes, settings: &PerDevice) {
        let mut buf = self.outputs.write();
        if buf.len() != self.nodes.len() {
            log::trace!(
                "Buffer not equal node length: buf:{}, nodes: {}",
                buf.len(),
                self.nodes.len()
            );
            return;
        }
        settings
            .interp_algo
            .interp(&self.nodes, &mut buf, in_nodes, settings);
    }
}

pub async fn start_interp_map(manager: DeviceHandle) -> MapHandle {
    let (mut input_map, map_handle) = InputMap::new(manager).await;
    tokio::spawn(async move {
        input_map.start().await;
    });
    map_handle
}

/// Needs to handle:
///
/// Taking input from games;
/// Triggering update pushed to devices;
///
struct InputMap {
    generation: u64,
    /// Increments by one each event tick. roughly 10ms per instant tick
    now: EventInstant,
    input_nodes: Nodes,
    device_manager: DeviceHandle,
    devices: Vec<MappingDevice>,
    event_recv: mpsc::UnboundedReceiver<InputEventMessage>,
    event_send: mpsc::UnboundedSender<InputEventMessage>,
    dirty_since_snap: bool,
    snapshot: Arc<ArcSwap<Snapshot>>,
    /// Whether input mapping has changed in a way that should require device output updates
    map_dirty: bool,
    event_marker: EventId,
    quick_event: Option<EventKey>,
    /// last map tick they were triggered.
    events: Vec<Event>,
    /// When "Triggered" it creates an event instance.
    event_instance: Vec<(EventInstant, EventKey, EventId)>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct EventKey(usize);

pub type EventId = usize;

slotmap::new_key_type! {
    pub struct NodeKey;
}

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
#[cfg(feature = "specta")]
#[derive(specta::Type)]
#[specta(remote = NodeKey)]
#[serde(rename = "NodeKey")]
struct NodeKeyDef {
    idx: u32,
    version: u32,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct Nodes {
    /// List of event's active nodes.
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<EventId, Vec<NodeKey>>))]
    pub transient: IntMap<EventId, Vec<NodeKey>>,
    #[cfg_attr(feature = "specta", specta(type = Vec<Option<InputNode>>))]
    pub nodes: SlotMap<NodeKey, InputNode>,
    pub active_streaming: Vec<NodeKey>,
}

impl Nodes {
    /// Updates nodes' final output value.
    /// Gets called each update, be careful.
    pub fn update_nodes(&mut self, cfg: &Arc<VrcSettings>) {
        let now = std::time::Instant::now();
        // TODO: Add velocity calculations/handling
        for key in self
            .active_streaming
            .iter()
            .chain(self.transient.values().flatten())
        {
            let Some(node) = self.nodes.get_mut(*key) else {
                log::error!("can't find node in active nodes");
                continue;
            };

            let mut add = 0.0_f32;
            let mut mult = 1.0_f32;
            let mut max = 0.0_f32;
            let mut min = 1.0_f32;
            let mut overridden = None;
            for slot in node.slots.iter_mut() {
                let val = match slot.source {
                    InputType::Weight => slot.history.latest().unwrap_or(0.0),
                    InputType::Velocity => slot.history.velocity_at(now).abs() * cfg.velocity_mult,
                };
                let weighted = val * slot.weight;

                match slot.layer {
                    InputLayer::Additive => add += weighted,
                    InputLayer::Subtractive => add -= weighted,
                    InputLayer::Multiplicative => mult *= weighted,
                    InputLayer::Max => max = max.max(weighted),
                    InputLayer::Gate => min = min.min(weighted),
                    InputLayer::Override => {
                        overridden = Some(weighted);
                        break; // override wins; stop accumulating
                    }
                }
            }

            node.value = match overridden {
                Some(v) => v.clamp(0.0, 1.0),
                None => (add * mult).max(max).min(min).clamp(0.0, 1.0),
            };
        }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct Snapshot {
    pub instant: EventInstant,
    pub active_events: Vec<(EventInstant, EventKey, EventId)>,
    pub events: Vec<Event>,
    pub input_nodes: Nodes,
}

#[derive(Default)]
struct ArmStat {
    count: u64,
    total_ns: u128,
    max_ns: u128,
}

impl ArmStat {
    #[inline]
    fn record(&mut self, ns: u128) {
        self.count += 1;
        self.total_ns += ns;
        if ns > self.max_ns {
            self.max_ns = ns;
        }
    }
    fn avg_us(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.total_ns as f64 / self.count as f64) / 1000.0
        }
    }
}

#[derive(Default)]
struct LoopStats {
    dirty: ArmStat,
    device: ArmStat,
    events: ArmStat,
    msg: ArmStat,
    max_backlog: usize,
}

impl InputMap {
    /// Assumes `DeviceManager` has been initialized.
    pub async fn new(manager: DeviceHandle) -> (Self, MapHandle) {
        let (tx, rx) = mpsc::unbounded_channel();
        let input_nodes = Nodes::default();
        let snapshot = Arc::new(ArcSwap::new(Arc::new(Snapshot::default())));

        let map = Self {
            now: 0,
            generation: 0,
            input_nodes,
            device_manager: manager,
            devices: Vec::new(),
            dirty_since_snap: true,
            snapshot: snapshot.clone(),
            event_recv: rx,
            event_send: tx.clone(),
            map_dirty: false,
            events: Vec::new(),
            event_marker: 15,
            quick_event: None,
            event_instance: Vec::new(),
        };

        let handle = MapHandle {
            event_sender: tx,
            snapshot,
        };

        (map, handle)
    }

    /// Blocks until this operation is cancelled.
    pub async fn start(&mut self) {
        let (dev_tx, mut dev_rx) = mpsc::channel(10);

        self.device_manager.register(dev_tx);

        let period = Duration::from_millis(10);
        let mut event_interval = interval(period);
        let mut last_tick = tokio::time::Instant::now();
        let mut cleanups = 0;

        let mut stats = LoopStats::default();
        let mut last_report = Instant::now();
        let mut cache = Cache::new(&get_config().vrc_settings);
        loop {
            // sample backlog before we block on select
            let backlog = self.event_recv.len();
            if backlog > stats.max_backlog {
                stats.max_backlog = backlog;
            }

            tokio::select! {
                msg = dev_rx.recv() => {
                    let t = Instant::now();
                    handle_device(self, msg);
                    stats.device.record(t.elapsed().as_nanos());
                }
                instant = event_interval.tick() => {
                    let diff = instant.duration_since(last_tick);
                    if diff > period + Duration::from_millis(2) {
                        log::warn!("Late event Tick: {:?}", diff);
                    }

                    if self.map_dirty || cleanups == 10 {
                        let cfg = cache.load();
                        let t = Instant::now();
                        self.update_devices(cfg);
                        self.map_dirty = false;
                        stats.dirty.record(t.elapsed().as_nanos());
                        cleanups = 0
                    } else {
                        cleanups += 1
                    }

                    if !self.event_instance.is_empty() {
                        let t = Instant::now();
                        handle_events(self).await;
                        stats.events.record(t.elapsed().as_nanos());

                    }
                    last_tick = instant;
                }
                msg = self.event_recv.recv() => {
                    let t = Instant::now();
                    handle_msg(self, msg);
                    stats.msg.record(t.elapsed().as_nanos());
                }
            }

            #[cfg(debug_assertions)]
            if last_report.elapsed() >= Duration::from_secs(1) {
                // log::info!(
                //     "[map] msg/s={} (avg {:.1}us max {:.1}us) | events/s={} (avg {:.1}us max {:.1}us) | dirty/s={} (avg {:.1}us) | dev/s={} | backlog_max={}",
                //     stats.msg.count, stats.msg.avg_us(), stats.msg.max_ns as f64 / 1000.0,
                //     stats.events.count, stats.events.avg_us(), stats.events.max_ns as f64 / 1000.0,
                //     stats.dirty.count, stats.dirty.avg_us(),
                //     stats.device.count,
                //     stats.max_backlog,
                // );
                stats = LoopStats::default();
                last_report = Instant::now();
            }
        }

        async fn handle_events(map: &mut InputMap) {
            /// Frees the transient key list and the InputNodes it owns.
            fn drop_instance(nodes: &mut Nodes, id: &EventId) {
                let Some(keys) = nodes.transient.remove(id) else {
                    return;
                };
                for key in keys {
                    nodes.nodes.remove(key);
                }
            }

            let instant = map.now;
            let nodes = &mut map.input_nodes;
            let events = &map.events;
            let mut changed = false;

            map.event_instance.retain(|(start, key, id)| {
                let Some(event) = events.get(key.0) else {
                    log::error!("Unable to find event definition for instance {}", id);
                    drop_instance(nodes, id);
                    changed = true;
                    return false;
                };

                // Not started yet. This also stops the u64 underflow on the first tick.
                if instant < *start {
                    return true;
                }

                let elapsed = instant - *start;
                if elapsed >= event.duration {
                    drop_instance(nodes, id);
                    changed = true;
                    return false;
                }

                let Some(node_keys) = nodes.transient.get(id) else {
                    log::error!("Nodes not present for event instance {}", id);
                    changed = true;
                    return false; // drop the instance instead of logging it every tick.
                };

                event.tick(&mut nodes.nodes, node_keys, elapsed);
                true
            });

            map.now += 1;
            map.map_dirty = true;
            if changed {
                map.dirty_since_snap = true;
            }
        }

        fn handle_device(map: &mut InputMap, msg: Option<DeviceOutEvents>) {
            if let Some(e) = msg { match e {
                DeviceOutEvents::DeviceInfoDirty(id) => {
                    handle_dirty_info(id, &map.device_manager, &mut map.devices)
                }
                DeviceOutEvents::NewDevice(id) => {
                    let Some(buf) = map
                        .device_manager
                        .with_device(&id, |d| d.get_feedback_buffer())
                    else {
                        log::warn!("Could not find new device: {id:?}");
                        return;
                    };
                    let Some(info) = map.device_manager.with_device(&id, |f| f.info()) else {
                        // This is actually the most common case with wifi devices
                        log::trace!("Could not find info for new device: {id:?}");
                        return;
                    };

                    map.devices.push(MappingDevice {
                        id,
                        outputs: buf,
                        nodes: info.get_nodes().to_vec(),
                    });
                }
                DeviceOutEvents::RemovedDevice(id) => {
                    map.devices.retain(|d| d.id != id);
                }
            } }
        }

        fn handle_msg(map: &mut InputMap, msg: Option<InputEventMessage>) {
            fn register_event(map: &mut InputMap, event: Event) -> EventKey {
                let idx = map.events.len();
                map.events.push(event);
                EventKey(idx)
            }

            fn start_event(map: &mut InputMap, key: EventKey) {
                let Some(event) = map.events.get(key.0) else {
                    log::error!("Unable to find event");
                    return;
                };

                if event.frames.nodes.is_empty() {
                    log::error!("Event '{}' has no frames", event.name);
                    return;
                }

                let id = map.event_marker;
                map.event_marker += 1;

                let now = std::time::Instant::now();
                let mut keys: Vec<NodeKey> = Vec::with_capacity(event.frames.nodes.len());

                for steps in event.frames.nodes.iter() {
                    // Const fields never come back from Steps::get, so seed the node here.
                    let mut slot = Slot {
                        muted: false,
                        source: InputType::Weight, // SET THIS: the InputType variant for scripted/event input.
                        layer: InputLayer::Additive,
                        weight: *steps.weight.first(),
                        history: History::new(),
                    };
                    slot.history.push_at(*steps.value.first(), now);

                    let mut node = InputNode::new(
                        *steps.position.first(),
                        NodeGroup::All,
                        InterpolationLayer::Default,
                        smallvec::smallvec![slot],
                        *steps.radius.first(),
                    );
                    node.muted = *steps.muted.first();

                    keys.push(map.input_nodes.nodes.insert(node));
                }

                map.input_nodes.transient.insert(id, keys);
                map.event_instance.push((map.now, key, id));
                map.map_dirty = true;
                map.dirty_since_snap = true;
            }

            match msg {
                Some(msg) => match msg {
                    InputEventMessage::MapDirty => {
                        map.map_dirty = true;
                    }
                    InputEventMessage::CancelEvents => {
                        map.input_nodes.transient.clear();
                        map.event_instance.clear();
                        map.dirty_since_snap = true;
                        map.map_dirty = true;
                    }
                    InputEventMessage::Flush => {
                        map.dirty_since_snap = true;
                        map.input_nodes.transient.clear();
                        map.input_nodes.nodes.clear();
                        map.input_nodes.active_streaming.clear();
                        map.events.clear();
                        map.generation += 1;
                        map.quick_event = None;
                        map.map_dirty = true;
                    }
                    InputEventMessage::RequestInfo { reply } => {
                        if map.dirty_since_snap {
                            map.snapshot.swap(Arc::new(Snapshot {
                                instant: map.now,
                                events: map.events.clone(),
                                active_events: map.event_instance.clone(),
                                input_nodes: map.input_nodes.clone(),
                            }));
                            log_err!(reply.send(()));
                            map.dirty_since_snap = false;
                        } else {
                            log_err!(reply.send(()));
                        }
                    }
                    InputEventMessage::Register { node, reply } => {
                        let key = map.input_nodes.nodes.insert(*node);
                        map.input_nodes.active_streaming.push(key);
                        map.dirty_since_snap = true;
                        log_err!(reply.send(key));
                        map.map_dirty = true;
                    }
                    InputEventMessage::UpdateNode {
                        key,
                        muted,
                        location,
                        radius,
                    } => {
                        let Some(node) = map.input_nodes.nodes.get_mut(key) else {
                            log::error!("Unable to find node with key");
                            return; // TODO: Replace with continue when drain introduced.
                        };

                        node.mute(muted);
                        node.location = location;
                        node.radius = radius;
                        map.dirty_since_snap = true;
                        map.map_dirty = true;
                    }
                    InputEventMessage::UpdateSlot {
                        key,
                        value,
                        weight,
                        muted,
                    } => {
                        let Some(node) = map.input_nodes.nodes.get_mut(key.node) else {
                            log::error!("Unable to find node with key (slot)");
                            return; // TODO: Replace with continue when drain introduced.
                        };

                        // check on node insert that it has at least one slot.
                        if let Some(slot) = node.slots.get_mut(key.slot_idx as usize) {
                            map.dirty_since_snap = true;
                            slot.history.push_at(value, std::time::Instant::now());
                            slot.weight = weight;
                            slot.muted = muted;
                            map.map_dirty = true;
                        } else {
                            log::error!("unable to find Slot #{} on key", key.slot_idx);
                        }
                    }
                    InputEventMessage::UpdateNodeField { key, field } => {
                        let Some(node) = map.input_nodes.nodes.get_mut(key) else {
                            log::error!("Unable to find node with key");
                            return; // TODO: Replace with continue when drain introduced.
                        };

                        match field {
                            NodeField::Location(l) => node.location = l,
                            NodeField::Muted(m) => node.mute(m),
                            NodeField::Radius(r) => node.radius = r,
                        }
                        map.map_dirty = true;
                        map.dirty_since_snap = true;
                    }
                    InputEventMessage::UpdateSlotField { key, field } => {
                        let Some(node) = map.input_nodes.nodes.get_mut(key.node) else {
                            log::error!("Unable to find node with key");
                            return; // TODO: Replace with continue when drain introduced.
                        };

                        // check on node insert that it has at least one slot.
                        if let Some(slot) = node.slots.get_mut(key.slot_idx as usize) {
                            match field {
                                SlotField::Muted(m) => slot.muted = m,
                                SlotField::Value(v) => {
                                    slot.history.push_at(v, std::time::Instant::now())
                                }
                                SlotField::Weight(w) => slot.weight = w,
                            }
                            map.map_dirty = true;
                            map.dirty_since_snap = true;
                        } else {
                            log::error!("unable to find Slot #{} on key", key.slot_idx);
                        }
                    }
                    InputEventMessage::BatchUpdate(batch) => handle_batch(map, &batch),
                    InputEventMessage::StartEvent(e) => {
                        start_event(map, e);
                    }
                    InputEventMessage::RegisterEvent { event, reply } => {
                        let key = register_event(map, *event);
                        log_err!(reply.send(key));
                    }
                    InputEventMessage::QuickEvent {
                        duration,
                        power,
                        location,
                    } => {
                        let duration = duration.max(1);
                        let key = match map.quick_event.clone() {
                            Some(key) => {
                                let Some(event) = map.events.get_mut(key.0) else {
                                    log::error!("Unable to retrieve event key");
                                    return;
                                };
                                event.duration = duration;
                                let Some(slot) = event.frames.nodes.get_mut(0) else {
                                    log::error!("QuickTrigger event has no frames");
                                    return;
                                };
                                slot.position = location.into();
                                slot.value = power.into();
                                key
                            }
                            None => {
                                let step = Steps {
                                    position: location.into(),
                                    muted: false.into(),
                                    value: power.into(),
                                    weight: 1.0.into(),
                                    radius: 0.0375.into(),
                                };

                                let event = Event {
                                    name: "QuickTrigger".to_string(),
                                    frames: Frames::new(vec![step]),
                                    duration,
                                };

                                let key = register_event(map, event);
                                map.quick_event = Some(key.clone());
                                key
                            }
                        };

                        start_event(map, key);
                    }
                },
                None => {
                    log::warn!("All channels dropped for map input. Restart required");
                }
            }
        }
    }

    /// pushes updates from map to devices, using our intermediary devices.
    fn update_devices(&mut self, cfg: &Arc<VrcSettings>) {
        // Calculate internal node values from slots.
        self.input_nodes.update_nodes(cfg);

        let _: () = self
            .devices
            .par_iter()
            .map(|d| {
                // could be done in parallel here. but few devices means not efficient (probably)
                let (_, settings) = state::get_device(&d.id);
                d.update_buffer(&self.input_nodes, &settings.load());
                self.device_manager
                    .with_device(&d.id, |d| d.buffer_updated());
            })
            .collect();
    }
}

fn handle_batch(map: &mut InputMap, msgs: &[BatchUpdateMsg]) {
    for msg in msgs {
        match msg {
            BatchUpdateMsg::Node {
                key,
                muted,
                location,
                radius,
            } => {
                let Some(node) = map.input_nodes.nodes.get_mut(*key) else {
                    log::error!("Unable to find node with key");
                    continue; // TODO: Replace with continue when drain introduced.
                };

                node.mute(*muted);
                node.location = *location;
                node.radius = *radius;
                map.dirty_since_snap = true;
                map.map_dirty = true;
            }
            BatchUpdateMsg::Slot {
                key,
                value,
                weight,
                muted,
            } => {
                let Some(node) = map.input_nodes.nodes.get_mut(key.node) else {
                    log::error!("Unable to find node with key (slot)");
                    continue; // TODO: Replace with continue when drain introduced.
                };

                // check on node insert that it has at least one slot.
                if let Some(slot) = node.slots.get_mut(key.slot_idx as usize) {
                    map.dirty_since_snap = true;
                    slot.history.push_at(*value, std::time::Instant::now());
                    slot.weight = *weight;
                    slot.muted = *muted;
                    map.map_dirty = true;
                } else {
                    log::error!("unable to find Slot #{} on key", key.slot_idx);
                }
            }
            BatchUpdateMsg::NodeField { key, field } => {
                let Some(node) = map.input_nodes.nodes.get_mut(*key) else {
                    log::error!("Unable to find node with key");
                    continue; // TODO: Replace with continue when drain introduced.
                };

                match field {
                    NodeField::Location(l) => node.location = *l,
                    NodeField::Muted(m) => node.mute(*m),
                    NodeField::Radius(r) => node.radius = *r,
                }
                map.map_dirty = true;
                map.dirty_since_snap = true;
            }
            BatchUpdateMsg::SlotField { key, field } => {
                let Some(node) = map.input_nodes.nodes.get_mut(key.node) else {
                    log::error!("Unable to find node with key");
                    continue; // TODO: Replace with continue when drain introduced.
                };

                // check on node insert that it has at least one slot.
                if let Some(slot) = node.slots.get_mut(key.slot_idx as usize) {
                    match field {
                        SlotField::Muted(m) => slot.muted = *m,
                        SlotField::Value(v) => slot.history.push_at(*v, std::time::Instant::now()),
                        SlotField::Weight(w) => slot.weight = *w,
                    }
                    map.map_dirty = true;
                    map.dirty_since_snap = true;
                } else {
                    log::error!("unable to find Slot #{} on key", key.slot_idx);
                }
            }
        }
    }
}

/// pull dirty info from individual devices
fn handle_dirty_info(id: DeviceId, dev: &DeviceHandle, devices: &mut Vec<MappingDevice>) {
    if let Some(info) = dev.with_device(&id, |d| d.info()) {
        match info {
            DeviceInfo::Websocket(w) => {
                let Some(device) = devices.iter_mut().find(|d| d.id == id) else {
                    // if device not found on our list, just continue.
                    return;
                };

                device.nodes = w.nodes;
                let out_len = device.outputs.read().len();
                if device.nodes.len() != out_len {
                    log::error!("Output buffer not same length on device: {}", w.name);
                }
            }
            DeviceInfo::Wifi(i) => {
                let Some(device) = devices.iter_mut().find(|d| d.id == id) else {
                    // if device not found on our list, just continue.
                    return;
                };
                device.nodes = i.nodes;
                let out_len = device.outputs.read().len();
                if device.nodes.len() != out_len {
                    log::error!("Output buffer not same length on device: {}", i.mac);
                }
            }
            DeviceInfo::BhapticBle(i) => {
                let Some(device) = devices.iter_mut().find(|d| d.id == id) else {
                    // if device not found on our list, just continue.
                    return;
                };
                device.nodes = i.nodes;
                let out_len = device.outputs.read().len();
                if device.nodes.len() != out_len {
                    log::error!("Output buffer not same length on device: {:?}", i.id);
                }
            }
            DeviceInfo::Internal(i) => {
                let Some(device) = devices.iter_mut().find(|d| d.id == id) else {
                    // if device not found on our list, just continue.
                    return;
                };
                device.nodes = i.nodes;
                let out_len = device.outputs.read().len();
                if device.nodes.len() != out_len {
                    log::error!("Output buffer not same length on internal device");
                }
            }
        }
    }
}

//const _: [u8; std::mem::size_of::<BatchUpdateMsg>()] = [];

pub enum InputEventMessage {
    /// Treats the map as if it has been changed on next map tick.
    MapDirty,
    /// increments generation, flushes all nodes.
    /// USE WITH CAUTION. INVALIDATES ALL nodeKeys, SlotKeys, and EventKeys
    Flush,
    /// updates the snapshot info if dirty since last call.
    RequestInfo {
        reply: oneshot::Sender<()>,
    },
    /// Sets node with `NodeId`'s intensity, and radius.
    Register {
        node: Box<InputNode>,
        reply: oneshot::Sender<NodeKey>,
    },
    UpdateNode {
        key: NodeKey,
        muted: bool,
        location: Vec3,
        radius: f32,
    },
    UpdateSlot {
        key: SlotKey,
        value: f32,
        weight: f32,
        muted: bool,
    },
    UpdateSlotField {
        key: SlotKey,
        field: SlotField,
    },
    UpdateNodeField {
        key: NodeKey,
        field: NodeField,
    },
    BatchUpdate(Box<Vec<BatchUpdateMsg>>),
    /// Register events for use in this map epoch
    RegisterEvent {
        event: Box<Event>,
        reply: oneshot::Sender<EventKey>,
    },
    /// Call a registered event.
    StartEvent(EventKey),
    QuickEvent {
        duration: u64,
        power: f32,
        location: Vec3,
    },
    CancelEvents,
}

pub enum BatchUpdateMsg {
    Node {
        key: NodeKey,
        muted: bool,
        location: Vec3,
        radius: f32,
    },
    Slot {
        key: SlotKey,
        value: f32,
        weight: f32,
        muted: bool,
    },
    SlotField {
        key: SlotKey,
        field: SlotField,
    },
    NodeField {
        key: NodeKey,
        field: NodeField,
    },
}

#[derive(EnumDiscriminants)]
#[strum_discriminants(derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", strum_discriminants(derive(specta::Type)))]
#[strum_discriminants(name(NodeFieldKind))]
pub enum NodeField {
    Muted(bool),
    Location(Vec3),
    Radius(f32),
}

#[derive(EnumDiscriminants)]
#[strum_discriminants(name(SlotFieldKind))]
#[cfg_attr(feature = "specta", strum_discriminants(derive(specta::Type)))]
#[strum_discriminants(derive(serde::Serialize, serde::Deserialize))]
pub enum SlotField {
    Value(f32),
    Weight(f32),
    Muted(bool),
}
