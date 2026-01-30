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
pub mod ble;
pub mod state;
mod vrc;

// local modules
use api::ApiManager;
use bhaptics::game::BhapticsGame;
use devices::{DeviceManager, init_device_manager};
use mapping::{get_global_map, global_map::InputMap};
use vrc::VrcInfo;

//standard imports
use commands::*;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration};
use std::panic::{take_hook, set_hook};
use tauri::{AppHandle, Manager, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_log::{Target, TargetKind};
use once_cell::sync::OnceCell;

use crate::ble::start_ble;
use crate::state::start_config;

// Provides a unified interface for interacting with external api's
pub static API_MANAGER: LazyLock<Arc<Mutex<ApiManager>>> = LazyLock::new(||{Arc::new(Mutex::new(ApiManager::new()))});
pub static DEVICE_MANAGER: OnceCell<DeviceManager> = OnceCell::new();

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

async fn start_async_tasks() {
    start_config(Duration::from_secs(1)).await;
    // ensures global map gets intialized somewhat early.
    let _ = get_global_map().await;

    // start wireless protocols
    start_ble(Duration::from_secs(1)).await;

    //start_apps()
}

fn main() {
    let manager = DeviceManager::new();
    init_device_manager(&mut manager);
    if let Err(e) = DEVICE_MANAGER.set(manager) {
        log::error!("Failed to start device manager");
    }

    let builder = std::thread::Builder::new().name("AsyncTasks".to_string());
    let handler = builder.spawn(|| {
        let tokio_rt = tokio::runtime::Runtime::new().expect("Tokio runtime failed to start");
        tokio_rt.block_on(start_async_tasks());
    }).expect("Failed building tasks thread");

    tauri::Builder::default()
        .plugin(tauri_plugin_serialplugin::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            println!("Instance already open, shutting down.");
            let _ = app
                .get_webview_window("main")
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
        .plugin(tauri_plugin_serialplugin::init())
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
                .max_file_size(500_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(10))
                .build(),
        )
        .manage(get_global_map())
        .manage(API_MANAGER)
        .setup(move |app| {
            let app_handle = app.handle();

            let default_panic = take_hook();
            set_hook(Box::new(move |info| {
                log::logger().flush(); // flush previous logs
                log::error!("Panic Captured: {info}");
                log::logger().flush(); // flush added info.
                default_panic(info);
            }));

            // Managers for game integrations; each handling connectivity and communications
            // Global VRC State; connection management and GlobalMap interaction
            /*
            let vrc_info: Arc<Mutex<VrcInfo>> = VrcInfo::new(
                Arc::clone(&global_map),
                Arc::clone(&api_manager),
                app_handle,
            ); 
            // Global Bhaptics state that manages game connection and inserts values into the GlobalMap
            let bhaptics: Arc<Mutex<BhapticsGame>> = BhapticsGame::new(Arc::clone(&global_map));

            app.manage(Arc::clone(&vrc_info));
            app.manage(Arc::clone(&bhaptics));

            // Initialize stuff that needs the app handle. (interacts directly with GUI)
            throw_vrc_notif(app_handle, vrc_info.clone());*/
            let mut lock = API_MANAGER.lock().unwrap();
            lock.refresh_caches();
            drop(lock);

            log::trace!("done with tauri setup");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_device_list,
            commands::get_vrc_info,
            commands::get_core_map,
            commands::upload_device_map,
            commands::update_device_multiplier,
            commands::update_device_offset,
            commands::update_vrc_distance_weight,
            commands::update_vrc_velocity_multiplier,
            bhaptics_launch_default,
            bhaptics_launch_vrch,
            commands::play_point,
            commands::swap_conf_nodes,
            commands::set_tags_radius,
            commands::set_node_radius,
            commands::start_device_update,
            commands::get_device_esp_model,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event.to_owned() {
                close_app(window);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
