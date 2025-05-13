// local modules
use crate::devices::{Device, DeviceType};
use crate::mapping::global_map::GlobalMap;
use crate::mapping::haptic_node::HapticNode;
use crate::mapping::NodeGroup;
use crate::set_device_store_field;
use crate::vrc::{config::GameMap, VrcInfo};
//standard imports
use runas::Command;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

/// Swaps the haptic node indices on the given device id
#[tauri::command]
pub fn swap_conf_nodes(
    device_id: String,
    index_1: u32,
    index_2: u32,
    device_state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), String> {
    let mut device_lock = device_state.lock().expect("Couldn't lock device list");

    log::trace!("Into command");
    for dev in device_lock.iter_mut() {
        if dev.id == device_id {
            let DeviceType::Wifi(wifi_cfg) = &mut dev.device_type;
            wifi_cfg.swap_nodes(index_1 as usize, index_2 as usize)?;
            let _ = wifi_cfg;
            
            drop(device_lock);
            log::trace!("Finished Command");
            return Ok(());
        }
    }

    Err(format!("No device with id to swap nodes: {:?}", device_id))
}

/// Plays the specified point for the duration in seconds at the power percentage of intensity.
#[tauri::command]
pub fn play_point(
    feedback_location: (f32, f32, f32), // xyz location to insert point
    power: f32,                         // the power percentage to play 1 = no change
    duration: f32,                      // When should this point be removed.
    global_map_state: tauri::State<'_, Arc<Mutex<GlobalMap>>>,
) -> Result<(), ()> {
    use strum::IntoEnumIterator;
    let all_bones: Vec<NodeGroup> = NodeGroup::iter().collect();
    let temp_node = HapticNode {
        x: -feedback_location.0, // TODO: Actually find out why these are swapped
        y: feedback_location.1,
        z: feedback_location.2,
        groups: all_bones,
    };

    let mut global_map = global_map_state.lock().expect("couldn't lock global map");
    let node_name = "Manual Play Node".to_string();
    if let Ok(_) = global_map.add_input_node(
        temp_node,
        vec!["Testing".to_string()],
        node_name.to_string(),
    ) {
        let _ = global_map.set_intensity(&node_name, power);
    }

    drop(global_map); // explicitly yeild our lock.

    std::thread::sleep(Duration::from_secs_f32(duration));

    let mut global_map = global_map_state.lock().expect("couldn't lock global map");
    let _ = global_map.pop_input_node(node_name);

    return Ok(());
}

#[tauri::command]
pub fn get_device_list(state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>) -> Vec<Device> {
    let devices = state.lock().unwrap();
    devices.clone()
}

#[tauri::command]
pub fn get_vrc_info(state: tauri::State<'_, Arc<Mutex<VrcInfo>>>) -> VrcInfo {
    let vrc_info = state.lock().unwrap();
    vrc_info.clone()
}

#[tauri::command]
pub async fn upload_device_map(
    id: String,
    config_json: String,
    devices_mutex: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), String> {
    log::info!("commanded to upload");

    // Deserialize the JSON string into a GameMap struct.
    let upload: GameMap =
        serde_json::from_str(&config_json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Extract a plain list of HapticNode from the config while preserving the indices.
    let haptic_nodes: Vec<HapticNode> = upload
        .nodes
        .into_iter()
        .map(|node| node.node_data)
        .collect();

    let mut devices = devices_mutex.lock().unwrap();
    if let Some(device) = devices.iter_mut().find(|d| d.id == id) {
        // Propagate changes if necessary.
        match &mut device.device_type {
            DeviceType::Wifi(wifi) => {
                return wifi.set_node_list(haptic_nodes);
            }
        }
    } else {
        return Err(format!("No Device with id: {}", id));
    }
}

#[tauri::command]
pub async fn update_device_multiplier(
    device_id: String,
    multiplier: f32,
    devices_store: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
    window: tauri::Window,
) -> Result<(), ()> {
    let start_offset = 0.0;
    let mut devices_lock = devices_store.lock().unwrap();
    if let Some(dev) = devices_lock.iter_mut().find(|d| d.id == device_id) {
        dev.factors.sens_mult = multiplier;
        dev.factors.start_offset = start_offset;
        set_device_store_field(&window, &device_id, "sens_mult", multiplier);
        set_device_store_field(&window, &device_id, "start_offset", start_offset);
    }
    Ok(())
}

/// Handles setting our app to launch instead of the bHapticsPlayer
#[tauri::command]
pub async fn bhaptics_launch_vrch() {
    // Launch the sidecar with the set argument.
    let path = dunce::canonicalize(r".\sidecars\elevated-register.exe").unwrap();
    let mut cmd = Command::new(path);
    let status = cmd.arg("set").show(true).gui(false).status();

    match status {
        Ok(status) => {
            if status.success() {
                log::info!("Registry set successfully.");
            } else {
                log::info!("Registry set failed");
            }
        }
        Err(e) => {
            log::info!("Failed to launch sidecar: {:?}", e);
        }
    }
    log::info!("Finished bhaptics_launch_vrch.");
}

/// Handles resetting bhaptics to be the default player
#[tauri::command]
pub async fn bhaptics_launch_default() {
    // Launch the sidecar with the "reset" argument.
    let path = dunce::canonicalize(r".\sidecars\elevated-register.exe").unwrap();
    let mut cmd = Command::new(path);
    cmd.arg("reset").show(true).gui(false);
    let status = cmd.status();

    match status {
        Ok(status) => {
            if status.success() {
                log::info!("Registry reset successfully.");
            } else {
                log::info!("Registry reset failed");
            }
        }
        Err(e) => {
            log::info!("Failed to launch sidecar: {:?}", e);
        }
    }
    log::info!("Finished bhaptics_launch_default.");
}
