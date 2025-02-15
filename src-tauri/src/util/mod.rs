use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

pub fn next_free_port_with_address(start_port: u16, address: IpAddr) -> Option<u16> {
    let mut port = start_port;
    loop {
        let socket_addr = SocketAddr::new(address, port);
        match UdpSocket::bind(socket_addr) {
            Ok(socket) => {
                // Successfully bound, port is free
                drop(socket);
                return Some(port);
            }
            Err(_) => {
                if port == u16::MAX {
                    return None; // No free port found
                }
                port += 1;
            }
        }
    }
}

pub fn next_free_port(start_port: u16) -> Option<u16> {
    next_free_port_with_address(start_port, IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))
}
