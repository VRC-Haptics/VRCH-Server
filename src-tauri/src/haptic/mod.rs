pub mod mdns;
mod recv;
mod btle;

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use recv::DeviceConnManager;
use rosc::{encoder, OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};

use crate::{util::next_free_port, vrc::Parameters};

#[derive(Serialize, Deserialize, Debug, Clone)]
enum DeviceType {
    Wifi(WifiDevice),
    Ble(BleDevice),
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    /// ID garunteed to be unique to that device 
    pub id: String,
    /// user-facing name 
    pub name: String,
    /// number of motors this device controls
    pub num_motors: u32,
    /// Not garunteed to change on death, but device should be removed if false
    pub is_alive: bool,
    /// Factors that are used in the modulation of devices
    pub factors: OutputFactors,
    pub device_type: DeviceType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Factors that affect or modulate output of Devices
pub struct OutputFactors {
    /// group name and start and end number
    pub addr_groups: Vec<AddressGroup>, 
    /// sensitivity multiplier (power limiter)
    pub sens_mult: f32,
    /// indexed parameters by group order
    param_index: Vec<String>,
    /// Menu parameters from last tick
    cached_menu: Parameters,
    /// All OSC parameters that have modified a value, and their last known values
    cached_param: HashMap<String, OscType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WifiDevice {
    pub mac: String,
    pub ip: Option<String>,
    pub recv_port: Option<u16>,
    pub port: u16,
    pub been_pinged: bool,
    pub is_alive: bool,
    connection_manager: DeviceConnManager,
}

/// Called on every server frame
trait Tick {
    fn tick(&mut self,
        addresses: &HashMap<String, Vec<rosc::OscType>>,
        menu: &Parameters,
        prefix: String,
    );
}

/// Called slightly before shutdown or when deleting a device. 
trait Stop {
    fn stop(&mut self);
}

// Delegate the Tick trait implementation to the inner types.
impl Tick for DeviceType {
    fn tick(&mut self,
        addresses: &HashMap<String, Vec<OscType>>,
        menu: &Parameters,
        prefix: String,
    ) {
        match self {
            DeviceType::Wifi(dev) => dev.tick(addresses, menu, prefix),
            DeviceType::Ble(dev) => dev.tick(addresses, menu, prefix),
        }
    }
}

// Delegate the Stop trait implementation to the inner types.
impl Stop for DeviceType {
    fn stop(&mut self) {
        match self {
            Device::Wifi(dev) => dev.stop(),
            Device::Ble(dev) => dev.stop(),
        }
    }
}

pub struct Packet {
    pub packet: Vec<u8>,
}

#[derive(Clone, Serialize, Debug, Deserialize)]
pub struct AddressGroup {
    pub name: String,
    pub start: u32,
    pub end: u32,
}

impl WifiDevice {
    #[allow(dead_code)]
    /// Instantiate new device instance
    pub fn new(mac: String, ip: String, send_port: u16, ttl: u32, full_name: String) -> Device {
        let display_name = full_name.split(".").next().unwrap().to_string();
        let recv_port = next_free_port(1000).unwrap();
        return Device {
            mac: mac,
            ip: ip.clone(),
            display_name: display_name,
            full_name: full_name,
            port: send_port,
            ttl: ttl,
            addr_groups: Vec::new(),
            conn_manager: DeviceConnManager::new(recv_port, "/hrtbt".to_string()),
            is_alive: true,
            sens_mult: 1.,
            num_motors: 0,
            been_pinged: false,
            param_index: Vec::new(),
            cached_param: HashMap::new(),
            cached_menu: Parameters::new(), //reuse so that we only have to edit in one place
        };
    }

    pub fn tick(
        &mut self,
        addresses: &HashMap<String, Vec<rosc::OscType>>,
        #[allow(unused_variables)] menu: &Parameters,
        prefix: String,
    ) -> Option<Packet> {
        if !self.been_pinged {
            // first round through we ping
            self.been_pinged = true;
            return Some(self.get_ping());
        }

        // manage hrtbt
        let now = SystemTime::now();
        let then = self.conn_manager.last_hrtbt.lock().unwrap();

        let diff = match now.duration_since(*then) {
            Ok(duration) => duration,
            Err(e) => {
                // Handle negative duration
                eprintln!("Duration issue, assuming alive: {:?}", e);
                Duration::from_secs(0)
            }
        };

        let ttl = Duration::from_secs(2);
        if diff > ttl && self.is_alive {
            self.is_alive = false;
            self.been_pinged = false;
            println!("Set to false");
        } else if diff <= ttl && !self.is_alive {
            self.is_alive = true;
        }

        //only rebuild parameters if the cache has been purged
        if self.cached_param.is_empty() && self.addr_groups.len() != 0 {
            println!("Cache empty, building groups on {}", self.display_name);

            //create motor addresses
            let mut ttl_motors = 0;
            let mut motor_addresses: Vec<String> = Vec::new();
            let groups = self.addr_groups.to_vec();
            for group in groups {
                ttl_motors += group.end - group.start + 1;
                for index in group.start..group.end + 1 {
                    let i: String = index.to_string();
                    motor_addresses.push(format!("{}/{}{}{}", prefix, group.name, "_", i));
                }
            }

            self.param_index = motor_addresses.clone();

            //create new cache
            for address in motor_addresses {
                self.cached_param.insert(address, OscType::Float(0.));
            }

            self.num_motors = ttl_motors;

            return Some(self.send_zero());
        }

        //see if motors updated
        let mut send_flag = false;
        for (address, old_param) in self.cached_param.iter_mut() {
            if let Some(new_values) = addresses.get(address) {
                if let Some(new_value) = new_values.first() {
                    match (old_param, new_value) {
                        (OscType::Float(ref mut old_float), OscType::Float(new_float)) => {
                            // Compare; update if different
                            if *old_float != *new_float {
                                *old_float = *new_float;
                                send_flag = true;
                            }
                        }
                        _ => {
                            unreachable!(
                                "Expected only OscType::Float variants in both old and new values"
                            );
                        }
                    }
                }
            }
        }

        //see if menu updated
        for (name, (addr, value)) in self.cached_menu.parameters.iter_mut() {
            if let Some(new_values) = addresses.get(addr) {
                if let Some(new_value) = new_values.first() {
                    match new_value {
                        OscType::Float(new_value) => {
                            // Compare; update if different
                            if *value != *new_value {
                                *value = *new_value;
                                println!("set:{name} to: {}", value);
                                send_flag = true;
                            }
                        }
                        _ => {
                            unreachable!(
                                "Expected only OscType::Float variants in both old and new values"
                            );
                        }
                    }
                }
            }
        }

        // send packet
        //if send_flag {
        let offset = self.sens_mult; //self.cached_menu.get("offset"); I give up
        let intensity = self.cached_menu.get("intensity");
        //println!("Cache after: {:?}", self.cached_param);
        let updated_floats: Vec<f32> = self
            .param_index
            .iter()
            .filter_map(|address| {
                if let Some(OscType::Float(value)) = self.cached_param.get(address) {
                    Some(*value * intensity * offset)
                } else {
                    None
                }
            })
            .collect();

        //println!("Updated floats: {:?}", updated_floats);
        let hex_message = self.compile_to_bytes(updated_floats);
        //println!("Hex Message: {}", hex_message);

        let message = rosc::OscMessage {
            addr: "/h".to_string(),
            args: vec![OscType::String(hex_message)],
        };
        let packet = rosc::OscPacket::Message(message);
        return Some(Packet {
            packet: rosc::encoder::encode(&packet).unwrap(),
        });
        //}

        return None;
    }

    /// Triggers rebuilding of cache and motor parameters.
    /// Should be called anytime any sort of device parameters are changed.
    #[allow(dead_code)]
    pub fn purge_cache(&mut self) {
        println!("{} purged cache: {:?}", self.mac, self.cached_param);
        self.cached_param.clear();
    }

    fn compile_to_bytes(&self, float_array: Vec<f32>) -> String {
        let out = float_array
            .iter()
            .map(|&num| {
                // Clamp the value between 0.0 and 1.0 to avoid overflow or underflow
                let clamped = num.clamp(0.0, 1.0);
                // Scale the float to the full range of a 16-bit integer [0, 65535]
                let scaled = (clamped * 0xffff as f32).round() as u16;
                // Format as a zero-padded 4-digit hexadecimal string
                format!("{:04x}", scaled)
            })
            // Concatenate all hexadecimal substrings into one
            .collect::<String>();
        return out;
    }

    pub fn get_ping(&self) -> Packet {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/ping".to_string(),
            args: vec![OscType::Int(self.conn_manager.recv_port as i32)],
        }))
        .unwrap();
        Packet { packet: msg_buf }
    }

    fn send_zero(&self) -> Packet {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/h".to_string(),
            args: vec![rosc::OscType::String(
                "0".repeat((self.num_motors * 4).try_into().unwrap()),
            )],
        }))
        .unwrap();
        Packet { packet: msg_buf }
    }
}