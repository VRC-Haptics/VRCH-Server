use dashmap::DashMap;
use enum_dispatch::enum_dispatch;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use std::sync::Arc;

use super::DeviceId;

static DEVICE_MANAGER: OnceCell<DeviceManager> = OnceCell::new();

#[enum_dispatch(Device)]
pub enum HapticDevice {
    Wifi(WifiDevice),
}

/// Info container for each device type
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "variant", content = "value")]
pub enum DeviceInfo {
    Wifi(WifiDeviceInfo),
}

/// The generic interface for physical haptic devices
#[enum_dispatch]
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

/// Commands a HapticDevice can invoke from the DeviceManager
pub enum DeviceMessage {
    Remove(DeviceId),
    /// Marks the device info for this ID as dirty, will update all subscribers.
    InfoDirty(DeviceId),
    Register(HapticDevice),
}

/// Events that will be passed to subscribers.
pub enum DeviceOutEvents {
    /// New device was added to list, most likely info not available.
    NewDevice(DeviceId),
    /// A device with ID has been removed,
    RemovedDevice(DeviceId),
    /// Info for a device has changed
    DeviceInfoDirty(DeviceId),
}

/// Thread safe abstraction layer over physical devices.
///
/// On creation; intializes each unique device listeners,
/// requires calling start_device_manager to actually start listening for devices.
///
/// Can be safely shared between threads with `Arc<DeviceManager>`.
pub struct DeviceManager {
    // Requires Arc to keep fully asynchronus
    devices: Arc<DashMap<DeviceId, HapticDevice>>,
    device_channel: Option<mpsc::Receiver<DeviceMessage>>,
    // doesn't need to be arc because we can just clone it.
    recieve_channel: mpsc::Sender<DeviceMessage>,
    // arc for internal loop stuff
    subscribers: Arc<Mutex<Vec<mpsc::Sender<DeviceOutEvents>>>>,
    shutdown: CancellationToken,
}

impl DeviceManager {
    /// Creates new manager.
    pub fn new() -> DeviceManager {
        let shutdown = CancellationToken::new();
        let (tx, rx) = mpsc::channel(5);

        DeviceManager {
            devices: Arc::new(DashMap::new()),
            device_channel: Some(rx),
            recieve_channel: tx,
            subscribers: Arc::new(Mutex::new(vec![])),
            shutdown: shutdown,
        }
    }

    /// Runs closure `fun` with device `id` as its input
    /// 
    /// To get INFO:
    /// 
    /// ```
    /// let info = manager.with_device("mac address", |d| d.info());
    ///  
    /// ```
    pub fn with_device<T, F>(&self, id: &DeviceId, fun: F) -> Option<T>
    where
        F: FnOnce(&HapticDevice) -> T,
    {
        self.devices.get(id).map(|d| fun(&d))
    }

    /// Sets feedback array for the device.
    pub fn set_feedback(&self, id: &DeviceId, values: &[f32]) {
        if let Some(d) = self.devices.get_mut(id) {
            d.set_feedback(values);
        }
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
    }
}

/// Handles intitializing all device listeners as well as managing device messaging.
pub async fn init_device_manager(manager: &mut DeviceManager) {
    let Some(mut rx) = manager.device_channel.take() else {
        log::error!("Manager init called after already called earlier.");
        return;
    };

    // initialize our device listeners

    // spawn our channel manager
    let clone = manager.shutdown.clone();
    let map = Arc::clone(&manager.devices);
    let subscribers = Arc::clone(&manager.subscribers);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    let Some(event) = msg else { break };

                    handle_device_message(event, &map, &subscribers);
                }

                _ = clone.cancelled() => {
                    break;
                }
            }
        }
    });
}

// outside macro allows for intellisense
fn handle_device_message(event:DeviceMessage, map: &DashMap<DeviceId, HapticDevice>, subscribers: &Mutex<Vec<mpsc::Sender<DeviceOutEvents>>> ) {
    let lock = subscribers.lock();
    
    match event {
        DeviceMessage::Remove(id) => {
            map.remove(&id);
            for sub in lock.iter() {
                sub.try_send(DeviceOutEvents::RemovedDevice(id.clone()));
            }
        },
        DeviceMessage::Register(d) => {
            let id = d.get_id();
            map.insert(id.clone(), d);
            for sub in lock.iter() {
                sub.try_send(DeviceOutEvents::NewDevice(id.clone()));
            }
        },
        DeviceMessage::InfoDirty(id) => {
            for sub in lock.iter() {
                sub.try_send(DeviceOutEvents::DeviceInfoDirty(id.clone()));
            }
        }
    };
}

pub struct WifiDevice {
    this: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WifiDeviceInfo {
    rssi: usize,
}

impl Device for WifiDevice {
    fn get_id(&self) -> DeviceId {
        DeviceId("this".to_string())
    }

    fn info(&self) -> DeviceInfo {
        DeviceInfo::Wifi(WifiDeviceInfo { rssi: 0 })
    }

    fn disconnect(&mut self) {}
    async fn set_feedback(&self, values: &[f32]) {}
    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>) {}
}
