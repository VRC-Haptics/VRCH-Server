use super::Id;
use super::{
    input_node::InputNode, interp::{InterpAlgo, Interpolate}, HapticNode
};

use std::{
    sync::Arc,
    fmt
};
use dashmap::DashMap;

/// Provides implementations for interpolating input haptic intensities to device nodes
pub struct GlobalMap {
    input_nodes: Arc<DashMap<Id, InputNode>>,
    global_offset: f32,
    global_enable: bool,
    refresh_callbacks: Vec<Box<dyn Fn(&DashMap<Id, InputNode>) + Send + Sync + 'static>>,
}

impl GlobalMap {
    pub fn new() -> GlobalMap {
        return GlobalMap {
            input_nodes: Arc::new(DashMap::new()),
            global_offset: 1.0,
            global_enable: true,
            refresh_callbacks: vec![]
        }
    }

    /// registers a function to be called on a refresh event
    pub fn register_refresh<F>(&mut self, fun: F)
    where
        F: Fn(&DashMap<Id, InputNode>) + Send + Sync + 'static,
    {
        self.refresh_callbacks.push(Box::new(fun));

        return;
    }

    /// called immediately before each device tick.
    /// It invites each of the game integrations to insert their values into the global map
    pub fn refresh_inputs(&mut self) {
        for callback in &self.refresh_callbacks {
            let clone = Arc::clone(&self.input_nodes);
            callback(&clone);
        }
    }

    /// checks for duplicates and registers input node for writing to
    pub fn add_input_node(&self, new_node: HapticNode, tags: Vec<String>, id: String) -> Result<(), DuplicateNodeIDError> {
        if let Some(existing) = self.input_nodes.get(&Id(id.clone())) {
            return Err(DuplicateNodeIDError{existing:existing.clone()});
        }

        self.input_nodes.insert(Id(id.clone()), InputNode::new(new_node, tags, Id(id)));

        Ok(())
    }

    /// Removes the input node from being used in haptic interpolation
    pub fn pop_input_node(&mut self, id: String) -> Result<InputNode, DoesNotExistError> {
        if let Some((_, node)) = self.input_nodes.remove(&Id(id.clone())) {
            return Ok(node);
        }

        Err(DoesNotExistError{id: id})
    }

    /// Sets a nodes intensity by id
    /// 
    /// Returns old value
    pub fn set_intensity(&mut self, id: String, new: f32) -> Result<f32, DoesNotExistError> {
        if let Some(mut node) = self.input_nodes.get_mut(&Id(id.clone())) {
            let old = node.get_intensity();
            node.set_intensity(new);
            return Ok(old);
        }

        Err(DoesNotExistError{id: id})
    }

    /// Returns the InputNodes Intensity by ID
    /// 
    /// `respect_enable`: toggles whether to ignore the global_enable parameter
    pub fn get_intensity(&mut self, id: String, respect_enable: bool) -> Result<f32, DoesNotExistError> {
        if !self.global_enable && respect_enable {
            return Ok(0.0);        
        }

        if let Some(node) = self.input_nodes.get(&Id(id.clone())) {
            return Ok(node.get_intensity() * self.global_offset);
        }

        Err(DoesNotExistError{id: id})
    }

    /// Returns the interpolated value for a given HapticNode
    /// 
    /// `node`: the input HapticNode
    /// 
    /// `algo`: the algorithm state that will be used to create the returned value
    /// 
    /// `respect_enable`: toggles whether to ignore the global_enable parameter
    /// 
    pub fn get_intensity_from_haptic(&self, node: &HapticNode, algo: &InterpAlgo, respect_enable: &bool) -> f32 {
        if *respect_enable && !self.global_enable {
            return 0.0;
        }
        let local  = Arc::clone(&self.input_nodes);
        let locals  = <DashMap<Id, InputNode> as Clone>::clone(&local).into_read_only();
        let values = locals.values();
        let input_list = values.collect::<Vec<&InputNode>>();
        algo.interp(node, input_list)
    }
}

/// ERRORS -----------------------

#[derive(Debug, Clone)]
pub struct DoesNotExistError {
    id: String
}

impl fmt::Display for DoesNotExistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"No registered node with id: {:?}", self.id)
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