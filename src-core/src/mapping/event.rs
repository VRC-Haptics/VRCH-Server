use crate::mapping::{EventDuration, EventInstant, NodeKey, Nodes, input_node::InputNode};
use glam::Vec3;
use slotmap::SlotMap;
use std::{usize, sync::Arc};


//const _: [u8; std::mem::size_of::<MaybeConst<()>>()] = [];
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum MaybeConst<T> 
where 
    T: Clone ,
{
    /// Is not constant, an index for each frame. 
    /// Will behave like constant after running out of frames.
    Var(Arc<[T]>),
    /// Will be constant for the duration of this event.
    Const(T),
}

impl<T: Clone> From<T> for MaybeConst<T> {
    fn from(val: T) -> Self {
        Self::Const(val)
    }
}

impl<T: Clone> From<Arc<[T]>> for MaybeConst<T> {
    fn from(val: Arc<[T]>) -> Self {
        Self::Var(val)
    }
}

impl<T: Clone> MaybeConst<T> {
    /// create constant variant.
    pub const fn new_const(val: T) -> Self {
        Self::Const(val)
    }

    /// create variable with steps.
    pub const fn new(val: Arc<[T]>) -> Self{
        Self::Var(val)
    }

    pub const fn is_const(&self) -> bool {
        match self {
            Self::Const(_) => true,
            Self::Var(_) => false,
        }
    }

    /// Gets the initial value of the value.
    pub fn first(&self) -> &T {
        match self {
            Self::Const(v) => v,
            Self::Var(l) => &l[0]
        }
    }

    pub fn get(&self, idx: usize) -> Option<&'_ T> {
        match self {
            Self::Const(_) => None,
            Self::Var(l) => l.get(idx)
        }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
/// A collection of values and consts, Constant values will only be initialized.
pub struct Frames {
    pub nodes: Vec<Steps>,
}

impl Frames {
    pub fn new(nodes: Vec<Steps>) -> Self {
        Self { nodes }
    }
}


#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Steps {
    pub position: MaybeConst<Vec3>,
    pub muted: MaybeConst<bool>,
    pub value: MaybeConst<f32>,
    pub weight: MaybeConst<f32>,
    pub radius: MaybeConst<f32>,
}

/// Represents a haptic event that takes place over time.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct Event {
    /// user facing name
    pub name: String,
    /// lots of identical events can be triggered.
    pub frames: Frames,
    /// The total duration of this even in event ticks (100hz)
    /// 
    /// Nodes will be active in the map for this long and modified by the frames inserted.
    pub duration: EventDuration,
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
    pub fn new(
        name: String,
        frames: Frames,
        duration: EventDuration,
    ) -> Result<Event, CreateEventError> {
        if duration < 1 {
            return Err(CreateEventError::NotEnoughSteps);
        }

        let ev = Event {
            name: name,
            frames: frames,
            duration: duration,
        };

        return Ok(ev);
    }

    pub fn num_nodes(&self) -> usize {
        self.frames.nodes.len()
    }

    /// Produces mutations to the map via the tx module.
    /// Not allowed to mutate self, event is a parent that is static for this map epoch.
    /// Only mutate the keys
    pub fn tick(&self, nodes: &mut SlotMap<NodeKey, InputNode>, keys: &[NodeKey], since_start: EventInstant) {

        let frames = &self.frames;
        for (idx, steps) in frames.nodes.iter().enumerate() {
            let Some(key) = keys.get(idx) else {
                log::error!("unable to find key for index: {idx}");
                continue;
            };

            let Some(node) = nodes.get_mut(*key) else {
                log::error!("unable to retrieve matching node for: {idx}");
                continue;
            };

            if let Some(&muted) = steps.muted.get(since_start as usize) {
                node.muted = muted;
            }
            if let Some(&weight) = steps.weight.get(since_start as usize) {
                node.slots[0].weight = weight;
            }
            if let Some(&value) = steps.value.get(since_start as usize) {
                node.slots[0].value = value;
            }
            if let Some(&pos) = steps.position.get(since_start as usize) {
                node.location = pos;
            }
            if let Some(&rad) = steps.radius.get(since_start as usize) {
                node.radius = rad;
            }
            
        }
    }

}

#[derive(Debug)]
pub enum CreateEventError {
    /// must contain atleast one step to execute.
    NotEnoughSteps,
}
