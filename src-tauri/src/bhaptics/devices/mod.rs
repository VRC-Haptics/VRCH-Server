mod ble;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri_plugin_blec::models::ScanFilter;
use tokio::sync::mpsc;
use uuid::{uuid, Uuid};

use crate::bhaptics::devices::ble::DEVICE_NAMES;
use crate::devices::Device;

const BHAPTICS_UUID: Uuid = uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e");

/// Non-blocking version that spawns background task
pub fn start_bt_nonblocking(
    device_list: Arc<Mutex<Vec<Device>>>,
) -> Result<(), tauri_plugin_blec::Error> {
    let ability = tauri_plugin_blec::check_permissions()?;
    if !ability {
        return Err(tauri_plugin_blec::Error::CharacNotAvailable(
            "No BLE permissions".to_string(),
        ));
    }

    // Spawn background task instead of blocking
    let device_list_clone = Arc::clone(&device_list);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async move {
            if let Err(e) = bt_scan_and_connect_loop(device_list_clone).await {
                log::error!("Bluetooth scanning error: {}", e);
            }
        });
    });

    Ok(())
}

/// Background scanning and connection loop
async fn bt_scan_and_connect_loop(
    device_list: Arc<Mutex<Vec<Device>>>,
) -> Result<(), tauri_plugin_blec::Error> {
    let handler = tauri_plugin_blec::get_handler().expect("Unable to get blec handler");
    let mut connected_devices = HashSet::new();

    loop {
        // Shorter scan intervals for responsiveness
        let (tx, mut rx) = mpsc::channel(10);

        // Non-blocking scan with shorter timeout
        if let Err(e) = handler.discover(Some(tx), 500, ScanFilter::None).await {
            log::warn!("Discovery error: {}", e);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }

        // Process discovered devices
        while let Some(devices) = rx.recv().await {
            let connection_tasks = devices
                .into_iter()
                .filter(|device| {
                    DEVICE_NAMES.contains(&device.name.as_str())
                        && !connected_devices.contains(&device.address)
                })
                .map(|device| {
                    let device_list_clone = Arc::clone(&device_list);

                    tokio::spawn(async move { connect_to_device(device, device_list_clone).await })
                })
                .collect::<Vec<_>>();

            // Wait for all connection attempts concurrently
            for task in connection_tasks {
                if let Ok(address) = task.await {
                    if let Some(addr) = address {
                        connected_devices.insert(addr);
                    }
                }
            }
        }

        // Brief pause before next scan cycle
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

/// Connect to a single device with timeout
async fn connect_to_device(
    device: tauri_plugin_blec::models::BleDevice,
    device_list: Arc<Mutex<Vec<Device>>>,
) -> Option<String> {
    let handler = match tauri_plugin_blec::get_handler() {
        Ok(h) => h,
        Err(_) => {
            log::error!("Failed to get BLE handler");
            return None;
        }
    };

    let address = device.address.clone();
    let name = device.name.clone();

    // Setup disconnect callback
    let name_clone = name.clone();
    let dev_clone = Arc::clone(&device_list);
    let mac_clone = address.clone();
    let disconnect = move || {
        log::trace!("{}: Device Disconnected", name_clone);
        let mut lock = dev_clone.lock().expect("Couldn't lock devices");
        lock.retain(|dev| dev.id != mac_clone);
    };

    log::trace!("Attempting to connect to: {}", name);

    // Add connection timeout
    let connection_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        handler.connect(&address, disconnect.into()),
    )
    .await;

    match connection_result {
        Ok(Ok(_)) => {
            // Verify connection with timeout
            let verification_result = tokio::time::timeout(
                tokio::time::Duration::from_secs(2),
                handler.connected_device(),
            )
            .await;

            match verification_result {
                Ok(Ok(conn)) if conn.is_connected => {
                    log::info!("Successfully connected: {}, {}", conn.name, conn.address);

                    // Add to device list
                    if let Ok(mut lock) = device_list.lock() {
                        // Add your Device creation logic here
                        // lock.push(Device::new(...));
                    }

                    Some(address)
                }
                Ok(Ok(_)) => {
                    log::warn!("Connection reported but device not connected: {}", name);
                    None
                }
                Ok(Err(e)) => {
                    log::warn!("Connection verification failed for {}: {}", name, e);
                    None
                }
                Err(_) => {
                    log::warn!("Connection verification timeout for: {}", name);
                    None
                }
            }
        }
        Ok(Err(e)) => {
            log::warn!("Failed to connect to {}: {}", name, e);
            None
        }
        Err(_) => {
            log::warn!("Connection timeout for: {}", name);
            None
        }
    }
}
