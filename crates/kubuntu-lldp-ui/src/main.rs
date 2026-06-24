#![forbid(unsafe_code)]

use kubuntu_lldp_core::{InterfaceSnapshot, LinkState, RuntimeSnapshot};

fn main() {
    let snapshot = RuntimeSnapshot {
        interface: InterfaceSnapshot {
            name: "eth0".to_string(),
            state: LinkState::Down,
            mac_address: None,
            ip_address: None,
        },
        neighbors: Vec::new(),
        dhcp_options: Vec::new(),
    };

    println!("kubuntu-lldp-ui bootstrap: {:?}", snapshot);
}

