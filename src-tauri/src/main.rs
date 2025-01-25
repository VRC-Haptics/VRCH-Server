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
use util::shutdown_device_listener;
use vrc::{discovery::get_vrc, VrcInfo};

//standard imports
use std::net::UdpSocket;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use std::thread;
use std::io::{self, Write};

async fn update_device_groups(
    mac: String,
    groups: Vec<AddressGroup>,
    devices_mutex: Arc<Mutex<Vec<Device>>>,
) -> Result<(), ()> {
    let mut devices = devices_mutex.lock().unwrap();
    if let Some(existing) = devices.iter_mut().find(|d| d.mac == mac) {
        existing.addr_groups = groups.clone();
        println!("updated groups to: {:?}", groups);
        existing.purge_cache();
    }
    Ok(())
}

fn recall_device_group(handle: &String, mac: &String) -> Option<Vec<AddressGroup>> {
    todo!();
}

async fn invalidate_cache(
    devices_mutex: Arc<Mutex<Vec<Device>>>,
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

    thread::spawn(move || {
        let timer = Duration::from_millis(10); // 100 Hz
        let device_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

        loop {
            thread::sleep(timer);
            {
                let mut device_list_guard = device_list.lock().unwrap();
                let vrc_info_guard = vrc_info.lock().expect("couldn't get mutable");

                let addresses = vrc_info_guard.raw_parameters.as_ref();
                let hashmap = addresses.read().expect("Poisoned OSC Hashmap");
                let menu = vrc_info_guard.menu_parameters.as_ref();
                let menu_parameters = menu.read().expect("couldnt get guard");

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

fn close_app(child_pid: Arc<Mutex<u32>>) {
    println!("Application is closing. Running cleanup...");
    let pid = child_pid.lock().expect("couldn't get lock on pid");
    shutdown_device_listener(*pid).expect("Failed to kill haptics process");

    //cleanup vrc TODO:
}

#[derive(Debug, serde::Deserialize, Default, Clone)]
struct Config {
    devices: Vec<DeviceConfig>
}

#[derive(Debug, serde::Deserialize, Default, Clone)]
struct DeviceConfig {
    mac: String,
    name: String,
    groups: Vec<AddressGroup>,
}

// Function to load and parse the config.json file
fn load_config<P: AsRef<std::path::Path>>(file_path: P) -> io::Result<Config> {
    // Read the file contents
    let config_data = std::fs::read_to_string(file_path)?;

    // Parse the JSON data into the Config struct
    let config: Config = serde_json::from_str(&config_data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(config)
}

fn main() {
    // Load the configuration
    let mut config = Config::default();
    match load_config("config.json") {
        Ok(in_conf) => {
            println!("Loaded configuration");
            config = in_conf;
        }
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            return;
        }
    }
    println!("Config: {:?}", config);

    let device_list: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new())); //device list
    let child_pid: Arc<Mutex<u32>> = Arc::new(Mutex::new(0)); //the child pid for the haptics sub process

    let handlers_pid = child_pid.clone();
    ctrlc::set_handler(move || {
        println!("Ctrl+C pressed. Shutting down...");
        close_app(handlers_pid.clone());
    }).expect("Error setting Ctrl+C handler");
    
    // start advertising and listening for vrc
    let vrc_info: Arc<Mutex<VrcInfo>> = Arc::new(Mutex::new(get_vrc())); // spawns a thread for advertising

    // start devices ticking and listening for added devices.
    tick_devices(vrc_info.clone(), device_list.clone()); // spawns thread that periodically ticks devices
    start_device_listener(device_list, child_pid.clone(), config); // spawns thread that modifies the device list
}
