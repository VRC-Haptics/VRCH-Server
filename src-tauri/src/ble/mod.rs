use btleplug::api::{BDAddr, Characteristic, PeripheralProperties};
use btleplug::api::{
    Central, Manager as _, Peripheral as _, ScanFilter, WriteType
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::collections::BTreeSet;
use std::sync::{LazyLock, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{OnceCell, RwLock, mpsc};
use tokio::time;
use uuid::{Uuid, uuid};
use dashmap::DashMap;

static IS_SCANNING: AtomicBool = AtomicBool::new(false);
static BLE_ADAPTER: RwLock<Option<Arc<Adapter>>> = RwLock::const_new(None);
static BLE_MANAGER: OnceCell<Manager> = OnceCell::const_new();
static AVAILABLE_DEVICES: LazyLock<DashMap<BDAddr, Peripheral>> = LazyLock::new(||{DashMap::new()});
static CONNECTED_DEVICES: LazyLock<DashMap<BDAddr, Peripheral>> = LazyLock::new(||{DashMap::new()});

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
    pub fn is_present<'a>(&self, chars: &'a BTreeSet<Characteristic>) -> Option<&'a Characteristic> {
        chars.iter()
            .find(|c| &c.uuid == &self.to_uuid())
    }
}

pub async fn send(address: &BDAddr, data: &[u8], char: &Characteristic) -> Result<(), BleError> {
    if let Some(d) = CONNECTED_DEVICES.get(address) {
        d.write(char, data, WriteType::WithoutResponse).await?;
        return Ok(());
    }
    Err(BleError::DeviceNotAvailable(*address))
}

pub async fn connect(address: &BDAddr) -> Result<BTreeSet<Characteristic>, BleError> {
    let device = AVAILABLE_DEVICES.get(&address);
    if let Some(d) = device {
        // Generic BleError
        d.connect().await?;
        d.discover_services().await?;
        let chars = d.characteristics();
        CONNECTED_DEVICES.insert(*address, d.clone());
        return Ok(chars);
    }

    // Should be device not found
    Err(BleError::DeviceNotAvailable(address.clone()))
}

/// Should only be called once. Initializes all internal values.
/// 
/// Only devices with a resolved name will be returned.
/// The Sender will send out an error before closing. Consider an error to be the last message.
/// All peripherials are garunteed to have a name and address.
pub async fn start_ble(
    collection_interval: Duration,
) -> Result<mpsc::Receiver<Result<Vec<PeripheralProperties>, BleError>>, BleError> {
    choose_new_adapter().await?;
    start_ble_scan(None).await?;

    let (tx, rx) = mpsc::channel(16);

    // spawn task that feeds devices back through channel.
    tokio::task::spawn(async move {
        // I hate how nested this is but it'll do for now.
        loop {
            match get_adapter().await {
                Ok(adapter) => {
                    match adapter.peripherals().await {
                        Ok(peripherals) => {
                            // collect all peripherials that have names
                            let mut current = Vec::new();
                            for p in peripherals {
                                let addr = p.address();
                                if CONNECTED_DEVICES.contains_key(&addr) {
                                    println!("Found connected device: {:?}", addr);
                                    continue; // don't overwrite connected device
                                }
                                if let Some(prop) = try_get_properties(&p).await {
                                    if get_ident(&prop).await.is_some() {
                                        current.push(prop);
                                    }
                                }
                                AVAILABLE_DEVICES.insert(p.address(), p);
                            }

                            // send detected peripherials
                            if tx.send(Ok(current)).await.is_err() {
                                break; // receiver dropped
                            }
                            tokio::time::sleep(collection_interval).await;
                        }
                        Err(e) => {
                            let _ = tx.send(Err(BleError::BtlePlug(e))).await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            };
        }
    });

    Ok(rx)
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
        },
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

    let first = adapters.into_iter().next().expect("already checked non-empty");
    
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
                adapter.stop_scan().await;
                IS_SCANNING.store(false, Ordering::SeqCst);
            });
        }

        return Ok(());
    }

    Err(BleError::AdapterUnavailable)
}

#[derive(Debug)]
pub enum BleError {
    /// BLE is not available on this machine
    BleNotAvailable,
    /// When a device is not avialable but an action is trying to be used
    DeviceNotAvailable(BDAddr),
    /// An adapter has not been chosen to start scanning or activate.
    AdapterNotSet,
    /// An error was encountered when intializing the BLE manager
    ManagerCreation(btleplug::Error),
    /// The chosen adapter for operation is no longer valid.
    AdapterUnavailable,
    /// A scan has already been started.
    AlreadyScanning,
    /// Generic BtlePlug Error, requires additional description in the function header when used
    ///
    /// Typically signals; "This operation failed unrecoverably"
    BtlePlug(btleplug::Error),
}

impl From<btleplug::Error> for BleError {
    fn from(e: btleplug::Error) -> Self {
        BleError::BtlePlug(e)
    }
}
