use std::{collections::HashSet, time::Duration};

use ergot::{
    exports::mutex::raw_impls::cs::CriticalSectionRawMutex,
    interface_manager::{interface_impls::nusb_bulk::NusbBulk, profiles::direct_edge::DirectEdge},
    net_stack::{ArcNetStack, NetStackHandle},
    toolkits::nusb_v0_1::{
        EdgeStack, RouterStack, find_new_devices, find_new_devices_with_filter,
        register_router_interface,
    },
};

#[tokio::main]
async fn main() {
    let stack = RouterStack::new();
    let sn = "F5D911";

    loop {
        let devs = find_new_devices_with_filter(&HashSet::new(), |d| {
            d.serial_number().unwrap_or("").contains(sn)
                && d.product_string()
                    .unwrap_or("")
                    .contains("Stage controller")
        })
        .await;
        for dev in devs {
            println!("Device found: {:?}", dev.info);
            let hdl = register_router_interface(&stack, dev, 64, 64)
                .await
                .unwrap();
            println!("{:?}", stack.manage_profile(|f| f.get_nets()));
        }
        println!("{:?}", stack.manage_profile(|f| f.get_nets()));
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
