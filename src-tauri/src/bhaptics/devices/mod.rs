mod ble;

use std::sync::{Arc, Mutex};
use tauri_plugin_blec::models::ScanFilter;
use uuid::{uuid, Uuid};
use tokio::sync::mpsc;

use crate::bhaptics::devices::ble::DEVICE_NAMES;
use crate::devices::Device;

const BHAPTICS_UUID: Uuid = uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e");

/// Returns 
pub async fn start_bt(device_list: Arc<Mutex<Vec<Device>>>) -> Result<(), tauri_plugin_blec::Error> {
    let ability = tauri_plugin_blec::check_permissions()?;
    if !ability {
        return Err(tauri_plugin_blec::Error::CharacNotAvailable("No BLE permissions".to_string()));
    }


    println!("Started bt stuff");
    let handler = tauri_plugin_blec::get_handler().expect("Unable to get blec handler");
    let (tx, mut rx) = mpsc::channel(1);
    let _ = handler.discover(Some(tx), 1000, ScanFilter::None).await?;
    // get the first bluetooth adapter
    println!("Before for loop");
    while let Some(devices) = rx.recv().await {
        for device in devices.clone() {
            for b_name in DEVICE_NAMES {
                if device.name == *b_name {
                    // log and remove device from device list when it disconnects.
                    let name_clone = device.name.clone();
                    let dev_clone = Arc::clone(&device_list);
                    let mac_clone = device.address.clone();
                    let disconnect = move || {
                        log::trace!("{}: Device Disconnected", name_clone);
                        let mut lock = dev_clone.lock().expect("Couldn't lock devices");
                        lock.retain(|dev| dev.id == mac_clone);
                    };

                    println!("Trying to connect to: {}", device.name);
                    let _ = handler.connect(&device.address, disconnect.into()).await;
                    let conn = handler.connected_device().await.expect("Panic on devices");
                    if conn.is_connected {
                        log::trace!("Connected: {}, {}", conn.name, conn.address);
                        println!("Is connected");
                    } else {
                        println!("isn't connected");
                    }
                }
            }
            
        }
    }

    Ok(())
}
