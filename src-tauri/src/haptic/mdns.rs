use std::{sync::{Arc, Mutex}, time::Duration};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::collections::HashSet;
use std::io;
use if_addrs::get_if_addrs;
use tauri::{AppHandle, Emitter};
use serde_json::Value;
use winapi::um::minwinbase::GetFileExMaxInfoLevel;

use crate::haptic::Device;
use crate::recall_device_group;

pub fn start_device_listener(
    app_handle: AppHandle,
    devices_state: tauri::State<'_, Arc<Mutex<Vec<Device>>>>,
    refresh_delay: u64,
) {

    let devices = devices_state.inner().clone();

    std::thread::spawn(move || {
        
        // Bind to all interfaces on port 8888.
        // This lets us receive packets sent to port 8888.
        let socket = UdpSocket::bind("0.0.0.0:8888").unwrap();
        let multicast_addr = Ipv4Addr::new(239, 0, 0, 1);
        multicast_all_interfaces(&socket, &multicast_addr).ok();
        
        println!("Listening for multicast messages on {}:8888...", multicast_addr);
        
        // Buffer to store incoming data.
        let mut buf = [0u8; 1024];
        
        // Main loop: receive and process incoming packets.
        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    let received = String::from_utf8_lossy(&buf[..size]);

                    // Parse JSON
                    if let Ok(json) = serde_json::from_str::<Value>(&received) {
                        let mac = json["mac"].as_str().unwrap_or("UNKNOWN_MAC").to_string();
                        let ip = json["ip"].as_str().unwrap_or("UNKNOWN_IP").to_string();
                        let name = json["name"].as_str().unwrap_or("Unknown Device").to_string();
                        let port:u16 = json["port"].as_u64().unwrap_or(1027) as u16;
                        let mut lock = devices.lock().unwrap();

                        // Check if device already exists
                        if !lock.iter().any(|d| d.mac == mac) {
                            println!("New device found: {} at {}", name, ip);

                            let mut new_device = Device::new(mac.clone(), ip.clone(), port, 0, name.clone());
                            
                            // Try to recall saved groups
                            if let Some(old_groups) = recall_device_group(&app_handle, &new_device.mac) {
                                new_device.addr_groups.extend(old_groups);
                            }

                            //try to recall saved offset
                            if let Some(old_offset) = crate::get_device_store_field(&app_handle, &mac, "sens_mult") {
                                new_device.sens_mult = old_offset;
                            }

                            app_handle.emit("device-added", new_device.clone()).unwrap();
                            lock.push(new_device);
                        } else {
                            // If the device already exists, probably needs a reset
                            if let Some(dev) = lock.iter_mut().find(|d| (d.mac == mac)) {
                                dev.been_pinged = false;
                            }
                            println!("Multicast for {}, which already exists", name);

                        }
                    } else {
                        println!("Invalid JSON received: {}", received);
                    }
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        println!("Timed out");
                    }
                }
            }
        } 
    });
}

/// Joins the socket to the multicast group on all eligible IPv4 interfaces.
fn multicast_all_interfaces(socket: &UdpSocket, multicast_addr: &Ipv4Addr) -> io::Result<()> {
    let interfaces = get_if_addrs()?;
    // it's just for the debug statement
    // because why not?
    let names_set: HashSet<String> = interfaces.iter()
    .map(|iface| iface.name.clone())
    .collect();
    let names: Vec<String> = names_set.into_iter().collect();

    println!("Searching Interfaces: {:?}", names);
    for iface in interfaces {
        // Check for IPv4 addresses.
        if let if_addrs::IfAddr::V4(v4_addr) = &iface.addr {
            if !iface.is_loopback() {
                println!("Joining multicast group on interface: {}", v4_addr.ip);
                // Attempt to join multicast group on this interface.
                if let Err(e) = socket.join_multicast_v4(multicast_addr, &v4_addr.ip) {
                    eprintln!("Failed to join multicast on interface {}: {}", v4_addr.ip, e);
                }
            }
        }
    }
    Ok(())
}