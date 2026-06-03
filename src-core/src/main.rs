use std::path::PathBuf;


use haptic_core::{file::AppRoot, start_server_blocking, state};

fn main() {
    // The lib logs via the `log` facade; without a logger these are dropped.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("trace"),
    )
    .init();


    // AppRoot is from file.rs — construct it however that module exposes.
    // Likely something like AppRoot::new(<path>); confirm against file.rs.
    let root = AppRoot::default("headless-vrch").expect("Unable to use approot name");

    let (_vrc, _map, _bhaptics, _devices) = start_server_blocking(root);
    log::info!("haptic server running — press Ctrl-C to exit");

    // force config to be saved so we can edit it.
    state::mark_dirty();

    // start_server_blocking returns immediately; keep the process alive so the
    // background runtime thread isn't killed. SIGINT (Ctrl-C) ends the process.
    loop {
        std::thread::park();
    }
}