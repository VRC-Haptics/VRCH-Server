use std::{net::{IpAddr, Ipv4Addr}, sync::{Arc, LazyLock}};
use if_addrs::get_if_addrs;
use tokio::{net::UdpSocket, sync::mpsc::Sender};
use dashmap::DashMap;

use crate::devices::DeviceHandle;

pub const DISCOVERY_PORT: u16 = 6868;
pub type ListenerMap = DashMap<IpAddr, Sender<Arc<Vec<u8>>>>;
static LISTENERS: LazyLock<ListenerMap> = LazyLock::new(DashMap::new);

pub fn get_listeners() -> &'static ListenerMap {
    &LISTENERS
}

pub async fn start_socket(manager: &mut DeviceHandle) {
    // EXPECT: Rather crash and get an error log than load and have user debug.
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))
        .await
        .expect("Unable to bind to discovery port");

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

    log::trace!("started wifi devices socket");
    let man_tx = manager.get_device_channel();

    // open socket on broadcast port
    // recieve packets
    // if ip is new, initiated device startup
    // otherwise forward raw bytes.
}
