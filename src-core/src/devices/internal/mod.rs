use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc;

use crate::{devices::{Device, DeviceId, DeviceInfo, DeviceMessage}, mapping::haptic_node::HapticNode};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct InternalDeviceInfo {
    pub nodes: Vec<HapticNode>,
}

/// A simple internal device intended for manipulating and evaluating map values.
#[derive(Debug)]
pub struct InternalDevice {
    id: DeviceId,
    info: Arc<Mutex<DeviceInfo>>,
    buffer: Arc<RwLock<Vec<f32>>>,
    manager: Option<mpsc::Sender<DeviceMessage>>,
}

impl InternalDevice {
    /// One feedback slot per node, zero-initialized.
    pub fn new(id: impl Into<DeviceId>, nodes: Vec<HapticNode>) -> Self {
        let buffer = Arc::new(RwLock::new(vec![0.0; nodes.len()]));
        Self {
            id: id.into(),
            info: Arc::new(Mutex::new(DeviceInfo::Internal(InternalDeviceInfo { nodes }))),
            buffer,
            manager: None,
        }
    }
}


impl Device for InternalDevice {
    fn get_id(&self) -> DeviceId { self.id.clone() }
    fn info(&self) -> DeviceInfo { self.info.lock().clone() }
    fn update_info(&self, new: DeviceInfo) { *self.info.lock() = new; }
    fn get_feedback_buffer(&self) -> Arc<RwLock<Vec<f32>>> { Arc::clone(&self.buffer) }
    fn buffer_updated(&self) {}
    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>) {
        self.manager = Some(tx);
    }
    fn disconnect(&mut self) {
        if let Some(tx) = &self.manager {
            let _ = tx.try_send(DeviceMessage::Remove(self.id.clone()));
        }
    }
}