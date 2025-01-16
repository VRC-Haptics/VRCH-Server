use crate::osc::server::OscServer; 
use crate::util::next_free_port;
use crate::VrcInfo;

use std::{collections::HashMap, net::Ipv4Addr};
use std::thread;
use std::sync::{Arc, RwLock, mpsc};

use rosc::OscMessage;
use oyasumivr_oscquery;
use serde;

pub fn get_vrc() -> VrcInfo {
    let raw_parameters = Arc::new(RwLock::new(HashMap::new()));

    let raw_params_for_callback = raw_parameters.clone();
    let on_receive = move |msg: OscMessage| {
        //push message to hashmap
        let mut params = raw_params_for_callback.write().unwrap();
        params.insert(msg.addr.clone(), msg.args); //TODO: see if locks are an issue using hashmaps like this
    };

    //create server before starting anything
    let recieving_port = next_free_port(1000).unwrap();
    let mut vrc_server = OscServer::new(
        recieving_port,
        Ipv4Addr::LOCALHOST,
        on_receive,
    );
    vrc_server.start();

    let mut osc_server = OscQueryServer::new(recieving_port);
    osc_server.start();

    return VrcInfo {
        osc_server: Some(vrc_server),
        query_server: Some(osc_server),
        in_port: Some(recieving_port.to_owned()),
        out_port: None,
        avatar: None,
        haptics_prefix: "avatars/parameters/h".to_string(),
        raw_parameters: raw_parameters,
    };
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
            tk_rt.block_on( async {
                // Initialize the OSCQuery server
                println!("In port: {}", in_port);
                let (host, port) = oyasumivr_oscquery::server::init(
                    "VRC Haptics",         // The name of your application (Shows in VRChat's UI)
                    in_port, 
                    "./sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
                ).await.unwrap();
                let addr = format!("{}:{}", host, port);
                println!("OscQuery on: {}", addr);
                oyasumivr_oscquery::server::receive_vrchat_avatar_parameters().await; // /avatar/*, /avatar/parameters/*, etc.
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
