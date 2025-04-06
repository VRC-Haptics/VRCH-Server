use crate::VrcInfo;
use super::parsing::{parse_incoming, OscInfo};
use super::OscPath;

use std::thread;
use std::time::Duration;
use std::io::{BufReader, BufRead};
use std::sync::{Mutex, mpsc, Arc};
use std::process::{Command, Stdio};

use oyasumivr_oscquery;
use oyasumivr_oscquery::{OSCMethod, OSCMethodAccessType};
use serde;
use dashmap::DashMap;

pub fn start_filling_available_parameters(vrc: Arc<Mutex<VrcInfo>>){
    let vrc_clone = Arc::clone(&vrc);
    thread::spawn(move || {
        // Launch the sidecar process.
        let mut child = Command::new("./sidecars/listen-for-vrc.exe")
            // Do not attach a terminal to the sidecar.
            .stdin(Stdio::null())
            // Capture its stdout so we can read the FOUND messages.
            .stdout(Stdio::piped())
            // Optionally inherit stderr to see error messages.
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to launch sidecar");

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);

        // Monitor the sidecar output line by line.
        for line in reader.lines() {
            match line {
                Ok(msg) => {
                    if let Some(port_str) = msg.strip_prefix("FOUND:") {
                        // Try parsing the port as a u16.
                        if let Ok(port) = port_str.trim().parse::<u16>() {
                            log::debug!("Received FOUND message with port: {}", port);
                            let mut vrc = vrc_clone.lock().expect("couldn't lock vrc");
                            let params = {
                                vrc.vrc_connected = true;
                                &vrc.available_parameters
                            };
                            // Call the sub-function with the extracted port.
                            run_vrc_http_polling(port, params);
                            // When run_vrc_http_polling returns, continue waiting for the next FOUND message.
                        } else {
                            log::error!("Error: Could not parse port from message: {}", msg);
                        }
                    } else {
                        log::error!("Received non-matching message: {}", msg);
                    }
                }
                Err(e) => {
                    log::error!("Error reading sidecar output: {}", e);
                    break;
                }
            }
        }
    });
}

/// polls the 
fn run_vrc_http_polling(port: u16, params: &DashMap<OscPath, OscInfo>) {
    let url = format!("http://127.0.0.1:{}/", port);
    log::debug!("Started polling: {}", port);

    let mut first = true;
    loop {
        // Make the HTTP request (using the blocking client for simplicity)
        match reqwest::blocking::get(&url) {
            Ok(response) => {
                if let Ok(text) = response.text() {
                    // Attempt to parse the non-standard formatted response.
                    let node_info = parse_incoming(&text);
                    // Handling this way so that we can do something when values change later
                    for node in node_info {
                        match params.get(&node.full_path) {
                            // path is being updated
                            Some(old_node) => { 
                                if *old_node != node {
                                    params.insert(node.full_path.clone(), node);
                                }
                            }
                            // path is new (should only happen on starup)
                            None => {
                                if !first { log::trace!("Inserting:{:?}", node) };
                                params.insert(node.full_path.clone(), node);
                            }
                        };
                    }
                } else {
                    log::error!("Failed to read response text");
                }
            }
            Err(err) => {
                if err.is_connect() {
                    log::error!("connection to VRC HTTP failed");
                    return
                } else {
                    log::error!("HTTP request failed: {}", err);
                }
            }
        }
        // Wait for a regular interval before the next query.
        thread::sleep(Duration::from_secs(2));
        first = false;
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct OscQueryServer {
    recv_port: u16,
    #[serde(skip)]
    stop_sender: Option<mpsc::Sender<()>>,
}

impl OscQueryServer {
    pub fn new(recieving_port: u16) -> Self {
        OscQueryServer {
            recv_port: recieving_port,
            stop_sender: None,
        }
    }

    pub fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        let in_port = self.recv_port.clone();
        self.stop_sender = Some(tx);

        thread::spawn(move || {
            log::debug!("Spawned VRC Advertising on port:{}", in_port);
            let tk_rt = tokio::runtime::Runtime::new().unwrap();
            tk_rt.block_on(async {
                // Initialize the OSCQuery server
                log::debug!("In port: {}", in_port);
                let (host, port) = oyasumivr_oscquery::server::init(
                    "VRC Haptics", // The name of your application (Shows in VRChat's UI)
                    in_port,
                    "./sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
                )
                .await
                .unwrap();
                let addr = format!("{}:{}", host, port);
                log::debug!("OscQuery on: {}", addr);
                oyasumivr_oscquery::server::add_osc_method(OSCMethod {
                    description: Some("Haptics Specific Parameters".to_string()),
                    address: "/avatar/parameters/h".to_string(),
                    ad_type: OSCMethodAccessType::Write,
                    value_type: None,
                    value: None,
                })
                .await; // /avatar/*, /avatar/parameters/*, etc.
                oyasumivr_oscquery::server::advertise().await.unwrap();
            });

            loop {
                // Check for stop signal
                if let Ok(_) = rx.try_recv() {
                    tk_rt.block_on(async {
                        let _ = oyasumivr_oscquery::server::deinit().await;
                    });
                    break;
                }
            }
        });
    }

    #[allow(dead_code)] // TODO: send deinit
    pub fn stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }
}
