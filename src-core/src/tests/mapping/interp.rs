use std::{sync::Arc, time::Instant};

use glam::Vec3;
use tokio::{time::sleep};

use super::*;

use crate::{
    mapping::{input_node::InterpolationLayer, interp::InterpState}, state::{PerDevice, update_device}, *
};


#[tokio::test]
async fn map_node_layer_linear() {
    let _ = env_logger::builder().try_init();
    let (map, dev) = start_map(true).await;

    let id: DeviceId = "testing".into();
    let mut node = default_node();
    node.location = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    node.interpolation_layer = InterpolationLayer::Linear;
    node.radius = 2.0;
    let slot = node.slots.first_mut().unwrap();
    slot.history.push_at(1.0, Instant::now());

    let key = add_node(&map, node).await;
    // create device after map is listening.
    device_at(Vec3::ZERO, id.clone(), &dev).await;
    map.mark_dirty();

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let output = get_val(id, 0, &dev).await;

    let state = map.get_state().await;
    let node = state.input_nodes.nodes.get(key).unwrap();
    // device is halfway into the nodes output, linear should mean halfway output
    assert!(
        (output - 0.5).abs() < 0.0001,
        "Linear: Node:{:.2}, Distance: {:.2}, Output:{:.5}",
        node.value,
        Vec3::ZERO.distance(node.location),
        output
    );
}

#[tokio::test]
async fn map_node_layer_default() {
    let _ = env_logger::builder().try_init();

    let id: DeviceId = "testing".into();
    let state = PerDevice {
        id: id.clone(),
        intensity: 1.0,
        offset: 0.000,
        interp_algo: InterpState::new(0.0, 0.0001)
    };
    update_device(Arc::new(state));
    let (map, dev) = start_map(true).await;
    
    let mut node = default_node();
    node.location = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    node.interpolation_layer = InterpolationLayer::Default;
    node.radius = 2.0;
    let slot = node.slots.first_mut().unwrap();
    slot.history.push_at(1.0, Instant::now());

    let key = add_node(&map, node.clone()).await;

    let slot = node.slots.first_mut().unwrap();
    node.location = Vec3 { x: 0.0, y: 0.5, z: 0.0 };
    slot.history.push_at(0.0, Instant::now());
    let key2 = add_node(&map, node).await;


    // create device after map is listening.
    device_at(Vec3::ZERO, "testing", &dev).await;
    map.mark_dirty();

    // wait for next map tick (@100hz)
    sleep(Duration::from_millis(20)).await;

    let output = get_val("testing", 0, &dev).await;

    let state = map.get_state().await;
    let node = state.input_nodes.nodes.get(key).unwrap();
    // device is halfway into the nodes output, linear should mean halfway output
    assert!(
        (output - 0.5).abs() < 0.0001,
        "Linear: Node:{:.2}, Distance: {:.2}, Output:{:.5}",
        node.value,
        Vec3::ZERO.distance(node.location),
        output
    );
}