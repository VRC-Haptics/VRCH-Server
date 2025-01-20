pub mod mdns;

use std::collections::HashMap;

use rosc::{encoder, OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};

use crate::vrc::Parameters;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    pub mac: String,
    pub ip: String,
    pub display_name: String,
    pub port: u16,
    pub ttl: u32,
    pub addr_groups: Vec<AddressGroup>, //group name and start and end number
    pub num_motors: u32,
    been_pinged: bool,
    param_index: Vec<String>, //indexed parameters by group order
    cached_param: HashMap<String, OscType>,
    cached_menu: Parameters,
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

impl Device {
    #[allow(dead_code)]
    /// Instantiate new device instance
    pub fn new(mac: String, ip: String, display_name: String, port: u16, ttl: u32) -> Device {
        return Device {
            mac: mac,
            ip: ip,
            display_name: display_name,
            port: port,
            ttl: ttl,
            addr_groups: Vec::new(),
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
        menu: &Parameters,
        prefix: String,
    ) -> Option<Packet> {
        if !self.been_pinged {
            // first round through we ping
            println!("tried to ping board: {}@{}", self.display_name, self.mac);
            self.been_pinged = true;
            return Some(self.get_ping());
        }

        if self.cached_param.is_empty() {
            //only rebuild parameters if the cache has been purged
            //println!("Cache empty, building groups: {:?}", self.addr_groups);

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
            //println!("Cached Parameters: {:?}", self.cached_param);

            self.num_motors = ttl_motors;

            return Some(self.send_zero());
        }

        let mut send_flag = false;
        //see if motors updated
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

        if send_flag {
            let offset = 0.1; //self.cached_menu.get("offset"); I give up
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
        }

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
            args: vec![OscType::Int(1000)],
        }))
        .unwrap();
        Packet { packet: msg_buf }
    }

    fn send_zero(&self) -> Packet {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/h".to_string(),
            args: vec![rosc::OscType::String("0".repeat((self.num_motors * 4).try_into().unwrap()))],
        }))
        .unwrap();
        Packet { packet: msg_buf }
    }
}
