use btleplug::api::{BDAddr, Characteristic, PeripheralProperties};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use parking_lot::Mutex;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::{
    mpsc::{self, Sender},
    OnceCell, RwLock,
};
use tokio::time;
use uuid::{uuid, Uuid};

use crate::devices::bhaptics::{BhapticBle, BhapticsModel};
use crate::devices::{DeviceMessage, HapticDevice};
use crate::log_err;

static IS_SCANNING: AtomicBool = AtomicBool::new(false);
static BLE_ADAPTER: RwLock<Option<Arc<Adapter>>> = RwLock::const_new(None);
static BLE_MANAGER: OnceCell<Manager> = OnceCell::const_new();
pub(crate) static CONNECTED_DEVICES: LazyLock<boxcar::Vec<Mutex<Option<Arc<Peripheral>>>>> =
    LazyLock::new(|| boxcar::Vec::new());

/// If this fails to initialize the manager and another instance is called it will try to initailize every time.
/// Be careful of repeated failed calling across tasks.
///
/// Errors:
///  - `ManagerCreation`
async fn ble_manager() -> Result<&'static Manager, BleError> {
    BLE_MANAGER
        .get_or_try_init(|| async { Manager::new().await.map_err(|e| BleError::from(e)) })
        .await
}

pub async fn disconnect(idx: usize) {
    if let Some(dev) = CONNECTED_DEVICES.get(idx) {
        let peripheral = dev.lock().clone();
        if let Some(p) = peripheral {
            log_err!(p.disconnect().await);
        }
        *dev.lock() = None;
    }
}

pub async fn send(idx: usize, data: &[u8], char: &Characteristic) -> Result<(), BleError> {
    let peripheral = CONNECTED_DEVICES
        .get(idx)
        .and_then(|d| d.lock().clone())
        .ok_or(BleError::DeviceNotAvailable)?;
    peripheral.write(char, data, WriteType::WithoutResponse).await?;
    Ok(())
}

async fn connect(per: &Peripheral) -> Result<BTreeSet<Characteristic>, BleError> {
    per.connect().await?;
    per.discover_services().await?;
    Ok(per.characteristics())
}

#[derive(Debug, Clone)]
pub struct BleHandle {
    channel: Sender<HandleMsg>,
}

impl BleHandle {
    pub fn disconnect(&self, idx: usize) {
        log_err!(self.channel.try_send(HandleMsg::Disconnect(idx)));
    }

    pub fn send(&self, idx: usize, data: Box<[u8]>, char: Arc<Characteristic>) {
        log_err!(self.channel.try_send(HandleMsg::Send(data, idx, char)));
    }
}

pub enum HandleMsg {
    Disconnect(usize),
    Send(Box<[u8]>, usize, Arc<Characteristic>),
}

/// Should only be called once. Initializes all internal values.
pub async fn start_ble(
    sender: Sender<DeviceMessage>,
    collection_interval: Duration,
) -> Result<BleHandle, BleError> {
    choose_new_adapter().await?;
    start_ble_scan(None).await?;

    let local_dev = sender.clone();
    let (tx, rx) = mpsc::channel::<HandleMsg>(10);
    let handle = BleHandle { channel: tx };

    // spawn task that feeds devices back through channel.
    let loop_handle = handle.clone();
    tokio::task::spawn(async move {
        // I hate how nested this is but it'll do for now.
        let mut rx = rx;
        loop {
            tokio::select! {
                        Some(msg) = rx.recv() => {
                            match msg {
                                HandleMsg::Disconnect(idx) => {
                                    let addr = CONNECTED_DEVICES
                                        .get(idx)
                                        .and_then(|slot| slot.lock().as_ref().map(|p| p.address()));
                                    if let Some(addr) = addr {
                                        log_err!(local_dev.send(DeviceMessage::Remove(addr.to_string().into())).await);
                                    }

                                    disconnect(idx).await;
                                }
                                HandleMsg::Send(data, idx, char) => {
                                    let e = send(idx, &data, &char).await;
                                    match e {
                                        Err(BleError::DeviceNotAvailable) | Err(BleError::BtlePlug(_)) => {
                                            let addr = CONNECTED_DEVICES
                                                .get(idx)
                                                .and_then(|slot| slot.lock().as_ref().map(|p| p.address()));
                                            if let Some(addr) = addr {
                                                log_err!(local_dev.send(DeviceMessage::Remove(addr.to_string().into())).await);
                                            }
                                            disconnect(idx).await;
                                        },
                                        _ => log_err!(e)
                                    }
                                }
                            }
                        }
                        _ = tokio::time::sleep(collection_interval) => {
                            match get_adapter().await {
                                Ok(adapter) => {
                                    match adapter.peripherals().await {
                                        Ok(peripherals) => {
                                            // connect to bhaptics peripherials.
                                            for p in peripherals {
                                                let addr = p.address();
                                                let already_connected = {
                                                    CONNECTED_DEVICES.iter().any(|(_, slot)| {
                                                        slot.lock().as_ref().map_or(false, |per| per.address() == addr)
                                                    })
                                                };
                                                if already_connected {
                                                    log::trace!("ALready filled slot");
                                                    log::trace!("{:?}", CONNECTED_DEVICES);
                                                    continue;
                                                }
                                                if let Some(prop) = try_get_properties(&p).await {
                                                    let ident = get_ident(&prop).await;
                                                    if let Some((name, addr)) = ident {
                                                        if let Some(mdl)  = BhapticsModel::from_name(name.as_str()) {
                                                            match connect(&p).await {
                                                                Ok(chars) => {
                                                                    let idx = CONNECTED_DEVICES.push(Some(p.into()).into());
                                                                    if let Some(char) = DeviceChar::BhapticsStableMotor.is_present(&chars) {
                                                                        log::trace!("Found BLE haptic device: {:?}", addr);
                                                                        let dev = BhapticBle::new(mdl, loop_handle.clone(), sender.clone(), *addr, idx, char.clone());
                                                                        log_err!(local_dev.send(DeviceMessage::Register(HapticDevice::BhapticBle(dev))).await);
                                                                    }
                                                                },
                                                                Err(e) => {
                                                                    log::error!("BLE connecting error: {e:?}");
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Bhaptics BLE found error: {e:?}");
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Bhaptics BLE found error: {e:?}");
                                    break;
                                }
                            };
                        }
                    }
        }
    });

    Ok(handle)
}

/// Starts a bluetooth scan and chooses an adapter if not already set.
/// Optional timeout will stop scanning after a certain duration.
/// Note: Will return immediatly regardless of duration.
///
/// Errors:
///     BtlePlug
///          - Error trying to start scan on adapter
///     AdapterUnavailable
async fn start_ble_scan(timeout: Option<Duration>) -> Result<(), BleError> {
    let scanning = IS_SCANNING.load(Ordering::SeqCst);
    if scanning {
        return Err(BleError::AlreadyScanning);
    }
    IS_SCANNING.store(true, Ordering::SeqCst);

    if let Ok(adapter) = get_adapter().await {
        adapter.start_scan(ScanFilter::default()).await?;

        if let Some(timeout) = timeout {
            tokio::task::spawn(async move {
                time::sleep(timeout).await;
                log_err!(adapter.stop_scan().await);
                IS_SCANNING.store(false, Ordering::SeqCst);
            });
        }

        return Ok(());
    }

    Err(BleError::AdapterUnavailable)
}

/// Chooses a working adapter and sets it as active.
///
/// Errors:
///  - `BleNotAvailable`
///  - `BtlePlug`; Manager trying to list adapters
async fn choose_new_adapter() -> Result<(), BleError> {
    let manager = ble_manager().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        return Err(BleError::BleNotAvailable);
    }

    let first = adapters
        .into_iter()
        .next()
        .expect("already checked non-empty");

    *BLE_ADAPTER.write().await = Some(Arc::new(first));

    Ok(())
}

/// Returns the active BLE adapter that is used for communication.
///
/// Errors:
///  - AdapterNotSet
async fn get_adapter() -> Result<Arc<Adapter>, BleError> {
    BLE_ADAPTER
        .read()
        .await
        .clone()
        .ok_or(BleError::AdapterNotSet)
}

pub enum DeviceChar {
    /// The stable characteristic for commanding bhaptics motors
    BhapticsStableMotor,
    Custom(Uuid),
}

impl PartialEq for DeviceChar {
    fn eq(&self, other: &Self) -> bool {
        self.to_uuid() == other.to_uuid()
    }
}

impl DeviceChar {
    /// Gives the UUID associated with this Characteristic
    fn to_uuid(&self) -> Uuid {
        match *self {
            DeviceChar::BhapticsStableMotor => uuid!("6e40000a-b5a3-f393-e0a9-e50e24dcca9e"),
            DeviceChar::Custom(u) => u,
        }
    }

    /// Determines whether this variant is present inside a set of characteristics
    pub fn is_present<'a>(
        &self,
        chars: &'a BTreeSet<Characteristic>,
    ) -> Option<&'a Characteristic> {
        chars.iter().find(|c| &c.uuid == &self.to_uuid())
    }
}

#[derive(Debug)]
pub enum BleError {
    /// BLE is not available on this machine
    BleNotAvailable,
    /// When a device is not avialable but an action is trying to be used
    DeviceNotAvailable,
    /// An adapter has not been chosen to start scanning or activate.
    AdapterNotSet,
    /// An error was encountered when intializing the BLE manager
    ManagerCreation(btleplug::Error),
    /// The chosen adapter for operation is no longer valid.
    AdapterUnavailable,
    /// A scan has already been started.
    AlreadyScanning,
    /// Generic BtlePlug Error
    ///
    /// Typically signals; "This operation failed unrecoverably"
    BtlePlug(btleplug::Error),
}

impl From<btleplug::Error> for BleError {
    fn from(e: btleplug::Error) -> Self {
        BleError::BtlePlug(e)
    }
}

/// Get's both the peripherials name and address.
async fn try_get_properties(device: &Peripheral) -> Option<PeripheralProperties> {
    let res = device.properties().await;
    match res {
        Ok(props) => {
            if let Some(p) = props {
                if p.local_name.is_some() {
                    Some(p)
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(e) => {
            println!("encountered error resolving name: {}", e);
            None
        }
    }
}

async fn get_ident<'a>(prop: &'a PeripheralProperties) -> Option<(&'a String, &'a BDAddr)> {
    if let Some(name) = &prop.local_name {
        Some((name, &prop.address))
    } else {
        None
    }
}
