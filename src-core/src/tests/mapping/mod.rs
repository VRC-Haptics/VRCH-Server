mod interp;

use std::time::Instant;

use glam::Vec3;
use smallvec::SmallVec;
use tokio::{sync::oneshot, time::sleep};

use crate::{
    devices::{internal::InternalDevice, Device, DeviceId, HapticDevice},
    mapping::{
        groups::NodeGroup,
        haptic_node::HapticNode,
        input_node::{History, InputNode, InterpolationLayer, Slot, SlotKey},
        InputEventMessage, NodeKey,
    },
    vrc::config::{InputLayer, InputType},
    *,
};

async fn start_map(init_device: bool) -> (MapHandle, DeviceHandle) {
    let mut dev = DeviceManager::new();
    if init_device {
        init_device_manager(&mut dev).await;
    }
    let d_handle = dev.get_handle();
    let map = start_interp_map(d_handle).await;
    (map, dev.get_handle())
}

fn default_node() -> InputNode {
    let slot = Slot {
        muted: false,
        source: InputType::Weight,
        layer: InputLayer::Additive,
        weight: 1.0,
        history: History::new(),
    };
    let mut vec = SmallVec::default();
    vec.push(slot);
    InputNode::new(
        Vec3::new(0.0, 1.0, 0.0),
        NodeGroup::All,
        InterpolationLayer::Default,
        vec,
        1.0,
    )
}

async fn add_node(handle: &MapHandle, node: InputNode) -> NodeKey {
    let (tx, rx) = oneshot::channel();
    handle
        .send_event(mapping::InputEventMessage::Register {
            node: Box::new(node.clone()),
            reply: tx,
        })
        .unwrap();
    let node = rx.await.unwrap();
    node
}

/// Devices must be created AFTER the map has been called.
/// A few ms should be plenty, since it all should be synchronous work.
/// The map has to startup and listen for messages from teh device manager.
async fn device_at(loc: Vec3, id: impl Into<DeviceId>, dev: &DeviceHandle) {
    let dev_ch = dev.get_device_channel();
    let out_node = vec![HapticNode {
        loc: loc,
        groups: NodeGroup::All,
    }];
    dev_ch
        .send(devices::DeviceMessage::Register(HapticDevice::Internal(
            InternalDevice::new(id, out_node),
        )))
        .await
        .unwrap();
}

async fn get_val(id: impl Into<DeviceId>, idx: usize, dev: &DeviceHandle) -> f32 {
    let buf = dev
        .with_device(&id.into(), |d| d.get_feedback_buffer())
        .unwrap();
    let out = buf.read().clone();
    *out.get(idx).unwrap()
}

#[tokio::test]
async fn map_simple() {
    let (map, _) = start_map(false).await;
    let state = map.get_state().await;
    assert_eq!(state.active_events.len(), 0);
    assert_eq!(state.events.len(), 0);
    assert_eq!(state.input_nodes.nodes.len(), 0);
}

#[tokio::test]
async fn map_add_node() {
    let (map, _) = start_map(false).await;

    assert!(map.get_state().await.input_nodes.nodes.len() == 0);

    let node = default_node();
    let key = add_node(&map, node).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let def_node = default_node();
    let _ = *nodes.nodes.get(key).unwrap();
    assert!(nodes.active_streaming.contains(&key));
}

#[tokio::test]
async fn map_mutate_node() {
    let (map, _) = start_map(false).await;

    let node = default_node();
    let key = add_node(&map, node).await;

    let def_node = default_node();
    let muted = !def_node.muted;
    let pos = Vec3::new(1.0, 0.0, 0.0);
    let rad = def_node.radius / 2.;

    map.send_event(InputEventMessage::UpdateNode {
        key,
        muted,
        location: pos,
        radius: rad,
    })
    .unwrap();

    // wait for map to process update (should be a few microseconds ideally)
    sleep(Duration::from_millis(10)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    assert_eq!(node.muted, muted);
    assert_eq!(node.radius, rad);
    assert_eq!(node.location, pos);
}

#[tokio::test]
async fn map_mutate_slot() {
    let (map, _) = start_map(false).await;

    let node = default_node();
    let key = add_node(&map, node).await;

    let def_node = default_node();
    let def_slot = def_node.slots.first().unwrap();

    let val = 1.0;
    let muted = !def_slot.muted;
    let weight = 0.5;

    let slot = SlotKey::new(key, 0);
    map.send_event(InputEventMessage::UpdateSlot {
        key: slot,
        value: val,
        weight,
        muted,
    })
    .unwrap();

    // wait for map to process update (should be a few microseconds ideally)
    sleep(Duration::from_millis(1)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    let slot = node.slots.first().unwrap();
    assert_eq!(slot.history.latest().unwrap(), val);
    assert_eq!(slot.weight, weight);
    assert_eq!(slot.muted, muted);
}

/// Two slots in additive should combine evenly.
#[tokio::test]
async fn map_slot_layer_add() {
    let (map, _) = start_map(false).await;

    let mut node = default_node();
    node.slots.clear();

    let mut half = History::default();
    half.push_at(0.5, Instant::now());
    let new_slot = Slot {
        muted: false,
        source: InputType::Weight,
        layer: InputLayer::Additive,
        weight: 1.0,
        history: half,
    };
    node.slots.push(new_slot.clone());
    node.slots.push(new_slot);

    let key = add_node(&map, node).await;

    let def_node = default_node();
    let def_slot = def_node.slots.first().unwrap();

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    assert!((node.value - 1.0).abs() < 0.001, "Value: {}", node.value);
}

/// Two slots in additive should combine evenly.
#[tokio::test]
async fn map_slot_layer_subtract() {
    let (map, _) = start_map(false).await;

    let mut node = default_node();
    node.slots.clear();

    let mut half = History::default();
    half.push_at(0.5, Instant::now());
    let mut new_slot = Slot {
        muted: false,
        source: InputType::Weight,
        layer: InputLayer::Additive,
        weight: 1.0,
        history: half,
    };
    node.slots.push(new_slot.clone());

    new_slot.layer = InputLayer::Subtractive;
    node.slots.push(new_slot);

    let key = add_node(&map, node).await;

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    assert!((node.value - 0.0).abs() < 0.001, "Value: {}", node.value);
}

/// A slot in multiply multiplies by the additive layer
#[tokio::test]
async fn map_slot_layer_multiply() {
    let (map, _) = start_map(false).await;

    let mut node = default_node();
    node.slots.clear();

    let mut half = History::default();
    half.push_at(0.5, Instant::now());
    let mut new_slot = Slot {
        muted: false,
        source: InputType::Weight,
        layer: InputLayer::Additive,
        weight: 1.0,
        history: half,
    };
    node.slots.push(new_slot.clone());

    new_slot.layer = InputLayer::Multiplicative;
    node.slots.push(new_slot);

    let key = add_node(&map, node).await;

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    assert!((node.value - 0.25).abs() < 0.001, "Value: {}", node.value);
}

#[tokio::test]
async fn map_slot_layer_gate() {
    let (map, _) = start_map(false).await;

    let mut node = default_node();
    node.slots.clear();

    let mut half = History::default();
    half.push_at(0.5, Instant::now());
    let mut new_slot = Slot {
        muted: false,
        source: InputType::Weight,
        layer: InputLayer::Additive,
        weight: 1.0,
        history: half,
    };
    node.slots.push(new_slot.clone());

    new_slot.layer = InputLayer::Gate;
    let mut hist = History::default();
    hist.push_at(0.25, Instant::now());
    new_slot.history = hist;
    node.slots.push(new_slot);

    let key = add_node(&map, node).await;

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    assert!((node.value - 0.25).abs() < 0.001, "Value: {}", node.value);
}

#[tokio::test]
async fn map_slot_layer_max() {
    let (map, _) = start_map(false).await;

    let mut node = default_node();
    node.slots.clear();

    let mut half = History::default();
    half.push_at(0.5, Instant::now());
    let mut new_slot = Slot {
        muted: false,
        source: InputType::Weight,
        layer: InputLayer::Additive,
        weight: 1.0,
        history: half,
    };
    node.slots.push(new_slot.clone());

    new_slot.layer = InputLayer::Max;
    let mut hist = History::default();
    hist.push_at(0.75, Instant::now());
    new_slot.history = hist;
    node.slots.push(new_slot);

    let key = add_node(&map, node).await;

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let state = map.get_state().await;
    let nodes = &state.input_nodes;
    let node = nodes.nodes.get(key).unwrap();
    assert!((node.value - 0.75).abs() < 0.001, "Value: {}", node.value);
}