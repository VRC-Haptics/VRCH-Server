pub mod mdns;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    MAC: String,
    IP: String,
    DisplayName: String,
    Port: u16,
    TTL: u32,
}
