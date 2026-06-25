#![forbid(unsafe_code)]

use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use kubuntu_lldp_core::{
    decode_request, encode_response, AgentRequest, AgentResponse, DhcpOptionRecord,
    DiscoveryProtocol, InterfaceSnapshot, LinkState, NeighborRecord, RuntimeSnapshot,
    DEFAULT_SOCKET_PATH,
};

const DISCOVERY_CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const DHCP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct AgentState {
    selected_interface: Option<String>,
    interfaces: Vec<InterfaceSnapshot>,
    neighbors: Vec<NeighborRecord>,
    dhcp_options: Vec<DhcpOptionRecord>,
    discovery_status: String,
    dhcp_status: String,
    last_error: Option<String>,
    desired_generation: u64,
    running_generation: Option<u64>,
}

impl AgentState {
    fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            selected_interface: self.selected_interface.clone(),
            interfaces: self.interfaces.clone(),
            neighbors: self.neighbors.clone(),
            dhcp_options: self.dhcp_options.clone(),
            discovery_status: self.discovery_status.clone(),
            dhcp_status: self.dhcp_status.clone(),
            last_error: self.last_error.clone(),
        }
    }
}

fn main() -> io::Result<()> {
    let socket_path = socket_path_from_args().unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH));
    prepare_socket_dir(&socket_path)?;

    let interfaces = list_interfaces()?;
    let selected_interface = interfaces
        .iter()
        .find(|iface| iface.name != "lo")
        .map(|iface| iface.name.clone())
        .or_else(|| interfaces.first().map(|iface| iface.name.clone()));

    let state = Arc::new(Mutex::new(AgentState {
        selected_interface,
        interfaces,
        neighbors: Vec::new(),
        dhcp_options: Vec::new(),
        discovery_status: "idle".to_string(),
        dhcp_status: "idle".to_string(),
        last_error: None,
        desired_generation: 0,
        running_generation: None,
    }));

    let monitor_state = Arc::clone(&state);
    thread::spawn(move || monitor_loop(monitor_state));

    if socket_path.exists() {
        let _ = fs::remove_file(&socket_path);
    }
    let listener = UnixListener::bind(&socket_path)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let request_state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, request_state) {
                        eprintln!("request failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("accept failed: {err}"),
        }
    }

    Ok(())
}

fn socket_path_from_args() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--socket-path" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn prepare_socket_dir(socket_path: &Path) -> io::Result<()> {
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn monitor_loop(state: Arc<Mutex<AgentState>>) {
    loop {
        let mut should_start = None;
        if let Ok(mut guard) = state.lock() {
            if let Ok(interfaces) = list_interfaces() {
                let selected = guard.selected_interface.clone();
                guard.interfaces = interfaces
                    .into_iter()
                    .map(|iface| {
                        let is_selected = selected.as_deref() == Some(iface.name.as_str());
                        refresh_interface(&iface.name, is_selected)
                    })
                    .collect();
            }

            if let Some(selected_name) = guard.selected_interface.clone() {
                let selected_up = guard
                    .interfaces
                    .iter()
                    .find(|iface| iface.name == selected_name)
                    .map(|iface| matches!(iface.state, LinkState::Up))
                    .unwrap_or(false);

                if selected_up
                    && guard.running_generation != Some(guard.desired_generation)
                {
                    guard.running_generation = Some(guard.desired_generation);
                    guard.discovery_status = "starting".to_string();
                    guard.dhcp_status = "waiting for discovery".to_string();
                    should_start = Some((selected_name, guard.desired_generation));
                }
            }
        }

        if let Some((interface, generation)) = should_start {
            let worker_state = Arc::clone(&state);
            thread::spawn(move || provision_interface(worker_state, interface, generation));
        }

        thread::sleep(Duration::from_secs(1));
    }
}

fn handle_connection(mut stream: UnixStream, state: Arc<Mutex<AgentState>>) -> io::Result<()> {
    let mut request_line = String::new();
    {
        let mut reader = BufReader::new(&stream);
        reader.read_line(&mut request_line)?;
    }

    let response = match decode_request(request_line.trim_end()) {
        Ok(AgentRequest::ListState) => {
            let guard = state.lock().expect("agent state poisoned");
            AgentResponse::State(guard.snapshot())
        }
        Ok(AgentRequest::SelectInterface { name }) => {
            select_interface(&state, name);
            let guard = state.lock().expect("agent state poisoned");
            AgentResponse::State(guard.snapshot())
        }
        Ok(AgentRequest::RetryProvisioning) => {
            retry_provisioning(&state);
            let guard = state.lock().expect("agent state poisoned");
            AgentResponse::State(guard.snapshot())
        }
        Err(err) => AgentResponse::Error { message: err },
    };

    stream.write_all(encode_response(&response).as_bytes())?;
    let _ = stream.shutdown(Shutdown::Write);
    Ok(())
}

fn select_interface(state: &Arc<Mutex<AgentState>>, name: String) {
    if let Ok(mut guard) = state.lock() {
        guard.selected_interface = Some(name);
        guard.desired_generation = guard.desired_generation.wrapping_add(1);
        guard.running_generation = None;
        guard.discovery_status = "queued".to_string();
        guard.dhcp_status = "queued".to_string();
        guard.last_error = None;
    }
}

fn retry_provisioning(state: &Arc<Mutex<AgentState>>) {
    if let Ok(mut guard) = state.lock() {
        guard.desired_generation = guard.desired_generation.wrapping_add(1);
        guard.running_generation = None;
        guard.discovery_status = "queued".to_string();
        guard.dhcp_status = "queued".to_string();
        guard.last_error = None;
    }
}

fn provision_interface(state: Arc<Mutex<AgentState>>, interface: String, generation: u64) {
    if let Err(err) = run_provisioning(&state, &interface, generation) {
        set_error(&state, generation, err.to_string());
    }

    if let Ok(mut guard) = state.lock() {
        if guard.desired_generation == generation {
            guard.running_generation = None;
        }
    }
}

fn run_provisioning(
    state: &Arc<Mutex<AgentState>>,
    interface: &str,
    generation: u64,
) -> io::Result<()> {
    set_status(
        state,
        generation,
        "discovery",
        format!("capturing LLDP/CDP on {interface}"),
    );
    let neighbors = capture_neighbors(interface)?;
    if !is_current_generation(state, generation) {
        return Ok(());
    }

    update_neighbors(state, generation, neighbors);
    set_status(
        state,
        generation,
        "discovery",
        "link-local discovery complete".to_string(),
    );

    set_status(
        state,
        generation,
        "dhcp",
        format!("requesting lease on {interface}"),
    );
    let dhcp_options = run_dhcp_client(interface)?;
    if !is_current_generation(state, generation) {
        return Ok(());
    }

    update_dhcp_options(state, generation, interface, dhcp_options)?;
    set_status(
        state,
        generation,
        "dhcp",
        "lease acquired and applied".to_string(),
    );
    refresh_interface_snapshot(state, interface);
    Ok(())
}

fn is_current_generation(state: &Arc<Mutex<AgentState>>, generation: u64) -> bool {
    state
        .lock()
        .map(|guard| guard.desired_generation == generation)
        .unwrap_or(false)
}

fn set_status(state: &Arc<Mutex<AgentState>>, generation: u64, which: &str, value: String) {
    if let Ok(mut guard) = state.lock() {
        if guard.desired_generation != generation {
            return;
        }
        match which {
            "discovery" => guard.discovery_status = value,
            "dhcp" => guard.dhcp_status = value,
            _ => {}
        }
    }
}

fn set_error(state: &Arc<Mutex<AgentState>>, generation: u64, value: String) {
    if let Ok(mut guard) = state.lock() {
        if guard.desired_generation != generation {
            return;
        }
        guard.last_error = Some(value);
        guard.discovery_status = "error".to_string();
        guard.dhcp_status = "error".to_string();
    }
}

fn update_neighbors(state: &Arc<Mutex<AgentState>>, generation: u64, neighbors: Vec<NeighborRecord>) {
    if let Ok(mut guard) = state.lock() {
        if guard.desired_generation != generation {
            return;
        }
        guard.neighbors = neighbors;
    }
}

fn update_dhcp_options(
    state: &Arc<Mutex<AgentState>>,
    generation: u64,
    interface: &str,
    dhcp_options: Vec<DhcpOptionRecord>,
) -> io::Result<()> {
    if let Ok(mut guard) = state.lock() {
        if guard.desired_generation != generation {
            return Ok(());
        }
        guard.dhcp_options = dhcp_options;
    }
    refresh_interface_snapshot(state, interface);
    Ok(())
}

fn refresh_interface_snapshot(state: &Arc<Mutex<AgentState>>, selected_name: &str) {
    if let Ok(mut guard) = state.lock() {
        if let Ok(interfaces) = list_interfaces() {
            guard.interfaces = interfaces
                .into_iter()
                .map(|iface| {
                    let is_selected = iface.name == selected_name;
                    refresh_interface(&iface.name, is_selected)
                })
                .collect();
        }
    }
}

fn list_interfaces() -> io::Result<Vec<InterfaceSnapshot>> {
    let mut interfaces = Vec::new();
    for entry in fs::read_dir("/sys/class/net")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        interfaces.push(refresh_interface(&name, false));
    }
    interfaces.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(interfaces)
}

fn refresh_interface(name: &str, is_selected: bool) -> InterfaceSnapshot {
    InterfaceSnapshot {
        name: name.to_string(),
        state: read_operstate(name).unwrap_or(LinkState::Unknown),
        mac_address: read_trimmed(format!("/sys/class/net/{name}/address")).ok(),
        ip_address: read_ipv4_address(name).ok().flatten(),
        is_selected,
    }
}

fn read_operstate(name: &str) -> io::Result<LinkState> {
    let state = read_trimmed(format!("/sys/class/net/{name}/operstate"))?;
    Ok(match state.as_str() {
        "up" => LinkState::Up,
        "down" => LinkState::Down,
        _ => LinkState::Unknown,
    })
}

fn read_ipv4_address(name: &str) -> io::Result<Option<String>> {
    let output = Command::new("ip")
        .args(["-o", "-4", "addr", "show", "dev", name])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tokens: Vec<_> = stdout.split_whitespace().collect();
    for window in tokens.windows(2) {
        if window[0] == "inet" {
            return Ok(Some(window[1].split('/').next().unwrap_or(window[1]).to_string()));
        }
    }

    Ok(None)
}

fn read_trimmed(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path).map(|text| text.trim().to_string())
}

fn capture_neighbors(interface: &str) -> io::Result<Vec<NeighborRecord>> {
    let mut command = Command::new("tcpdump");
    command
            .args([
                "-i",
                interface,
                "-U",
                "-w",
                "-",
                "-s",
                "2048",
                "-c",
                "25",
                "ether proto 0x88cc or ether[20:2] = 0x2000",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    let output = run_command_with_timeout(
        &mut command,
        DISCOVERY_CAPTURE_TIMEOUT,
    )?;

    let mut neighbors = parse_neighbor_pcap(&output.stdout);
    neighbors.sort_by(|left, right| {
        protocol_rank(&left.protocol)
            .cmp(&protocol_rank(&right.protocol))
            .then(left.system_name.cmp(&right.system_name))
            .then(left.port_id.cmp(&right.port_id))
    });
    neighbors.dedup();
    Ok(neighbors)
}

fn parse_neighbor_pcap(bytes: &[u8]) -> Vec<NeighborRecord> {
    if bytes.len() < 24 {
        return Vec::new();
    }

    let magic = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let little_endian = match magic {
        0xa1b2c3d4 | 0xa1b23c4d => false,
        _ => {
            let le = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            match le {
                0xa1b2c3d4 | 0xa1b23c4d => true,
                _ => return Vec::new(),
            }
        }
    };
    let mut offset = 24;

    let mut records = Vec::new();
    while offset + 16 <= bytes.len() {
        let incl_len = read_u32(&bytes[offset + 8..offset + 12], little_endian) as usize;
        offset += 16;
        if offset + incl_len > bytes.len() {
            break;
        }
        if let Some(record) = parse_ethernet_frame(&bytes[offset..offset + incl_len]) {
            records.push(record);
        }
        offset += incl_len;
    }

    records
}

fn parse_ethernet_frame(frame: &[u8]) -> Option<NeighborRecord> {
    if frame.len() < 14 {
        return None;
    }

    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    if ethertype == 0x88cc {
        return parse_lldp(&frame[14..]);
    }

    if frame[14..].len() >= 8
        && frame[14] == 0xaa
        && frame[15] == 0xaa
        && frame[16] == 0x03
        && frame[17] == 0x00
        && frame[18] == 0x00
        && frame[19] == 0x0c
        && frame[20] == 0x20
        && frame[21] == 0x00
    {
        return parse_cdp(&frame[22..]);
    }

    None
}

fn parse_lldp(payload: &[u8]) -> Option<NeighborRecord> {
    let mut chassis_id = None;
    let mut port_id = None;
    let mut system_name = None;
    let mut system_description = None;
    let mut offset = 0usize;

    while offset + 2 <= payload.len() {
        let header = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        offset += 2;
        let tlv_type = header >> 9;
        let tlv_len = (header & 0x01ff) as usize;
        if tlv_type == 0 {
            break;
        }
        if offset + tlv_len > payload.len() {
            break;
        }
        let value = &payload[offset..offset + tlv_len];
        match tlv_type {
            1 => chassis_id = decode_lldp_identity(value),
            2 => port_id = decode_lldp_identity(value),
            5 => system_name = bytes_to_string(value),
            6 => system_description = bytes_to_string(value),
            _ => {}
        }
        offset += tlv_len;
    }

    if chassis_id.is_none()
        && port_id.is_none()
        && system_name.is_none()
        && system_description.is_none()
    {
        return None;
    }

    Some(NeighborRecord {
        protocol: DiscoveryProtocol::Lldp,
        chassis_id,
        port_id,
        system_name,
        system_description,
    })
}

fn parse_cdp(payload: &[u8]) -> Option<NeighborRecord> {
    if payload.len() < 4 {
        return None;
    }

    let mut device_id = None;
    let mut port_id = None;
    let mut software = None;
    let mut platform = None;
    let mut offset = 4usize;

    while offset + 4 <= payload.len() {
        let tlv_type = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        let tlv_len = u16::from_be_bytes([payload[offset + 2], payload[offset + 3]]) as usize;
        if tlv_len < 4 || offset + tlv_len > payload.len() {
            break;
        }
        let value = &payload[offset + 4..offset + tlv_len];
        match tlv_type {
            0x0001 => device_id = bytes_to_string(value),
            0x0003 => port_id = bytes_to_string(value),
            0x0005 => software = bytes_to_string(value),
            0x0006 => platform = bytes_to_string(value),
            _ => {}
        }
        offset += tlv_len;
    }

    if device_id.is_none() && port_id.is_none() && software.is_none() && platform.is_none() {
        return None;
    }

    Some(NeighborRecord {
        protocol: DiscoveryProtocol::Cdp,
        chassis_id: device_id.clone(),
        port_id,
        system_name: device_id,
        system_description: match (software, platform) {
            (Some(software), Some(platform)) => Some(format!("{software} | {platform}")),
            (Some(software), None) => Some(software),
            (None, Some(platform)) => Some(platform),
            (None, None) => None,
        },
    })
}

fn decode_lldp_identity(value: &[u8]) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    let subtype = value[0];
    let data = &value[1..];
    let decoded = bytes_to_string(data).unwrap_or_else(|| bytes_to_hex(data));
    Some(format!("{subtype}:{decoded}"))
}

fn bytes_to_string(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn run_dhcp_client(interface: &str) -> io::Result<Vec<DhcpOptionRecord>> {
    let _ = Command::new("ip")
        .args(["link", "set", "dev", interface, "up"])
        .status();

    let output = run_command_with_timeout(
        Command::new("dhclient")
            .args(["-4", "-v", "-1", interface])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        DHCP_TIMEOUT,
    )?;

    let mut options = parse_dhcp_output(&output.stdout);
    options.extend(parse_dhcp_leases(interface));
    options.sort_by(|left, right| left.name.cmp(&right.name).then(left.value.cmp(&right.value)));
    options.dedup();
    Ok(options)
}

fn parse_dhcp_output(output: &[u8]) -> Vec<DhcpOptionRecord> {
    let text = String::from_utf8_lossy(output);
    let mut records = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("bound to ") {
            if let Some((address, _)) = rest.split_once(' ') {
                records.push(DhcpOptionRecord {
                    code: Some(50),
                    name: "yiaddr".to_string(),
                    value: address.trim_end_matches('.').to_string(),
                });
            }
        }
    }
    records
}

fn parse_dhcp_leases(interface: &str) -> Vec<DhcpOptionRecord> {
    let mut records = Vec::new();
    let mut paths = vec![
        PathBuf::from(format!("/var/lib/dhcp/dhclient.{interface}.leases")),
        PathBuf::from("/var/lib/dhcp/dhclient.leases"),
    ];
    for path in paths.drain(..) {
        if let Ok(text) = fs::read_to_string(&path) {
            records.extend(parse_dhcp_lease_text(&text));
            if !records.is_empty() {
                break;
            }
        }
    }
    records
}

fn parse_dhcp_lease_text(text: &str) -> Vec<DhcpOptionRecord> {
    let mut current_block = Vec::new();
    let mut in_lease = false;
    let mut last_complete = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("lease ") && trimmed.ends_with('{') {
            in_lease = true;
            current_block.clear();
            continue;
        }
        if in_lease && trimmed == "}" {
            last_complete = current_block.clone();
            in_lease = false;
            continue;
        }
        if in_lease {
            if let Some(rest) = trimmed.strip_prefix("option ") {
                if let Some(record) = parse_dhcp_option_line(rest) {
                    current_block.push(record);
                }
            }
        }
    }

    last_complete
}

fn parse_dhcp_option_line(line: &str) -> Option<DhcpOptionRecord> {
    let line = line.trim_end_matches(';').trim();
    let (name, value) = line.split_once(' ')?;
    let code = dhcp_option_code(name);
    Some(DhcpOptionRecord {
        code,
        name: name.to_string(),
        value: value.trim_matches('"').to_string(),
    })
}

fn dhcp_option_code(name: &str) -> Option<u8> {
    match name {
        "subnet-mask" => Some(1),
        "routers" => Some(3),
        "domain-name-servers" => Some(6),
        "host-name" => Some(12),
        "domain-name" => Some(15),
        "broadcast-address" => Some(28),
        "interface-mtu" => Some(26),
        "static-routes" => Some(33),
        "nis-domain" => Some(40),
        "ntp-servers" => Some(42),
        "requested-address" => Some(50),
        "lease-time" => Some(51),
        "server-identifier" => Some(54),
        "parameter-request-list" => Some(55),
        "renewal-time" => Some(58),
        "rebinding-time" => Some(59),
        _ => None,
    }
}

fn run_command_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = command.spawn()?;
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(_status) = child.try_wait()? {
            return child.wait_with_output();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return child.wait_with_output();
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn read_u32(bytes: &[u8], little_endian: bool) -> u32 {
    let array = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if little_endian {
        u32::from_le_bytes(array)
    } else {
        u32::from_be_bytes(array)
    }
}

fn protocol_rank(protocol: &DiscoveryProtocol) -> u8 {
    match protocol {
        DiscoveryProtocol::Cdp => 0,
        DiscoveryProtocol::Lldp => 1,
    }
}
