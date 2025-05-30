use std::thread;
use std::sync::mpsc;

use oyasumivr_oscquery::{OSCMethod, OSCMethodAccessType};
use serde;
use oyasumivr_oscquery;



/// Handles advertising our server for vrc to send values to if we need it.
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
                    address: "/avatar/parameters/*".to_string(),
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
