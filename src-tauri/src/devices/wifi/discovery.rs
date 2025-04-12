use if_addrs::get_if_addrs;
use serde_json::Value;
use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::{ atomic::AtomicBool, atomic::Ordering, Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::devices::{Device, DeviceType, WifiDevice};


/// Listen for wifi based device advertisements
pub fn start_wifi_listener(
    app_handle: AppHandle,
    devices_state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
) -> Arc<AtomicBool> {
    // Create a cancellation flag.
    let cancelled = Arc::new(AtomicBool::new(false));
    // Lock our device list.
    let devices = devices_state.inner().clone();
    let cancelled_clone = cancelled.clone();

    std::thread::spawn(move || {
        // Bind to all interfaces on port 8888 and register for multicast.
        let socket = UdpSocket::bind("0.0.0.0:8888").unwrap();
        let multicast_addr = Ipv4Addr::new(239, 0, 0, 1);
        multicast_all_interfaces(&socket, &multicast_addr).ok();
        log::trace!(
            "Listening for Devices on {}:8888",
            multicast_addr
        );

        // Buffer to store incoming data.
        let mut buf = [0u8; 1024];

        // Main loop: receive and process incoming packets.
        while !cancelled_clone.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buf) {
                Ok((size, _)) => {
                    let received = String::from_utf8_lossy(&buf[..size]);

                    // Parse JSON
                    if let Ok(json) = serde_json::from_str::<Value>(&received) {
                        let mac = json["mac"].as_str().unwrap_or("UNKNOWN_MAC").to_string();
                        let ip = json["ip"].as_str().unwrap_or("UNKNOWN_IP").to_string();
                        let name = json["name"]
                            .as_str()
                            .unwrap_or("Unknown Device")
                            .to_string();
                        let port: u16 = json["port"].as_u64().unwrap_or(1027) as u16;
                        let mut lock = devices.lock().unwrap();

                        // Check if device already exists
                        if !lock.iter().any(|d| d.id == mac) {
                            log::trace!("New device found: {} at {}", name, ip);

                            let new_device =
                                WifiDevice::new(mac.clone(), ip.clone(), port, name.clone());
                            let mut full_device = Device::from_wifi(new_device, &app_handle);

                            //try to recall saved offset
                            if let Some(old_offset) =
                                crate::get_device_store_field(&app_handle, &mac, "sens_mult")
                            {
                                full_device.factors.sens_mult = old_offset;
                            }

                            if let Err(e) = app_handle.emit("device-added", full_device.clone()) {
                                log::error!("Failed to emit device-added: {:?}", e);
                            }
                            lock.push(full_device);
                        } else {
                            // If the device already exists, probably needs a reset
                            if let Some(dev) = lock.iter_mut().find(|d| (d.id == mac)) {
                                match &mut dev.device_type {
                                    DeviceType::Wifi(ex) => ex.been_pinged = false,
                                    //_ => panic!("Unexpected device type with same ID as new wifi device"),
                                }
                            }
                            log::debug!("Multicast for {}, which already exists", name);
                        }
                    } else {
                        log::error!("Invalid JSON received: {}", received);
                    }
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        println!("Timed out");
                    } else {
                        log::error!("Recieved error: {}", e);
                    }
                }
            }
        }
        log::info!("WiFi listener terminated due to cancellation.");
    });
    // Return the cancellation handle so the caller can stop the listener.
    cancelled
}

/// Joins the socket to the multicast group on all eligible IPv4 interfaces.
fn multicast_all_interfaces(socket: &UdpSocket, multicast_addr: &Ipv4Addr) -> io::Result<()> {
    let interfaces = get_if_addrs()?;
    for iface in interfaces {
        // Check for IPv4 addresses.
        if let if_addrs::IfAddr::V4(v4_addr) = &iface.addr {
            if !iface.is_loopback() {
                log::trace!("Listening on: {}", v4_addr.ip);
                // Attempt to join multicast group on this interface.
                if let Err(e) = socket.join_multicast_v4(multicast_addr, &v4_addr.ip) {
                    log::error!(
                        "Failed to join multicast on interface {}: {}",
                        v4_addr.ip, e
                    );
                }
            }
        }
    }
    Ok(())
}
