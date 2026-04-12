mod ble;

pub use ble::{send, start_ble};

use btleplug::api::{BDAddr, Characteristic};
use parking_lot::RwLock;
use std::{
    sync::{Arc, LazyLock},
    time::Instant,
};
use tokio::sync::mpsc::Sender;

use crate::{
    bhaptics::maps::x40_vest::{x40_vest_back, x40_vest_front},
    devices::{bhaptics::ble::BleHandle, DeviceId, DeviceInfo, DeviceMessage},
    log_err,
    mapping::{haptic_node::HapticNode, NodeGroup},
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
pub struct BhapticInfo {
    pub id: DeviceId,
    pub nodes: Vec<HapticNode>,
    pub model: BhapticsModel,
}

/// Describes where on teh 40nibble array each buffer index gets put.
/// The device does a max of a few pins and we just need to hit one for each motor.
const X16_NIBBLE_INDICES: [usize; 16] =
    [0, 1, 4, 5, 10, 11, 14, 15, 20, 21, 24, 25, 30, 31, 34, 35];

    /// Describes which nodes to keep from our x40 list
const X16_NODE_INDICES: [usize; 16] = [
    // Front row 0 (top)
    0, 1, 2, 3,
    // Front row 4 (bottom)
    12, 13, 14, 15,
    // Back row 0 (top)
    20, 21, 22, 23,
    // Back row 4 (bottom)
    32, 33, 34, 35,
];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum BhapticsModel {
    TacsuitX16,
}

impl BhapticsModel {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "TactSuitX16" => Some(BhapticsModel::TacsuitX16),
            _ => None,
        }
    }

    /// Number of motors to allocate in the input map
    pub fn motor_num(&self) -> usize {
        match self {
            BhapticsModel::TacsuitX16 => 16,
        }
    }

    // how big the output buffer we transmit over bluetooth is
    pub fn buffer_size(&self) -> usize {
        match self {
            BhapticsModel::TacsuitX16 => 20,
        }
    }

    /// Encode motor feedback into the BLE write buffer.
    /// `feedback` should have exactly `self.buffer_size()` elements, each 0.0..=1.0.
    pub fn encode_feedback(&self, buf: &mut [u8], feedback: &[f32]) {
        if buf.len() < (feedback.len() / 2) {
            log::error!("Output buffer is smaller than feedback length");
            return;
        };
        match self {
            BhapticsModel::TacsuitX16 => {
                if buf.len() < 20 {
                    log::error!("Output buffer must be 20 bytes");
                    return;
                }
                buf[..20].fill(0);
                for (motor, &nibble_idx) in X16_NIBBLE_INDICES.iter().enumerate() {
                    let val = feedback
                        .get(motor)
                        .map(|v| (v.clamp(0.0, 1.0) * 15.0).round() as u8)
                        .unwrap_or(0);
                    let byte_idx = nibble_idx / 2;
                    if nibble_idx % 2 == 0 {
                        buf[byte_idx] |= val << 4;
                    } else {
                        buf[byte_idx] |= val;
                    }
                }
            }
        }
    }

    pub fn nodes(&self) -> &'static [HapticNode] {
        match self {
            BhapticsModel::TacsuitX16 => {
                static NODES: LazyLock<Vec<HapticNode>> = LazyLock::new(|| {
                    let front = x40_vest_front().rows;
                    let back = x40_vest_back().rows;
                    let all: Vec<_> = front.iter().chain(back.iter()).collect();

                    X16_NODE_INDICES
                        .iter()
                        .map(|&i| {
                            let v = all[i];
                            HapticNode {
                                x: v.x,
                                y: v.y,
                                z: v.z,
                                groups: vec![NodeGroup::All],
                            }
                        })
                        .collect()
                });
                &NODES
            }
        }
    }
}

#[derive(Debug)]
pub struct BhapticBle {
    handle: BleHandle,
    buffer: Arc<RwLock<Vec<f32>>>,
    output: Arc<RwLock<Vec<u8>>>,
    last_send: Arc<RwLock<Instant>>,
    map_tx: Sender<DeviceMessage>,
    address: BDAddr,
    id: DeviceId,
    connected_idx: usize,
    model: BhapticsModel,
    motor_char: Arc<Characteristic>,
}

impl BhapticBle {
    pub fn new(
        mdl: BhapticsModel,
        handle: BleHandle,
        map: Sender<DeviceMessage>,
        addr: BDAddr,
        idx: usize,
        char: Characteristic,
    ) -> Self {
        let id = addr.to_string().into();

        BhapticBle {
            handle,
            buffer: Arc::new(RwLock::new(vec![0.0; mdl.motor_num()])),
            output: Arc::new(RwLock::new(vec![0u8; mdl.buffer_size()])),
            map_tx: map,
            address: addr,
            id,
            connected_idx: idx,
            model: mdl,
            motor_char: Arc::new(char),
            last_send: Arc::new(RwLock::new(
                Instant::now() - std::time::Duration::from_millis(100),
            )),
        }
    }
}

impl super::Device for BhapticBle {
    fn get_id(&self) -> super::DeviceId {
        self.id.clone()
    }

    fn info(&self) -> super::DeviceInfo {
        DeviceInfo::BhapticBle(BhapticInfo {
            id: self.id.clone(),
            nodes: self.model.nodes().to_vec(),
            model: self.model.clone(),
        })
    }

    fn update_info(&self, new: super::DeviceInfo) {
        let DeviceInfo::BhapticBle(new) = new else {
            return;
        };

        let BhapticInfo { id, nodes, model } = new;

        return;
    }

    fn get_feedback_buffer(&self) -> Arc<RwLock<Vec<f32>>> {
        Arc::clone(&self.buffer)
    }

    fn buffer_updated(&self) {
        let now = Instant::now();
        {
            let last = self.last_send.read();
            if now.duration_since(*last).as_millis() < 50 {
                return;
            }
        }
        *self.last_send.write() = now;

        let mut out = self.output.write();
        let feedback = self.buffer.read();
        self.model.encode_feedback(&mut out, &feedback);
        self.handle.send(
            self.connected_idx,
            out.clone().into_boxed_slice(),
            Arc::clone(&self.motor_char),
        );
    }

    async fn set_manager_channel(&mut self, tx: Sender<DeviceMessage>) {
        self.map_tx = tx;
    }

    fn disconnect(&mut self) {
        self.handle.disconnect(self.connected_idx);
    }
}
