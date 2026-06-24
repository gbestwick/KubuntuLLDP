#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryProtocol {
    Cdp,
    Lldp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceSnapshot {
    pub name: String,
    pub state: LinkState,
    pub mac_address: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighborRecord {
    pub protocol: DiscoveryProtocol,
    pub chassis_id: Option<String>,
    pub port_id: Option<String>,
    pub system_name: Option<String>,
    pub system_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhcpOptionRecord {
    pub code: u8,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub interface: InterfaceSnapshot,
    pub neighbors: Vec<NeighborRecord>,
    pub dhcp_options: Vec<DhcpOptionRecord>,
}

