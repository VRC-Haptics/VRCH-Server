// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_camel_case_types)]

// make local modules available
mod haptic;
mod vrc;
pub mod util;
pub mod osc;

//use local modules
use haptic::{Device, mdns::start_device_listener};
use vrc::{discovery::get_vrc, VrcInfo};
use util::shutdown_device_listener;

//standard imports
use std::sync::{Arc, Mutex};
use tauri::{Manager, WindowEvent, Window};

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

pub fn close_vrc() {
    let tk_rt = tokio::runtime::Runtime::new().unwrap();
    tk_rt.block_on(async {
       oyasumivr_oscquery::server::deinit().await.expect("couldn't close vrc query");
    });
}

fn close_app(window: &Window) {
    let app_handle = window;
    //cleanup haptics sidecar
    let state = app_handle.state::<Arc<Mutex<u32>>>();
    println!("Application is closing. Running cleanup...");
    let pid = state.lock().expect("couldn't get lock on pid");
    shutdown_device_listener(*pid).expect("Failed to kill haptics process");

    //cleanup vrc TODO:

}

fn main() {
    let device_list: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new())); //device list
    let child_pid: Arc<Mutex<u32>> = Arc::new(Mutex::new(0)); //the child pid for the haptics sub process
    let vrc_info: Arc<Mutex<VrcInfo>> = Arc::new(Mutex::new(get_vrc())); //the vrc state

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .manage(device_list)
        .manage(child_pid)
        .manage(vrc_info)
        .setup(|app| {
            let app_handle = app.app_handle();
            start_device_listener(app_handle.clone(), app.state(), app.state());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_device_list, get_vrc_info])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event.to_owned() {
                close_app(window);
            }})
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
