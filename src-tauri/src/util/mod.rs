use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::process::Command;

#[tauri::command]
pub fn shutdown_device_listener(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .spawn()
            .expect("Failed to kill process")
            .wait()
            .expect("Failed to wait on child process");
    }

    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-s", "SIGINT", &pid.to_string()])
            .spawn()
            .expect("Failed to kill process")
            .wait()
            .expect("Failed to wait on child process");
    }

    Ok(())
}

pub fn next_free_port_with_address(start_port: u16, address: IpAddr) -> Option<u16> {
    let mut port = start_port;
    loop {
        let socket_addr = SocketAddr::new(address, port);
        match TcpListener::bind(socket_addr) {
            Ok(socket) => {
                // Successfully bound, port is free
                drop(socket);
                return Some(port);
            }
            Err(_) => {
                if port == u16::MAX {
                    return None; // No free port found
                }
                port += 1; // Increment port and try again
            }
        }
    }
}

pub fn next_free_port(start_port: u16) -> Option<u16> {
    next_free_port_with_address(start_port, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
}
