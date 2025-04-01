pub mod config;
mod connection_manager;
pub mod discovery;

// outside imports
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::time::{Duration, SystemTime};
use std::vec;

// local imports
use crate::devices::OutputFactors;
use crate::mapping::global_map::GlobalMap;
use crate::mapping::haptic_node::HapticNode;
use crate::util::next_free_port;
use connection_manager::WifiConnManager;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
/// The DeviceType that handles generic wifi haptic devices
pub struct WifiDevice {
    // this devices mac address, used as id in Device::from_wifi().
    pub mac: String,
    // This devices ip
    pub ip: String,
    // keeps the user-facing name
    pub name: String,
    // Flag for keeping from pinging on every tick() call
    pub been_pinged: bool,
    // Push whatever device map we have in memory to physical Device
    pub push_map: bool,
    // Last time a query, "GET", command was sent. used for debouncing
    pub last_queried: SystemTime,
    // The Port We Send data to
    pub send_port: u16,
    // Abstracts communication.
    connection_manager: WifiConnManager,
}

#[derive(Debug)]
pub struct Packet {
    pub packet: Vec<u8>,
}

impl WifiDevice {
    #[allow(dead_code)]
    /// Instantiate new device instance
    pub fn new(mac: String, ip: String, send_port: u16, name: String) -> WifiDevice {
        let recv_port = next_free_port(1500).unwrap();
        let connection_manager = WifiConnManager::new(&recv_port, "/hrtbt".to_string());

        return WifiDevice {
            mac: mac,
            ip: ip.clone(),
            name: name,
            been_pinged: false,
            push_map: false,
            last_queried: SystemTime::UNIX_EPOCH,
            send_port: send_port,
            connection_manager: connection_manager,
        };
    }

    pub fn stop(&self) {
        println!("DO stop stuff now")
    }

    /// Called in regular intervals. Optionally returns a packet to be sent to the device.
    pub fn tick(
        &mut self,
        is_alive: &mut bool,
        factors: &mut OutputFactors,
        inputs: &GlobalMap,
    ) -> Option<Packet> {
        if !self.been_pinged {
            // first round through we ping
            self.been_pinged = true;
            let packet = self.build_ping();
            println!("Packet: {:?}", packet);
            return Some(self.build_ping());
        }

        // keep track of heartbeat timings and whatnot
        manage_hrtbt(is_alive, &mut self.been_pinged, &self.connection_manager);

        // check if we filled out the wifiConfig yet
        if let Some(conf) = self.connection_manager.config.read().unwrap().as_ref() {

            //push config to device if necessary
            if self.push_map {
                self.push_map = false;
                let conn_lock = self.connection_manager.config.read().unwrap();
                match conn_lock.as_ref() {
                    Some(config) => {
                        return Some(self.build_set_map(&config.node_map));
                    },
                    // Possiblity to get caught in loop if we try to set without recieving the config.
                    None => {
                        return None;
                    }
                }
            }

            // Collect haptic values
            let mut intensities = vec![];
            for node in conf.node_map.iter() {
                intensities.push(inputs.get_intensity_from_haptic(&node, &factors.interp_algo, &true));
            }
            intensities.reverse(); //undo reversing from push
            return Some(self.compile_message(&intensities));
        } else {
            // If no mapping configuration found
            // Gather the configuration
            let now = SystemTime::now();
            let diff = now
                .duration_since(self.last_queried)
                .expect("Error getting difference");
            if diff > Duration::from_millis(500) || self.last_queried == SystemTime::UNIX_EPOCH {
                self.last_queried = now;
                return Some(self.build_get_all());
            }

            return None;
        }
    }

    /// Sends updated message
    fn build_set_map(&self, map: &Vec<HapticNode>) -> Packet {
        let base = "SET NODE_MAP ".to_string();
        // Convert each HapticNode into its 8-byte hex representation.
        let hex_str: String = map
            .iter()
            .map(|node| {
                let bytes = node.to_bytes();
                // For each byte, produce a two-digit hex string.
                bytes
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<String>()
            })
            .collect();

        let full = base + &hex_str;

        let message = rosc::OscMessage {
            addr: "/command".to_string(),
            args: vec![OscType::String(full)],
        };
        let packet = rosc::OscPacket::Message(message);

        Packet {
            packet: rosc::encoder::encode(&packet).unwrap(),
        }
    }

    /// Compiles into valid motor packet
    fn compile_message(&self, float_array: &Vec<f32>) -> Packet {
        let hex_message = self.compile_to_bytes(float_array);

        let message = rosc::OscMessage {
            addr: "/h".to_string(),
            args: vec![OscType::String(hex_message)],
        };
        let packet = rosc::OscPacket::Message(message);
        return Packet {
            packet: rosc::encoder::encode(&packet).unwrap(),
        };
    }

    /// compiles an array of floats to a hexadecimal string
    fn compile_to_bytes(&self, float_array: &Vec<f32>) -> String {
        let out = float_array
            .iter()
            .map(|&num| {
                let clamped = num.clamp(0.0, 1.0);
                let scaled = (clamped * 0xffff as f32).round() as u16;
                format!("{:04x}", scaled)
            })
            // Concatenate all hexadecimal substrings into one
            .collect::<String>();
        return out;
    }

    pub fn build_ping(&self) -> Packet {
        println!("Setting port: {}", self.connection_manager.recv_port);
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/ping".to_string(),
            args: vec![OscType::Int(self.connection_manager.recv_port.into())],
        }))
        .unwrap();
        Packet { packet: msg_buf }
    }

    fn build_get_all(&self) -> Packet {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/command".to_string(),
            args: vec![OscType::String("get all".to_string())],
        }))
        .unwrap();
        Packet { packet: msg_buf }
    }

    /// Sets the wifi connection manager's node map and flags it for transmission.
    pub fn set_node_list(&mut self, list: Vec<HapticNode>) -> Result<(), String> {
        let mut lock = self.connection_manager.config.write().unwrap();
        if let Some(mut wifi_con) = lock.take() {
            wifi_con.node_map = list;
            self.push_map;
            return Ok(());
        } else {
            return Err("no_map".to_string());
        }
    }
}

/// Manipulates the given flags according to the heartbeat timings.
fn manage_hrtbt(
    is_alive: &mut bool,
    _been_pinged: &mut bool,
    connection_manager: &WifiConnManager,
) {
    let now = SystemTime::now();
    let then = connection_manager.last_hrtbt.lock().unwrap();

    let diff = match now.duration_since(*then) {
        Ok(duration) => duration,
        Err(e) => {
            // Handle negative duration
            eprintln!("Duration issue, assuming alive: {:?}", e);
            Duration::from_secs(0)
        }
    };

    let ttl = Duration::from_secs(2);
    if diff > ttl && is_alive.to_owned() {
        *is_alive = false;
        *_been_pinged = false;
        println!("Set to false");
    } else if diff <= ttl && !is_alive.to_owned() {
        *is_alive = true;
    }
}
