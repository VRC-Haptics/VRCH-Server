use enum_dispatch::enum_dispatch;
use super::{DeviceMessage, DeviceId, DeviceInfo};
use tokio::sync::mpsc;


/// The generic interface for physical haptic devices
#[enum_dispatch(HapticDevice)]
pub trait Device {
    /// Returns device id that should be unique to this device
    /// 
    /// Since id is required to index it should be avialable at device initalization
    fn get_id(&self) -> DeviceId;
    /// Returns the info related to this device.
    /// All info should not be required at device start and will be edited as the device lives on.
    fn info(&self) -> DeviceInfo;
    /// set the feedback to these values.
    /// Will be registered and sent at varying rates depending on internal device types.
    /// Note; number of values could be the incorrect length due to race conditions
    async fn set_feedback(&self, values: &[f32]);
    /// Allows this device to interact with the DeviceManager directly.
    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>);
    /// Allows for the hardware device to cleanly sever it's connection if commanded to drop this device.
    ///
    /// If protocol allows for one-sided disconnections feel free to leave this empty.
    ///
    /// Is expected to block until completed.
    fn disconnect(&mut self);
}