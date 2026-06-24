#![forbid(unsafe_code)]

use kubuntu_lldp_core::{
    DhcpOptionRecord, DiscoveryProtocol, InterfaceSnapshot, LinkState, NeighborRecord,
    RuntimeSnapshot,
};

fn main() {
    let snapshot = RuntimeSnapshot {
        interface: InterfaceSnapshot {
            name: "eth0".to_string(),
            state: LinkState::Down,
            mac_address: None,
            ip_address: None,
        },
        neighbors: vec![NeighborRecord {
            protocol: DiscoveryProtocol::Lldp,
            chassis_id: None,
            port_id: None,
            system_name: None,
            system_description: None,
        }],
        dhcp_options: vec![DhcpOptionRecord {
            code: 0,
            value: "placeholder".to_string(),
        }],
    };

    println!("kubuntu-lldp-agent bootstrap: {:?}", snapshot);
}

