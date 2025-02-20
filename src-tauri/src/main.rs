// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_camel_case_types)]

// make local modules available
mod haptic;
pub mod osc;
pub mod util;
mod vrc;

//use local modules
use haptic::{mdns::start_device_listener, AddressGroup, Device};
use vrc::{discovery::get_vrc, VrcInfo};

//standard imports
use serde_json::{from_value, json};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Window, WindowEvent};
use tauri_plugin_store::StoreExt;
use rosc::OscType;
use std::io::{self, Write};

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

#[tauri::command]
async fn update_device_groups(
    window: tauri::Window,
    mac: String,
    groups: Vec<AddressGroup>,
    devices_mutex: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), ()> {
    let app_handle = window.app_handle();
    let store = app_handle
        .store("known_devices.json")
        .expect("couldn't access known_devices.json");

    let mut devices = devices_mutex.lock().unwrap();
    if let Some(existing) = devices.iter_mut().find(|d| d.mac == mac) {
        existing.addr_groups = groups.clone();
        store.set(mac, json!(groups));
        println!("updated groups to: {:?}", groups);
        existing.purge_cache();
    }
    Ok(())
}

fn recall_device_group(handle: &AppHandle, mac: &String) -> Option<Vec<AddressGroup>> {
    let store = handle.store("known_devices.json").unwrap();
    if let Some(old_groups) = store.get(mac) {
        return Some(from_value(old_groups).unwrap());
    } else {
        return None;
    }
}

#[tauri::command]
async fn set_address(
    vrc_mutex: tauri::State<'_, Arc<Mutex<VrcInfo>>>,
    address: String,
    percentage: f32,
) -> Result<(), ()> {
    let vrc =  vrc_mutex.lock().unwrap();
    let mut parameters = vrc.raw_parameters.as_ref().write().unwrap();

    println!("set parameter: {:?}, to {:?}", address, percentage);
    parameters.insert(address, vec![OscType::Float(percentage)]);

    Ok(())
}

#[tauri::command]
async fn invalidate_cache(
    devices_mutex: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Result<(), ()> {
    let mut devices = devices_mutex.lock().unwrap();
    let iterator = devices.iter_mut();
    for device in iterator {
        device.purge_cache();
    }
    Ok(())
}

fn tick_devices(vrc_info: Arc<Mutex<VrcInfo>>, device_list: Arc<Mutex<Vec<Device>>>) {
    println!("starting tick");
    io::stdout().flush().unwrap();

    tauri::async_runtime::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_millis(10)); // 100 Hz
        let device_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

        loop {
            timer.tick().await;
            {
                let mut device_list_guard = device_list.lock().unwrap();
                let vrc_info_guard = vrc_info.lock().expect("couldn't get mutable");

                let addresses = vrc_info_guard.raw_parameters.as_ref();
                let hashmap = addresses.read().expect("Poisoned OSC Hashmap");
                let menu = vrc_info_guard.menu_parameters.as_ref();
                let menu_parameters = menu.read().expect("couldn't get guard");

                // Remove devices that need to be killed.
                device_list_guard.retain(|device| !device.kill_me);

                for device in device_list_guard.iter_mut() {

                    if let Some(packet) = device.tick(
                         &hashmap, 
                         &menu_parameters,
                        "/avatar/parameters/h".to_string()
                    ) {
                        if let Err(err) = device_socket
                            .send_to(&packet.packet, format!("{}:{}", device.ip, device.port))
                        {
                            eprintln!("Failed to send to {}: {}", device.display_name, err);
                        }
                    }
                }
            }
        }
    });
}

fn close_app(_: &Window) {
    println!("TODO: Properly shut down")
    //cleanup vrc TODO:
}

fn main() {
    let device_list: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new())); //device list
    let vrc_info: Arc<Mutex<VrcInfo>> = Arc::new(Mutex::new(get_vrc())); //the vrc state

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .manage(device_list.clone())
        .manage(vrc_info.clone())
        .setup(move |app| {
            let app_handle = app.handle();
            tick_devices(vrc_info.clone(), device_list.clone());
            start_device_listener(app_handle.clone(), app.state(), 2);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_device_list,
            get_vrc_info,
            invalidate_cache,
            update_device_groups,
            set_address
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event.to_owned() {
                close_app(window);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
