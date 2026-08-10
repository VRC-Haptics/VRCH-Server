use std::{fmt, net::SocketAddr};
use std::sync::Arc;
use anyhow::Context;
use tokio::{io, net::{ToSocketAddrs, UdpSocket}};

use rosc::OscPacket;
use tokio_util::sync::CancellationToken;

/// An osc Server dedictated to sendig and receiving across a single UDP connection.
#[derive(serde::Serialize, Clone)]
pub struct OscServer {
    pub local: SocketAddr,
    pub remote: SocketAddr,
    pub filter_prefix: String,
    pub send: bool,
    #[serde(skip)]
    pub socket: Arc<UdpSocket>,
    #[serde(skip)]
    close_handle: CancellationToken,
}

impl fmt::Debug for OscServer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("OscServer")
            .field("remote", &self.local)
            .field("close_handle", &self.close_handle)
            .field("on_receive", &"Function Pointer")
            .finish()
    }
}

impl OscServer {
    /// create new Osc Server, it will need to be started with the start() command
    /// THE ADDRESS IS USUALLY JUST "0.0.0.0". THIS IS THE ADDRESS WE OWN.
    pub async fn new<F, A>(local: SocketAddr, remote: Option<A>, on_receive: F) -> anyhow::Result<Self>
    where
        F: Fn(OscPacket) + Send + Sync + 'static,
        A: ToSocketAddrs
    {
        // udp sockets can be shared by arcs safely
        let socket = UdpSocket::bind(local).await.context("Failed to bind Socket")?;
        let mut send = false;
        if let Some(addr) = remote {
            socket.connect(addr).await.context("Binding to address: {addr:?}")?;
            send = true;
        }
        let socket = Arc::new(socket);
        let send_socket = Arc::clone(&socket);

        let token = CancellationToken::new();
        let tok_clone = token.clone();
        let callback = on_receive;
        tokio::spawn(
            token.run_until_cancelled_owned(async move {
                log::trace!(
                    "Spawned UDP OSC Server on: {}",
                    socket.local_addr().unwrap()
                );

                let mut buf = [0u8; rosc::decoder::MTU];
                loop {
                    match socket.recv_from(&mut buf).await {
                        Ok((size, _src))=> {
                            match rosc::decoder::decode_udp(&buf[..size]) {
                                Ok((_, packet)) => callback(packet),
                                Err(e) => {
                                    if let rosc::OscError::BadPacket(_) = e {
                                        continue;
                                    }
                                    log::error!("Failed to decode OSC packet: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Error receiving packet: {:?}", e);
                        }
                    }

                }
            })
        );

        Ok(OscServer {
            local: send_socket.local_addr().context("Fetching Local Addr")?,
            remote: send_socket.peer_addr().context("Could not fetch peer addr")?,
            close_handle: tok_clone,
            send,
            filter_prefix: "".to_string(),
            socket: Arc::clone(&send_socket),
        })
    }

    /// send data to the connected
    pub async fn send(&self, data: &[u8]) -> io::Result<usize> {
        self.socket.send(data).await
    }

    //kills the server thread.
    pub fn stop(&mut self) {
        self.close_handle.cancel()
    }
}
