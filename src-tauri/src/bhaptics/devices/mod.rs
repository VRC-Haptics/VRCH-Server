use std::time::Duration;
use tauri_plugin_blec::models::ScanFilter;
use uuid::{uuid, Uuid};
use tokio::sync::mpsc;
use tokio::time;

const BHAPTICS_UUID: Uuid = uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e");

pub async fn start_bt() -> Result<(), tauri_plugin_blec::Error> {
    println!("Started bt stuff");
    let handler = tauri_plugin_blec::get_handler().expect("Unable to get blec handler");
    let (tx, mut rx) = mpsc::channel(1);
    let _ = handler.discover(Some(tx), 1000, ScanFilter::None).await?;
    // get the first bluetooth adapter
    println!("Before for loop");
    while let Some(devices) = rx.recv().await {
        for device in devices.clone() {
            if device.name.contains("Tact") {
                println!("Trying to connect to: {}", device.name);
                let _ = handler.connect(&device.address, (|| println!("disconnected")).into()).await;
                let this = handler.connected_device().await.expect("Panic on devices");
                println!("{:?}", this);
                println!("Device: {:?}", device);
            }
        }
        println!("Discovered {devices:?}");
    }

    Ok(())
}
