use crate::{osc::server::OscServer, util::next_free_port, vrcInfo};

use rosc::OscMessage;
use std::{collections::HashMap, net::Ipv4Addr, time::Duration};
use tokio::runtime::Runtime;

pub fn get_vrc() -> vrcInfo {
    let expected = next_free_port(7058).expect("Ran out of ports").clone();
    let recieving_port = expected;

    // TODO: FUlly implement callback
    let on_receive = |msg: OscMessage| {
        println!("Received OSC message: {:?}", msg);
    };

    //create server before starting anything
    let mut vrc_server = OscServer::new(
        recieving_port.to_owned(),
        Ipv4Addr::new(127, 0, 0, 1),
        on_receive,
    );

    //start server
    let tk_rt = Runtime::new().unwrap();
    tk_rt.block_on(async { vrc_server.start().await });



    tk_rt.spawn(async move {
        // Initialize the OSCQuery server
        oyasumivr_oscquery::server::init(
            "VRC Haptics",                // The name of your application (Shows in VRChat's UI)
            expected,                     // The port your OSC server receives data on
            "./sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
        ).await.expect("couldn't start sidecar");

        // Configure which data we want to receive from VRChat
        oyasumivr_oscquery::server::receive_vrchat_avatar_parameters().await; // we only want parameters

        // Now we can start broadcasting the advertisement for the OSC and OSCQuery server
        oyasumivr_oscquery::server::advertise().await.unwrap();

        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    return vrcInfo {
        osc_server: Some(vrc_server),
        in_port: Some(recieving_port.to_owned()),
        out_port: None,
        avatar: None,
        raw_parameters: HashMap::new(),
    };
}
