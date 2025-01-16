use crate::osc::server::OscServer; 
use crate::util::next_free_port;
use crate::vrcInfo;

use std::{collections::HashMap, net::Ipv4Addr};
use std::thread;
use std::sync::{Arc, Mutex, mpsc};

use rosc::OscMessage;
use oyasumivr_oscquery;
use serde;

pub fn get_vrc() -> vrcInfo {
    let recieving_port = next_free_port(7058).unwrap();

    // TODO: FUlly implement callback
    let on_receive = |msg: OscMessage| {
        println!("Received OSC message: {:?}", msg);
    };

    //create server before starting anything
    let mut vrc_server = OscServer::new(
        recieving_port,
        Ipv4Addr::LOCALHOST,
        on_receive,
    );
    vrc_server.start();

    let mut osc_server = OscQueryServer::new(recieving_port);
    osc_server.start();

    return vrcInfo {
        osc_server: Some(vrc_server),
        query_server: Some(osc_server),
        in_port: Some(recieving_port.to_owned()),
        out_port: None,
        avatar: None,
        raw_parameters: HashMap::new(),
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
            let tk_rt = tokio::runtime::Runtime::new().unwrap();
            tk_rt.block_on( async {
                // Initialize the OSCQuery server
                oyasumivr_oscquery::server::init(
                    "OyasumiVR Test",         // The name of your application (Shows in VRChat's UI)
                    in_port, 
                    "/../../../src-tauri/sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
                ).await.unwrap();
                oyasumivr_oscquery::server::receive_vrchat_avatar_parameters().await; // /avatar/*, /avatar/parameters/*, etc.
                oyasumivr_oscquery::server::advertise().await.unwrap();
            });

            println!("to query loop");
            loop {

                // Check for stop signal
                if let Ok(_) = rx.try_recv() {
                    println!("Stopping oyasumi_oscquery server.");
                    break;
                }
            }
        });
    }

    pub fn stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }
}
