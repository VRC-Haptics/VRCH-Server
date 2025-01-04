// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_camel_case_types)]

use std::sync::{Arc, Mutex};
use tauri:: Manager;

mod haptic;
use haptic::{Device, mdns::start_device_listener};


#[tauri::command]
fn get_device_list(state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>) -> Vec<Device> {
    let devices = state.lock().unwrap();
    devices.clone()
}

fn main() {
    let device_list: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .manage(device_list)
        .setup(|app| {
            let app_handle = app.app_handle();
            start_device_listener(app_handle.clone(), app.state());
            Ok(()) // Return Ok(()) to satisfy the expected return type
        })
        .invoke_handler(tauri::generate_handler![
            get_device_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
