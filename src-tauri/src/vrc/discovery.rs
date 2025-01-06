use crate::{osc::server::OscServer, util::next_free_port, vrcInfo};

use tokio::runtime::Runtime;
use std::{collections::HashMap, net::Ipv4Addr};
use rosc::OscMessage;

pub fn get_vrc () -> vrcInfo {
    let recieving_port = next_free_port(8085).expect("Ran out of ports");

    // TODO: FUlly implement callback
    let on_receive = |msg: OscMessage| {
        println!("Received OSC message: {:?}", msg);
    };

    //create server before starting anything
    let mut vrc_server = OscServer::new(recieving_port, Ipv4Addr::new(127, 0, 0, 1), on_receive);
    
    //start server
    let tk_rt = Runtime::new().unwrap();
    tk_rt.block_on(async {vrc_server.start().await});
    
    // start advertisement
    tk_rt.block_on(async {
        // Initialize the OSCQuery server
        oyasumivr_oscquery::server::init(
            "VRC Haptics",         // The name of your application (Shows in VRChat's UI)
            recieving_port,                     // The port your OSC server receives data on
            "./sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
        ).await.unwrap();

        // Configure which data we want to receive from VRChat
        oyasumivr_oscquery::server::receive_vrchat_avatar_parameters().await; // we only want parameters

        // Now we can start broadcasting the advertisement for the OSC and OSCQuery server
        oyasumivr_oscquery::server::advertise().await.unwrap();
    });

    return vrcInfo {
        osc_server: Some(vrc_server),
        in_port: Some(recieving_port),
        out_port: None,
        avatar: None,
        raw_parameters: HashMap::new(),
    }
}