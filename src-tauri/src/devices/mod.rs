//mod ble;
pub mod serial;
//mod traits;
mod bhaptics;
//pub mod update;
pub mod wifi;
//pub mod device;

use dashmap::DashMap;
use enum_dispatch::enum_dispatch;
use parking_lot::{Mutex, RwLock};
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use wifi::{WifiDevice, WifiDeviceInfo};

use crate::{
    devices::wifi::start_wifi_devices,
    mapping::{
        haptic_node::HapticNode,
    },
};

pub type EditCallback<T> = dyn FnOnce(&HapticDevice) -> T;

#[enum_dispatch]
/// All Haptic Devices implement the `Device` trait
/// and are not garunteed to provide anything else.
///
/// Individual exposed functions for each device type are prone to change,
/// and are not stable in the least.
pub enum HapticDevice {
    Wifi(WifiDevice),
}

/// Info container for each device type
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "variant", content = "value")]
pub enum DeviceInfo {
    Wifi(WifiDeviceInfo),
}

impl DeviceInfo {
    pub fn get_nodes(&self) -> &Vec<HapticNode> {
        match self {
            DeviceInfo::Wifi(inf) => {
                return &inf.nodes;
            }
        }
    }

    pub fn set_nodes(&mut self, new: Vec<HapticNode>) {
        match self {
            DeviceInfo::Wifi(ref mut inf) => {
                inf.nodes = new;
            }
        }
    }
}

/// The generic interface for physical haptic devices
#[enum_dispatch(HapticDevice)]
pub trait Device {
    /// Returns device id that should be unique to this device
    ///
    /// Since id is required to index it should be available at device initalization
    fn get_id(&self) -> DeviceId;
    /// Returns the info related to this device.
    /// All info should not be required at device start and will be edited as the device lives on.
    fn info(&self) -> DeviceInfo;
    fn update_info(&self, new: DeviceInfo);
    /// Retrieves the feedback buffer that can be written to to update feedback.
    ///
    /// IMPORTANT: Not garunteed to be pushed to device until
    fn get_feedback_buffer(&self) -> Arc<RwLock<Vec<f32>>>;
    /// Forces device to treat buffer like it has new data inside.
    fn buffer_updated(&self);
    /// Allows this device to interact with the DeviceManager directly.
    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>);
    /// Initiates this devices shutdown process, this should include sending a remove request over the socket.
    fn disconnect(&mut self);
}

/// Commands a HapticDevice can invoke from the DeviceManager
enum DeviceMessage {
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

/// can be freely cloned, provides cheap access to individual devices.
pub struct DeviceHandle {
    devices: Arc<DashMap<DeviceId, HapticDevice>>,
    subscribers: Arc<Mutex<Vec<mpsc::Sender<DeviceOutEvents>>>>,
    device_sender: mpsc::Sender<DeviceMessage>,
}

impl Clone for DeviceHandle {
    fn clone(&self) -> Self {
        Self {
            devices: Arc::clone(&self.devices),
            subscribers: Arc::clone(&self.subscribers),
            device_sender: self.device_sender.clone(),
        }
    }
}

impl DeviceHandle {

    /// checks if a device is still here.
    pub fn exists(&self, id: &DeviceId) -> bool {
        self.devices.contains_key(id)
    }

    pub fn get_device_channel(&self) -> mpsc::Sender<DeviceMessage> {
        self.device_sender.clone()
    }

    /// Registers a callback for when a device event happens, like connecting or disconnecting.
    pub fn register(&self, tx: mpsc::Sender<DeviceOutEvents>) {
        let mut sub = self.subscribers.lock();
        sub.push(tx);
    }

    /// gathers all devices in the map
    pub fn devices(&self) -> Vec<DeviceId> {
        self.devices
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
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

    pub fn with_device_mut<T, F>(&self, id: &DeviceId, fun: F) -> Option<T>
    where
        F: FnOnce(&mut HapticDevice) -> T,
    {
        self.devices.get_mut(id).map(|mut d| fun(&mut d))
    }
}

/// A thin, thread safe abstraction layer over physical devices,
/// AFTER the `init_device_manager` has been called.
///
/// # USE initialiaztion function at top of main.
pub struct DeviceManager {
    // Requires Arc to keep fully asynchronus
    devices: Arc<DashMap<DeviceId, HapticDevice>>,
    device_receiver: Option<mpsc::Receiver<DeviceMessage>>,
    // doesn't need to be arc because we can just clone it.
    device_sender: mpsc::Sender<DeviceMessage>,
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
            device_receiver: Some(rx),
            device_sender: tx,
            subscribers: Arc::new(Mutex::new(vec![])),
            shutdown: shutdown,
        }
    }

    pub fn get_handle(&self) -> DeviceHandle {
        DeviceHandle { devices: Arc::clone(&self.devices), subscribers: Arc::clone(&self.subscribers), device_sender: self.device_sender.clone() }
    }

    pub fn register(&self, tx: mpsc::Sender<DeviceOutEvents>) {
        let mut sub = self.subscribers.lock();
        sub.push(tx);
    }

    pub fn get_device_channel(&self) -> mpsc::Sender<DeviceMessage> {
        self.device_sender.clone()
    }

    /// Runs closure `fun` with device `id` as its input
    /// TODO: Isolate per-device commands to only be from handlers.
    /// To get INFO:
    ///
    /// ```
    /// let info = manager.with_device("mac address", |d| d.info());
    ///  
    /// ```
    pub fn with_device<T, F>(&self, id: &DeviceId, fun: F) -> Option<T>
    where
        F: Fn(&HapticDevice) -> T,
    {
        self.devices.get(id).map(|d| fun(&d))
    }

    pub fn with_device_mut<T, F>(&self, id: &DeviceId, fun: F) -> Option<T>
    where
        F: Fn(&mut HapticDevice) -> T,
    {
        self.devices.get_mut(id).map(|mut d| fun(&mut d))
    }

    /// gathers all devices in the map
    pub fn devices(&self) -> Vec<DeviceId> {
        self.devices
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// checks if a device is still here.
    pub fn exists(&self, id: &DeviceId) -> bool {
        self.devices.contains_key(id)
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        self.devices.iter_mut().map(|mut pair| {
            let this = pair.value_mut();
            this.disconnect()
        });
    }
}

/// Handles intitializing all device listeners as well as managing device messaging.
pub async fn init_device_manager(manager: &mut DeviceManager) {
    let Some(mut rx) = manager.device_receiver.take() else {
        log::error!("Manager init called after already called earlier.");
        return;
    };

    // initialize our device listeners
    start_wifi_devices(&mut manager.get_handle());

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
fn handle_device_message(
    event: DeviceMessage,
    map: &DashMap<DeviceId, HapticDevice>,
    subscribers: &Mutex<Vec<mpsc::Sender<DeviceOutEvents>>>,
) {
    let lock = subscribers.lock();

    match event {
        DeviceMessage::Remove(id) => {
            log::trace!("removing wifi device: {:?}", id);
            map.remove(&id);
            for sub in lock.iter() {
                sub.try_send(DeviceOutEvents::RemovedDevice(id.clone()));
            }
        }
        DeviceMessage::Register(d) => {
            let id = d.get_id();
            map.insert(id.clone(), d);
            for sub in lock.iter() {
                sub.try_send(DeviceOutEvents::NewDevice(id.clone()));
            }
        }
        DeviceMessage::InfoDirty(id) => {
            for sub in lock.iter() {
                sub.try_send(DeviceOutEvents::DeviceInfoDirty(id.clone()));
            }
        }
    };
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DeviceId(pub String);

impl Deref for DeviceId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for DeviceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for DeviceId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// The firmware type returned from the device.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum ESP32Model {
    /// All original ESP32 variants
    ESP32,
    /// Standard ESP32-S2
    ESP32S2,
    /// ESP32-S2 with 16MB flash
    ESP32S2FH16,
    /// ESP32-S2 with 32MB flash  
    ESP32S2FH32,
    ESP32S3,
    ESP32C3,
    ESP32C2,
    ESP32C6,
    ESP8266,
    Unknown,
}

impl ESP32Model {
    pub fn ota_auth_port(&self) -> u16 {
        match *self {
            ESP32Model::ESP32
            | ESP32Model::ESP32S2
            | ESP32Model::ESP32C2
            | ESP32Model::ESP32C3
            | ESP32Model::ESP32C6
            | ESP32Model::ESP32S2FH16
            | ESP32Model::ESP32S2FH32
            | ESP32Model::ESP32S3 => return 3232,
            ESP32Model::ESP8266 => return 8266,
            ESP32Model::Unknown => return 3232,
        }
    }
}

impl ESP32Model {
    /// Parse platform string from device (e.g., "PLATFORM ESP32-D0WDQ6")
    pub fn from_platform_string(platform: &str) -> Self {
        let model = platform
            .strip_prefix("PLATFORM ")
            .unwrap_or(platform)
            .trim();

        Self::from_model_string(model)
    }

    /// Parse raw model string (e.g., "ESP32-D0WDQ6")
    pub fn from_model_string(model: &str) -> Self {
        match model {
            // ESP8266
            "ESP8266" => Self::ESP8266,

            // ESP32 variants (all map to ESP32)
            s if s.starts_with("ESP32-D0WDQ6") => Self::ESP32,
            s if s.starts_with("ESP32-D0WD") => Self::ESP32,
            "ESP32-D2WD" => Self::ESP32,
            "ESP32-PICO-D2" => Self::ESP32,
            "ESP32-PICO-D4" => Self::ESP32,
            "ESP32-PICO-V3-02" => Self::ESP32,
            "ESP32-D0WDR2-V3" => Self::ESP32,

            // ESP32-S2 variants
            "ESP32-S2" => Self::ESP32S2,
            "ESP32-S2FH16" => Self::ESP32S2FH16,
            "ESP32-S2FH32" => Self::ESP32S2FH32,
            s if s.starts_with("ESP32-S2") => Self::ESP32S2, // Fallback for "ESP32-S2 (Unknown)"

            // Other models
            "ESP32-S3" => Self::ESP32S3,
            "ESP32-C3" => Self::ESP32C3,
            "ESP32-C2" => Self::ESP32C2,
            "ESP32-C6" => Self::ESP32C6,

            _ => Self::Unknown,
        }
    }

    /// Get display name for the model
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ESP32 => "ESP32",
            Self::ESP32S2 => "ESP32-S2",
            Self::ESP32S2FH16 => "ESP32-S2 (16MB)",
            Self::ESP32S2FH32 => "ESP32-S2 (32MB)",
            Self::ESP32S3 => "ESP32-S3",
            Self::ESP32C3 => "ESP32-C3",
            Self::ESP32C2 => "ESP32-C2",
            Self::ESP32C6 => "ESP32-C6",
            Self::ESP8266 => "ESP8266",
            Self::Unknown => "Unknown",
        }
    }
}

impl serde::Serialize for ESP32Model {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.display_name())
    }
}
