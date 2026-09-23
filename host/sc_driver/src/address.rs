//! Module to deal with connected device address.
//!
//! We know that the connected device hangs on the netstack, so it's network ID can never be zero.

use std::sync::atomic::{AtomicU32, Ordering};

use ergot::Address;

static ADDRESS_U32: AtomicU32 = AtomicU32::new(0);

/// My access to the atomically stored device address.
pub struct DeviceAdress {}

impl DeviceAdress {
    /// Get an address, if it is available, otherwise return "None".
    pub fn get() -> Option<Address> {
        let addr = ADDRESS_U32.load(Ordering::Relaxed);

        if addr == 0 {
            None
        } else {
            Some(Address {
                network_id: (addr >> 16) as u16,
                node_id: (addr >> 8) as u8,
                port_id: addr as u8,
            })
        }
    }

    /// Reset address to `None`.
    pub fn reset_to_none() {
        ADDRESS_U32.store(0, Ordering::Relaxed);
    }

    /// Set the global address to the given address.
    pub fn set(address: &Address) {
        let addr = (u32::from(address.network_id) << 16)
            | (u32::from(address.node_id) << 8)
            | u32::from(address.port_id);
        ADDRESS_U32.store(addr, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn set_and_get_specific_address_to_atomic_u32(network_id in 1..u16::MAX, node_id in 0..u8::MAX, port_id in 0..u8::MAX) {
            let address = Address {
                network_id,
                node_id,
                port_id,
            };

            DeviceAdress::set(&address);

            let gotten_address = DeviceAdress::get().unwrap();
            assert_eq!(gotten_address, address);
        }
    }

    #[test]
    fn get_address_when_not_set() {
        assert!(DeviceAdress::get().is_none());
    }
}
