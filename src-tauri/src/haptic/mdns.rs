use std::{sync::{Arc, Mutex}, time::Duration};
use tauri::{AppHandle, Emitter};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::{get_device_store_field, haptic::Device};
use crate::recall_device_group;

pub fn start_device_listener(
    app_handle: AppHandle,
    devices_state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
    refresh_delay: u64,
) {

    let devices = devices_state.inner().clone();

    std::thread::spawn(move || {
        let daemon = ServiceDaemon::new().expect("Failed to create daemon");
        let reciever = daemon.browse("_haptics._udp.local.").unwrap();

        loop {
            if let Ok(event) = reciever.recv_timeout(Duration::from_secs(refresh_delay)) {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let mut lock = devices.lock().unwrap();
                        let mac = info.get_property_val_str("MAC").expect("couldn't get MAC records");
                        // debounce (if we kept heartbeat)
                        for device in lock.iter() {
                            if device.mac == mac {
                                return;
                            }
                        }
                        println!("Added device: {}", info.get_fullname());
                        let built_device = info_to_device(info, &app_handle);
                        app_handle.emit("device-added", built_device.clone()).unwrap();
                        lock.push(built_device);
                    }
                    ServiceEvent::ServiceRemoved(_, full_name) => {
                        let mut lock = devices.lock().unwrap();
                        if let Some(index) = lock.iter().position(|device| device.full_name == full_name) {
                            let device = lock.remove(index);
                            println!("removing device: {}@{}", device.display_name, device.ip);
                            // Emit the removed device
                            app_handle.emit("device-removed", device).unwrap();
                        }
                    }
                    _ => ()
                }
            } else {
                let devices_lock = devices.lock().expect("couldn't get lock on services");
                for service in devices_lock.iter() {
                    let name = &service.full_name;
                    let response = daemon.verify(name.to_string(), Duration::from_secs(7));
                    match response {
                        Ok(_) => (),
                        Err(error) => match_error(error),
                    }
                }
            }
        }
    });
}

fn match_error(err: mdns_sd::Error) {
    match err {
        mdns_sd::Error::Again => println!("Couldn't send query for service"),
        mdns_sd::Error::Msg(msg) => println!("verify failed with messge: {}", msg),
        mdns_sd::Error::ParseIpAddr(msg) => println!("Failed to parse ip adress: {}", msg),
        _ => println!("Unknown error verifying"),
    }
}

fn info_to_device(info: ServiceInfo, app_handle: &tauri::AppHandle) -> Device {
    let mac = info.get_property_val_str("MAC").expect("couldn't get MAC records");
    let ip = info.get_addresses_v4().iter().next().unwrap().to_string();
    let mut new_device = Device::new(mac.to_string(), ip, info.get_port(), info.get_host_ttl(), info.get_fullname().to_string());
    // Try to recall saved groups
    if let Some(old_groups) = recall_device_group(app_handle, &new_device.mac) {
        new_device.addr_groups.extend(old_groups);
    }

    //try to recall saved offset
    if let Some(old_offset) = get_device_store_field(app_handle, mac, "sens_mult") {
        new_device.sens_mult = old_offset;
    }

    return new_device;
}
