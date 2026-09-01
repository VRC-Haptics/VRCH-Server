use std::time::{Duration, Instant};
use arraydeque::{ArrayDeque, Wrapping};
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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(into = "HistoryView")]
pub struct History {
    pub samples: ArrayDeque<(f32, Instant), WINDOW, Wrapping>,
}

impl PartialEq for History {
    fn eq(&self, other: &Self) -> bool {
        self.samples.len() == other.samples.len()
            && self.samples.iter()
                .zip(other.samples.iter())
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

/// Largest gap between two samples still treated as continuous motion.
/// Anything wider is a dropped packet / new gesture and is not bridged.
const MAX_SAMPLE_GAP: Duration = Duration::from_millis(250);
/// How long after the newest sample velocity is still reported, fading to 0.
const VELOCITY_HOLD: Duration = Duration::from_millis(150);
const MIN_DT: f32 = 1e-4;

impl History {
    /// Average finite-difference velocity (units/second) over the recent window.
    /// Take absolute value to get speed.
    pub fn velocity_at(&self, now: Instant) -> f32 {
        // front == newest (push_front)
        let Some((_, newest_time)) = self.samples.front() else {
            return 0.0;
        };

        // No fresh input: fade out rather than holding a stale reading.
        let staleness = now.saturating_duration_since(*newest_time);
        if staleness >= VELOCITY_HOLD {
            return 0.0;
        }
        let decay = 1.0 - (staleness.as_secs_f32() / VELOCITY_HOLD.as_secs_f32());

        let mut num = 0.0f32;
        let mut denom = 0.0f32;
        // iteration is newest-first, so `previous` is the *newer* half of each pair
        let mut previous: Option<(f32, Instant)> = None;
        for (value, time) in self.samples.iter().copied() {
            if let Some((newer_val, newer_time)) = previous {
                let gap = newer_time.saturating_duration_since(time);
                if gap > MAX_SAMPLE_GAP {
                    break;
                }
                let dt = gap.as_secs_f32();
                if dt >= MIN_DT {
                    num += (newer_val - value) / dt;
                    denom += 1.0;
                }
            }
            previous = Some((value, time));
        }

        if denom > 0.0 {
            (num / denom) * decay
        } else {
            0.0
        }
    }


    pub fn view(&self) -> HistoryView {
        let mut values = [0.0f32; WINDOW];
        for (i, (v, _)) in self.samples.iter().enumerate() {
            values[i] = *v;
        }
        HistoryView {
            values,
            len: self.samples.len(),
        }
    }

    pub fn new() -> Self {
        Self {
            samples: ArrayDeque::new()
        }
    }

    pub fn push_at(&mut self, value: f32, time: Instant) {
        let _ = self.samples.push_front((value, time));
    }

    pub fn latest(&self) -> Option<f32> {
        self.samples.front().map(|v| v.0)
    }

    /// Valid samples, newest-first. Each entry is (value, time).
    pub fn history(&self) -> impl DoubleEndedIterator<Item = &(f32, Instant)> + ExactSizeIterator + '_ {
        // newest were inserted via push_front, so front-to-back == newest-first
        self.samples.iter()
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
