use crate::devices::{get_devices, Device, DeviceId, DeviceType, wifi::WifiDevice};
use crate::state;
use if_addrs::get_if_addrs;
use serde_json::Value;
use std::io;
use std::net::Ipv4Addr;
use std::sync::{atomic::AtomicBool, atomic::Ordering};
use tauri::{AppHandle, Emitter};

use dashmap::DashMap;
use std::{
    net::SocketAddr,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::{
    net::UdpSocket,
    sync::{
        mpsc::{channel, Receiver, Sender},
        OnceCell,
    },
};

pub const DISCOVERY_PORT: u32 = 6868;

static BROADCAST_SOCKET: OnceCell<Arc<UdpSocket>> = OnceCell::const_new();

async fn get_broadcast() -> &'static Arc<UdpSocket> {
    BROADCAST_SOCKET
        .get_or_init(|| async {
            Arc::new(
                UdpSocket::bind(format!("0.0.0.0:{DISCOVERY_PORT}"))
                    .await
                    .expect("Unable to bind to discovery port"),
            )
        })
        .await
}

pub async fn start_listen_broadcast() {
    let socket = get_broadcast().await;
    let devices = get_devices();

    tokio::task::spawn(async move {
        let mut buf = [0u8; 1024];
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
                        
                        new_device.run_wifi_device();
                    } else {
                        // If the device already exists, probably needs a reset
                        if let Some(mut dev) = devices.get_mut(&DeviceId(mac)) {
                            match &mut dev.device_type {
                                DeviceType::Wifi(ex) => ex.re_ping(),
                                _ => {
                                    log::error!("Unexpected device type with same ID as new wifi device")
                                }
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
    });
}
