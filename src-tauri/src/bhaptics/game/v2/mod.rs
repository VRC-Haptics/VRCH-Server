use std::sync::{Arc, Mutex};
use crate::mapping::event::Event;

pub struct BhapticsApiV2 {
 smth: String
}

impl BhapticsApiV2 {
    /// Creates a new instance, starts the server on a separate thread,
    /// and returns an Arc-wrapped and Mutex-guarded game state.
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(BhapticsApiV2 {
            smth: "THIS".to_string()
        }))
    }

    /// Returns the list of events that were triggerd during this tick.
    pub fn tick(&mut self) -> Vec<Event> {
        Vec::new()
    }
}