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
use crate::util::math::Vec3;
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

    /// Called in regular intervals. Optionally returns a packet to be sent to the device.
    pub fn tick(
        &mut self,
        // if the Device should be considered ready for haptics
        is_alive: &mut bool,
        // output factors specific to nodes under this device only
        factors: &mut OutputFactors,
        // The inputs that will be used to give feedback.
        inputs: &GlobalMap,
    ) -> Option<Packet> {
        if !self.been_pinged {
            // first round through we ping
            self.been_pinged = true;
            return Some(self.build_ping());
        }

        // keep track of heartbeat timings and whatnot
        manage_hrtbt(is_alive, &mut self.been_pinged, &self.connection_manager);

        // check if we recieved and parsed the config yet.
        if let Some(conf) = self.connection_manager.config.write().unwrap().as_ref() {
            //push config to device if necessary
            if self.push_map {
                self.push_map = false;
                let set_map = self.build_set_map(&conf.node_map);
                return Some(set_map);
            }

            // Collect haptic values and scale to output.
            let mut intensities =
                inputs.get_intensity_from_haptic(&conf.node_map, &factors.interp_algo, &true);
            let global_offset = inputs.standard_menu.lock().expect("Global Lock").intensity;
            intensities
                .iter_mut()
                .for_each(|x| *x = scale(*x, factors, global_offset));
            return Some(self.compile_message(&intensities));
        } else {
            // If no mapping configuration found
            // Gather the configuration
            let now = SystemTime::now();
            let diff = now
                .duration_since(self.last_queried)
                .expect("Error getting difference");
            if diff > Duration::from_millis(2000) || self.last_queried == SystemTime::UNIX_EPOCH {
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

        // compile to osc formatted packet
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
        log::info!("Setting port: {}", self.connection_manager.recv_port);
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
        if let Some(wifi_con) = lock.as_mut() {
            wifi_con.node_map = list;
            self.push_map = true;
            return Ok(());
        } else {
            return Err("no_map".to_string());
        }
    }

    /// Swaps the configured nodes at locations
    pub fn swap_nodes(&mut self, pos_1: Vec3, pos_2: Vec3) -> Result<(), String> {
        let mut lock = self.connection_manager.config.write().unwrap();
        if let Some(wifi_con) = lock.as_mut() {
            // get index of nodes
            let mut index1:Option<usize> = None;
            let mut index2:Option<usize> = None;
            for (index, node) in wifi_con.node_map.iter().enumerate() {
                if node.to_vec3().close_to(&pos_1, EPSILON) {
                    index1 = Some(index);
                    log::debug!("Found node 1 at index: {:?}", index1);
                } else if node.to_vec3().close_to(&pos_2, EPSILON) {
                    index2 = Some(index);
                    log::debug!("Found node 2 at index: {:?}", index2);
                }
            }

            // swap them
            if let (Some(i1), Some(i2)) = (index1, index2) {
                let first = wifi_con.node_map[i1].clone();
                wifi_con.node_map[i1] = wifi_con.node_map[i2].clone();
                wifi_con.node_map[i2] = first;
                self.push_map = true;
                return Ok(());
            } else {
                return Err("Couldn't find both nodes to swap".to_string());
            }

        } else {
            return Err("no_map".to_string());
        }
    }
}

const EPSILON: f32 = 0.0001;

/// scales a float value according to the output factors.
fn scale(val: f32, factors: &OutputFactors, global_offset: f32) -> f32 {
    if val <= EPSILON {
        return 0.0;
    }
    if 1.0 - val <= EPSILON {
        return factors.sens_mult;
    }
    
    let range = factors.sens_mult - factors.start_offset;
    (val*global_offset) * range + factors.start_offset
}

/// Manipulates the given flags according to the heartbeat timings.
/// If is_alive is set to false, device will be removed from being tracked immediatly.
/// If is_alive is set to false, device will be removed from being tracked immediatly.
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
            log::error!("Duration issue, assuming alive: {:?}", e);
            Duration::from_secs(0)
        }
    };

    // if outlived time to live and we are currently set as alive
    let ttl = Duration::from_secs(3);
    // if outlived time to live and we are currently set as alive
    if diff > ttl && is_alive.to_owned() {
        *is_alive = false;
        *_been_pinged = false;
        log::trace!("Set to false");
    // if the not ttl has passed.
    } else if diff <= ttl {
    // if the not ttl has passed.
    } else if diff <= ttl {
        *is_alive = true;
    }
}
