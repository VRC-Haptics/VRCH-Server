use crate::osc::server::OscServer;
use crate::vrc::Parameters;
use crate::VrcInfo;

use super::parsing::{parse_incoming, OscInfo};

use std::net::IpAddr;
use std::sync::{
    Mutex,
    mpsc, 
    Arc, 
    RwLock,
};
use std::{
    collections::HashMap, 
    net::Ipv4Addr,
    thread,
    time::Duration,
};


use oyasumivr_oscquery;
use oyasumivr_oscquery::{OSCMethod, OSCMethodAccessType};
use rosc::{OscMessage, OscType};
use serde;

use regex::Regex;
use futures_util::{pin_mut, stream::StreamExt};
use mdns::{Error, Record, RecordKind};

/// default http port is 8060
/// default server name is "OSCQueryService"

fn remove_version(path: &str) -> String {
    // This regex matches "VF" followed by exactly two digits.
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
        // Create a new Tokio runtime in this thread.
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async move {
            let service_type = "_oscjson._tcp.local.";
            let stream = mdns::discover::interface(service_type, Duration::from_secs(3), Ipv4Addr::LOCALHOST)
                .expect("couldn't start discovery")
                .listen();
            pin_mut!(stream);
            println!("hit stream");

            while let Some(Ok(response)) = stream.next().await {
                let records = response.records();
        
                for record in records {
                    let r_type = &record.kind;
                    match r_type {
                        RecordKind::SRV { priority, weight, port, ref target } => {
                            println!("SRV Record - Priority: {}, Weight: {}, Port: {}, Target: {}", priority, weight, port, target);
                            handle_service_resolved(&target, *port, &params);
                        },
                        r => println!("Other Record: {:?}", r),
                    }
                }
            }
        });
    });
}

fn handle_service_resolved(full_name: &str, port: u16, params: &Arc<Mutex<HashMap<String, OscInfo>>>) {
    println!("Resolved service: {}", full_name);
    if full_name.contains("VRChat-Client-") {
        run_vrc_http_polling(port, params.clone());
    }
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
                                    param_lock.insert(node.full_path.clone(), node);
                                }
                            }
                            None => {
                                param_lock.insert(node.full_path.clone(), node);
                            }
                        };
                    }
                } else {
                    eprintln!("Failed to read response text");
                }
            }
            Err(err) => eprintln!("HTTP request failed: {}", err),
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
