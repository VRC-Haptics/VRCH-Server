use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

use crate::haptic::Device;

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

            let raw = split.to_owned().nth(1).unwrap();
            let device = serde_json::from_str::<Device>(raw).unwrap();

            let mut devices = devices.lock().unwrap();
            match log_type {
                "_ADD" => {devices.push(device.clone());
                    println!("device added: {:?}", device);
                    app_handle.emit("device-added", device).unwrap();}
                "_RMV" => {devices.retain(|d| d.MAC != device.MAC);
                    app_handle.emit("device-removed", device).unwrap();}
                "_DBUG" => println!("Debug messsage from sidecar: {}", raw),
                &_ => println!("Encountered unknown log type for mdns sidecar: {:#?}", devices)
            }
        }
    });
}