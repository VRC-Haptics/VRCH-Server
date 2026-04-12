mod ble;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::{uuid, Uuid};

use crate::bhaptics::devices::ble::DEVICE_NAMES;
use crate::devices::Device;
