use btleplug::api::BDAddr;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BhapticsDevice {
    TacsuitX16,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct BhapticBleDevice {
    address: BDAddr,
    model: BhapticsDevice,
    nodes: String,
}

impl BhapticBleDevice {
    //pub fn tick(&mut self, factors: OutputFactors, inputs: &InputMap) {
//
    //}
}