use crate::devices::OutputNodes;
use crate::mapping::input_node::InputType;

use super::event::Event;
use super::Id;
use super::{
    input_node::InputNode,
    interp::{InterpAlgo, Interpolate},
    HapticNode,
};

use dashmap::{mapref::one::RefMut, DashMap};
use std::sync::{Mutex, RwLock};
use std::time::Duration;
use std::{fmt, sync::Arc};

/// The common factors that will be used across all devices to modify output.
/// Game inputs should insert values that will be used in device calculations here.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StandardMenu {
    pub intensity: f32, // multiplier set by user in-game
    pub enable: bool,   // Flat enable or disable all haptics
}

/// Provides implementations for interpolating input haptic intensities to device nodes
/// 
/// Should be fully threadsafe.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GlobalMap {
    active_events: Arc<RwLock<Vec<Event>>>,
    input_nodes: Arc<RwLock<Vec<InputNode>>>,
    pub standard_menu: Arc<Mutex<StandardMenu>>,
}

impl Clone for GlobalMap {
    fn clone(&self) -> Self {
        Self {
            active_events: Arc::clone(&self.active_events),
            input_nodes: Arc::clone(&self.input_nodes),
            standard_menu: Arc::clone(&self.standard_menu),
        }
    }
}

impl GlobalMap {
    /// NOTE: starts event manager, only intended to be started once per program.
    pub async fn new() -> Arc<GlobalMap> {
        let map = Arc::new(GlobalMap {
            active_events: Arc::new(RwLock::new(Vec::new())),
            input_nodes: Arc::new(RwLock::new(Vec::new())),
            standard_menu: Arc::new(Mutex::new(StandardMenu {
                intensity: 1.0,
                enable: true,
            }))
        });

        // spawn event manager.
        let events = Arc::clone(&map);
        tokio::spawn(async move {
            loop {
                // Tick every event and keep only those that should continue running.
                let mut lock = events.active_events.write().expect("Failed to lock events");
                let mut nodes = events.input_nodes.write().expect("Unable to get nodes ");
                lock.retain_mut(|event| {
                    let finished = event.tick(&mut nodes);
                    !finished
                });

                tokio::time::sleep(Duration::from_millis(10));
            } 
        });

        map
    }

    /// sets all input nodes with a given tag to the radius.
    pub fn set_radius_by_tag(&mut self, target_tag: &str, new_radius: f32) {
        let mut lock_mut = self.input_nodes.write().expect("couldn't get node write");
        lock_mut
            .iter_mut()
            .filter(|entry| entry.tags.contains(&target_tag.to_string()))
            .for_each(|mut entry| {
                entry.set_radius(new_radius);
            });
    }

    /// Performs function f with the specified node carrying id: `id`
    pub fn with_node_mut<F, R>(&self, id: &Id, f: F) -> Option<R>
    where
        F: FnOnce(&mut InputNode) -> R,
    {
        let mut lock = self.input_nodes.write().expect("unable to lock");
        lock.iter_mut().find(|n| n.get_id() == id).map(f)
    }


    /// Start a singular input event.
    pub fn start_event(&mut self, event: Event) {
        let mut lock = self.active_events.write().expect("Failed to lock events");
        lock.push(event);
    }

    /// Start a list of events, consumes the events vector.
    pub fn start_events(&mut self, events: &mut Vec<Event>) {
        let mut lock = self.active_events.write().expect("Failed to lock events");
        lock.append(events);
    }

    /// Clear all playing events.
    pub fn clear_events(&mut self, tag: &String) {
        let mut lock = self.active_events.write().expect("Failed to lock events");
        lock.retain(|event| !event.tags.contains(tag));
    }

    /// checks for duplicates and registers input node to the map
    pub fn add_input_node(
        &self,
        new_node: HapticNode,
        tags: Vec<String>,
        id: String,
        radius: f32,
        input_type: Option<InputType>,
    ) -> Result<(), DuplicateNodeIDError> {
        let lock = self.input_nodes.read().expect("couldn't get node write");
        if let Some(existing) = lock.iter().find(|n| *n.get_id() == *id) {
            return Err(DuplicateNodeIDError {
                existing: existing.clone(),
            });
        }

        let mut lock_mut = self.input_nodes.write().expect("couldn't get node write");
        lock_mut.push(
            InputNode::new(
                new_node,
                tags,
                Id(id),
                radius,
                input_type.unwrap_or_else(|| InputType::INTERP),
            ),
        );

        Ok(())
    }

    /// Removes the input node from being used in haptic interpolation
    pub fn pop_input_node(&self, id: &str) -> Result<InputNode, DoesNotExistError> {
    let mut lock = self.input_nodes.write().expect("couldn't get node write");
    let idx = lock.iter().position(|n| n.get_id() == id);
    
    match idx {
        Some(i) => Ok(lock.swap_remove(i)),
        None => Err(DoesNotExistError { id: id.to_string() }),
    }
}

    /// Removes all input nodes with the given tag.
    pub fn remove_all_with_tag(&self, tag: &String) {
        let mut lock = self.input_nodes.write().expect("couldn't get node write");
        lock.retain(|node| !node.tags.contains(tag));
    }

    /// Sets a nodes intensity by id
    ///
    /// Returns old value
    pub fn set_intensity(&mut self, id: &str, new: f32) -> Result<f32, DoesNotExistError> {
        let mut lock = self.input_nodes.write().expect("unable to lock");
        
        if let Some(node) = lock.iter_mut().find(|n| n.get_id() == id) {
            let old = node.get_intensity();
            node.set_intensity(new);
            return Ok(old);
        }

        Err(DoesNotExistError { id: id.to_string() })
    }

    /// Returns the InputNodes Intensity by ID
    ///
    /// `respect_enable`: toggles whether to ignore the global_enable parameter
    pub fn get_intensity(
        &mut self,
        id: String,
        respect_enable: bool,
    ) -> Result<f32, DoesNotExistError> {
        let menu_lock = self.standard_menu.lock().expect("unable to get lock");
        if !menu_lock.enable && respect_enable {
            return Ok(0.0);
        }

        let lock = self.input_nodes.read().expect("Failed locking inputs");
        if let Some(node) = lock.iter().find(|n| *n.get_id() == *id) {
            return Ok(node.get_intensity() * menu_lock.intensity);
        }

        Err(DoesNotExistError { id: id })
    }

    /// Returns the interpolated value for a given HapticNode list
    ///
    /// `node`: the input HapticNode list
    ///
    /// `algo`: the algorithm state that will be used to create the returned value
    ///
    /// `respect_enable`: toggles whether to ignore the global_enable parameter
    /// 
    /// `output`: the output buffer to fill with return values. Scaled between 0 and 1.
    ///
    pub fn get_intensity_from_haptic(
        &self,
        nodes: &mut OutputNodes,
        algo: &InterpAlgo,
        respect_enable: &bool,
    ) {
        let menu_lock = self.standard_menu.lock().expect("unable to get lock");
        if *respect_enable && !menu_lock.enable {
            let output = nodes.outputs_mut();
            output.fill(0.0);
            return;
        }
        let lock = self.input_nodes.read().expect("unable to lock input nodes");
        algo.interp(node_list, &lock, output);
    }
}

/// ERRORS -----------------------

#[derive(Debug, Clone)]
pub struct DoesNotExistError {
    id: String,
}

impl fmt::Display for DoesNotExistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No registered node with id: {:?}", self.id)
    }
}

#[derive(Debug, Clone)]
pub struct DuplicateNodeIDError {
    existing: InputNode,
}

impl fmt::Display for DuplicateNodeIDError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ID with same name exists already: {:?}", self.existing)
    }
}
