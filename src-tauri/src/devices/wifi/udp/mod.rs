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

pub mod broadcast;

pub enum UdpError {
    GenericTokio(tokio::io::Error),
}

impl From<tokio::io::Error> for UdpError {
    fn from(value: tokio::io::Error) -> Self {
        Self::GenericTokio(value)
    }
}

static SERVER_SOCKET: OnceCell<Arc<UdpSocket>> = OnceCell::const_new();
static LISTENERS: LazyLock<DashMap<SocketAddr, Vec<Sender<Arc<[u8]>>>>> =
    LazyLock::new(DashMap::new);

async fn get_socket() -> &'static Arc<UdpSocket> {
    SERVER_SOCKET
        .get_or_init(|| async {
            Arc::new(
                UdpSocket::bind("0.0.0.0:0")
                    .await
                    .expect("Unable to bind to address."),
            )
        })
        .await
}

/// Retrieves a tokio channel that recieves a reference to all recieved packets.
/// 
/// On Drop; The reciever will be cleaned up periodically.
pub async fn subscribe(addr: SocketAddr) -> Receiver<Arc<[u8]>> {
    let (tx, rx) = channel(5);
    LISTENERS.entry(addr).or_insert_with(Vec::new).push(tx);
    rx
}

async fn handle_packet(addr: &SocketAddr, data: Arc<[u8]>) {
    if let Some(senders) = LISTENERS.get(addr) {
        for sender in senders.iter() {
            let _ = sender.send(data.clone()).await;
        }
    }
}

async fn clean_subscribers() {
    for mut list in LISTENERS.iter_mut() {
        list.retain(|sender| !sender.is_closed());
    }
}

/// Sends the specified `data` to `address` over the common socket
pub async fn send_udp(data: &[u8], addr: &SocketAddr) -> Result<usize, UdpError> {
    let sock = get_socket().await;
    sock.send_to(data, addr)
        .await
        .map_err(|e| UdpError::GenericTokio(e))
}

/// Intended to only be called once, initializes the common UDP socket server.
pub async fn start_udp() {
    // initialize socket
    let socket = get_socket().await;

    // start forwarding messages to the recievers
    tokio::task::spawn(async move {
        let mut buf = [0u8; 65535];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data: Arc<[u8]> = buf[..len].into();
                    handle_packet(&addr, data).await;
                }
                Err(e) => {
                    eprintln!("recv error: {e}");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
    });

    // spawn task to periodically check all subscribers are still alive.
    tokio::task::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            clean_subscribers();
        }
    });
}
