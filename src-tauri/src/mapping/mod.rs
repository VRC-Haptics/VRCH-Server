pub mod event;
//pub mod global_map;
pub mod haptic_node;
pub mod input_node;
pub mod interp;

use crate::log_err;
use parking_lot::{Mutex, RwLock};
use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};
use tokio::sync::{
    mpsc::{
        self,
        error::{SendError, TrySendError},
    },
    Notify,
};

use event::Event;
use haptic_node::HapticNode;
use input_node::InputNode;
use interp::Interpolate;
use uuid::Uuid;

use crate::{
    devices::{Device, DeviceHandle, DeviceId, DeviceInfo, DeviceOutEvents},
    state::{self, PerDevice},
    util::math::Vec3,
};

/// Snapshot of map state.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, specta::Type)]
pub struct MapInfo {
    nodes: Vec<InputNode>,
    events: Vec<Event>,
}

/// Implements cheap clone, can be shared between threads safely.
pub struct MapHandle {
    event_sender: mpsc::Sender<InputEventMessage>,
    input_nodes: Arc<RwLock<Vec<InputNode>>>,
    active_events: Arc<RwLock<Vec<Event>>>,
    map_dirty: Arc<Notify>,
}

impl MapHandle {
    /// marks that some change has been made and should be propogated to devices
    pub fn mark_dirty(&self) {
        self.map_dirty.notify_one();
    }

    /// clones snapshot of map state
    pub fn get_state(&self) -> MapInfo {
        let nodes = self.input_nodes.read().clone();
        let events = self.active_events.read().clone();
        MapInfo {
            nodes: nodes,
            events: events,
        }
    }

    pub async fn send_event(
        &self,
        msg: InputEventMessage,
    ) -> Result<(), SendError<InputEventMessage>> {
        self.event_sender.send(msg).await
    }

    pub fn send_event_blocking(
        &self,
        msg: InputEventMessage,
    ) -> Result<(), TrySendError<InputEventMessage>> {
        self.event_sender.try_send(msg)
    }

    /// collects the outputs of function f, for each node that has the given tag
    ///
    /// Similar to `has_tag`
    pub fn has_tag_mut<F, T>(&self, tag: &String, fun: F) -> Vec<T>
    where
        F: Fn(&mut InputNode) -> T,
    {
        let mut gather = vec![];
        let mut nodes = self.input_nodes.write();
        for node in nodes.iter_mut() {
            if node.tags.contains(tag) {
                gather.push(fun(node));
            }
        }
        gather
    }

    /// collects the outputs of function f, for each node that has the given tag
    pub fn has_tag<F, T>(&self, tag: String, fun: F) -> Vec<T>
    where
        F: Fn(&InputNode) -> T,
    {
        let mut gather = vec![];
        let nodes = self.input_nodes.read();
        for node in nodes.iter() {
            if node.tags.contains(&tag) {
                gather.push(fun(node));
            }
        }
        gather
    }

    /// Performs function f on input node with id: `id`
    pub fn with_node<F, T>(&self, id: &NodeId, f: F) -> Option<T>
    where
        F: FnOnce(&InputNode) -> T,
    {
        let nodes = self.input_nodes.read();
        let node = nodes.iter().find(|n| *n.get_id() == *id)?;
        Some(f(node))
    }

    /// Same as `with_node` but with a mutable reference.
    ///
    /// This does take a mutable write and locks the entire map list.
    /// Spamming this function is not desireable.
    pub fn with_node_mut<F, T>(&self, id: &NodeId, f: F) -> Option<T>
    where
        F: FnOnce(&mut InputNode) -> T,
    {
        let mut nodes = self.input_nodes.write();
        let node = nodes.iter_mut().find(|n| *n.get_id() == *id)?;
        Some(f(node))
    }
}

impl Clone for MapHandle {
    /// Cheap clone, ideally not every itteration but not expensive either.
    fn clone(&self) -> Self {
        Self {
            event_sender: self.event_sender.clone(),
            input_nodes: Arc::clone(&self.input_nodes),
            active_events: Arc::clone(&self.active_events),
            map_dirty: self.map_dirty.clone(),
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
    pub fn update_buffer(&self, in_nodes: &Vec<InputNode>, settings: &PerDevice) {
        let mut buf = self.outputs.write();
        if buf.len() != self.nodes.len() {
            log::trace!(
                "Buffer not equal node length: buf:{}, nodes: {}",
                buf.len(),
                self.nodes.len()
            );
            return;
        }
        settings.interp_algo.interp(&self.nodes, &mut buf, in_nodes, settings);
    }
}

pub async fn start_interp_map(manager: &DeviceHandle) -> MapHandle {
    let (mut input_map, map_handle) = InputMap::new(manager.clone()).await;
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
    /// Needs to be shareable so that events can be ticked asyncrhonously.
    active_events: Arc<RwLock<Vec<Event>>>,
    input_nodes: Arc<RwLock<Vec<InputNode>>>,
    manager: DeviceHandle,
    devices: Arc<Mutex<Vec<MappingDevice>>>,
    event_recv: mpsc::Receiver<InputEventMessage>,
    event_send: mpsc::Sender<InputEventMessage>,
    /// Whether input mapping has changed in a way that should require device output updates
    map_dirty: Arc<Notify>,
}

impl InputMap {
    /// Assumes `DeviceManager` has been intialized.
    pub async fn new(manager: DeviceHandle) -> (Self, MapHandle) {
        let (tx, rx) = mpsc::channel(10);
        let input_nodes = Arc::new(RwLock::new(Vec::new()));
        let events = Arc::new(RwLock::new(Vec::new()));
        let dirty_flag = Arc::new(Notify::new());

        let map = Self {
            active_events: events.clone(),
            input_nodes: Arc::clone(&input_nodes),
            manager: manager,
            devices: Arc::new(Mutex::new(Vec::new())),
            event_recv: rx,
            event_send: tx.clone(),
            map_dirty: Arc::clone(&dirty_flag),
        };

        let handle = MapHandle {
            event_sender: tx,
            input_nodes: input_nodes,
            active_events: events,
            map_dirty: dirty_flag,
        };

        return (map, handle);
    }

    /// Blocks until this operation is cancelled.
    pub async fn start(&mut self) {
        let (dev_tx, mut dev_rx) = mpsc::channel(10);

        // handle messages about devices being added/removed/changed
        let man_clone = self.manager.clone();
        let devices_clone = Arc::clone(&self.devices);
        tokio::spawn(async move {
            loop {
                match dev_rx.recv().await {
                    Some(e) => match e {
                        DeviceOutEvents::DeviceInfoDirty(id) => {
                            handle_dirty_info(id, &man_clone, &devices_clone)
                        }
                        DeviceOutEvents::NewDevice(id) => {
                            let mut devices = devices_clone.lock();
                            let Some(buf) = man_clone.with_device(&id, |d| d.get_feedback_buffer())
                            else {
                                log::warn!("Could not find new device: {id:?}");
                                drop(devices);
                                continue;
                            };
                            let Some(info) = man_clone.with_device(&id, |f| f.info()) else {
                                // This is actually the most common case with wifi devices
                                log::trace!("Could not find info for new device: {id:?}");
                                drop(devices);
                                continue;
                            };

                            devices.push(MappingDevice {
                                id: id,
                                outputs: buf,
                                nodes: info.get_nodes().to_vec(),
                            });
                        }
                        DeviceOutEvents::RemovedDevice(id) => {
                            let mut devices = devices_clone.lock();
                            devices.retain(|d| d.id != id);
                        }
                    },
                    None => {}
                }
            }
        });

        // handle our 100hz event ticks
        let events = self.active_events.clone();
        let in_nodes = self.input_nodes.clone();
        let dirty_event_clone = self.map_dirty.clone();
        tokio::spawn(async move {
            loop {
                {
                    let mut nodes = in_nodes.write();
                    let mut events = events.write();
                    events.retain_mut(|event| {
                        let finished = event.tick(&mut nodes);
                        !finished
                    });
                }
                dirty_event_clone.notify_one();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        // register for device events last to hopefully stop big race conditions.
        self.manager.register(dev_tx);

        loop {
            tokio::select! {
                msg = self.event_recv.recv() => {
                    match msg {
                        Some(msg) => match msg {
                            InputEventMessage::InsertNode(node) => {
                                let mut nodes = self.input_nodes.write();
                                if !nodes.iter().any(|f| f.get_id() == node.get_id()) {
                                    nodes.push(node);
                                }
                            }
                            InputEventMessage::UpdateNode(id, int, radius) => {
                                let mut nodes = self.input_nodes.write();
                                let Some(node) = nodes.iter_mut().find(|d| *d.get_id() == id) else {
                                    log::warn!("Tried to update node that doesn't exist with id: {id:?}");
                                    return;
                                };
                                node.intensity = int.unwrap_or(node.intensity);
                                node.radius = radius.unwrap_or(node.radius);
                            }
                            InputEventMessage::RemoveWithTags(tags) => {
                                let mut nodes = self.input_nodes.write();
                                for tag in tags {
                                    nodes.retain(|n| !n.tags.contains(&tag));
                                }
                            }
                            InputEventMessage::StartEvent(e) => self.start_event(e),
                            InputEventMessage::StartEvents(mut e) => self.start_events(&mut e),
                            InputEventMessage::CancelAllWithTags(t) => {
                                let num = self.cancel_tags(&t);
                                log::trace!("Canceled {num} events with tags: {:?}", t);
                            }
                        },
                        None => {
                            log::warn!("All channels dropped for map input. Restart required");
                            break;
                        }
                    }
                }
                _ = self.map_dirty.notified() => {
                    self.update_devices();
                }
            }
        }
    }

    /// pushes updates from map to devices
    fn update_devices(&self) {
        let devices = self.devices.lock();
        let in_nodes = self.input_nodes.read();
        for device in devices.iter() {
            // could be done in parallel here. but few devices means not effeicnet (probably)
            let (_, settings) = state::get_device(&device.id);
            device.update_buffer(&in_nodes, &settings.load());
            self.manager.with_device(&device.id, |d| d.buffer_updated());
        }
    }

    fn cancel_tags(&mut self, tags: &Vec<String>) -> usize {
        let mut events = self.active_events.write();
        let num = events.len();
        for tag in tags {
            events.retain(|e| e.tags.contains(&tag));
        }
        num - events.len()
    }

    /// Start a singular input event.
    fn start_event(&mut self, event: Event) {
        let mut lock = self.active_events.write();
        lock.push(event);
    }

    /// Start a list of events, consumes the events vector.
    fn start_events(&mut self, events: &mut Vec<Event>) {
        let mut lock = self.active_events.write();
        lock.append(events);
    }
}

/// pull dirty info from individual devices
fn handle_dirty_info(id: DeviceId, dev: &DeviceHandle, devices: &Mutex<Vec<MappingDevice>>) {
    if let Some(info) = dev.with_device(&id, |d| d.info()) {
        match info {
            DeviceInfo::Wifi(i) => {
                let mut lock = devices.lock();
                let Some(device) = lock.iter_mut().find(|d| d.id == id) else {
                    // if device not found on our list, just continue.
                    return;
                };
                device.nodes = i.nodes;
                let out_len = device.outputs.read().len();
                if device.nodes.len() != out_len {
                    log::error!("Output buffer not same length on device: {}", i.mac);
                }
            },
            DeviceInfo::BhapticBle(i) => {
                let mut lock = devices.lock();
                let Some(device) = lock.iter_mut().find(|d| d.id == id) else {
                    // if device not found on our list, just continue.
                    return;
                };
                device.nodes = i.nodes;
                let out_len = device.outputs.read().len();
                if device.nodes.len() != out_len {
                    log::error!("Output buffer not same length on device: {:?}", i.id);
                }
            }
        }
    }
}

pub enum InputEventMessage {
    /// Sets node with `NodeId`'s intensity, and radius.
    UpdateNode(NodeId, Option<f32>, Option<f32>),
    InsertNode(InputNode),
    /// Removes all `InputNodes` with tags. This includes all input nodes created by events.
    RemoveWithTags(Vec<String>),
    StartEvent(Event),
    StartEvents(Vec<Event>),
    /// cancels all events with tags in string
    CancelAllWithTags(Vec<String>),
}

/// Descriptors for location groups.
/// Allows for segmented Interpolation
#[derive(
    PartialEq, serde::Deserialize, serde::Serialize, Clone, Debug, strum::EnumIter, specta::Type,
)]
pub enum NodeGroup {
    Head,
    UpperArmRight,
    UpperArmLeft,
    LowerArmRight,
    LowerArmLeft,
    TorsoRight,
    TorsoLeft,
    TorsoFront,
    TorsoBack,
    UpperLegRight,
    UpperLegLeft,
    LowerLegRight,
    LowerLegLeft,
    FootRight,
    FootLeft,
    /// A meta tag reserved for in-server use only.
    /// Should not be exported to devices or imported from games.
    All,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, specta::Type)]
/// Id unique to the node it references.
/// if an Id is equal, it is garunteed to be the same HapticNode, with location in space and tags
pub struct NodeId(pub String);

impl PartialEq<str> for NodeId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl Into<String> for NodeId {
    fn into(self) -> String {
        self.0
    }
}

impl From<String> for NodeId {
    fn from(value: String) -> Self {
        NodeId(value)
    }
}

impl NodeId {
    pub fn new() -> Self {
        NodeId(Uuid::new_v4().to_string())
    }
}

impl NodeGroup {
    /// maps node groups into two points defining an axis that runs through the center of the model.
    /// See NodeGroupPoints in standard unity project.
    pub fn to_points(&self) -> (Vec3, Vec3) {
        // helper: flip the sign of the X component for both points
        fn mirror_x(p: (Vec3, Vec3)) -> (Vec3, Vec3) {
            let (a, b) = p;
            (Vec3::new(-a.x, a.y, a.z), Vec3::new(-b.x, b.y, b.z))
        }

        return match self {
            NodeGroup::TorsoRight
            | NodeGroup::TorsoLeft
            | NodeGroup::TorsoFront
            | NodeGroup::TorsoBack => (
                Vec3::new(0., 0.735000014, -0.00800000038),
                Vec3::new(0., 1.43400002, -0.0130000003),
            ),
            NodeGroup::Head => (
                Vec3::new(0., 1.70700002, 0.0529999994),
                Vec3::new(0., 1.43400002, -0.0130000003),
            ),
            NodeGroup::UpperArmRight => (
                Vec3::new(0.172999993, 1.35599995, -0.0260000005),
                Vec3::new(0.336199999, 1.15139997, -0.0151000004),
            ),
            NodeGroup::LowerArmRight => (
                Vec3::new(0.336199999, 1.14470005, -0.0244999994),
                Vec3::new(0.4736, 0.944899976, 0.0469000004),
            ),
            NodeGroup::UpperLegRight => (
                Vec3::new(0.0689999983, 0.921999991, 0.00100000005),
                Vec3::new(0.134000003, 0.479000002, -0.0280000009),
            ),
            NodeGroup::LowerLegRight => (
                Vec3::new(0.134000003, 0.479000002, -0.0280000009),
                Vec3::new(0.173999995, 0.0879999995, -0.0729999989),
            ),
            NodeGroup::FootRight => (
                Vec3::new(0.173999995, 0.0879999995, -0.0729999989),
                Vec3::new(0.226300001, 0.0199999996, 0.0320000015),
            ),
            NodeGroup::UpperArmLeft => mirror_x(NodeGroup::UpperArmRight.to_points()),
            NodeGroup::LowerArmLeft => mirror_x(NodeGroup::LowerArmRight.to_points()),
            NodeGroup::UpperLegLeft => mirror_x(NodeGroup::UpperLegRight.to_points()),
            NodeGroup::LowerLegLeft => mirror_x(NodeGroup::LowerLegRight.to_points()),
            NodeGroup::FootLeft => mirror_x(NodeGroup::FootRight.to_points()),
            NodeGroup::All => (Vec3::new(0., 0., 0.), Vec3::new(0., 0., 0.)),
        };
    }

    /// Given a string containing at least 2 raw bytes, interpret the first two bytes as
    /// a little-endian u16 bitflag and convert that into a Vec<NodeGroup>.
    pub fn parse_from_str(s: &str) -> Vec<NodeGroup> {
        let bytes = s.as_bytes();
        if bytes.len() < 2 {
            // Not enough data; return an empty vector
            return Vec::new();
        }
        let flag = u16::from_le_bytes([bytes[0], bytes[1]]);
        Self::from_bitflag(flag)
    }

    /// Converts a slice of NodeGroup into a bitflag.
    pub fn to_bitflag(groups: &[NodeGroup]) -> u16 {
        let mut flag: u16 = 0;
        for group in groups {
            flag |= match group {
                NodeGroup::Head => 1 << 0,
                NodeGroup::UpperArmRight => 1 << 1,
                NodeGroup::UpperArmLeft => 1 << 2,
                NodeGroup::TorsoRight => 1 << 3,
                NodeGroup::TorsoLeft => 1 << 4,
                NodeGroup::TorsoFront => 1 << 5,
                NodeGroup::TorsoBack => 1 << 6,
                NodeGroup::UpperLegRight => 1 << 7,
                NodeGroup::UpperLegLeft => 1 << 8,
                NodeGroup::FootRight => 1 << 9,
                NodeGroup::FootLeft => 1 << 10,
                NodeGroup::LowerArmRight => 1 << 11,
                NodeGroup::LowerArmLeft => 1 << 12,
                NodeGroup::LowerLegRight => 1 << 13,
                NodeGroup::LowerLegLeft => 1 << 14,
                NodeGroup::All => 0,
            }
        }
        flag
    }

    /// Converts a bitflag back into a vector of NodeGroup variants.
    pub fn from_bitflag(flag: u16) -> Vec<NodeGroup> {
        let mut groups = Vec::new();
        if flag & (1 << 0) != 0 {
            groups.push(NodeGroup::Head);
        }
        if flag & (1 << 1) != 0 {
            groups.push(NodeGroup::UpperArmRight);
        }
        if flag & (1 << 2) != 0 {
            groups.push(NodeGroup::UpperArmLeft);
        }
        if flag & (1 << 3) != 0 {
            groups.push(NodeGroup::TorsoRight);
        }
        if flag & (1 << 4) != 0 {
            groups.push(NodeGroup::TorsoLeft);
        }
        if flag & (1 << 5) != 0 {
            groups.push(NodeGroup::TorsoFront);
        }
        if flag & (1 << 6) != 0 {
            groups.push(NodeGroup::TorsoBack);
        }
        if flag & (1 << 7) != 0 {
            groups.push(NodeGroup::UpperLegRight);
        }
        if flag & (1 << 8) != 0 {
            groups.push(NodeGroup::UpperLegLeft);
        }
        if flag & (1 << 9) != 0 {
            groups.push(NodeGroup::FootRight);
        }
        if flag & (1 << 10) != 0 {
            groups.push(NodeGroup::FootLeft);
        }
        if flag & (1 << 11) != 0 {
            groups.push(NodeGroup::LowerArmRight);
        }
        if flag & (1 << 12) != 0 {
            groups.push(NodeGroup::LowerArmLeft);
        }
        if flag & (1 << 13) != 0 {
            groups.push(NodeGroup::LowerLegRight);
        }
        if flag & (1 << 14) != 0 {
            groups.push(NodeGroup::LowerLegLeft);
        }
        groups
    }
}
