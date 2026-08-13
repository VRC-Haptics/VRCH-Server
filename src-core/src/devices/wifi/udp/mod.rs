use std::{net::Ipv4Addr, time::Duration};
use anyhow::Context;
use if_addrs::get_if_addrs;
use serde_json::Value;
use tokio::net::UdpSocket;

use crate::{devices::{DeviceHandle, DeviceId, HapticDevice, wifi::WifiDevice}, log_err};

pub async fn start_discovery(dev: DeviceHandle) -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:6868")
        .await?;
    let multicast_addr: Ipv4Addr = "239.0.0.1".parse().unwrap();
    if let Ok(interfaces) = get_if_addrs() {
        for iface in interfaces {
            if let if_addrs::IfAddr::V4(v4) = &iface.addr {
                if !iface.is_loopback() {
                    if let Err(e) = socket.join_multicast_v4(multicast_addr, v4.ip) {
                        log::error!("Failed to join multicast on {}: {}", v4.ip, e);
                    }
                }
            }
        }
    }

    tokio::spawn(async move {
        let tx = dev.get_device_channel();
        let mut buf = [0u8; 1024];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    log::debug!("Recieved udp broadcast: {addr:?}");

                    // Parse JSON
                    let received = String::from_utf8_lossy(&buf[..len]);
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
                        if !dev.exists(id) {
                            log::trace!("New device found: {} at {}", name, ip);

                            let device = WifiDevice::new(mac.clone(), ip.clone(), port, name.clone(), dev.get_device_channel()).await.context("Making Device");
                            match device {
                                Ok(device) => {
                                    log_err!(tx.send(crate::devices::DeviceMessage::Register(
                                        HapticDevice::Wifi(device),
                                    )).await)
                                }
                                Err(e) => {
                                    log::error!("Unable to create device: {id:?}: {e:?}");
                                }
                            }
                        } else {
                            // If the device already exists, probably needs a reset
                            let fun = |d: &HapticDevice| match d {
                                HapticDevice::Wifi(d) => d.reset_ping(),
                                _ => log::error!("Device with id:{id:?} already registered and is not wifi"),
                            };
                            dev.with_device(id, &fun);
                            log::debug!("Multicast for {}, which already exists", name);
                        }
                    } else {
                        log::error!("Invalid JSON received: {}", received);
                    }
                    // TODO: create new device if not already present
                }
                Err(e) => {
                    eprintln!("recv error: {e}");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
    });

    Ok(())
}
