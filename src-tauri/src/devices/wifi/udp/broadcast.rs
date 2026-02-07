use crate::devices::DeviceManager;
use crate::devices::DeviceHandle;
use crate::devices::{wifi::WifiDevice, DeviceId, HapticDevice};
use serde_json::Value;

use std::sync::Arc;
use tokio::{net::UdpSocket, sync::OnceCell};

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

pub async fn start_listen_broadcast(manager: &mut DeviceHandle) {
    let socket = get_broadcast().await;
    let manager = manager.clone();
    let tx = manager.get_device_channel();

    tokio::task::spawn(async move {
        let mut buf = [0u8; 1024];

        loop {
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

                        let id = &DeviceId(mac.clone());

                        // Check if device already exists
                        if !manager.exists(id) {
                            log::trace!("New device found: {} at {}", name, ip);

                            if let Some(device) =
                                WifiDevice::new(mac.clone(), ip.clone(), port, name.clone(), manager.get_device_channel()).await
                            {
                                tx.send(crate::devices::DeviceMessage::Register(
                                    HapticDevice::Wifi(device),
                                ));
                            }
                        } else {
                            // If the device already exists, probably needs a reset
                            let fun = |d: &HapticDevice| match d {
                                HapticDevice::Wifi(d) => d.reset_ping(),
                                _ => log::error!("Device type already registered is not wifi"),
                            };
                            manager.with_device(id, &fun);
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
    });
}
