use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::UdpSocket;
use tokio::task::JoinHandle;
use warp::Filter;


#[derive(serde::Serialize, serde::Deserialize)]
pub struct OscQueryServer {
    recv_port: u16,
    #[serde(skip)]
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    #[serde(skip)]
    mdns: Option<ServiceDaemon>,
}

impl Debug for OscQueryServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OscQueryServer")
            .field("recv_port", &self.recv_port)
            .field("cancel", &self.cancel).finish()
    }
}

fn get_lan_ip() -> String {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.connect("8.8.8.8:80").unwrap();
    socket.local_addr().unwrap().ip().to_string()
}

impl OscQueryServer {
    pub fn new(recv_port: u16) -> Self {
        Self {
            recv_port,
            cancel: None,
            mdns: None,
        }
    }

    pub async fn start(&mut self) {
        let lan_ip = get_lan_ip();
        let osc_port = self.recv_port;

        // Pick a TCP port for the HTTP server
        let tcp_listener = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let tcp_port = tcp_listener.local_addr().unwrap().port();
        drop(tcp_listener);

        // 1. HTTP server serving OSCQuery JSON
        let root_response = serde_json::json!({
            "FULL_PATH": "/",
            "ACCESS": 0,
            "CONTENTS": {
                "avatar": {
                    "FULL_PATH": "/avatar",
                    "ACCESS": 0,
                    "CONTENTS": {
                        "parameters": {
                            "FULL_PATH": "/avatar/parameters",
                            "ACCESS": 2,
                            "DESCRIPTION": "Haptics Specific Parameters"
                        }
                    }
                }
            }
        });

        let host_info = serde_json::json!({
            "NAME": "VRC Haptics",
            "OSC_IP": lan_ip,
            "OSC_PORT": osc_port,
            "OSC_TRANSPORT": "UDP",
            "EXTENSIONS": {
                "ACCESS": true,
                "VALUE": true
            }
        });

        let root = root_response.clone();
        let host = host_info.clone();

        let routes = warp::get().and(warp::path::end()).and(warp::query::raw().or(warp::any().map(|| String::new())).unify()).map(move |query: String| {
            if query.contains("HOST_INFO") {
                warp::reply::json(&host)
            } else {
                warp::reply::json(&root)
            }
        });

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            warp::serve(routes)
                .bind(([0, 0, 0, 0], tcp_port)).await
                .graceful(async {
                    let _ = shutdown_rx.await;
                })
                .run()
                .await;
        });

        // 2. mDNS advertisement
        let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
        let service = ServiceInfo::new(
            "_oscjson._tcp.local.",
            "VRC Haptics",
            &format!("VRC-Haptics.local."),
            &lan_ip,
            tcp_port,
            HashMap::new(),
        )
        .expect("Failed to create service info");

        mdns.register(service).expect("Failed to register mDNS service");

        log::debug!("OSCQuery advertising at {}:{}, OSC on port {}", lan_ip, tcp_port, osc_port);

        self.cancel = Some(shutdown_tx);
        self.mdns = Some(mdns);
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.cancel.take() {
            let _ = tx.send(());
        }
        if let Some(mdns) = self.mdns.take() {
            let _ = mdns.shutdown();
        }
    }
}