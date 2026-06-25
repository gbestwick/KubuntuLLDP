#![forbid(unsafe_code)]

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/kubuntu-lldp/kubuntu-lldp.sock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryProtocol {
    Cdp,
    Lldp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    Down,
    Up,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceSnapshot {
    pub name: String,
    pub state: LinkState,
    pub mac_address: Option<String>,
    pub ip_address: Option<String>,
    pub is_selected: bool,
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
    pub code: Option<u8>,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub selected_interface: Option<String>,
    pub interfaces: Vec<InterfaceSnapshot>,
    pub neighbors: Vec<NeighborRecord>,
    pub dhcp_options: Vec<DhcpOptionRecord>,
    pub discovery_status: String,
    pub dhcp_status: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRequest {
    ListState,
    SelectInterface { name: String },
    RetryProvisioning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentResponse {
    State(RuntimeSnapshot),
    Error { message: String },
}

pub fn encode_request(request: &AgentRequest) -> String {
    match request {
        AgentRequest::ListState => "LIST_STATE".to_string(),
        AgentRequest::SelectInterface { name } => {
            format!("SELECT_INTERFACE\t{}", escape_field(name))
        }
        AgentRequest::RetryProvisioning => "RETRY_PROVISIONING".to_string(),
    }
}

pub fn decode_request(line: &str) -> Result<AgentRequest, String> {
    let mut parts = line.trim_end().splitn(2, '\t');
    let kind = parts.next().unwrap_or_default();
    match kind {
        "LIST_STATE" => Ok(AgentRequest::ListState),
        "SELECT_INTERFACE" => {
            let name = parts.next().ok_or("missing interface name")?;
            Ok(AgentRequest::SelectInterface {
                name: unescape_field(name)?,
            })
        }
        other => Err(format!("unknown request '{other}'")),
    }
}

pub fn encode_response(response: &AgentResponse) -> String {
    match response {
        AgentResponse::Error { message } => {
            format!("ERROR\nmessage={}\nEND\n", escape_field(message))
        }
        AgentResponse::State(snapshot) => {
            let mut out = String::from("STATE\n");
            out.push_str("selected=");
            push_optional_field(&mut out, snapshot.selected_interface.as_deref());
            out.push('\n');

            for iface in &snapshot.interfaces {
                out.push_str("interface=");
                out.push_str(&escape_field(&iface.name));
                out.push('|');
                out.push_str(match iface.state {
                    LinkState::Down => "down",
                    LinkState::Up => "up",
                    LinkState::Unknown => "unknown",
                });
                out.push('|');
                push_optional_field(&mut out, iface.mac_address.as_deref());
                out.push('|');
                push_optional_field(&mut out, iface.ip_address.as_deref());
                out.push('|');
                out.push_str(if iface.is_selected { "1" } else { "0" });
                out.push('\n');
            }

            for neighbor in &snapshot.neighbors {
                out.push_str("neighbor=");
                out.push_str(match neighbor.protocol {
                    DiscoveryProtocol::Cdp => "cdp",
                    DiscoveryProtocol::Lldp => "lldp",
                });
                out.push('|');
                push_optional_field(&mut out, neighbor.chassis_id.as_deref());
                out.push('|');
                push_optional_field(&mut out, neighbor.port_id.as_deref());
                out.push('|');
                push_optional_field(&mut out, neighbor.system_name.as_deref());
                out.push('|');
                push_optional_field(&mut out, neighbor.system_description.as_deref());
                out.push('\n');
            }

            for option in &snapshot.dhcp_options {
                out.push_str("dhcp=");
                push_optional_code(&mut out, option.code);
                out.push('|');
                out.push_str(&escape_field(&option.name));
                out.push('|');
                out.push_str(&escape_field(&option.value));
                out.push('\n');
            }

            out.push_str("discovery_status=");
            out.push_str(&escape_field(&snapshot.discovery_status));
            out.push('\n');
            out.push_str("dhcp_status=");
            out.push_str(&escape_field(&snapshot.dhcp_status));
            out.push('\n');
            out.push_str("last_error=");
            push_optional_field(&mut out, snapshot.last_error.as_deref());
            out.push('\n');

            out.push_str("END\n");
            out
        }
    }
}

pub fn decode_response(text: &str) -> Result<AgentResponse, String> {
    let mut lines = text.lines();
    let first = lines.next().ok_or("missing response header")?;
    if first == "ERROR" {
        let message = lines
            .next()
            .ok_or("missing error message")?
            .strip_prefix("message=")
            .ok_or("malformed error message")?;
        return Ok(AgentResponse::Error {
            message: unescape_field(message)?,
        });
    }

    if first != "STATE" {
        return Err(format!("unexpected response header '{first}'"));
    }

    let selected_line = lines.next().ok_or("missing selected interface line")?;
    let selected = selected_line
        .strip_prefix("selected=")
        .ok_or("malformed selected interface line")?;
    let selected_interface = if selected == "-" {
        None
    } else {
        Some(unescape_field(selected)?)
    };

    let mut interfaces = Vec::new();
    let mut neighbors = Vec::new();
    let mut dhcp_options = Vec::new();
    let mut discovery_status = String::from("idle");
    let mut dhcp_status = String::from("idle");
    let mut last_error = None;

    for line in lines {
        if line == "END" {
            break;
        } else if let Some(value) = line.strip_prefix("discovery_status=") {
            discovery_status = unescape_field(value)?;
        } else if let Some(value) = line.strip_prefix("dhcp_status=") {
            dhcp_status = unescape_field(value)?;
        } else if let Some(value) = line.strip_prefix("last_error=") {
            last_error = if value == "-" {
                None
            } else {
                Some(unescape_field(value)?)
            };
        }
        if let Some(payload) = line.strip_prefix("interface=") {
            let parts = split_escaped(payload, 5)?;
            let state = match parts[1].as_str() {
                "down" => LinkState::Down,
                "up" => LinkState::Up,
                _ => LinkState::Unknown,
            };
            interfaces.push(InterfaceSnapshot {
                name: parts[0].clone(),
                state,
                mac_address: if parts[2] == "-" {
                    None
                } else {
                    Some(parts[2].clone())
                },
                ip_address: if parts[3] == "-" {
                    None
                } else {
                    Some(parts[3].clone())
                },
                is_selected: parts[4] == "1",
            });
        } else if let Some(payload) = line.strip_prefix("neighbor=") {
            let parts = split_escaped(payload, 5)?;
            let protocol = match parts[0].as_str() {
                "cdp" => DiscoveryProtocol::Cdp,
                _ => DiscoveryProtocol::Lldp,
            };
            neighbors.push(NeighborRecord {
                protocol,
                chassis_id: if parts[1] == "-" {
                    None
                } else {
                    Some(parts[1].clone())
                },
                port_id: if parts[2] == "-" {
                    None
                } else {
                    Some(parts[2].clone())
                },
                system_name: if parts[3] == "-" {
                    None
                } else {
                    Some(parts[3].clone())
                },
                system_description: if parts[4] == "-" {
                    None
                } else {
                    Some(parts[4].clone())
                },
            });
        } else if let Some(payload) = line.strip_prefix("dhcp=") {
            let parts = split_escaped(payload, 3)?;
            dhcp_options.push(DhcpOptionRecord {
                code: if parts[0] == "-" {
                    None
                } else {
                    Some(
                        parts[0]
                            .parse()
                            .map_err(|err| format!("invalid DHCP code: {err}"))?,
                    )
                },
                name: parts[1].clone(),
                value: parts[2].clone(),
            });
        }
    }

    Ok(AgentResponse::State(RuntimeSnapshot {
        selected_interface,
        interfaces,
        neighbors,
        dhcp_options,
        discovery_status,
        dhcp_status,
        last_error,
    }))
}

pub fn escape_field(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '|' => out.push_str("\\p"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn unescape_field(value: &str) -> Result<String, String> {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let esc = chars.next().ok_or("trailing escape")?;
        match esc {
            '\\' => out.push('\\'),
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            'p' => out.push('|'),
            other => return Err(format!("unknown escape sequence '\\{other}'")),
        }
    }
    Ok(out)
}

fn split_escaped(value: &str, expected_fields: usize) -> Result<Vec<String>, String> {
    let mut fields = Vec::with_capacity(expected_fields);
    let mut current = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '|' {
            fields.push(current);
            current = String::new();
            continue;
        }

        if ch == '\\' {
            let esc = chars.next().ok_or("trailing escape")?;
            current.push(match esc {
                '\\' => '\\',
                't' => '\t',
                'n' => '\n',
                'r' => '\r',
                'p' => '|',
                other => return Err(format!("unknown escape sequence '\\{other}'")),
            });
            continue;
        }

        current.push(ch);
    }
    fields.push(current);

    if fields.len() != expected_fields {
        return Err(format!(
            "expected {expected_fields} fields, got {}",
            fields.len()
        ));
    }

    Ok(fields)
}

fn push_optional_field(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => out.push_str(&escape_field(value)),
        None => out.push('-'),
    }
}

fn push_optional_code(out: &mut String, value: Option<u8>) {
    match value {
        Some(value) => out.push_str(&value.to_string()),
        None => out.push('-'),
    }
}
