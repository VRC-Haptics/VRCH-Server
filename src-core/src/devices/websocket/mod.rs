//! src/devices/websocket.rs
//!
//! Simple WebSocket haptic device. We run a server; each client connection is
//! one device. Wire protocol:
//!   client -> server (text/JSON):
//!     { "type": "hello",      "id": "...", "name": "...", "nodes": [HapticNode, ...] }
//!     { "type": "updateInfo", "nodes": [HapticNode, ...] }
//!   server -> client (binary): tightly packed little-endian f32, one per node.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Notify};
use tokio::time::{interval, timeout};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

use crate::devices::{Device, DeviceId, DeviceInfo, DeviceMessage};
use crate::mapping::haptic_node::HapticNode;

const WS_BIND_ADDR: &str = "0.0.0.0:8431";
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const KEEPALIVE: Duration = Duration::from_secs(10);

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WebsocketDeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub nodes: Vec<HapticNode>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClientMessage {
    Hello {
        id: String,
        #[serde(default)]
        name: String,
        nodes: Vec<HapticNode>,
    },
    UpdateInfo {
        nodes: Vec<HapticNode>,
    },
}

#[derive(Debug)]
pub struct WebsocketDevice {
    id: DeviceId,
    info: Arc<RwLock<DeviceInfo>>,
    buffer: Arc<RwLock<Vec<f32>>>,
    dirty: Arc<Notify>,
    token: CancellationToken,
    manager: Option<mpsc::Sender<DeviceMessage>>,
}

impl Device for WebsocketDevice {
    fn get_id(&self) -> DeviceId {
        self.id.clone()
    }

    fn info(&self) -> DeviceInfo {
        self.info.read().clone()
    }

    fn update_info(&self, new: DeviceInfo) {
        // The map does NOT resize our output buffer on node change; it only logs
        // a mismatch. So we keep buffer length == node count here.
        let n = new.get_nodes().len();
        {
            let mut buf = self.buffer.write();
            if buf.len() != n {
                buf.resize(n, 0.0);
            }
        }
        *self.info.write() = new;
    }

    fn get_feedback_buffer(&self) -> Arc<RwLock<Vec<f32>>> {
        self.buffer.clone()
    }

    fn buffer_updated(&self) {
        // Coalescing wakeup. We always send the *latest* buffer, never dedupe,
        // so a held-constant value keeps transmitting.
        self.dirty.notify_one();
    }

    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>) {
        self.manager = Some(tx);
    }

    fn disconnect(&mut self) {
        self.token.cancel();
        if let Some(m) = &self.manager {
            let _ = m.try_send(DeviceMessage::Remove(self.id.clone()));
        }
    }
}

#[inline]
fn encode_feedback(buf: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() * 4);
    for v in buf {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Binds the WS listener and spawns the accept loop, then returns. Mirrors
/// `start_wifi_devices`: non-blocking init.
pub async fn start_websocket_devices(device_channel: mpsc::Sender<DeviceMessage>) {
    let listener = match TcpListener::bind(WS_BIND_ADDR).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("Failed to bind websocket device listener on {WS_BIND_ADDR}: {e}");
            return;
        }
    };
    log::info!("Websocket device listener bound on {WS_BIND_ADDR}");

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let chan = device_channel.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr, chan).await {
                            log::trace!("ws device connection {addr} ended: {e}");
                        }
                    });
                }
                Err(e) => {
                    log::warn!("ws accept error: {e}");
                }
            }
        }
    });
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    device_channel: mpsc::Sender<DeviceMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws = accept_async(stream).await?;

    // First frame must be Hello.
    let first = timeout(HELLO_TIMEOUT, ws.next())
        .await
        .map_err(|_| "hello timeout")?
        .ok_or("closed before hello")??;

    let hello = match first {
        Message::Text(t) => serde_json::from_str::<ClientMessage>(&t)?,
        _ => return Err("first frame was not text hello".into()),
    };
    let (id, name, nodes) = match hello {
        ClientMessage::Hello { id, name, nodes } => (DeviceId(id), name, nodes),
        ClientMessage::UpdateInfo { .. } => return Err("expected hello, got updateInfo".into()),
    };

    let buffer = Arc::new(RwLock::new(vec![0.0_f32; nodes.len()]));
    let dirty = Arc::new(Notify::new());
    let token = CancellationToken::new();
    let info = Arc::new(RwLock::new(DeviceInfo::Websocket(WebsocketDeviceInfo {
        id: id.clone(),
        name,
        nodes,
    })));

    let device = WebsocketDevice {
        id: id.clone(),
        info: info.clone(),
        buffer: buffer.clone(),
        dirty: dirty.clone(),
        token: token.clone(),
        manager: Some(device_channel.clone()),
    };
    device_channel
        .send(DeviceMessage::Register(device.into()))
        .await?;
    log::info!("Registered ws device {id:?} from {addr}");

    let (mut sink, mut source) = ws.split();

    // Writer: stream feedback on every notify + periodic keepalive ping.
    let w_token = token.clone();
    let w_buffer = buffer.clone();
    let w_dirty = dirty.clone();
    let writer = tokio::spawn(async move {
        let mut ka = interval(KEEPALIVE);
        loop {
            tokio::select! {
                _ = w_token.cancelled() => break,
                _ = w_dirty.notified() => {
                    let payload = { encode_feedback(&w_buffer.read()) };
                    if sink.send(Message::Binary(payload.into())).await.is_err() {
                        break;
                    }
                }
                _ = ka.tick() => {
                    if sink.send(Message::Ping(Default::default())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = sink.close().await;
        w_token.cancel();
    });

    // Reader: handle info updates / close.
    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            msg = source.next() => match msg {
                Some(Ok(Message::Text(t))) => {
                    match serde_json::from_str::<ClientMessage>(&t) {
                        Ok(ClientMessage::UpdateInfo { nodes }) => {
                            {
                                let mut buf = buffer.write();
                                if buf.len() != nodes.len() {
                                    buf.resize(nodes.len(), 0.0);
                                }
                            }
                            info.write().set_nodes(nodes);
                            let _ = device_channel
                                .send(DeviceMessage::InfoDirty(id.clone()))
                                .await;
                        }
                        Ok(ClientMessage::Hello { .. }) => {
                            log::warn!("ws device {id:?} sent duplicate hello, ignoring");
                        }
                        Err(e) => log::warn!("ws device {id:?} bad message: {e}"),
                    }
                }
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(e)) => {
                    log::trace!("ws device {id:?} read error: {e}");
                    break;
                }
                _ => {}
            }
        }
    }

    token.cancel();
    let _ = writer.await;
    let _ = device_channel.send(DeviceMessage::Remove(id.clone())).await;
    log::info!("ws device {id:?} disconnected");
    Ok(())
}