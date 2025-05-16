// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![warn(unused_extern_crates)]

// make local modules available
pub mod api;
mod bhaptics;
mod commands;
mod devices;
pub mod mapping;
pub mod osc;
pub mod util;
mod vrc;

// local modules
use api::ApiManager;
use bhaptics::game::BhapticsGame;
use devices::wifi::discovery::start_wifi_listener;
use devices::{Device, DeviceType};
use mapping::global_map::GlobalMap;
use vrc::VrcInfo;

//standard imports
use commands::*;
use serde_json::json;
use std::io::{self, Write};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_store::StoreExt;

use tokio::time::{Instant, Interval};

/// Helper to set persistant store values
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

    // Try to retrieve the existing device data.d
    if let Some(mut device_data) = store.get(mac) {
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
    } else {
        // create new device data instance.
        let mut device_data = json!({});

        // Insert the field.
        let map = device_data.as_object_mut().unwrap();
        map.insert(field.to_string(), serde_json::to_value(value).unwrap());

        // Write back the updated device data.
        store.set(mac, device_data);
    };
}

/// Helper to get persistant store values
fn get_device_store_field<T: serde::de::DeserializeOwned>(
    app_handle: &tauri::AppHandle,
    mac: &str,
    field: &str,
) -> Option<T> {
    let store = app_handle
        .store("known_devices.json")
        .expect("couldn't access known_devices.json");

    if let Some(device_data) = store.get(mac) {
        let map = device_data.as_object()?;

        map.get(field)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    } else {
        None
    }
}

/// Waits until the next tick and logs an error if an overrun occurs.
async fn wait_for_tick(
    timer: &mut Interval,
    prev_tick: &mut Instant,
    period: &Duration,
) -> Instant {
    let tick_instant = timer.tick().await;
    let schedule_slip = tick_instant.duration_since(*prev_tick);

    if schedule_slip > *period {
        let missed = schedule_slip.as_micros() / period.as_micros();
        log::error!(
            "Device loop timer slipped by {:?} ({} missed tick(s)). High CPU Usage Likely.",
            schedule_slip,
            missed
        );
    }
    tick_instant
}

fn tick_devices(
    device_list: Arc<Mutex<Vec<Device>>>,
    input_list: Arc<Mutex<GlobalMap>>,
    _: &tauri::AppHandle,
) {
    log::info!("starting tick");
    io::stdout().flush().unwrap();

    tauri::async_runtime::spawn(async move {
        let period = Duration::from_millis(10); // 100 Hz
        let mut timer = tokio::time::interval(period);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip); // don't ever call too fast.
        let device_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

        let mut prev_tick = Instant::now();

        loop {
            prev_tick = wait_for_tick(&mut timer, &mut prev_tick, &period).await;
            {
                let mut device_list_guard = device_list.lock().unwrap();

                // Remove devices that need to be killed.
                // Collect removed devices (dead devices).
                device_list_guard.retain(|device| {
                    if device.is_alive {
                        true
                    } else {
                        log::info!("Removed device: {:?}", device.name);
                        false
                    }
                });

                let mut inputs_guard = input_list.lock().expect("couldn't find inputs guard");
                inputs_guard.refresh_inputs();
                for device in device_list_guard.iter_mut() {
                    // handle device specific tick functions
                    match &mut device.device_type {
                        DeviceType::Wifi(wifi_device) => {
                            // Send packet if we got one from tick
                            if let Some(packet) = wifi_device.tick(
                                &mut device.is_alive,
                                &mut device.factors,
                                &inputs_guard,
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

fn close_app(window: &Window) {
    log::info!("Cleaning up and Shutting Down.");
    let bhaptics = window.state::<Arc<Mutex<BhapticsGame>>>();
    let bh_lock = bhaptics.lock().expect("unable to lock bhaptics");
    bh_lock.shutdown();
    log::trace!("Shutdown bhaptics server");
    //cleanup vrc TODO:
}

/// Opens a window if we can't use the default VRC ports.
/// Using OSCQuery results in inconsistent delivery of packets.
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
    // Core state machines that interface devices and the haptics providers
    // The GlobalMap; provides interpolated feedback values.
    let input_list: Arc<Mutex<GlobalMap>> = Arc::new(Mutex::new(GlobalMap::new()));
    // Global device list; contains all active devices.
    let device_list: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
    // Provides a unified interface for interacting with external api's
    let api_manager: Arc<Mutex<ApiManager>> = Arc::new(Mutex::new(ApiManager::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.get_webview_window("main")
                       .expect("no main window")
                       .set_focus();
        }))
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(Target::new(TargetKind::Webview))
                .target(Target::new(TargetKind::LogDir {
                    file_name: Some("logs".to_string()),
                }))
                .filter(|metadata| {
                    !metadata.target().starts_with("mio")
                        && !metadata.target().starts_with("reqwest")
                })
                .max_file_size(20_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .manage(Arc::clone(&input_list))
        .manage(Arc::clone(&device_list))
        .manage(Arc::clone(&api_manager))
        .setup(move |app| {
            // Managers for game integrations; each handling connectivity and communications
            // Global VRC State; connection management and GlobalMap interaction
            let vrc_info: Arc<Mutex<VrcInfo>> =
                VrcInfo::new(Arc::clone(&input_list), Arc::clone(&api_manager));
            // Global Bhaptics state that manages game connection and inserts values into the GlobalMap
            let bhaptics: Arc<Mutex<BhapticsGame>> = BhapticsGame::new(Arc::clone(&input_list));

            app.manage(Arc::clone(&vrc_info));
            app.manage(Arc::clone(&bhaptics));

            let app_handle = app.handle();
            // Initialize stuff that needs the app handle. (interacts directly with GUI)
            tick_devices(device_list.clone(), input_list.clone(), app_handle);
            start_wifi_listener(app_handle.clone(), app.state());
            throw_vrc_notif(app_handle, vrc_info.clone());
            let mut lock = api_manager.lock().unwrap();
            lock.refresh_caches();
            drop(lock);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_device_list,
            commands::get_vrc_info,
            commands::upload_device_map,
            commands::update_device_multiplier,
            commands::update_device_offset,
            bhaptics_launch_default,
            bhaptics_launch_vrch,
            commands::play_point,
            commands::swap_conf_nodes,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event.to_owned() {
                close_app(window);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
