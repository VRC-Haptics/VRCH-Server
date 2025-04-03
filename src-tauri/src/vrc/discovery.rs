use crate::osc::server::OscServer;
use crate::vrc::Parameters;
use crate::VrcInfo;

use super::parsing::{parse_incoming, OscInfo};

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::thread;
use std::time::Duration;
use std::io::{BufReader, BufRead};
use std::sync::{
    Mutex,
    mpsc, 
    Arc, 
    RwLock,
};
use std::process::{
    Command, 
    Stdio
};


use oyasumivr_oscquery;
use oyasumivr_oscquery::{OSCMethod, OSCMethodAccessType};
use rosc::{OscMessage, OscType};
use serde;
use regex::Regex;

/// Removes the VRC Fury naming from the parameters
fn remove_version(path: &str) -> String {
    let re = Regex::new(r"VF\d{2}").unwrap();
    // Replace all matches with an empty string.
    re.replace_all(path, "").to_string()
}

pub fn get_vrc() -> VrcInfo {
    let all_parameters: Arc<Mutex<HashMap<String, OscInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    start_filling_all_parameters(all_parameters.clone());

    let raw_parameters = Arc::new(RwLock::new(HashMap::new()));
    let raw_menu = Arc::new(RwLock::new(Parameters::new()));
    let first_message = Arc::new(RwLock::new(false));

    let haptics_prefix = "/avatar/parameters/h";
    let haptics_menu_prefix = "/avatar/parameters/h_param";
    let haptics_prefix_clone = haptics_prefix.to_string();

    let raw_params_for_callback = raw_parameters.clone();
    let raw_menu_for_callback = raw_menu.clone();
    let first_message_callback = first_message.clone();
    let on_receive = move |msg: OscMessage| {
        let addr = remove_version(&msg.addr);

        if *first_message_callback.read().unwrap() == false {
            *first_message_callback.write().unwrap() = true;
        }

        if addr.starts_with(haptics_prefix) {
            let mut params = raw_params_for_callback.write().unwrap();
            params.insert(msg.addr.clone(), msg.args.clone());
        }

        if addr.starts_with(haptics_menu_prefix) {
            println!("in menu prefix: {}", addr);
            let mut menu = raw_menu_for_callback.write().unwrap();

            //see if it needs to be put in the parameters
            for (_, (param, value)) in menu.parameters.iter_mut() {
                if param == &addr {
                    match msg
                        .args
                        .first()
                        .expect("No value with menu parameter")
                        .to_owned()
                    {
                        OscType::Float(msg_float) => {
                            *value = msg_float;
                        }
                        _ => {
                            unreachable!("Expected only OscType::Float in menu parameters");
                        }
                    }
                    break;
                }
            }
        }
    };
    //create server before starting anything
    let recieving_port = 9001;
    let mut vrc_server = OscServer::new(recieving_port, Ipv4Addr::LOCALHOST, on_receive);
    let port_used = vrc_server.start();

    let mut osc_server = OscQueryServer::new(recieving_port);
    if port_used != recieving_port {
        osc_server.start();
        log::warn!("Not using VRC dedicated ports, expect slower operations.");
    }

    return VrcInfo {
        vrc_connected: false,
        osc_server: Some(vrc_server),
        query_server: Some(osc_server),
        in_port: Some(port_used),
        out_port: None,
        avatar: None,
        haptics_prefix: haptics_prefix_clone,
        menu_parameters: raw_menu,
        raw_parameters: raw_parameters,
    };
}

fn start_filling_all_parameters(params: Arc<Mutex<HashMap<String, OscInfo>>>) {
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
                            println!("Received FOUND message with port: {}", port);
                            // Call the sub-function with the extracted port.
                            run_vrc_http_polling(port, Arc::clone(&params));
                            // When run_vrc_http_polling returns, continue waiting for the next FOUND message.
                        } else {
                            eprintln!("Error: Could not parse port from message: {}", msg);
                        }
                    } else {
                        println!("Received non-matching message: {}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading sidecar output: {}", e);
                    break;
                }
            }
        }
    });
}

/// polls the 
fn run_vrc_http_polling(port: u16, params: Arc<Mutex<HashMap<String, OscInfo>>>) {
    let url = format!("http://127.0.0.1:{}/", port);
    println!("Started polling: {}", port);
    
    loop {
        // Make the HTTP request (using the blocking client for simplicity)
        match reqwest::blocking::get(&url) {
            Ok(response) => {
                if let Ok(text) = response.text() {
                    // Attempt to parse the non-standard formatted response.
                    let node_info = parse_incoming(&text);
                    // Handling this way so that we can do something when values change later
                    let mut param_lock = params.lock().expect("Unable to get param lock");
                    for node in node_info {
                        match param_lock.get(&node.full_path) {
                            Some(old_node) => {
                                if old_node != &node {
                                    log::trace!("Inserting:{:?}", node);
                                    param_lock.insert(node.full_path.clone(), node);
                                }
                            }
                            None => {
                                log::trace!("Inserting:{:?}", node);
                                param_lock.insert(node.full_path.clone(), node);
                            }
                        };
                    }
                } else {
                    eprintln!("Failed to read response text");
                }
            }
            Err(err) => {
                if err.is_connect() {
                    log::error!("connection to VRC HTTP failed");
                    return
                } else {
                    eprintln!("HTTP request failed: {}", err);
                }
            }
        }
        // Wait for a regular interval before the next query.
        thread::sleep(Duration::from_secs(2));
    }
}

#[derive(serde::Serialize, Debug, Clone)]
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
            println!("Spawned VRC Advertising on port:{}", in_port);
            let tk_rt = tokio::runtime::Runtime::new().unwrap();
            tk_rt.block_on(async {
                // Initialize the OSCQuery server
                println!("In port: {}", in_port);
                let (host, port) = oyasumivr_oscquery::server::init(
                    "VRC Haptics", // The name of your application (Shows in VRChat's UI)
                    in_port,
                    "./sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
                )
                .await
                .unwrap();
                let addr = format!("{}:{}", host, port);
                println!("OscQuery on: {}", addr);
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
