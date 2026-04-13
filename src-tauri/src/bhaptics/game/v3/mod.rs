mod auth_message;

use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader},
    net::SocketAddr,
    sync::Arc,
};

use super::network::event_map::PatternLocation;
use crate::{
    bhaptics::{game::network, maps::pattern_to_events},
    log_err,
    mapping::{
        event::Event, haptic_node::HapticNode, input_node::{InputNode, InputType},
        InputEventMessage, MapHandle, NodeGroup,
    },
};
use strum::IntoEnumIterator;

use futures_util::{SinkExt, StreamExt};
use rustls_pemfile::{certs, pkcs8_private_keys};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::{rustls, TlsAcceptor};
use tokio_util::sync::CancellationToken;
use tokio_websockets::Message;

const PATH_TO_CERT: &str = "security/localhost.crt";
const PATH_TO_KEY: &str = "security/localhost.key";

pub(crate) struct ApiInfo {
    pub application_id: String,
    pub api_key: String,
    pub creator_id: String,
    pub workspace_id: String,
}

/// per Connection local state.
struct ConnectionState {
    game_mapping: HashMap<String, Vec<Event>>,
    api_info: Option<ApiInfo>,
    name: Option<String>,
}

pub async fn run_server(map: MapHandle, token: CancellationToken) {
    if let Err(e) = run_server_inner(map, token).await {
        log::error!("bHaptics V3 server error: {:?}", e);
    }
}

async fn run_server_inner(map: MapHandle, token: CancellationToken) -> io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 15882));
    let certs = load_certs(PATH_TO_CERT)?;
    let key = load_key(PATH_TO_KEY)?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind(&addr).await?;
    log::info!("bHaptics V3 API server started on {}", addr);

    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let acceptor = acceptor.clone();
                        let map = map.clone();
                        let child = token.child_token();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, acceptor, map, child).await {
                                log::error!("V3 connection error: {:?}", e);
                            }
                        });
                    }
                    Err(e) => log::error!("V3 accept error: {:?}", e),
                }
            }
        }
    }

    log::info!("bHaptics V3 listener terminated.");
    Ok(())
}


async fn handle_connection(
    stream: tokio::net::TcpStream,
    acceptor: TlsAcceptor,
    map: MapHandle,
    token: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tls_stream = acceptor.accept(stream).await?;
    let (_request, ws_stream) = tokio_websockets::ServerBuilder::new()
        .accept(tls_stream)
        .await?;
    log::info!("V3 WebSocket connection established");

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Forward outgoing messages to the WebSocket writer half.
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    insert_bhaptics_maps(&map).await;

    let mut state = ConnectionState {
        game_mapping: HashMap::new(),
        api_info: None,
        name: None,
    };

    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            frame = ws_read.next() => {
                match frame {
                    Some(Ok(msg)) if msg.is_text() => {
                        let raw = msg.as_text().expect("checked is_text");
                        handle_message(raw, &mut state, &map, &tx).await;
                    }
                    Some(Ok(msg)) if msg.is_ping() || msg.is_pong() => {}
                    Some(Ok(_)) => log::warn!("V3: non-text message received"),
                    Some(Err(e)) => {
                        log::error!("V3 WebSocket error: {:?}", e);
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    remove_bhaptics_maps(&map).await;
    log::info!("V3 connection closed");
    Ok(())
}

/// processes each message received over the websocket
async fn handle_message(
    raw: &str,
    state: &mut ConnectionState,
    map: &MapHandle,
    ws_tx: &mpsc::UnboundedSender<Message>,
) {
    match serde_json::from_str::<ReceivedMessage>(raw) {
        Ok(ReceivedMessage::SdkRequestAuthInit(contents)) => {
            handle_auth(&contents, state, ws_tx).await;
        }
        Ok(ReceivedMessage::SdkPlay(payload)) => {
            handle_play(&payload, state, map).await;
        }
        Ok(ReceivedMessage::SdkStopAll(_)) => {
            log_err!(
                map.send_event(InputEventMessage::CancelAllWithTags(
                    vec!["Bhaptics_V3".to_string()]
                ))
                .await
            );
        }
        Err(e) => log::error!("V3 decode error: {} | raw: {:?}", e, raw),
    }
}

/// Responds to the authorization message specifically.
async fn handle_auth(
    contents: &str,
    state: &mut ConnectionState,
    ws_tx: &mpsc::UnboundedSender<Message>,
) {
    log::info!("V3: Received Auth Init");

    let parsed = match auth_message::parse_auth_init(contents) {
        Ok(p) => p,
        Err(e) => {
            log::error!("V3: Failed to parse auth: {}", e);
            return;
        }
    };

    state.name = Some(parsed.name);
    state.api_info = Some(ApiInfo {
        application_id: parsed.application_id.clone(),
        api_key: parsed.api_key.clone(),
        creator_id: parsed.creator_id,
        workspace_id: parsed.workspace_id,
    });

    // Respond to the game immediately so it starts sending events.
    let response = create_init_response();
    if let Ok(json) = serde_json::to_string(&response) {
        let _ = ws_tx.send(Message::text(json));
    }

    // Fetch mappings from the bHaptics HTTP API.
    let api_key = parsed.api_key;
    let app_id = parsed.application_id;

    match tokio::task::spawn_blocking(move || network::fetch_mappings(api_key, app_id, -1)).await {
        Ok(Ok(mapping)) => {
            for hapt in mapping.haptic_mappings {
                let key = hapt.key.clone();
                let events = pattern_to_events(hapt);
                state.game_mapping.insert(key, events);
            }
            log::info!("V3: Loaded {} event mappings", state.game_mapping.len());
        }
        Ok(Err(e)) => log::error!("V3: fetch_mappings error: {:?}", e),
        Err(e) => log::error!("V3: mapping task panicked: {:?}", e),
    }
}

/// Sends a play message to the map
async fn handle_play(payload: &str, state: &ConnectionState, map: &MapHandle) {
    match serde_json::from_str::<SdkPlayMessage>(payload) {
        Ok(msg) => {
            if let Some(events) = state.game_mapping.get(&msg.event_name) {
                log_err!(
                    map.send_event(InputEventMessage::StartEvents(events.clone()))
                        .await
                );
                log::trace!("V3: Started event: {}", msg.event_name);
            } else {
                log::trace!("V3: Unknown event: {}", msg.event_name);
            }
        }
        Err(e) => log::error!("V3: SdkPlay parse error: {} | {:?}", e, payload),
    }
}

/// On connection; initializes our nodes we will address through events later.
async fn insert_bhaptics_maps(map: &MapHandle) {
    for loc in PatternLocation::iter() {
        for index in 0..loc.motor_count() {
            let pos = loc.to_position(index);
            let node = HapticNode {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                groups: vec![NodeGroup::All],
            };
            let tags = vec!["Bhaptics_V3".to_string(), loc.to_input_tag().to_string()];
            if let Some(id) = loc.to_id(index) {
                let input = InputNode::new(node, tags, id, 0.1, InputType::ADDITIVE);
                log_err!(map.send_event(InputEventMessage::InsertNode(input)).await);
            }
        }
    }
}

/// Removes all nodes associated with our connection.
async fn remove_bhaptics_maps(map: &MapHandle) {
    log_err!(
        map.send_event(InputEventMessage::RemoveWithTags(vec![
            "Bhaptics_V3".to_string()
        ]))
        .await
    );
}


fn load_certs(path: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_key(path: &str) -> io::Result<PrivateKeyDer<'static>> {
    pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
        .next()
        .unwrap()
        .map(Into::into)
}

fn create_init_response() -> Vec<SendMessage> {
    vec![
        SendMessage::ServerReady,
        SendMessage::ServerEventNameList(vec!["event_names".to_string()]),
        SendMessage::ServerEventList(vec![]),
    ]
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "message")]
enum ReceivedMessage {
    SdkRequestAuthInit(String),
    SdkPlay(String),
    SdkStopAll(Option<String>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "Type", content = "message")]
pub enum SendMessage {
    ServerReady,
    ServerEventNameList(Vec<String>),
    ServerEventList(Vec<ServerEvent>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEvent {
    pub event_name: String,
    pub event_time: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SdkPlayMessage {
    event_name: String,
    request_id: u32,
    position: u32,
    intensity: f32,
    duration: f32,
    offset_angle_x: f32,
    offset_y: f32,
}