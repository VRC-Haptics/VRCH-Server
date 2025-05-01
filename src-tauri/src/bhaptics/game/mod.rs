/// A mess of serialization crap that sorta works to deserialize the weirdly formatted AuthenticationInit Message
mod auth_message;
mod player_messages;
pub mod network;

use auth_message::handle_auth_init;
use serde;

use std::{
    fs::File,
    io::{self, BufReader},
    net::SocketAddr,
    sync::{Arc, Mutex},
    thread,
};

use futures_util::{SinkExt, StreamExt};
use rustls_pemfile::{certs, pkcs8_private_keys};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::{rustls, TlsAcceptor};
use tokio_util::sync::CancellationToken;
use tokio_websockets::Message;
use tokio_rustls::rustls::crypto::CryptoProvider;

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

/// Holds information for the bhaptics game server.
pub struct BhapticsGame {
    // if a game has been connected
    pub game_connected: bool,
    // info for bHaptics API
    pub api_info: Option<ApiInfo>,
    // user facing name
    pub name: Option<String>,
    // app sdk version
    pub sdk_api_version: Option<u32>,
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

impl BhapticsGame {
    /// Creates a new instance, starts the server on a separate thread,
    /// and returns an Arc-wrapped and Mutex-guarded game state.
    pub fn new() -> Arc<Mutex<Self>> {
        let shutdown_token = CancellationToken::new();
        let game = Arc::new(Mutex::new(BhapticsGame {
            game_connected: false,
            api_info: None,
            name: None,
            sdk_api_version: None,
            ws_sender: None,
            shutdown_token: shutdown_token.clone(),
        }));

        let game_clone = Arc::clone(&game);
        // Spawn the server thread.
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async {
                if let Err(e) = run_server(game_clone, shutdown_token.clone()).await {
                    log::error!("Server error: {:?}", e);
                }
            });
        });

        game
    }

    /// Shuts down the TCP listener by signalling the cancellation token.
    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }

    /// Sends a text message over the established WebSocket connection.
    ///
    /// If no connection is available, a warning is logged.
    pub fn send(&self, msg: Vec<SendMessage>) {
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

/// Runs the server by setting up TLS, binding the TCP listener, and
/// handling incoming connections with cancellation support.
async fn run_server(
    game: Arc<Mutex<BhapticsGame>>,
    shutdown_token: CancellationToken,
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
    log::info!("bHaptics server started on {}", addr);

    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                break;
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _)) => {
                        let acceptor_clone = acceptor.clone();
                        let game_clone = Arc::clone(&game);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(
                                stream,
                                acceptor_clone,
                                game_clone
                            ).await {
                                log::error!("Connection error: {:?}", e);
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to accept connection: {:?}", e);
                    }
                }
            }
        }
    }
    log::info!("Listener loop terminated.");
    Ok(())
}

/// Handles an individual incoming connection, performing the TLS handshake,
/// upgrading to WebSocket, and managing messaging.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    acceptor: TlsAcceptor,
    game: Arc<Mutex<BhapticsGame>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = acceptor.accept(stream).await?;
    let (_request, ws_stream) = tokio_websockets::ServerBuilder::new()
        .accept(stream)
        .await?;
    log::info!("New WebSocket connection established");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<tokio_websockets::Message>();

    // Store the sender in the shared game state.
    {
        let mut game_lock = game.lock().unwrap();
        game_lock.ws_sender = Some(tx);
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

    // Process incoming messages.
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) if msg.is_text() => {
                msg_received(msg, Arc::clone(&game));
            }
            Ok(msg) if msg.is_ping() || msg.is_pong() => {
                // Ignore ping/pong messages.
            }
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
fn msg_received(msg: Message, game: Arc<Mutex<BhapticsGame>>) {
    // Convert the message into a String.
    let raw_text = msg.as_text().expect("Failed to convert message to text");
    let decoded: RecievedMessage =
        serde_json::from_str(&raw_text).expect("couldn't decode incoming packet");

    //  Need to handle errors here
    match decoded {
        RecievedMessage::SdkRequestAuthInit(contents) => handle_auth_init(&contents, game),
        RecievedMessage::SdkPlay(event) => handle_sdk_play(&event, &game),
        RecievedMessage::SdkStopAll => log::error!("SdkStopAll not impelemented"),
    }
}

fn handle_sdk_play(input: &str, _game: &Arc<Mutex<BhapticsGame>>) {
    let content: SdkPlayMessage =
        serde_json::from_str(input).expect("Couldn't decode play request");
    log::debug!("Play Event: {:?}", content);
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
    SdkStopAll,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "Type", content = "message")]
pub enum SendMessage {
    ServerReady,
    ServerEventNameList(Vec<String>),
    ServerEventList(Vec<ServerEvent>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerEvent {
    #[serde(rename = "eventName")]
    event_name: String,
    #[serde(rename = "eventTime")]
    event_time: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SdkPlayMessage {
    #[serde(rename = "eventName")]
    event_name: String,
    #[serde(rename = "requestId")]
    request_id: u32,
    position: u32,
    intensity: f32,
    duration: f32,
    #[serde(rename = "offsetAngleX")]
    offset_angle_x: f32,
    #[serde(rename = "offsetY")]
    offset_y: f32,
}
