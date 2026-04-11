use btleplug::api::BDAddr;

use crate::devices::DeviceId;

pub struct BhapticInfo {
    id: DeviceId,
    
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BhapticsModel {
    TacsuitX16,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct BhapticBleDevice {
    address: BDAddr,
    model: BhapticsModel,
    nodes: String,
}

impl super::Device for BhapticBleDevice {
    fn get_id(&self) -> super::DeviceId {
        self.address.to_string_no_delim().into()
    }

    fn info(&self) -> super::DeviceInfo {
        
    }
}