mod player_messages;
/// A mess of serialization crap that sorta works to deserialize the weirdly formatted AuthenticationInit Message
mod auth_message;

use auth_message::AuthInitMessage;

use std::fs::File;
use std::io::Read;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use native_tls::{Identity, TlsAcceptor};
use rustls::lock::Mutex;
use serde;
use async_tungstenite::;

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
}

pub struct ApiInfo {
    application_id: String,
    api_key: String,
    creator_id: String,
    workspace_id: String,
}

impl BhapticsGame {
    /// Create an instance of BhapticsGame. 
    /// Instantiates the Websocket server and prepares to recieve connections
    pub fn new() -> Arc<Mutex<BhapticsGame>> {
        let game = BhapticsGame {
            game_connected: false,
            api_info: None,
            name: None,
            sdk_api_version: None,
            recv_msgs: Vec::new(),
        };
        let shared_game = Arc::new(Mutex::new(game));

        // setup the game conneciton server.
        let game_clone = Arc::clone(&shared_game);
        thread::spawn(move || {
             // Create a new Tokio runtime in this thread.
             let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
             rt.block_on(async move {
            // Setup TLS
            let mut file =
                File::open("./security/bhaptics.pfx").expect("unable to get security identity");
            let mut identity = vec![];
            file.read_to_end(&mut identity).unwrap();
            let identity = Identity::from_pkcs12(&identity, "bhaptics").unwrap();
            let tls_acceptor = TlsAcceptor::builder(identity)
                .build()
                .expect("Failed to build TLS acceptor.");
            let tls_acceptor = Arc::new(tls_acceptor);

            let server =
                TcpListener::bind("127.0.0.1:15882").expect("unable to bind to bhaptics port");
            log::info!("Started bHaptics Server");

            // Accept incoming connections.
            for stream in server.incoming() {
                let game_thread = Arc::clone(&game_clone);
                let stream = stream.expect("unable to unwrap the stream");
                let tls_acceptor_thread = Arc::clone(&tls_acceptor);

                thread::spawn(move || {
                    let tls_stream = tls_acceptor_thread
                        .accept(stream)
                        .expect("Failed to accept TLS connection.");

                    // Upgrade the TLS stream to a WebSocket connection.
                    let mut websocket =
                        tungstenite::accept(tls_stream).expect("Failed to upgrade to WebSocket.");

                    // Read messages from the WebSocket.
                    loop {
                        match websocket.read() {
                            Ok(msg) => {
                                if msg.is_ping() || msg.is_pong() {
                                    continue;
                                } else if msg.is_text() {
                                    log::trace!("Received text message");
                                    // Pass the message along with the shared game instance.
                                    msg_received(msg, Arc::clone(&game_thread));
                                } else {
                                    log::error!("Received non-text message: {:?}", msg.into_data());
                                }

                                // Mark game as connected.
                                let mut game_lock =
                                    game_thread.lock().expect("couldn't lock game for update");
                                game_lock.game_connected = true;
                            }
                            Err(e) => {
                                log::error!("Error reading message: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
        });

        shared_game
    }

    pub fn do_something(&self) {
        log::info!("Doing something");
    }
}

/// Handles decoding message strings into their respective structs.
fn msg_received(msg: Message, game: Arc<Mutex<BhapticsGame>>) {
    // Convert the message into a String.
    let raw_text = msg.into_text()
        .expect("Failed to convert message to text");
    let decoded: RecievedMessage = serde_json::from_str(&raw_text)
        .expect("couldn't decode incoming packet");

    //  Need to handle errors here
    match decoded {
        RecievedMessage::SdkRequestAuthInit(contents) => 
            handle_auth_init(&contents, game),
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "message")]
/// Intermediary enum to direct string parsing
enum RecievedMessage {
    /// The first message sent from the game
    SdkRequestAuthInit(String),
}

/// Handler for SdkRequestAuthInit messages.
fn handle_auth_init(contents: &str, game: Arc<Mutex<BhapticsGame>>) {
    log::info!("Recieved Auth Init message.");
    
    let new = contents.replace(r"\\", "");

    //Trim weird extra escape characters
    let init_msg = AuthInitMessage::from_message_str(&new);
    match init_msg {
        Ok(msg) => {          
            let new_info = ApiInfo {
                application_id: msg.authentication.application_id,
                api_key: msg.authentication.sdk_api_key,
                creator_id: msg.haptic.message.creator,
                workspace_id: msg.haptic.message.workspace_id,
            };

            let mut game_lock = game.lock()
                .expect("could not lock BhapticsGame");
            game_lock.api_info = Some(new_info);

            game_lock.name = Some(msg.haptic.message.name);
            game_lock.sdk_api_version = Some(msg.haptic.message.version);

            log::info!("Need to handle saving this info maybe?");
        },
        Err(err) => {
            log::error!("Unable to parse authorization message: {}", err);
        }
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "Type", content = "message")]
enum SentMessage {
    ServerReady,
    ServerEventNameList(Vec<String>),
    ServerEventList(Vec<HapticEvent>)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct HapticEvent {
    #[serde(rename = "eventName")]
    event_name: String,
    #[serde(rename = "eventTime")]
    event_time: u32,
}
