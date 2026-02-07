// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![warn(unused_extern_crates)]

// make local modules available
pub mod api;
//mod bhaptics;
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
//use bhaptics::game::BhapticsGame;
use devices::{DeviceManager, init_device_manager};
use mapping::InputMap;
use vrc::VrcGame;

//standard imports
use commands::*;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration};
use std::panic::{take_hook, set_hook};
use tauri::{AppHandle, Manager, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_log::{Target, TargetKind};
use once_cell::sync::OnceCell;

use crate::devices::DeviceHandle;
use crate::{
    ble::start_ble,
    state::start_config_save,
    mapping::MapHandle,
    vrc::VrcHandle,
};

// Provides a unified interface for interacting with external api's
pub static API_MANAGER: LazyLock<Mutex<ApiManager>> = LazyLock::new(||{Mutex::new(ApiManager::new())});
pub static DEVICE_MANAGER: OnceCell<DeviceHandle> = OnceCell::new();

fn close_app(window: &Window) {
    log::info!("Cleaning up and Shutting Down.");
    log::trace!("Shutdown bhaptics server");
    //cleanup vrc TODO:
}

/// Opens a window if we can't use the default VRC ports.
/// Using OSCQuery results in inconsistent delivery of packets.
/* TODO: actually throw this when needed
fn throw_vrc_notif(app: &AppHandle, vrc: Arc<Mutex<VrcGame>>) {
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
}*/

async fn start_async_tasks(manager: DeviceHandle) -> (VrcHandle, MapHandle) {
    start_config_save(Duration::from_secs(1)).await;
    // initialize input map.
    let (mut input_map, map_handle) = InputMap::new(manager).await;
    tokio::spawn(async move {
        input_map.start().await;
    });

    // TODO: Move into device manager init.
    start_ble(Duration::from_secs(1)).await;

    //start_apps
    let mut vrc = VrcGame::new(map_handle.clone(), &API_MANAGER).await;
    let vrc_handle = vrc.get_handle();
    tokio::spawn(async move {
        vrc.run().await;
    });

    (vrc_handle, map_handle)

}

#[tokio::main]
async fn main() {
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    // init logging and stuff first
    let plugins = tauri::Builder::default()
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
                        && !metadata.target().starts_with("reqwest") && !metadata.target().starts_with("btleplug")
                })
                .max_file_size(500_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(10))
                .build(),
    ).manage(DEVICE_MANAGER.get().unwrap().clone())
        .manage(vrc)
        .manage(map)
        .setup( move |app: &mut tauri::App| {
            let app_handle = app.handle();

            let default_panic = take_hook();
            set_hook(Box::new(move |info| {
                log::logger().flush(); // flush previous logs
                log::error!("Panic Captured: {info}");
                log::logger().flush(); // flush added info.
                default_panic(info);
            }));

            let mut lock = API_MANAGER.lock().unwrap();
            lock.refresh_caches();
            drop(lock);

            log::trace!("done with tauri setup");
            Ok(())
        });


    let mut manager = DeviceManager::new();
    init_device_manager(&mut manager);
    if let Err(e) = DEVICE_MANAGER.set(manager.get_handle()) {
        log::error!("Failed to start device manager");
    }
    log::trace!("Here");

    let (vrc, map) = start_async_tasks(manager.get_handle()).await;

    
    plugins
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
