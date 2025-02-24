use btleplug::api::{bleuuid::uuid_from_u16, Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::time::Duration;
use uuid::Uuid;
use tokio::time;

pub const BEE_HAPTICS_UUID: str = "6e400001-b5a3-f393-e0a9-e50e24dcca9e";

pub async fn start_bt() -> Result<(), btleplug::Error> {
    let uuid = Uuid::parse_str(&BEE_HAPTICS_UUID);
    let manager = Manager::new().await.unwrap();

    // get the first bluetooth adapter
    let adapters = manager.adapters().await?;
    let central: Adapter = adapters.into_iter().nth(0).unwrap();

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await?;
    // instead of waiting, you can use central.events() to get a stream which will
    // notify you of new devices, for an example of that see examples/event_driven_discovery.rs
    time::sleep(Duration::from_secs(2)).await;


    Ok(())
}

pub async fn filter_bt(central: &Adapter) -> Option<Vec<Peripheral>> {
    let mut haptics = Vec::new();
    for periph in central.peripherals().await.unwrap() {
        if periph.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|name| name.contains("LEDBlue"))
        {
            return Some(haptics);
        }
        //periph.await;
        //if let Some(props) = periph {
        //    // If properties include services, check if BEE_HAPTICS_UUID is present.
        //    if let Some(services) = props.services {
        //        if services.contains(&BEE_HAPTICS_UUID) {
        //            haptics.push(periph);
        //        }
        //    }
        //}
    }

    if haptics.len() > 0 {
        return Some(haptics);
    } else {
        return None;
    }
}