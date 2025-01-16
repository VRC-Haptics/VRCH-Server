pub mod mdns;

use std::collections::HashMap;

use rosc::{encoder, OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    pub MAC: String,
    pub IP: String,
    pub DisplayName: String,
    pub Port: u16,
    pub TTL: u32,
    pub addr_groups: Vec<AddressGroup>, //group name and start and end number
    pub num_motors: u32,
    been_pinged: bool,
    param_index: Vec<String>, //indexed parameters by group order
    cached_param: HashMap<String, OscType>,
}

impl Device {
    /// Instantiate new device instance
    pub fn new(mac: String, ip: String, display_name: String, port: u16, ttl: u32) -> Device {
        return Device {
            MAC: mac,
            IP: ip,
            DisplayName: display_name,
            Port: port,
            TTL: ttl,
            addr_groups: Vec::new(),
            num_motors: 0,
            been_pinged: false,
            param_index: Vec::new(),
            cached_param: HashMap::new(),
        };
    }

    pub fn tick(
        &mut self,
        addresses: &HashMap<String, Vec<rosc::OscType>>,
        prefix: String,
    ) -> Option<Packet> {
        if !self.been_pinged {
            // first round through we ping
            self.been_pinged = true;
            return Some(self.get_ping());
        }

        if self.cached_param.is_empty() {
            //create motor addresses
            let mut ttl_motors = 0;
            let mut motor_addresses: Vec<String> = Vec::new();
            let groups = self.addr_groups.to_vec();
            for group in groups {
                ttl_motors += group.end - group.start + 1;
                for index in group.start..group.end + 1 {
                    let i: String = index.to_string();
                    motor_addresses.push(format!("{}{}{}{}", prefix, group.name, "_", i));
                }
            }

            self.param_index = motor_addresses.clone();

            //create new cache
            for address in motor_addresses {
                self.cached_param.insert(address, OscType::Float(0.));
            }

            self.num_motors = ttl_motors;
        }

        let mut send_flag = false;
        for (address, old) in self.cached_param.iter_mut() {
            if let Some(new_vec) = addresses.get(address) {
                let new = new_vec.first().expect("Empty message at motor send");
                match (old.to_owned(), new) {
                    (OscType::Float(ref mut old_float), OscType::Float(new_float)) => {
                        if *old_float != *new_float {
                            *old_float = *new_float;
                            send_flag = true;
                        }
                    }
                    _ => unreachable!(
                        "Expected only OscType::Float variants in both old and new values"
                    ),
                }
            }
        }

        if send_flag {
            let updated_floats: Vec<f32> = self
                .param_index
                .iter()
                .filter_map(|address| {
                    if let Some(OscType::Float(value)) = self.cached_param.get(address) {
                        Some(*value)
                    } else {
                        None
                    }
                })
                .collect();

            let hex_message = self.compile_to_bytes(updated_floats);
            return Some(Packet {
                packet: hex_message,
            });
        }

        return None;
    }

    /// Triggers rebuilding of cache and motor parameters.
    /// Should be called anytime any sort of device parameters are chagned.
    pub fn purge_cache(&mut self) {
        self.cached_param.clear();
    }

    fn compile_to_bytes(&self, float_array: Vec<f32>) -> Vec<u8> {
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
        return out.into_bytes();
    }

    pub fn get_ping(&self) -> Packet {
        println!("sent ping to: {}", self.DisplayName);
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/ping".to_string(),
            args: vec![OscType::Int(1000)],
        }))
        .unwrap();
        Packet { packet: msg_buf }
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
