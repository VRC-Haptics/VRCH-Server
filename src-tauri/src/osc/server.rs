use std::fmt;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

use rosc::{OscMessage, OscPacket};
use tokio::sync::mpsc;

use crate::util::next_free_port_with_address;

#[derive(serde::Serialize, Clone)]
pub struct OscServer {
    pub port: u16,
    pub address: Ipv4Addr,
    pub filter_prefix: String,
    #[serde(skip)]
    close_handle: Option<mpsc::Sender<()>>,
    #[serde(skip)]
    on_receive: Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>,
}

impl fmt::Debug for OscServer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("OscServer")
            .field("port", &self.port)
            .field("address", &self.address)
            .field("close_handle", &self.close_handle)
            .field("on_receive", &"Function Pointer")
            .finish()
    }
}

impl OscServer {
    /// create new Osc Server, it will need to be started with the start() command
    /// THE ADDRESS IS USUALLY JUST "0.0.0.0". THIS IS THE ADDRESS WE OWN.
    pub fn new<F>(port: u16, address: Ipv4Addr, on_receive: F) -> Self
    where
        F: Fn(OscMessage) + Send + Sync + 'static,
    {
        OscServer {
            port,
            address,
            close_handle: None,
            filter_prefix: "".to_string(),
            on_receive: Arc::new(Mutex::new(on_receive)),
        }
    }

    /// Starts a server listening in a new thread.
    pub fn start(&mut self) -> u16 {
        let mut used_port = self.port;
        let addr = format!("{}:{}", self.address, self.port);
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(_) => {
                // The desired port is not available. Look for a fallback.
                if let Some(free_port) = next_free_port_with_address(self.port, std::net::IpAddr::V4(self.address)) {
                    used_port = free_port;
                    let addr = format!("{}:{}", self.address, free_port);
                    UdpSocket::bind(&addr).unwrap() //assume we will be able to bind to thisone
                } else {
                    panic!("Couldn't find free port");
                }
            }
        };

        let on_receive = Arc::clone(&self.on_receive);
        let filter_prefix = self.filter_prefix.clone();

        let (tx, mut rx) = mpsc::channel(1);
        self.close_handle = Some(tx);

        thread::spawn(move || {
            println!("Spawned UDP OSC Server on: {}", socket.local_addr().unwrap());

            let mut buf = [0u8; rosc::decoder::MTU];
            loop {
                // Check for stop signal
                if let Ok(_) = rx.try_recv() {
                    println!("Stopping server thread.");
                    break;
                }

                match socket.recv_from(&mut buf) {
                    Ok((size, _src)) => {
                        if let Ok((left_over, packet)) = rosc::decoder::decode_udp(&buf[..size]) {
                            if !left_over.is_empty() {
                                println!(
                                    "leftover bytes: {} on socket: {}",
                                    String::from_utf8_lossy(left_over),
                                    socket.local_addr().unwrap().to_string()
                                );
                            }
                            handle_packet(packet, &on_receive, &filter_prefix);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving packet: {:?}", e);
                    }
                }
            }
        });
        return used_port;
    }

    //kills the server thread.
    pub fn stop(&mut self) {
        if let Some(handle) = self.close_handle.take() {
            let _ = handle.send(());
        }
    }
}

/// recursively handle packets
fn handle_packet(
    packet: OscPacket,
    callback: &Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>,
    filter_prefix: &str,
) {
    match packet {
        OscPacket::Bundle(bundle) => {
            for packet in bundle.content {
                handle_packet(packet, callback, filter_prefix);
            }
        }
        OscPacket::Message(message) => {
            handle_message(message, callback, filter_prefix);
        }
    }
}

/// handle the messages with a callback.
fn handle_message(
    message: OscMessage,
    callback: &Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>,
    filter_prefix: &str,
) {
    let address = &message.addr;

    if filter_prefix == "".to_string() || address.starts_with(filter_prefix) {
        let cb = callback.lock().unwrap();
        cb(message);
    }
    return;
}
