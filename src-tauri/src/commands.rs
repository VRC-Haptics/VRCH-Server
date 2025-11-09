// local modules
use crate::devices::{
    update::{Firmware, UpdateMethod},
    Device, DeviceType, ESP32Model,
};
use crate::mapping::event::Event;
use crate::mapping::haptic_node::HapticNode;
use crate::mapping::{global_map::GlobalMap, Id};

use crate::util::math::Vec3;
use crate::vrc::{config::GameMap, VrcInfo};
use crate::{set_device_store_field, set_store_field};
//standard imports
use runas::Command;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::time::Duration;

#[tauri::command]
pub fn get_device_esp_model(
    id: String,
    devices: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<ESP32Model, String> {
    let lock = devices.lock().expect("Lock could not be held");
    let device = lock.iter().find(|d| d.id == id);
    if let Some(device) = device.as_ref() {
        return Ok(device.get_esp_type());
    } else {
        log::error!("Couldn't find device with specified id.");
        return Err("Failed: ID".to_string());
    }
}

#[tauri::command]
pub fn start_device_update(
    fw: Firmware,
    devices: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), String> {
    let lock = devices.lock().expect("Lock could not be held");
    let devices = lock.iter().find(|d| d.id == fw.id);
    if let Some(device) = devices {
        fw.do_update(device)?;
    } else {
        log::error!("Couldn't find device with specified id.");
        return Err("Failed: ID".to_string());
    }

    Ok(())
}

#[tauri::command]
pub fn set_tags_radius(
    tag: String,
    radius: f32,
    global_map: tauri::State<'_, Arc<Mutex<GlobalMap>>>,
) -> Result<(), ()> {
    let lock = global_map.lock().expect("this is wrong");
    lock.set_radius_by_tag(&tag, radius);
    Ok(())
}

#[tauri::command]
pub fn set_node_radius(
    id: String,
    radius: f32,
    global_map: tauri::State<'_, Arc<Mutex<GlobalMap>>>,
) -> Result<(), String> {
    let mut lock = global_map.lock().unwrap();
    let node = lock.get_mut_node(&Id(id));
    if let Some(mut node) = node {
        node.set_radius(radius);
        Ok(())
    } else {
        Err("Can't find node".to_string())
    }
}

/// Swaps the haptic node indices on the given device id
#[tauri::command]
pub fn swap_conf_nodes(
    device_id: String,
    pos1: Vec3,
    pos2: Vec3,
    device_state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), String> {
    let mut device_lock = device_state.lock().expect("Couldn't lock device list");

    for dev in device_lock.iter_mut() {
        if dev.id == device_id {
            let DeviceType::Wifi(wifi_cfg) = &mut dev.device_type;
            log::debug!("swapping nodes with positions: {:?}, {:?}", pos1, pos2);
            wifi_cfg.swap_nodes(pos1, pos2)?;
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
    let event = Event::new(
        "Play Point".to_string(),
        crate::mapping::event::EventEffectType::Location(Vec3 {
            x: feedback_location.0,
            y: feedback_location.1,
            z: feedback_location.2,
        }),
        vec![power],
        Duration::from_secs_f32(duration),
        vec!["UI".to_string()],
    )
    .expect("unable to create play point event");

    let mut global_map = global_map_state.lock().expect("couldn't lock global map");

    global_map.start_event(event);

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

/// Gets the core haptics map that is used to drive feedback.
#[tauri::command]
pub fn get_core_map(state: tauri::State<'_, Arc<Mutex<GlobalMap>>>) -> GlobalMap {
    let map = state.lock().expect("Unable to lock global map");
    map.clone()
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
    let mut devices_lock = devices_store.lock().unwrap();
    if let Some(dev) = devices_lock.iter_mut().find(|d| d.id == device_id) {
        dev.factors.sens_mult = multiplier;
        set_device_store_field(&window, &device_id, "sens_mult", multiplier);
    }
    Ok(())
}

#[tauri::command]
pub async fn update_device_offset(
    device_id: String,
    offset: f32,
    devices_store: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
    window: tauri::Window,
) -> Result<(), ()> {
    let mut devices_lock = devices_store.lock().unwrap();
    if let Some(dev) = devices_lock.iter_mut().find(|d| d.id == device_id) {
        dev.factors.start_offset = offset;
        set_device_store_field(&window, &device_id, "start_offset", offset);
    }
    Ok(())
}

#[tauri::command]
pub async fn update_vrc_velocity_multiplier(vel_multiplier: f32, window: tauri::Window) {
    let vrc_state = window.app_handle().state::<Arc<Mutex<VrcInfo>>>();
    let mut vrc_lock = vrc_state.lock().expect("couldn't lock vrc");
    vrc_lock.vel_multiplier = vel_multiplier;
    set_store_field(window.app_handle(), "velocity_multiplier", vel_multiplier);
}

#[tauri::command]
pub async fn update_vrc_distance_weight(distance_weight: f32, window: tauri::Window) {
    let vrc_state = window.app_handle().state::<Arc<Mutex<VrcInfo>>>();
    let mut vrc_lock = vrc_state.lock().expect("couldn't lock vrc");
    vrc_lock.dist_weight = distance_weight;
    set_store_field(window.app_handle(), "distance_weight", distance_weight);
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
