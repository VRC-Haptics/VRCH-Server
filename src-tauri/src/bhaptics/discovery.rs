use crate::util::next_free_tcp_port;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use log;

use serde::Serialize;
use serde_json;

const UDP_PORT: u16 = 15884;

pub struct Bhaptics {
    // if we are broadcasting for bhaptics games
    pub broadcasting: Arc<Mutex<bool>>,
    // if a game has been connected
    pub game_connected: Mutex<bool>,
    // available haptic events for this game
    pub events: Mutex<HashMap<bhapticsKey, bhapticsEvent>>,
    // The port tcp messages go through
    pub tcp_port: Arc<u16>,
    // tcp connection for games
    game_stream: Arc<Mutex<Option<TcpStream>>>,
    // game ping handle
    broadcast_handle: Option<thread::JoinHandle<()>>,
}

impl Bhaptics {
    pub fn new() -> Bhaptics {
        let tcp_port = next_free_tcp_port(1000).unwrap();

        let mut baptics = Bhaptics {
            broadcasting: Arc::new(Mutex::new(true)),
            game_connected: Mutex::new(false),
            events: Mutex::new(HashMap::new()),
            tcp_port: Arc::new(tcp_port),
            game_stream: Arc::new(Mutex::new(None)),
            broadcast_handle: None,
        };

        baptics.start_tcp_listener_thread();
        baptics.start_broadcast_thread();

        return baptics;
    }

    /// Periodically sends UdpMessage over the specified port.
    fn start_broadcast_thread(&mut self) {
        {
            let mut flag = self
                .broadcasting
                .lock()
                .expect("Failed to lock broadcasting flag");
            *flag = true;
        }
        let broadcasting = Arc::clone(&self.broadcasting);
        let port = Arc::clone(&self.tcp_port);

        // Spawn the thread.
        let handle = thread::spawn(move || {
            let socket = UdpSocket::bind("0.0.0.0:0").expect("Could not bind UDP socket");
            let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::), UDP_PORT);

            // Loop until broadcasting is set to false.
            loop {
                if !*broadcasting
                    .lock()
                    .expect("Failed to lock broadcasting flag")
                {
                    break;
                }

                // Create message
                let msg = UdpMessage {
                    user_id: "rust".to_string(),
                    port: *port,
                };

                // Serialize the message to JSON.
                let json_str = serde_json::to_string(&msg).expect("Failed to serialize message");
                if let Err(e) = socket.send_to(json_str.as_bytes(), target) {
                    log::error!("Error sending UDP message: {}", e);
                }// else {
                    //log::info!("Sent message:{} to: {:?}", json_str, target);
                //}
                thread::sleep(Duration::from_secs(1));
            }
        });
        self.broadcast_handle = Some(handle);
    }

    // Start a thread that listens on the TCP stream for an AuthenticationMessage.
    fn start_tcp_listener_thread(&mut self) {
        // clone to local variables
        let broadcasting = Arc::clone(&self.broadcasting);
        let tcp_port = *self.tcp_port;
        let game_stream = Arc::clone(&self.game_stream);

        let address = format!("{}:{}", Ipv4Addr::LOCALHOST, tcp_port);
        let listener = TcpListener::bind(&address).expect("couldn't bind TCP listener");
        log::info!("TCP listener bound on {}", address);

        thread::spawn(move || {
            // Continue accepting connections as long as broadcasting is true.
            while *broadcasting
                .lock()
                .expect("Failed to lock broadcasting flag")
            {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        log::info!("Accepted TCP connection from {}", addr);
                        {
                            let mut gs = game_stream.lock().expect("Failed to lock game_stream");
                            *gs = Some(stream.try_clone().expect("Failed to clone TcpStream"));
                        }
                        // Process the connection.
                        let mut reader = BufReader::new(stream);
                        loop {
                            let mut line = String::new();
                            match reader.read_line(&mut line) {
                                Ok(0) => {
                                    // Connection closed.
                                    log::info!("TCP connection from {} closed.", addr);
                                    let mut gs =
                                        game_stream.lock().expect("Failed to lock game_stream");
                                    *gs = None;
                                    break;
                                }
                                Ok(_) => {
                                    log::info!("Received TCP message: {}", line.trim_end());
                                    if let Ok(auth_msg) =
                                        serde_json::from_str::<AuthenticationMessage>(&line)
                                    {
                                        log::info!("AuthenticationMessage received: appID:\"{}\" apiKey:\"{}\"",
                                            auth_msg.applicationId, auth_msg.sdkApiKey);
                                        // Stop UDP broadcasting.
                                        let mut flag = broadcasting
                                            .lock()
                                            .expect("Failed to lock broadcasting flag");
                                        *flag = false;
                                        break;
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error reading from TCP stream: {}", e);
                                    let mut gs =
                                        game_stream.lock().expect("Failed to lock game_stream");
                                    *gs = None;
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error accepting connection: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            log::info!("TCP listener thread terminating.");
        });
    }

    pub fn do_something(&self) {
        log::info!("Doing something");
    }
}

/// We send this message to initialize the connection with the game.
#[derive(Serialize)]
struct UdpMessage {
    #[serde(rename = "userId")]
    user_id: String,
    port: u16,
}

///
#[derive(serde::Deserialize, Debug)]
struct AuthenticationMessage {
    applicationId: String,
    sdkApiKey: String,
    version: u8,
}

/// Class to hold all information needed to play back a bhaptics pattern.
pub struct bhapticsEvent {
    todo: bool,
}
/// Wrapper class to help differentiate the bhaptics keys that relate to the bhaptics events
pub struct bhapticsKey {
    key: String,
}
