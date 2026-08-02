use std::time::{Duration, Instant};
use glam::Vec3;
use smallvec::SmallVec;

use crate::mapping::NodeKey;
use crate::mapping::groups::NodeGroup;
use crate::vrc::config::{InputLayer, InputType};

const DEFAULT_NODE_SLOTS: usize = 2;

/// Points to an input slot, within a node, within an interpolation layer.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SlotKey {
    pub node: NodeKey,
    pub slot_idx: u8,
}

impl SlotKey {
    pub fn new(node: NodeKey, idx: u8) -> Self {
        SlotKey { node, slot_idx: idx }
    }
}

const WINDOW: usize = 5;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub struct Slot {
    pub muted: bool,
    pub source: InputType,
    pub layer: InputLayer,
    pub weight: f32,
    #[serde(skip)]
    pub history: History,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, Debug, Clone, Copy, Default)]
pub struct HistoryView {
    pub values: [f32; WINDOW],
    pub len: usize,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(into = "HistoryView")]
pub struct History {
    pub stage: Stage,
    pub samples: [(f32, Instant); WINDOW],
    pub len: usize,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
enum Stage {
    Empty,
    S0,
    S1,
    S2,
    S3,
    S4,
}

impl Stage {
    pub const fn next(self) -> Self {
        match self {
            Self::Empty => Self::S0,
            Self::S0 => Self::S1,
            Self::S1 => Self::S2,
            Self::S2 => Self::S3,
            Self::S3 => Self::S4,
            Self::S4 => Self::S0,
        }
    }

    pub const fn index(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::S0 => 0,
            Self::S1 => 1,
            Self::S2 => 2,
            Self::S3 => 3,
            Self::S4 => 4,
        }
    }
}

impl PartialEq for History {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && self.samples[..self.len]
                .iter()
                .zip(&other.samples[..other.len])
                .all(|(a, b)| a.0 == b.0)
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl From<History> for HistoryView {
    fn from(h: History) -> Self {
        h.view()
    }
}

/// 10hz is the OSC update rate
const VELOCITY_TIMEOUT: Duration = Duration::from_millis(11);

impl History {
    /// Average velocity from `self.samples[old_idx]` to `now`, treating the
    /// signal as held constant since the newest sample.
    fn velocity_between(&self, now: Instant, old_idx: usize) -> f32 {
        if self.len < 2 {
            return 0.0;
        }
        let old_idx = old_idx.clamp(1, self.len - 1);
        let new = self.samples[0];
        let old = self.samples[old_idx];

        let stale = now.saturating_duration_since(new.1);
        if stale >= VELOCITY_TIMEOUT {
            return 0.0;
        }

        // Denominator spans old sample -> now, not old -> new. This is what
        // makes a stalled input decay instead of latching.
        let elapsed = now.saturating_duration_since(old.1).as_secs_f32();
        if elapsed <= 0.0 {
            return 0.0;
        }

        // Linear taper so the value reaches 0 continuously at the timeout
        // rather than stepping off a cliff.
        let fade = 1.0 - stale.as_secs_f32() / VELOCITY_TIMEOUT.as_secs_f32();
        ((new.0 - old.0) / elapsed) * fade
    }

    /// gives absolute speed, at this instant. Applies smoothing and slow data updates.
    pub fn speed_windowed_at(&self, now: Instant) -> f32 {
        self.velocity_between(now, self.len.saturating_sub(1)).abs()
    }

    pub fn view(&self) -> HistoryView {
        let mut values = [0.0f32; WINDOW];
        for i in 0..self.len {
            values[i] = self.samples[i].0;
        }
        HistoryView {
            values,
            len: self.len,
        }
    }

    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            stage: Stage::Empty,
            samples: [(0.0, now); WINDOW], // never reads defaults because of stages
            len: 0,
        }
    }

    pub fn push_at(&mut self, value: f32, time: Instant) {
        self.stage = self.stage.next();
        self.samples[self.stage.index() as usize] = (value, time);
        self.len = (self.len + 1).min(WINDOW);
    }

    pub fn latest(&self) -> Option<f32> {
        if self.stage == Stage::Empty {
            None
        } else {
            Some(self.samples[self.stage.index() as usize].0)
        }
    }

    /// Valid samples, newest-first. Each entry is (value, time).
    pub fn history(&self) -> &[(f32, Instant)] {
        &self.samples[..self.len]
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
/// Represents information at a point in space that will go into computing outputs.
/// 
/// To have a Greedy layer drive multiple devices it must have the ALL group tag present. 
pub struct InputNode {
    pub muted: bool,
    pub location: Vec3,
    /// A slot is an single input value.
    #[cfg_attr(feature = "specta", specta(type = Vec<Slot>))]
    pub slots: SmallVec<[Slot; DEFAULT_NODE_SLOTS]>,
    pub radius: f32,
    /// purely for convenience, should be determined by the map.
    pub interpolation_layer: InterpolationLayer,
    pub groups: NodeGroup,
    /// The most recently calculated output value of this node.
    pub value: f32,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
/// Describes how this node should affect the layers. 
pub enum InterpolationLayer {
    /// Grabs any nodes within it's radius,forcefully drives it at this value.
    Greedy,
    /// Exponentially weights nodes within it's radius, nodes at 0 pull to zero.
    #[default]
    Default,
    /// Weights nodes according to their proximity to this node
    Linear,
    /// Weights all outputs at  in its radius.
    Area,
    /// Drives **all** nodes exactly at output without regard to distance.
    Global,
}

impl InputNode {
    /// Factory for creating InputNode's
    pub fn new(
        location: Vec3,
        groups: NodeGroup,
        layer: InterpolationLayer,
        slots: SmallVec<[Slot; DEFAULT_NODE_SLOTS]>,
        radius: f32,
    ) -> InputNode {
        return InputNode {
            muted: false,
            location,
            slots,
            radius,
            interpolation_layer: layer,
            groups,
            value: 0.0,
        };
    }

    pub const fn always_apply(&self) -> bool {
        self.groups.intersects(NodeGroup::All)
    }

    pub fn set_position(&mut self, pos: Vec3) {
        self.location.x = pos.x;
        self.location.y = pos.y;
        self.location.z = pos.z;
    }

    pub fn mute(&mut self, val: bool) {
        self.muted = val;
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }
}
