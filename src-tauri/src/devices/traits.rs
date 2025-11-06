/*

use crate::{
    devices::{DeviceType, OutputFactors},
    mapping::global_map::GlobalMap,
};

/// Called on every server frame (~100hz)
/// Should handle sending, recieving, killing, etc.
trait Tick {
    fn tick(&mut self, is_alive: &mut bool, factors: &mut OutputFactors, inputs: &GlobalMap);
}

/// Called slightly before shutdown or when deleting a device.
trait Stop {
    fn stop(&mut self);
}

// Delegate the Stop trait implementation to the inner types.
impl Stop for DeviceType {
    fn stop(&mut self) {
        log::error!("Stop not implemented for devices yet.");
    }
}*/