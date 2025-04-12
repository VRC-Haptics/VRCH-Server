// local modules
use crate::devices::{Device, DeviceType};
use crate::mapping::haptic_node::HapticNode;
use crate::vrc::{VrcInfo, OscPath, config::GameMap};
use crate::set_device_store_field;
//standard imports
use rosc::OscType;
use runas::Command;
use std::sync::{Arc, Mutex};

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
pub async  fn update_device_multiplier(
    device_id: String,
    multiplier: f32,
    devices_store: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
    window: tauri::Window,
) -> Result<(), ()> {
    let mut devices_lock = devices_store.lock().unwrap();
    if let Some(dev) = devices_lock.iter_mut().find(|d| d.id == device_id) {
        dev.factors.sens_mult = multiplier;
        set_device_store_field(&window, &device_id, "sens_mult", multiplier);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_address(
    vrc_mutex: tauri::State<'_, Arc<Mutex<VrcInfo>>>,
    address: String,
    percentage: f32,
) -> Result<(), ()> {
    let vrc = vrc_mutex.lock().unwrap();

    log::info!("set parameter: {:?}, to {:?}", address, percentage);
    vrc.parameter_cache.insert(OscPath(address), OscType::Float(percentage));

    Ok(())
}

/// Handles setting our app to launch instead of the bHapticsPlayer
#[tauri::command]
pub async fn bhaptics_launch_vrch() {

    // Launch the sidecar with the set argument.
    let path = dunce::canonicalize(r".\sidecars\elevated-register.exe").unwrap();
    let mut cmd = Command::new(path);
    let status = cmd.arg("set")
        .show(true)
        .gui(false)
        .status();

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