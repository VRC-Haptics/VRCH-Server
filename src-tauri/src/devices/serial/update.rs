use tauri::{AppHandle, State};
use tauri_plugin_serialplugin::{
    commands::{available_ports, open},
    error, state::{DataBits, FlowControl},
};

use crate::devices::ESP32Model;

/// Uses tauri serial so it requires the app
///
/// Assumes port is alrady valid and contains a device that matches the fw type.
pub fn serial_flash(
    fw: Vec<u8>,
    target: ESP32Model,
    com_port: String,
    baud: u32,
    serial: State<'_, tauri_plugin_serialplugin::desktop_api::SerialPort<tauri::Wry>>,
    app: AppHandle<tauri::Wry>,
) -> Result<(), SerialFlashError> {
    // Open Port
    open(
        app.clone(),
        serial.clone(),
        com_port.clone(),
        baud,
        Some(DataBits::Eight),
        Some(FlowControl::None),
        None,
        None,
        Some(3000u64),
    ).map_err(SerialFlashError::SerialError)?;


    // must close it whatever happens after opening it


    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum SerialFlashError {
    #[error("Some error occured with the serial connection")]
    SerialError(#[from] tauri_plugin_serialplugin::error::Error),
}
