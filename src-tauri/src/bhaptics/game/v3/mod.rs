mod auth_message;

use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader},
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use auth_message::handle_auth_init;
use crate::mapping::{event::Event, global_map::GlobalMap, haptic_node::HapticNode, NodeGroup};
use super::{BhapticsGame, PatternLocation};
use serde;
use strum::IntoEnumIterator;

use futures_util::{SinkExt, StreamExt};
use rustls_pemfile::{certs, pkcs8_private_keys};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::{rustls, TlsAcceptor};
use tokio_util::sync::CancellationToken;
use tokio_websockets::Message;

/// Holds information for the bhaptics v3 api.
pub struct BhapticsApiV3 {
    // list of events that represent 'EventName' String.
    pub game_mapping: HashMap<String, Vec<Event>>,
    // events that were triggered since last tick.
    new_events: Vec<String>,
    // info for bHaptics API
    pub api_info: Option<ApiInfo>,
    // user facing name
    pub name: Option<String>,
    // Channel for sumbitting messages to our sender.
    ws_sender: Option<mpsc::UnboundedSender<Message>>,
    // shuts down the TCP server.
    shutdown_token: CancellationToken,
}

pub struct ApiInfo {
    application_id: String,
    api_key: String,
    creator_id: String,
    workspace_id: String,
}

impl BhapticsApiV3 {
    /// Creates a new instance, starts the server on a separate thread,
    /// and returns an Arc-wrapped and Mutex-guarded game state.
    pub fn new(game:Arc<Mutex<GlobalMap>>) -> Arc<Mutex<Self>> {
        let shutdown_token = CancellationToken::new();
        let api = Arc::new(Mutex::new(BhapticsApiV3 {
            game_mapping: HashMap::new(),
            new_events: Vec::new(),
            api_info: None,
            name: None,
            ws_sender: None,
            shutdown_token: shutdown_token.clone(),
        }));

        let api_clone = Arc::clone(&api);
        // this block runs at most once, no matter how many times new() is called
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async move {
                log::trace!("Started bhaptics thread");
                if let Err(e) = run_server(api_clone, shutdown_token, game).await {
                    log::error!("Server error: {e:?}");
                }
            });
        });

        api
    }

    /// Returns the list of events that were triggerd during this tick.
    pub fn tick(&mut self) -> Vec<Event> {
        let mut new_events = Vec::new();
        let new_events_names = std::mem::take(&mut self.new_events);
        
        // Iterate over the events collected for THIS tick
        for event_name in new_events_names { 
            if let Some(mapped_events) = self.game_mapping.get(&event_name) {
                new_events.extend(mapped_events.iter().cloned());
            } else {
                log::trace!("Unknown event with name: {}", event_name);
            }
        }

        new_events
    }

    /// Kills this API instance
    /// 
    /// Shuts down the TCP listener by signalling the cancellation token.
    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }

    /// Sends a text message over the established WebSocket connection.
    ///
    /// If no connection is available, a warning is logged.
    fn send(&self, msg: Vec<SendMessage>) {
        if let Some(ref sender) = self.ws_sender {
            let data = serde_json::to_string(&msg).expect("couldn't create json for sending");
            if let Err(e) = sender.send(Message::text(data)) {
                log::error!("Failed to send message: {}", e);
            }
        } else {
            log::warn!("No WebSocket connection available.");
        }
    }
}

/// Inserts the default bhaptics maps as inputs.
fn insert_bhaptics_maps(map: Arc<Mutex<GlobalMap>>) {
    let input_lock = map.lock().expect("Unable to lock input_list");

    for loc in PatternLocation::iter() {
        for index in 0..loc.motor_count() {
            let position = loc.to_position(index);
            let node = HapticNode {
                x: position.x,
                y: position.y,
                z: position.z,
                groups: vec![NodeGroup::All], //TODO: Actually make the groups apply right.
            };
            let tags = vec![
                "Bhaptics_V3".to_string(),
                loc.to_input_tag().to_string(),
            ];
            if let Some(id) = loc.to_id(index) {
                // doesn't really matter if it is already there, we want to keep only one instance.
                let _ = input_lock.add_input_node(node, tags, id.0);
            }
        }
    }
}

/// Removes all bhaptics maps from the global input list.
fn remove_bhaptics_maps(map: Arc<Mutex<GlobalMap>>) {
    let input_lock = map.lock().expect("Unable to lock inputs");
    input_lock.remove_all_with_tag(&"Bhaptics_V3".to_string());
}


/// Runs the server by setting up TLS, binding the TCP listener, and
/// handling incoming connections with cancellation support.
async fn run_server(
    api: Arc<Mutex<BhapticsApiV3>>,
    shutdown_token: CancellationToken,
    game: Arc<Mutex<GlobalMap>>,
) -> io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 15882));
    let certs = load_certs(PATH_TO_CERT).expect("couldn't load cert");
    let key = load_key(PATH_TO_KEY).expect("couldn't load key");

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = match TcpListener::bind(&addr).await {
        Ok(list) => list,
        Err(e) => {
            log::error!("Error connecting to bhaptics dedicated port: {}", e);
            return Err(e);
        }
    };
    log::info!("bHaptics V3 API server started on {}", addr);

    // loop every time we gain a new connection.
    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                break;
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _)) => {
                        let acceptor_clone = acceptor.clone();
                        let game_clone = Arc::clone(&api);
                        let map_clone = Arc::clone(&game);
                        tokio::spawn(async move {
                            insert_bhaptics_maps(map_clone.clone());
                            if let Err(e) = handle_connection(
                                stream,
                                acceptor_clone,
                                game_clone
                            ).await {
                                log::error!("Connection error: {:?}", e);
                                
                            };
                            remove_bhaptics_maps(map_clone);
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to accept connection: {:?}", e);
                    }
                }
            }
        }
        // after each disconneciton remove our maps from input.
    }
    log::info!("Listener loop terminated.");
    Ok(())
}

/// Handles an individual incoming connection, performing the TLS handshake,
/// upgrading to WebSocket, and managing messaging.
/// Blocks until connection is terminated.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    acceptor: TlsAcceptor,
    api: Arc<Mutex<BhapticsApiV3>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = acceptor.accept(stream).await?;
    let (_request, ws_stream) = tokio_websockets::ServerBuilder::new()
        .accept(stream)
        .await?;
    log::info!("New WebSocket connection established");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<tokio_websockets::Message>();

    // Store sending tx into api class
    {
        let mut api_lock = api.lock().unwrap();
        api_lock.ws_sender = Some(tx);
    }

    // Spawn a task to forward outgoing messages.
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                log::error!("Failed to send message over WebSocket");
                break;
            }
        }
    });

    log::trace!("Connected to a bhaptics v3 game.");

    // Process incoming messages.
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) if msg.is_text() => {
                msg_received(msg, Arc::clone(&api));
            }
            Ok(msg) if msg.is_ping() || msg.is_pong() => { /* Ignore ping/pong messages.*/ }
            Ok(_) => {
                log::warn!("Received non-text message");
            }
            Err(e) => {
                log::error!("WebSocket error: {:?}", e);
                break;
            }
        }
    }
    Ok(())
}

/// Handles decoding message strings into their respective structs.
fn msg_received(msg: Message, api: Arc<Mutex<BhapticsApiV3>>) {
    let raw_text = msg
        .as_text()
        .expect("Failed to convert message to text");

    match serde_json::from_str::<RecievedMessage>(raw_text) {
        Ok(RecievedMessage::SdkRequestAuthInit(contents)) => {
            handle_auth_init(&contents, api)
        }
        Ok(RecievedMessage::SdkPlay(event)) => {
            handle_sdk_play(api, &event);
            log::trace!("Played event: {}", event);
        }
        Ok(RecievedMessage::SdkStopAll(_)) => {
            log::trace!("Stop all not implemented");
        }
        Err(e) => log::error!("Error decoding {:?}: {}", raw_text, e),
    }
}


fn handle_sdk_stop(game: &Arc<Mutex<BhapticsGame>>) {
    let lock = game.lock().expect("Couldn't get game lock");
    let mut in_list = lock.global_map.lock().expect("couldn't lock input_list");
    in_list.clear_events(&"Bhaptics".to_string());
}

fn handle_sdk_play(api: Arc<Mutex<BhapticsApiV3>>, input: &str) {
    let mut api_lock = api.lock().expect("Api lock");
    
    let content = serde_json::from_str::<SdkPlayMessage>(input);
    match content {
        Ok(content) => {
            api_lock.new_events.push(content.event_name);
        }
        Err(err) => log::error!(
            "Error decoding bhaptics play message: {} \n Content: {:?}", err, input
        ),
    }
}

const PATH_TO_CERT: &str = "security/localhost.crt";
const PATH_TO_KEY: &str = "security/localhost.key";

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
    let messages: Vec<SendMessage> = vec![
        SendMessage::ServerReady,
        SendMessage::ServerEventNameList(vec!["event_names".to_string()]),
        SendMessage::ServerEventList(vec![]),
    ];

    return messages;
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "message")]
/// Intermediary enum to direct string parsing
enum RecievedMessage {
    /// The first message sent from the game
    SdkRequestAuthInit(String),
    /// The message that triggers the start of a haptic event
    SdkPlay(String),
    /// Clears all active events
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
    event_name: String,
    event_time: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkPlayMessage {
    event_name: String,
    request_id: u32,
    position: u32,
    intensity: f32,
    duration: f32,
    offset_angle_x: f32,
    offset_y: f32,
}
