use std::time::{SystemTime, Duration};
use crate::util::math::Vec3;
use std::sync::Arc;
use dashmap::DashMap;
use super::{haptic_node::HapticNode, input_node::InputNode, Id, NodeGroup};

/// Describes what effect an event should have.
#[derive(Clone)]
pub enum EventEffectType {
    /// Will try to set this node id to this value,
    /// Does not remove node after finished.
    SingleNode(Id),
    /// Same as single node, but with a list of them.
    MultipleNodes(Vec<Id>),
    /// Effects all InputNodes with the given tag.
    Tags(Vec<String>),
    /// Inserts a node at the given location, automatically removes node when event expires.
    Location(Vec3),
    /// Divides locations between the duration of the event and moves the node to that location.
    MovingLocation(Vec<Vec3>),
}

/// Represents a haptic event that takes place over time.
#[derive(Clone)]
pub struct Event {
    /// user facing name
    pub name: String,
    /// how should this event effect the input map
    pub effect: EventEffectType,
    /// The different outputs that should be output at different times.
    steps: Vec<f32>,
    /// The total duration of this event.
    /// 
    /// Steps will be distributed across this duration.
    pub duration: Duration,
    /// Tags that will be inserted to each node created by this event.
    pub tags: Vec<String>,
    managed_nodes: Vec<Id>, // nodes we have control over.
    time_step: Duration,
    steps_completed: usize,
    start_time: Option<SystemTime>,
}

impl Event {
    /// creates a new instance of an event. 
    /// 
    /// `name`: The user facing name that will be displayed in the ui.
    /// 
    /// `effect`: The effect that this event should have.
    /// 
    /// `steps`: The value that will be used by the EffectType at each step.
    /// 
    /// `duration`: The duration this event will be spread over (num_steps/duration must be > 10ms) 
    /// 
    /// `tags`: Any special tags to add to the event during operation. (atleast one required) Useful for clearing all events
    /// associated with a given event source.
    pub fn new(name: String, effect: EventEffectType, steps: Vec<f32>, duration: Duration, tags: Vec<String>) -> Result<Event, CreateEventError> {
        if steps.len() < 1 {
            return Err(CreateEventError::NotEnoughSteps);
        }

        let time_step = duration.clone().div_f32(steps.len() as f32);
        if time_step.as_millis() < 9 { // for rounding safety, rather permissive than error.
            return Err(CreateEventError::TooSmallTimestep);
        }

        if tags.is_empty() {
            log::warn!("Event without tags are not recommended: {}", name);
        }
        
        let ev = Event {
            name: name, 
            effect: effect, 
            steps: steps.clone(), 
            duration: duration,
            tags: tags,
            managed_nodes: Vec::new(),
            time_step: time_step,
            steps_completed: 0,
            start_time: None,
        };

        return Ok(ev);
    }

    /// Propogates the changes this event represents into the gameMap at this time.
    /// 
    /// Returns whether this event should be removed from the pool.
    pub fn tick(&mut self, input_nodes: Arc<DashMap<Id, InputNode>>) -> bool {
        // will return early if initiation isn't needed.
        self.initiate(&input_nodes);

        // get current time.
        let now = SystemTime::now();
        let start = self.start_time.get_or_insert(now);
        let elapsed = match now.duration_since(*start) {
            Ok(d) => d,
            // I hate timing errors.
            Err(_) => Duration::ZERO,
        };

        let should_have_fired = (elapsed.as_nanos() / self.time_step.as_nanos()) as usize;

        // apply effects if we need to.
        while self.steps_completed <= should_have_fired
            && self.steps_completed < self.steps.len()
        {
            let value = self.steps[self.steps_completed];
            self.apply_effect(value, &input_nodes);
            self.steps_completed += 1;
        };

        // if the final effects have happened, clean up our stuff.
        if elapsed >= self.duration {
            self.cleanup(&input_nodes);
            return true;
        }

        false
    }

    /// Initiates the input_nodes state to handle our event.
    /// 
    /// Returns early if start_time is already defined.
    fn initiate(&mut self, input_nodes: &DashMap<Id, InputNode>) {
        if self.start_time.is_some() { return; }         // already started
        //log::trace!("Starting event: {}", self.name);

        match &self.effect {
            EventEffectType::Location(pos) => {
                let id = Id::new();
                let haptic_node = HapticNode {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                    groups: vec![NodeGroup::All],
                };
                input_nodes.insert(id.clone(), InputNode::new(haptic_node, self.tags.clone(), id.clone()));
                self.managed_nodes.push(id);
            }
            EventEffectType::MovingLocation(path) if !path.is_empty() => {
                let id = Id::new();
                let first = path.first().unwrap(); // verified non-zero at new() function
                let haptic_node = HapticNode {
                    x: first.x,
                    y: first.y,
                    z: first.z,
                    groups: vec![NodeGroup::All],
                };
                input_nodes.insert(id.clone(), InputNode::new(haptic_node, self.tags.clone(), id.clone()));
                self.managed_nodes.push(id);
            }
            _ => {}
        }

        self.start_time = Some(SystemTime::now());
    }

    /// Applies the described effect at for a given value.
    fn apply_effect(&self, value: f32, input_nodes: &DashMap<Id, InputNode>) {
        match &self.effect {
            EventEffectType::SingleNode(id) => {
                if let Some(mut node) = input_nodes.get_mut(id) {
                    node.set_intensity(value);
                }
            }
            EventEffectType::MultipleNodes(ids) => {
                for id in ids {
                    if let Some(mut node) = input_nodes.get_mut(id) {
                        node.set_intensity(value);
                    }
                }
            }
            EventEffectType::Tags(tags) => {
                input_nodes
                    .iter_mut()
                    .filter(|kv| tags.iter().any(|t| kv.value().tags.contains(t)))
                    .for_each(|mut kv| kv.set_intensity(value));
            }
            EventEffectType::Location(_) => {
                let id = self.managed_nodes.first().unwrap(); // initiate is called first, which garuntees atleast one managed node.
                if let Some(mut node) = input_nodes.get_mut(id) {
                    node.set_intensity(value);
                }
            }
            EventEffectType::MovingLocation(waypoints) => {
                let idx = self.steps_completed.min(waypoints.len() - 1);

                let id = self.managed_nodes.first().unwrap(); // initiate is called first, which garuntees atleast one managed node.
                if let Some(mut node) = input_nodes.get_mut(id) {
                    node.set_position(waypoints[idx]);
                    node.set_intensity(value);
                }
            }
        }
    }

    /// cleans up the leftover nodes when an event is finished.
    fn cleanup(&self, input_nodes: &DashMap<Id, InputNode>) {
        //log::trace!("Finished event: {}", self.name);
        match &self.effect {
            EventEffectType::Location(_) | EventEffectType::MovingLocation(_) => {
                // Remove transient node(s) that were spawned only for this event
                for ids in &self.managed_nodes {
                    input_nodes.remove(ids);
                };
            },
            EventEffectType::SingleNode(id) => {
                if let Some(mut node) = input_nodes.get_mut(id) {
                    node.set_intensity(0.);
                } 
            }
            _ => { /* nothing to remove */ }
        }
    }
}

pub enum CreateEventError {
    /// must contain atleast one step to execute.
    NotEnoughSteps,
    /// duration/steps must result in atleast a 10ms period.
    TooSmallTimestep,
    /// empty tags are not allowed, mainly for debugging.
    EmptyTags,
}