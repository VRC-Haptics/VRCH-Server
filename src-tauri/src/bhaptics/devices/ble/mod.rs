pub mod tactal;
pub mod x40;

use crate::bhaptics::devices::ble::tactal::tactal_ble_device;
use crate::bhaptics::devices::ble::x40::x40_ble_device;
use crate::mapping::haptic_node::HapticNode;

use once_cell::sync::Lazy;

/// All Bhaptics Devices.
pub static BLE_DEVICES: Lazy<Vec<bHapticDevice>> =
    Lazy::new(|| vec![tactal_ble_device(), x40_ble_device()]);

/// List of bhaptics device names from senseshift
pub static DEVICE_NAMES: &'static [&'static str] = &[
    //"Tactal_",
    //"TactSuitX40",
    "TactGlove (R",
    "TactGlove (L",
    "Tactosy2_R",
    "Tactosy2_L",
    "TactosyF_R",
    "TactosyF_L",
    "TactosyH_R",
    "TactosyH_L",
    "TactSuitX16",
];

pub struct bHapticDevice {
    ble_name: String,
    nodes: Vec<HapticNode>,
}
