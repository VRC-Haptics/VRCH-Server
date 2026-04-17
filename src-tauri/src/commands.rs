// local modules
use crate::{devices::{
    Device, DeviceHandle, DeviceId, DeviceInfo, ESP32Model, update::Firmware, //update::{Firmware, UpdateMethod}
}, mapping::{MapHandle, MapInfo}, state::{self, GitRepo, PerDevice, VrcSettings}, vrc::{VrcHandle, VrcInfo}};
use crate::mapping::event::Event;
use crate::mapping::haptic_node::HapticNode;
use crate::mapping::{InputEventMessage};
use crate::log_err;

use crate::{
    util::math::Vec3,
    vrc::{config::GameMap},
};
//standard imports
use runas::Command;
use std::sync::Arc;
use tokio::time::Duration;

#[tauri::command]
#[specta::specta]
pub fn get_device_esp_model(
    id: String,
    devices: tauri::State<'_, DeviceHandle>,
) -> Result<ESP32Model, String> {
    let Some(this) = devices.with_device(&id.into(), |d| d.info().get_esp32()) else {
        return Err("unable to find device with id".to_string());
    };
    return Ok(this);
}


#[tauri::command]
#[specta::specta]
/// typescript seems to throw a fit with formats here. So invoke bypasses most of this. EUUUGH
pub async fn start_device_update(
    fw: Firmware,
    devices: tauri::State<'_, DeviceHandle>,
) -> Result<(), String> {
    let devices = devices.inner().clone();
    tokio::task::spawn_blocking(move || {
        log::trace!("Starting OTA Update");
        devices
            .with_device(&fw.id.clone().into(), |d| fw.do_update(d))
            .ok_or_else(|| "unable to find device with id".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
#[specta::specta]
pub fn set_tags_radius(
    tag: String,
    radius: f32,
    map: tauri::State<'_, MapHandle>,
) -> Result<(), ()> {
    map.has_tag_mut(&tag, |n| {
        n.set_radius(radius);
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_node_radius(
    id: String,
    radius: f32,
    map: tauri::State<'_, MapHandle>,
) -> Result<(), String> {
    if map.with_node_mut(&id.into(), |n| n.set_radius(radius)).is_some() {
        return Ok(());
    } else {
        return Err("Failed to get device".into());
    }
}

const EPSILON: f32 = 0.001;

/// Swaps the haptic node indices on the given device id
#[tauri::command]
#[specta::specta]
pub fn swap_conf_nodes(
    device_id: String,
    pos1: Vec3,
    pos2: Vec3,
    devices: tauri::State<'_, DeviceHandle>,
) -> Result<(), String> {
    devices
        .with_device_mut(&device_id.clone().into(), |d| {
            let mut info = d.info();
            let mut nodes = info.get_nodes().to_owned();

            let mut index1: Option<usize> = None;
            let mut index2: Option<usize> = None;

            for (index, node) in nodes.iter().enumerate() {
                if node.to_vec3().close_to(&pos1, EPSILON) {
                    index1 = Some(index);
                    log::debug!("Found node 1 at index: {}", index);
                } else if node.to_vec3().close_to(&pos2, EPSILON) {
                    index2 = Some(index);
                    log::debug!("Found node 2 at index: {}", index);
                }
            }

            match (index1, index2) {
                (Some(i1), Some(i2)) => {
                    nodes.swap(i1, i2);
                    info.set_nodes(nodes);
                    d.update_info(info);
                    Ok(())
                }
                _ => Err(format!(
                    "Could not find both nodes at {:?} and {:?}",
                    pos1, pos2
                )),
            }
        })
        .unwrap_or_else(|| Err(format!("No device with id: {:?}", device_id)))
}

/// Plays the specified point for the duration in seconds at the power percentage of intensity.
#[tauri::command]
#[specta::specta]
pub fn play_point(
    feedback_location: (f32, f32, f32), // xyz location to insert point
    power: f32,                         // the power percentage to play 1 = no change
    duration: f32,                      // When should this point be removed.
    map: tauri::State<'_, MapHandle>,
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

    log_err!(map.send_event_blocking(InputEventMessage::StartEvent(event)));
    return Ok(());
}

#[tauri::command]
#[specta::specta]
pub fn get_repositories() -> Vec<GitRepo> {
    state::get_config().devices.ota_repositories.lock().clone()
}

#[tauri::command]
#[specta::specta]
pub fn set_repositories(repos: Vec<GitRepo>) {
    *state::get_config().devices.ota_repositories.lock() = repos;
    state::mark_dirty();
}

#[tauri::command]
#[specta::specta]
pub fn set_wifi_timeout(timeout: f32) {
    log::trace!("set to: {timeout:2}");
    state::get_config().devices.wifi_device_timeout.store(Arc::new(timeout));
    state::mark_dirty();
}

#[tauri::command]
#[specta::specta]
pub fn get_wifi_timeout() -> f32 {
    **state::get_config().devices.wifi_device_timeout.load()
}

#[tauri::command]
#[specta::specta]
pub fn get_device_list(dev: tauri::State<'_, DeviceHandle>) -> Vec<(DeviceId, Option<DeviceInfo>)> {
    let mut devices = vec![];
    let ids = dev.devices();
    for id in ids {
        let info = dev.with_device(&id, |d| d.info());
        devices.push((id, info));
    }
    devices
}

#[tauri::command]
#[specta::specta]
pub fn get_vrc_info(vrc: tauri::State<'_, VrcHandle>) -> VrcInfo {
    vrc.get_info()
}

#[tauri::command]
#[specta::specta]
/// sets all vrc relevant info. It is all behind an arcswap so it is the same cost to set all or one of them.
pub fn set_vrc(mult: f32, ratio: f32, samples: usize, smooth_s: Duration) {
    let shared = &state::get_config().vrc_settings;
    let mut new = VrcSettings::clone(&shared.load());
    new.velocity_mult = mult;
    new.velocity_ratio = ratio;
    new.sample_cache = samples;
    new.smoothing_time = smooth_s;

    shared.swap(Arc::new(new));
    state::mark_dirty();
}

#[tauri::command]
#[specta::specta]
/// handles persisting and splitting out individual states to where they need to go.
pub fn set_device_info(dev: tauri::State<'_, DeviceHandle>, id: DeviceId, inf: DeviceInfo) {
    dev.with_device_mut(&id, |f| f.update_info(inf));
}

/// Gets the core haptics map that is used to drive feedback.
#[tauri::command]
#[specta::specta]
pub fn get_core_map(map: tauri::State<'_, MapHandle>) -> MapInfo {
    map.get_state()
}

#[tauri::command]
#[specta::specta]
pub async fn upload_device_map(
    id: String,
    config_json: String,
    device: tauri::State<'_, DeviceHandle>,
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

    let res = device.with_device_mut(&id.clone().into(), |d| {
        let mut info = d.info();
        info.set_nodes(haptic_nodes);
        d.update_info(info);
    });

    state::mark_dirty();
    if res.is_none() {
        return Err(format!("No Device with id: {}", id))
    } else {
        Ok(())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn update_device_multiplier(
    device_id: DeviceId,
    multiplier: f32,
)  {
    let (_, dev) = state::get_device(&device_id);
    let guard = dev.load();
    let mut new = PerDevice::clone(&guard);
    new.intensity = multiplier;
    state::update_device(Arc::new(new));
    state::mark_dirty();
}

#[tauri::command]
#[specta::specta]
pub async fn update_device_offset(
    device_id: DeviceId,
    offset: f32,
) {
    let (_, dev) = state::get_device(&device_id);
    let guard = dev.load();
    let mut new = PerDevice::clone(&guard);
    new.offset = offset;
    state::update_device(Arc::new(new));
    state::mark_dirty();
}

/// Handles setting our app to launch instead of the bHapticsPlayer
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
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
