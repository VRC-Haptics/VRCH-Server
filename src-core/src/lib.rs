#![warn(unused_extern_crates)]
// Keep Futures from being left un-awaited. Use crate::log_err for convenient handling.
#![deny(unused_must_use)]

// make local modules available
pub mod api;
pub mod file;
mod network;
pub mod bhaptics;
pub mod devices;
pub mod mapping;
pub mod osc;
pub mod state;
pub mod util;
pub(crate) mod wrappers;
pub mod vrc;

// rexports
pub use glam; 

// local modules
use api::ApiManager;
use devices::{init_device_manager, DeviceManager};
use vrc::VrcGame;

//standard imports
use once_cell::sync::OnceCell;
use std::panic::{set_hook, take_hook};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex;

use crate::bhaptics::game::BhapticHandle;
use crate::devices::Device;
use crate::devices::{DeviceHandle, bhaptics::start_ble};
use crate::file::{AppRoot, ROOT_DIR, resolve_dir};
use crate::mapping::start_interp_map;
use crate::{
    mapping::MapHandle,
    vrc::VrcHandle,
};

#[macro_export]
/// Handles an unhandled result by printing if it failed. Optionally add context after the input to use this message instead of the default.
///
/// # Usage:
/// ```
/// pub fn returns_result() -> Result<(), String> {
///     Err("Unique Error");
/// }
///
/// log_err(returns_result());
/// -> "Lazily handled error: Unique Error"
///
/// log_err(returns_result(), "Error peforming action");
/// -> "Error performing action: Unique Error"
///
/// ```
macro_rules! log_err {
    ($expr:expr) => {
        if let Err(e) = $expr {
            log::warn!("[{}:{}] Lazily handled error: {e:?}", file!(), line!());
        }
    };
    ($expr:expr, $($arg:tt)+) => {
        if let Err(e) = $expr {
            log::warn!("[{}:{}] {}: {e:?}", file!(), line!(), format_args!($($arg)+));
        }
    };
}

// Provides a unified interface for interacting with external api's
pub static API_MANAGER: OnceLock<Mutex<ApiManager>> = OnceLock::new();
pub static DEVICE_MANAGER: OnceCell<DeviceHandle> = OnceCell::new();

async fn start_async_tasks(manager: DeviceHandle) -> (VrcHandle, MapHandle, BhapticHandle) {
    // initialize input map.
    let map_handle = start_interp_map(&manager).await;

    // TODO: Move into device manager init.
    log_err!(start_ble(manager.get_device_channel(), Duration::from_secs(1)).await);
    let bhaptic = bhaptics::game::start_bhaptics(map_handle.clone()).await;

    //start_apps
    let mut vrc = VrcGame::new(map_handle.clone(), API_MANAGER.get().expect("ApiManager should be initialized before use")).await;
    let vrc_handle = vrc.get_handle();
    tokio::spawn(async move {
        vrc.run().await;
    });

    (vrc_handle, map_handle, bhaptic)
}

/// Handles spawning the various components of the haptic server.
pub async fn start_server(root: AppRoot) -> (VrcHandle, MapHandle, BhapticHandle, DeviceHandle) {
    log_err!(ROOT_DIR.set(root));

    // map fetching api points to cache in the cache folder
    let api_manager = ApiManager::new(resolve_dir(file::Directory::Maps));
    log_err!(API_MANAGER.set(Mutex::new(api_manager)));

    // config goes in root directory
    state::set_config_dir(ROOT_DIR.get().unwrap().to_path_buf());
    let _ = state::get_config();
    state::init_save_loop().await;

    {
        let mut api = API_MANAGER.get().expect("ApiManager should be intialized before use").lock().await;
        api.refresh_caches().await;
    }

    let mut manager = DeviceManager::new();
    init_device_manager(&mut manager).await;
    if let Err(e) = DEVICE_MANAGER.set(manager.get_handle()) {
        log::error!("Failed to start device manager: {:?}", e);
    }
    let device_handle = manager.get_handle();

    let (vrc, map, bh) = start_async_tasks(device_handle.clone()).await;

    (vrc, map, bh, device_handle)
}


/// Starts the various components of the server and returns their handles. 
/// 
/// Same as start_server but does not rely on an existing runtime.
pub fn start_server_blocking(root: AppRoot) -> (VrcHandle, MapHandle, BhapticHandle, DeviceHandle) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    let _running_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("unable to start sub tokio runtime");
        rt.block_on(async move {
            let blob = start_server(root).await;
            let _ = tx.send(blob);

            // Keep the runtime alive after sending handles
            std::future::pending::<()>().await;
        });
    });

    rx.recv().expect("server thread failed to send handles")
}
