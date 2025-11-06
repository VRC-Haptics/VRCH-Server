use std::net::{Ipv4Addr, TcpListener, UdpSocket, TcpStream};
use std::time::Duration;
use std::io::{Read, Write};
use uuid::Uuid;

use crate::devices::update::Firmware;

/// Connect to port 8266
///
/// send b"0 0 0 0"
///
/// try to send password with md5
///
/// wait for part confirm/deny repsonse
///
/// write fw lenght 4 bytes
/// write fw hash 32bytes
/// send fw
///
const FLASH: &str = "0";
const AUTH: &str = "200";
const REQUEST_PORT: u16 = 8266;

/// Trys to update, using the firmware. Returns success.
///
/// Transmits 'fw-update': (pctg: f32, id: String)
pub fn update_ota(bytes: Vec<u8>, password: String, device_ip: Ipv4Addr) -> Option<()> {
    let tcp_listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(l) => l,
        Err(e) => {
            log::error!("Failed to bind TCP listener: {}", e);
            return None;
        }
    };

    let tcp_port = tcp_listener.local_addr().ok()?.port();
    // Set non-blocking so we can timeout
    tcp_listener.set_nonblocking(true).ok()?;

    log::debug!("OTA: Authenticating");
    let fw_size = bytes.len();
    let fw_hash = md5_string(bytes.clone());

    if !authenticate(device_ip, fw_hash, fw_size, tcp_port, password) {
        log::error!("Unable to authenticate with device");
        return None;
    }

    log::debug!("Authentication succeded.");

    log::info!("Waiting for device ready...");
    
    let mut stream = match wait_for_connection(tcp_listener, Duration::from_secs(10)) {
        Some(s) => s,
        None => {
            log::error!("Unable to find Device connection.");
            return None;
        }
    };

    match upload_firmware(&mut stream, &bytes) {
        Some(_) => log::trace!("Successfully trasnferred firmware"),
        None => {
            log::error!("Firmware upload failed");
            return None;
        }
    }
    Some(())
}

fn upload_firmware(stream: &mut TcpStream, firmware: &[u8]) -> Option<()> {
    const CHUNK_SIZE: usize = 2048;
    let mut offset = 0;
    let mut skip_buffer = [0u8; 4];
    
    while offset < firmware.len() {
        let chunk_end = (offset + CHUNK_SIZE).min(firmware.len());
        let chunk = &firmware[offset..chunk_end];
        
        // Send chunk
        if let Err(e) = stream.write_all(chunk) {
            log::error!("Write failed: {}", e);
            return None;
        }
        
        if let Err(e) = stream.flush() {
            log::error!("Flush failed: {}", e);
            return None;
        }
        
        // Skip 4 bytes (device acknowledgment)
        if let Err(e) = stream.read_exact(&mut skip_buffer) {
            log::error!("Failed to read ack: {}", e);
            return None;
        }
        
        offset = chunk_end;
        
        let progress = (offset as f32 / firmware.len() as f32 * 100.0) as u32;
        log::debug!("Upload progress: {}%", progress);
    }
    
    log::info!("Upload complete, waiting for confirmation...");
    
    // Wait for OK response
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    
    let mut response = Vec::new();
    match stream.read_to_end(&mut response) {
        Ok(_) => {
            let response_str = String::from_utf8_lossy(&response);
            if response_str.contains("OK") {
                log::info!("Firmware update successful");
                Some(())
            } else {
                log::error!("Invalid response: {}", response_str);
                None
            }
        }
        Err(e) => {
            log::error!("Failed to read response: {}", e);
            None
        }
    }
}

fn wait_for_connection(listener: TcpListener, timeout: Duration) -> Option<TcpStream> {
    let start = std::time::Instant::now();
    
    loop {
        match listener.accept() {
            Ok((stream, addr)) => {
                log::info!("Device connected from: {}", addr);
                stream.set_read_timeout(Some(Duration::from_millis(1000))).ok()?;
                stream.set_write_timeout(Some(Duration::from_millis(1000))).ok()?;
                return Some(stream);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() > timeout {
                    log::error!("Connection timeout");
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                log::error!("Accept failed: {}", e);
                return None;
            }
        }
    }
}

/// Authenticates the ESP connected to the out_socket for the given firmware payload.
pub fn authenticate(
    device_ip: Ipv4Addr,
    fw_hash: String,
    fw_size: usize,
    // port we will send the TCP stream from.
    fw_port: u16,
    password: String,
) -> bool {
    let out_socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Couldn't allocate UDP Socket: {}", e);
            return false;
        }
    };

    let our_port = out_socket
        .local_addr()
        .unwrap_or((Ipv4Addr::UNSPECIFIED, 0).into());

    log::trace!("Local port bound to: {:?}", our_port.clone());

    if let Err(e) = out_socket.connect((device_ip, REQUEST_PORT)) {
        log::error!("Couldn't connect to device: {e:?}");
        return false;
    }

    let invitation: String = format!("{FLASH} {fw_port} {fw_size} {fw_hash}");
    log::trace!("Sending: {}", invitation.clone());
    out_socket.send(invitation.as_bytes());

    let mut auth_buf = [0u8; 128];
    let len_recv = match out_socket.recv(&mut auth_buf) {
        Ok(len) => len,
        Err(e) => {
            log::error!("{e}");
            return false;
        }
    };

    log::trace!(
        "Got Auth Response: {}",
        String::from_utf8_lossy(&auth_buf[..len_recv])
    );
    if auth_buf[..len_recv].starts_with(b"AUTH") {
        let signature = md5_string(Uuid::new_v4());
        let hashed_pass = md5_string(password);
        let nonce = &auth_buf[5..len_recv - 1]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let payload = md5_string(format!("{hashed_pass}:{nonce}:{signature}"));

        let full_msg = format!("{AUTH} {signature} {payload}");

        match out_socket.send(full_msg.as_bytes()) {
            Ok(_) => log::trace!("auth package sent successfully"),
            Err(e) => {
                log::error!("Auth package unable to send: {e:?}");
                return false;
            }
        }

        // get authentication response
        let mut recv_buff = [0u8; 128];
        let len = match out_socket.recv(&mut recv_buff) {
            Ok(len) => len,
            Err(e) => {
                log::error!("Unable to recieve authentication response: {e:?}");
                return false;
            }
        };

        let response = recv_buff[0..len]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        log::trace!("Auth response recieved: {}", response);

        return recv_buff[0..len].starts_with(b"OK");
    } else if auth_buf[..len_recv].starts_with(b"OK") {
        // no need for auth
        return true;
    } else {
        log::error!("Authentication from device not valid");
        return false;
    }
}

fn md5_string<T: AsRef<[u8]>>(contents: T) -> String {
    md5::compute(contents)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}
