//! Detects new devices every second and registers them with the net stack.

use std::{collections::HashSet, time::Duration};

use ergot::{
    Address,
    toolkits::nusb_v0_1::{RouterStack, find_new_devices_with_filter, register_router_interface},
};
use log::info;

use crate::{
    address::DeviceAdress,
    communicator::{MTU, OUT_BUFFER_SIZE},
};

const PRODUCT_STRING: &str = "Stage controller"; // version independent

/// This task detects a device filtered for a serial number and registers it with the router stack.
///
/// The serial number must be an exact match. We further filter by product ID, as in above defined
/// `PRODUCT_STRING` value. Note that the product ID must only contain this string as a substring.
///
/// Panics if multiple stage controller devices with the same serial number are found.
pub async fn register_selected_device(stack: RouterStack, serial_number: String) {
    loop {
        let devices = find_new_devices_with_filter(&HashSet::new(), |d| {
            d.serial_number().unwrap_or("").contains(&serial_number)
                && d.product_string().unwrap_or("").contains(PRODUCT_STRING)
        })
        .await;

        for dev in devices {
            println!("Found device {:?}", dev.info); // FIXME:
            info!("Found device {:?}", dev.info);

            // register new device with stack.
            let _hdl = register_router_interface(&stack, dev, MTU, OUT_BUFFER_SIZE)
                .await
                .unwrap();

            // register net address with driver.
            let nets = stack.manage_profile(|p| p.get_nets());
            if nets.len() > 1 {
                panic!(
                    "Multiple devices with the same serial number were found. Please report as bug."
                )
            }

            if let Some(net) = nets.first() {
                DeviceAdress::set(&Address {
                    network_id: *net,
                    node_id: 2,
                    port_id: 0,
                });
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Register a single device with the network stack.
///
/// Panics if more or less than a single stage controller are found.
pub async fn register_single_device(stack: RouterStack) {
    let serial_number = get_all_devices().await;
    if serial_number.len() != 1 {
        panic!(
            "Did not find exactly one stage controller device. Serial numbers found: {:?}",
            serial_number
        );
    }

    let sn = serial_number
        .into_iter()
        .next()
        .expect("exactly one value present");
    let _ = tokio::task::spawn(register_selected_device(stack, sn));
}

/// Get all stage controller devices that are currently connected.
///
/// This uses the above defined `PRODUCT_STRING` to search for compatible stage controllers.
/// Then a HashSet of serial numbers is returned.
///
/// Panics if no serial number is found.
pub async fn get_all_devices() -> HashSet<String> {
    let mut serial_numbers = HashSet::new();

    let devices = find_new_devices_with_filter(&HashSet::new(), |d| {
        d.product_string().unwrap_or("").contains(PRODUCT_STRING)
    })
    .await;

    for dev in devices {
        serial_numbers.insert(
            dev.info
                .usb_serial_number
                .expect("serial number present")
                .into(),
        );
    }

    info!("Found the following serial numbers: {:?}", serial_numbers);

    serial_numbers
}
