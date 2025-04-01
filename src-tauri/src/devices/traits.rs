use crate::devices::{
    DeviceType,
    OutputFactors
};

pub enum MenuType {
    Float(f32),
    Int(i32),
    String(String),
}

/// Called on every server frame (~100hz)
/// Should handle sending, recieving, killing, etc.
trait Tick {
    fn tick(&mut self, is_alive: &mut bool, factors: &mut OutputFactors);
}

/// Called slightly before shutdown or when deleting a device.
trait Stop {
    fn stop(&mut self);
}

// Delegate the Tick trait implementation to the inner types.
impl Tick for DeviceType {
    fn tick(&mut self, is_alive: &mut bool, factors: &mut OutputFactors) {
        match self {
            DeviceType::Wifi(dev) => {
                dev.tick(is_alive, factors);
            }
            _ => println!("unknown device type"),
        }
    }
}

// Delegate the Stop trait implementation to the inner types.
impl Stop for DeviceType {
    fn stop(&mut self) {
        match self {
            DeviceType::Wifi(dev) => dev.stop(),
        }
    }
}
