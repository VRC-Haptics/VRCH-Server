use serde;
use tokio_util::sync::CancellationToken;

use oyasumivr_oscquery;
use oyasumivr_oscquery::{OSCMethod, OSCMethodAccessType};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct OscQueryServer {
    recv_port: u16,
    #[serde(skip)]
    cancel: Option<CancellationToken>,
}

impl OscQueryServer {
    pub fn new(recieving_port: u16) -> Self {
        OscQueryServer {
            recv_port: recieving_port,
            cancel: None,
        }
    }

    pub async fn start(&mut self) {
        let cancel = CancellationToken::new();
        self.cancel = Some(cancel.clone());
        let in_port = self.recv_port;

        tokio::spawn(async move {
            log::debug!("Spawned VRC Advertising on port:{}", in_port);

            let (host, port) = oyasumivr_oscquery::server::init(
                "VRC Haptics",
                in_port,
                "./sidecars/vrc-sidecar.exe",
            )
            .await
            .unwrap();

            log::debug!("OscQuery on: {}:{}", host, port);

            oyasumivr_oscquery::server::add_osc_method(OSCMethod {
                description: Some("Haptics Specific Parameters".to_string()),
                address: "/avatar/parameters/*".to_string(),
                ad_type: OSCMethodAccessType::Write,
                value_type: None,
                value: None,
            })
            .await;

            oyasumivr_oscquery::server::advertise().await.unwrap();

            cancel.cancelled().await;

            let _ = oyasumivr_oscquery::server::deinit().await;
            log::debug!("OscQuery server stopped");
        });
    }

    pub fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
    }
}