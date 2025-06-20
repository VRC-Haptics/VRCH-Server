use super::event::Event;
use super::Id;
use super::{
    input_node::InputNode,
    interp::{InterpAlgo, Interpolate},
    HapticNode,
};

use dashmap::DashMap;
use std::sync::Mutex;
use std::{fmt, sync::Arc};

/// The common factors that will be used across all devices to modify output.
/// Game inputs should insert values that will be used in device calculations here.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StandardMenu {
    pub intensity: f32, // multiplier set by user in-game
    pub enable: bool,   // Flat enable or disable all haptics
}

/// Provides implementations for interpolating input haptic intensities to device nodes
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GlobalMap {
    active_events: Vec<Event>,
    input_nodes: Arc<DashMap<Id, InputNode>>,
    pub standard_menu: Arc<Mutex<StandardMenu>>,
    #[serde(skip)]
    refresh_callbacks:
        Vec<Box<dyn Fn(&DashMap<Id, InputNode>, &Mutex<StandardMenu>) + Send + Sync + 'static>>,
}

impl Clone for GlobalMap {
    /// THIS DOES NOT CLONE THE REFRESH_CALLBACKS
    fn clone(&self) -> Self {
        Self {
            active_events: self.active_events.clone(),
            input_nodes: Arc::clone(&self.input_nodes),
            standard_menu: Arc::clone(&self.standard_menu),
            // callbacks are intentionally *not* cloned
            refresh_callbacks: Vec::new(),
        }
    }
}

impl GlobalMap {
    pub fn new() -> GlobalMap {
        return GlobalMap {
            active_events: Vec::new(),
            input_nodes: Arc::new(DashMap::new()),
            standard_menu: Arc::new(Mutex::new(StandardMenu {
                intensity: 1.0,
                enable: true,
            })),
            refresh_callbacks: vec![],
        };
    }

    /// Start a singular input event.
    pub fn start_event(&mut self, event: Event) {
        self.active_events.push(event);
    }

    /// Start a list of events, consumes the events vector.
    pub fn start_events(&mut self, events: &mut Vec<Event>) {
        self.active_events.append(events);
    }

    /// Clear all playing events.
    pub fn clear_events(&mut self, tag: &String) {
        self.active_events.retain(|event| !event.tags.contains(tag));
    }

    /// registers a function to be called on a refresh event before every device update.
    pub fn register_refresh<F>(&mut self, fun: F)
    where
        F: Fn(&DashMap<Id, InputNode>, &Mutex<StandardMenu>) + Send + Sync + 'static,
    {
        self.refresh_callbacks.push(Box::new(fun));
    }

    /// called immediately before each device tick.
    /// It invites each of the game integrations to insert their values into the global map.
    /// Then cycles all events.
    pub fn refresh_inputs(&mut self) {
        // refresh direct game inputs.
        for callback in &self.refresh_callbacks {
            let clone = Arc::clone(&self.input_nodes);
            let menu = Arc::clone(&self.standard_menu);
            callback(&clone, &menu);
        }
        // Tick every event and keep only those that should continue running.
        self.active_events.retain_mut(|event| {
            let finished = event.tick(Arc::clone(&self.input_nodes));
            !finished
        });
    }

    /// checks for duplicates and registers input node for writing to
    pub fn add_input_node(
        &self,
        new_node: HapticNode,
        tags: Vec<String>,
        id: String,
    ) -> Result<(), DuplicateNodeIDError> {
        if let Some(existing) = self.input_nodes.get(&Id(id.clone())) {
            return Err(DuplicateNodeIDError {
                existing: existing.clone(),
            });
        }

        self.input_nodes
            .insert(Id(id.clone()), InputNode::new(new_node, tags, Id(id)));

        Ok(())
    }

    /// Removes the input node from being used in haptic interpolation
    pub fn pop_input_node(&mut self, id: String) -> Result<InputNode, DoesNotExistError> {
        if let Some((_, node)) = self.input_nodes.remove(&Id(id.clone())) {
            return Ok(node);
        }

        Err(DoesNotExistError { id: id })
    }

    /// Removes all input nodes with the given tag.
    pub fn remove_all_with_tag(&self, tag: &String) {
        self.input_nodes.retain(|_, node| !node.tags.contains(tag));
    }

    /// Sets a nodes intensity by id
    ///
    /// Returns old value
    pub fn set_intensity(&mut self, id: &str, new: f32) -> Result<f32, DoesNotExistError> {
        if let Some(mut node) = self.input_nodes.get_mut(&Id(id.to_string())) {
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

        if let Some(node) = self.input_nodes.get(&Id(id.clone())) {
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
    pub fn get_intensity_from_haptic(
        &self,
        node_list: &Vec<HapticNode>,
        algo: &InterpAlgo,
        respect_enable: &bool,
    ) -> Vec<f32> {
        let menu_lock = self.standard_menu.lock().expect("unable to get lock");
        if *respect_enable && !menu_lock.enable {
            return vec![0.0; node_list.len()];
        }
        let local = Arc::clone(&self.input_nodes);
        let locals = <DashMap<Id, InputNode> as Clone>::clone(&local).into_read_only();
        let values = locals.values();
        let input_list = values.collect::<Vec<&InputNode>>();
        algo.interp(node_list, input_list)
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
