use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

use crate::haptic::Device;
use serde_json::Value;

pub fn start_device_listener(app_handle: AppHandle, devices_state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>, pid_state: tauri::State<'_, Arc<Mutex<u32>>>) {
    let mut cmd = Command::new("sidecars/tracker-sidecar.exe")
            .arg("0")
            .arg("_haptics._udp.local")
            .arg("--debug")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute command");
    {
        let mut pid = pid_state.lock().unwrap();
        *pid = cmd.id();
    }
    let devices = devices_state.inner().clone();

    thread::spawn(move || {

        let stdout = cmd.stdout.take().unwrap();
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            let long_raw = line.unwrap();
            let split = long_raw.splitn(2, ":");
            let log_type = split.to_owned().nth(0).unwrap();
            if !log_type.starts_with("_") {
                println!("{}", long_raw);
                continue;
            }

            // split off debug message
            let raw = split.to_owned().nth(1).unwrap();
            let device = make_new_device(raw);

            let mut devices = devices.lock().unwrap();
            match log_type {
                "_ADD" => {devices.push(device.clone());
                    println!("device added: {:?}", device);
                    app_handle.emit("device-added", device).unwrap();}
                "_RMV" => {devices.retain(|d| d.mac != device.mac);
                    app_handle.emit("device-removed", device).unwrap();}
                "_DBUG" => println!("Debug messsage from sidecar: {}", raw),
                &_ => println!("Encountered unknown log type for mdns sidecar: {:#?}", devices)
            }
        }
    });
}

fn make_new_device(raw: &str) -> Device {
    let parsed: Value = serde_json::from_str(raw).unwrap();

    // Extract fields required by Device::new()
    let mac = parsed.get("MAC").and_then(Value::as_str).unwrap().to_string();
    let ip = parsed.get("IP").and_then(Value::as_str).unwrap().to_string();
    let display_name = parsed.get("DisplayName").and_then(Value::as_str).unwrap().to_string();
    let port = parsed.get("Port").and_then(Value::as_u64).unwrap() as u16;
    let ttl = parsed.get("TTL").and_then(Value::as_u64).unwrap() as u32;

    Device::new(mac, ip, display_name, port, ttl)
}