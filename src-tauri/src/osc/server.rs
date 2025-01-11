use rosc::{OscPacket, OscMessage};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::fmt;

#[derive(serde::Serialize, Clone)]
pub struct OscServer {
    port: u16,
    address: Ipv4Addr,
    filter_prefix: String,
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

    pub async fn start(&mut self) {
        let addr = SocketAddr::new(self.address.into(), self.port);
        let socket = UdpSocket::bind(addr).await.unwrap();
        let (tx, mut rx) = mpsc::channel(1);
        let filter_prefix = self.filter_prefix.clone();
        self.close_handle = Some(tx);

        let on_receive = Arc::clone(&self.on_receive);
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                tokio::select! {
                    _ = rx.recv() => {
                        println!("Server shutting down");
                        break;
                    }
                    Ok((size, _)) = socket.recv_from(&mut buf) => {
                        if let Ok(packet) = rosc::decoder::decode_udp(&buf[..size]) {
                            println!("Handling packet: ");
                            handle_packet(packet.1, &on_receive, &filter_prefix);
                        }
                    }
                }
            }
        });
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.close_handle.take() {
            let _ = handle.send(());
        }
    }
}


fn handle_packet(packet: OscPacket, callback: &Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>, filter_prefix:&str) {
    match packet {
        OscPacket::Message(msg) => {
            println!("Got message: {}", msg.addr);
            if filter_prefix == "".to_string() || msg.addr.starts_with(filter_prefix) {
                let cb = callback.lock().unwrap();
                cb(msg);
            }
            return;
        }
        OscPacket::Bundle(bundle) => {
            for p in bundle.content {
                handle_packet(p, callback, filter_prefix);
            }
        }
    }
}
