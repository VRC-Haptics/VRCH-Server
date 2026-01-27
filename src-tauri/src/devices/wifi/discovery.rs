use if_addrs::get_if_addrs;
use serde_json::Value;
use std::io;
use std::net::{Ipv4Addr };
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use tauri::{AppHandle, Emitter};
use tokio::net::{UdpSocket};

use crate::devices::{get_devices, Device, DeviceType, WifiDevice, DeviceId};
use crate::state;

pub const DISCOVERY_PORT: u32 = 6868;

/// Listen for wifi based device advertisements
pub async fn start_wifi_listener(
    app_handle: AppHandle,
) -> Arc<AtomicBool> {
    // Create a cancellation flag.
    let cancelled = Arc::new(AtomicBool::new(false));
    // Lock our device list.
    let devices = get_devices();
    let cancelled_clone = cancelled.clone();

    tokio::task::spawn(async move {
        // Bind to all interfaces on port 6868 and register for multicast.

        let socket = UdpSocket::bind(format!("0.0.0.0:{DISCOVERY_PORT}")).await.expect("Unable to bind to discovery port");
        let multicast_addr = Ipv4Addr::new(239, 0, 0, 1);
        multicast_all_interfaces(&socket, &multicast_addr).ok();
        log::trace!(
            "Listening for Devices on {}:{}",
            multicast_addr,
            DISCOVERY_PORT.to_string()
        );

        // Buffer to store incoming data.
        let mut buf = [0u8; 1024];

        // Main loop: receive and process incoming packets.
        while !cancelled_clone.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buf).await {
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

                        // Check if device already exists
                        if !devices.iter().any(|d| d.id == DeviceId(mac.clone())) {
                            log::trace!("New device found: {} at {}", name, ip);

                            let new_device =
                                WifiDevice::new(mac.clone(), ip.clone(), port, name.clone());
                            let mut full_device = Device::from_wifi(new_device, &app_handle);

                            //try to recall saved multiplier
                            if let Some(old_offset) = 
                                state::get_device(&mac, |d| d.intensity).flatten()
                            {
                                full_device.factors.sens_mult = old_offset;
                            }

                            if let Some(old_offset) = state::get_device(&mac, |d| d.offset).flatten() {
                                full_device.factors.start_offset = old_offset;
                            }

                            if let Err(e) = app_handle.emit("device-added", full_device.clone()) {
                                log::error!("Failed to emit device-added: {:?}", e);
                            }
                            devices.insert(full_device.id.clone(), full_device);
                        } else {
                            // If the device already exists, probably needs a reset
                            if let Some(mut dev) = devices.get_mut(&DeviceId(mac)) {
                                match &mut dev.device_type {
                                    DeviceType::Wifi(ex) => ex.been_pinged = false,
                                    _ => panic!("Unexpected device type with same ID as new wifi device"),
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
                        log::info!("Timed out");
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
                if let Err(e) = socket.join_multicast_v4(*multicast_addr, v4_addr.ip) {
                    log::error!(
                        "Failed to join multicast on interface {}: {}",
                        v4_addr.ip,
                        e
                    );
                }
            }
        }
    }
    Ok(())
}
