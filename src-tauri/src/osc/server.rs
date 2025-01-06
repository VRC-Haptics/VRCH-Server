use rosc::{OscPacket, OscMessage};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

pub struct OscServer {
    port: u16,
    address: Ipv4Addr,
    close_handle: Option<mpsc::Sender<()>>,
    on_receive: Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>,
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
            on_receive: Arc::new(Mutex::new(on_receive)),
        }
    }

    pub async fn start(&mut self) {
        let addr = SocketAddr::new(self.address.into(), self.port);
        let socket = UdpSocket::bind(addr).await.unwrap();
        let (tx, mut rx) = mpsc::channel(1);
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
                            handle_packet(packet.1, &on_receive);
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


fn handle_packet(packet: OscPacket, callback: &Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>) {
    match packet {
        OscPacket::Message(msg) => {
            let cb = callback.lock().unwrap();
            cb(msg);
        }
        OscPacket::Bundle(bundle) => {
            for p in bundle.content {
                handle_packet(p, callback);
            }
        }
    }
}
