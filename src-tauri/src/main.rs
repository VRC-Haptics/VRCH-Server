// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_camel_case_types)]

// make local modules available
mod bhaptics;
mod haptic;
pub mod mapping;
pub mod osc;
pub mod util;
mod vrc;

// local modules
use bhaptics::discovery::Bhaptics;
use haptic::wifi::discovery::start_wifi_listener;
use haptic::{Device, DeviceType};
use mapping::haptic_node::HapticNode;
use vrc::{discovery::get_vrc, VrcInfo};

//standard imports
use rosc::OscType;
use runas::Command;
use serde_json::json;
use std::io::{self, Write};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_store::StoreExt;

#[tauri::command]
fn get_device_list(state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>) -> Vec<Device> {
    let devices = state.lock().unwrap();
    devices.clone()
}

#[tauri::command]
fn get_vrc_info(state: tauri::State<'_, Arc<Mutex<VrcInfo>>>) -> VrcInfo {
    let vrc_info = state.lock().unwrap();
    vrc_info.clone()
}

/// Helper to set store values
fn set_device_store_field<T: serde::Serialize>(
    window: &tauri::Window,
    mac: &str,
    field: &str,
    value: T,
) {
    let app_handle = window.app_handle();
    let store = app_handle
        .store("known_devices.json")
        .expect("couldn't access known_devices.json");

    // Try to retrieve the existing device data.
    let mut device_data = store.get(mac).unwrap();

    // Ensure we have a JSON object.
    if !device_data.is_object() {
        device_data = json!({});
    }

    // Insert or update the field.
    if let Some(map) = device_data.as_object_mut() {
        map.insert(field.to_string(), serde_json::to_value(value).unwrap());
    }

    // Write back the updated device data.
    store.set(mac, device_data);
}

fn get_device_store_field<T: serde::de::DeserializeOwned>(
    app_handle: &tauri::AppHandle,
    mac: &str,
    field: &str,
) -> Option<T> {
    let store = app_handle
        .store("known_devices.json")
        .expect("couldn't access known_devices.json");

    let device_data = store.get(mac).unwrap();
    let map = device_data.as_object()?;

    map.get(field)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

#[derive(serde::Deserialize, Debug)]
struct DeviceMapUpload {
    device_map: Vec<HapticNode>,
}

#[tauri::command]
async fn upload_device_map(
    id: String,
    config_json: String,
    devices_mutex: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), String> {
    log::info!("commanded to upload");
    let upload: DeviceMapUpload =
        serde_json::from_str(&config_json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let mut devices = devices_mutex.lock().unwrap();
    if let Some(device) = devices.iter_mut().find(|d| d.id == id) {
        device.map.set_device_map(upload.device_map);

        //propogate changes if necessary
        match &mut device.device_type {
            DeviceType::Wifi(wifi) => {
                wifi.push_map = true;
            }
        }
    } else {
        return Err(format!("No Device with id: {}", id));
    }
    Ok(())
}

#[tauri::command]
async fn update_device_multiplier(
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
async fn set_address(
    vrc_mutex: tauri::State<'_, Arc<Mutex<VrcInfo>>>,
    address: String,
    percentage: f32,
) -> Result<(), ()> {
    let vrc = vrc_mutex.lock().unwrap();
    let mut parameters = vrc.raw_parameters.as_ref().write().unwrap();

    log::info!("set parameter: {:?}, to {:?}", address, percentage);
    parameters.insert(address, vec![OscType::Float(percentage)]);

    Ok(())
}

/// Handles setting our app to launch instead of the bHapticsPlayer
#[tauri::command]
async fn bhaptics_launch_vrch() {

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
async fn bhaptics_launch_default() {
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

fn tick_devices(device_list: Arc<Mutex<Vec<Device>>>, app: &tauri::AppHandle) {
    log::info!("starting tick");
    io::stdout().flush().unwrap();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_millis(10)); // 100 Hz
        let device_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

        loop {
            timer.tick().await;
            {
                let mut device_list_guard = device_list.lock().unwrap();

                // Remove devices that need to be killed.
                // Collect removed devices (dead devices).
                let removed_devices: Vec<Device> = device_list_guard
                    .iter()
                    .filter(|device| !device.is_alive)
                    .cloned()
                    .collect();

                // Print the removed devices.
                for device in &removed_devices {
                    log::info!("Removed device: {:?}", device.name);
                    // NOTE: ALWAYS EMIT DEVICE-REMOVED or added, otherwise many issues.....
                    app_handle.emit("device-removed", device).unwrap();
                }

                // Remove dead devices
                device_list_guard.retain(|device| device.is_alive);

                for device in device_list_guard.iter_mut() {
                    // handle device specific tick functions
                    match &mut device.device_type {
                        DeviceType::Wifi(wifi_device) => {
                            // Send packet if we got one from tick
                            if let Some(packet) = wifi_device.tick(
                                &mut device.is_alive,
                                &mut device.factors,
                                &mut device.map,
                            ) {
                                let addr = format!("{}:{}", wifi_device.ip, wifi_device.send_port);
                                // TODO: Actually error handle
                                let _ = device_socket.send_to(&packet.packet, addr);
                            }
                        }
                    }
                }
            }
        }
    });
}

fn close_app(_: &Window) {
    log::info!("TODO: Properly shut down")
    //cleanup vrc TODO:
}

fn throw_vrc_notif(app: &AppHandle, vrc: Arc<Mutex<VrcInfo>>) {
    let vrc_lock = vrc.lock().unwrap();
    if vrc_lock.in_port.unwrap() != 9001 {
        app.dialog()
            .message(format!(
                "Default VRC ports busy, expect higher latency. Port: {}",
                vrc_lock.in_port.unwrap()
            ))
            .kind(MessageDialogKind::Warning)
            .title("Ports Unavailable")
            .show(|result| match result {
                true => (),
                false => (),
            });
    }
}

fn main() {
    // Get the path of the running executable.
    match std::env::current_exe() {
        Ok(exe_path) => {
        // Extract the parent directory.
        if let Some(parent) = exe_path.parent() {
            println!("Parent folder: {}", parent.display());
        } else {
            eprintln!("Failed to determine the parent folder.");
        }
        }
        Err(e) => eprintln!("Failed to get current exe: {}", e),
    }

    let device_list: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new())); //device list
    let vrc_info: Arc<Mutex<VrcInfo>> = Arc::new(Mutex::new(get_vrc())); //the vrc state
    let baptics: Arc<Mutex<Bhaptics>> = Arc::new(Mutex::new(Bhaptics::new()));
                                                                        
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("logs".to_string()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview,
                ))
                .max_file_size(50_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .build(),
        )
        .manage(device_list.clone())
        .manage(vrc_info.clone())
        .setup(move |app| {
            let app_handle = app.handle();
            tick_devices(device_list.clone(), app_handle);
            start_wifi_listener(app_handle.clone(), app.state());
            throw_vrc_notif(app_handle, vrc_info.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_device_list,
            get_vrc_info,
            upload_device_map,
            set_address,
            update_device_multiplier,
            bhaptics_launch_default,
            bhaptics_launch_vrch,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event.to_owned() {
                close_app(window);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    let lock = baptics.lock().unwrap();
    lock.do_something();
}
