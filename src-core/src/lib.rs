#![warn(unused_extern_crates)]
// Keep Futures from being left un-awaited. Use crate::log_err for convenient handling.
#![deny(unused_must_use)]

// make local modules available
pub mod api;
pub mod bhaptics;
pub mod devices;
pub mod file;
pub mod mapping;
mod migrate;
mod network;
pub mod osc;
pub mod state;
pub mod util;
pub mod vrc;
pub(crate) mod wrappers;

#[cfg(test)]
mod tests;

// rexports
pub use glam;

// local modules
use api::ApiManager;
use devices::{init_device_manager, DeviceManager};
use vrc::VrcGame;

//standard imports
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::bhaptics::game::BhapticHandle;
use crate::devices::websocket::start_websocket_devices;
use crate::devices::{bhaptics::start_ble, DeviceHandle};
use crate::file::{resolve_dir, AppRoot, RESOURCE_DIR, ROOT_DIR};
use crate::mapping::start_interp_map;
use crate::state::get_config;
use crate::{mapping::MapHandle, vrc::VrcHandle};

#[doc(hidden)]
pub mod __log_err {
    use std::fmt::{Debug, Display};
    pub struct Wrap<T>(pub T);
    pub trait ViaDisplay {
        fn fmt_err(&self) -> String;
    }
    pub trait ViaDebug {
        fn fmt_err(&self) -> String;
    }
    // Display wins (fewer autorefs); Debug is the fallback.
    impl<T: Display> ViaDisplay for Wrap<&T> {
        fn fmt_err(&self) -> String {
            format!("{:#}", self.0)
        }
    }
    impl<T: Debug> ViaDebug for &Wrap<&T> {
        fn fmt_err(&self) -> String {
            format!("{:?}", self.0)
        }
    }
}

#[macro_export]
/// Handles an unhandled result by printing if it failed. Optionally add context after the input to use this message instead of the default.
///
/// # Usage:
/// ```
/// pub fn returns_result() -> Result<(), String> {
///     Err("Unique Error");
/// }
///
/// log_err!(returns_result());
/// -> "Lazily handled error: Unique Error"
///
/// log_err!(returns_result(), "Error peforming action");
/// -> "Error performing action: Unique Error"
///
/// ```
macro_rules! log_err {
    ($expr:expr) => {
        if let Err(e) = $expr {
            #[allow(unused_imports)]
            use $crate::__log_err::{ViaDisplay as _, ViaDebug as _};
            log::warn!("[{}:{}] Lazily handled error: {}", file!(), line!(),
                       (&$crate::__log_err::Wrap(&e)).fmt_err());
        }
    };
    ($expr:expr, $($arg:tt)+) => {
        if let Err(e) = $expr {
            #[allow(unused_imports)]
            use $crate::__log_err::{ViaDisplay as _, ViaDebug as _};
            log::warn!("[{}:{}] {}: {}", file!(), line!(), format_args!($($arg)+),
                       (&$crate::__log_err::Wrap(&e)).fmt_err());
        }
    };
}

// Provides a unified interface for interacting with external api's
pub static API_MANAGER: OnceLock<Mutex<ApiManager>> = OnceLock::new();
pub static DEVICE_MANAGER: OnceCell<DeviceHandle> = OnceCell::new();

async fn start_async_tasks(
    manager: DeviceHandle,
) -> (Option<VrcHandle>, MapHandle, Option<BhapticHandle>) {
    let conf = get_config().server.load();

    // initialize input map.
    let map_handle = start_interp_map(manager.clone()).await;

    // TODO: Move into device manager init.
    if conf.enable_ble_bhaptic {
        log_err!(start_ble(manager.get_device_channel(), Duration::from_secs(1)).await);
    }
    if conf.enable_websocket_devices {
        start_websocket_devices(manager.get_device_channel()).await;
    }
    let bhaptic = if conf.enable_bhaptic_game {
        Some(bhaptics::game::start_bhaptics(map_handle.clone()).await)
    } else {
        None
    };

    //start_apps
    let vrc = if conf.enable_vrc {
        let mut vrc = VrcGame::new(
            map_handle.clone(),
            API_MANAGER
                .get()
                .expect("ApiManager should be initialized before use"),
        )
        .await;
        let vrc_handle = vrc.get_handle();
        tokio::spawn(async move {
            vrc.run().await;
        });
        Some(vrc_handle)
    } else {
        None
    };

    return (vrc, map_handle, bhaptic);
}

/// Handles spawning the various components of the haptic server.
pub async fn start_server(
    root: AppRoot,
    resources: PathBuf,
) -> (
    Option<VrcHandle>,
    MapHandle,
    Option<BhapticHandle>,
    DeviceHandle,
) {
    log_err!(ROOT_DIR.set(root));
    log_err!(RESOURCE_DIR.set(resources));

    log_err!(
        file::seed_from_resources(),
        "Failed to seed bundled resources"
    );

    // map fetching api points to cache in the cache folder
    let api_manager = ApiManager::new(resolve_dir(file::Directory::Maps));
    log_err!(API_MANAGER.set(Mutex::new(api_manager)));

    // config goes in root directory
    state::set_config_dir(ROOT_DIR.get().unwrap().to_path_buf());
    let _ = state::get_config();
    state::init_save_loop().await;

    {
        let mut api = API_MANAGER
            .get()
            .expect("ApiManager should be intialized before use")
            .lock()
            .await;
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
pub fn start_server_blocking(
    root: AppRoot,
    resources: PathBuf,
) -> (
    Option<VrcHandle>,
    MapHandle,
    Option<BhapticHandle>,
    DeviceHandle,
) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    let _running_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("unable to start sub tokio runtime");
        rt.block_on(async move {
            let blob = start_server(root, resources).await;
            let _ = tx.send(blob);

            // Keep the runtime alive after sending handles
            std::future::pending::<()>().await;
        });
    });

    rx.recv().expect("server thread failed to send handles")
}
