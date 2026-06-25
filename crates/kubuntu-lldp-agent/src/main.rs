#![forbid(unsafe_code)]

use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use kubuntu_lldp_core::{
    decode_request, encode_response, AgentRequest, AgentResponse, DhcpOptionRecord,
    InterfaceSnapshot, LinkState, NeighborRecord, RuntimeSnapshot, DEFAULT_SOCKET_PATH,
};

#[derive(Debug, Clone)]
struct AgentState {
    selected_interface: Option<String>,
    interfaces: Vec<InterfaceSnapshot>,
    neighbors: Vec<NeighborRecord>,
    dhcp_options: Vec<DhcpOptionRecord>,
}

impl AgentState {
    fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            selected_interface: self.selected_interface.clone(),
            interfaces: self.interfaces.clone(),
            neighbors: self.neighbors.clone(),
            dhcp_options: self.dhcp_options.clone(),
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
            let mut guard = state.lock().expect("agent state poisoned");
            let available = list_interfaces()?;
            if available.iter().any(|iface| iface.name == name) {
                guard.selected_interface = Some(name);
                let selected = guard.selected_interface.clone();
                guard.interfaces = available
                    .into_iter()
                    .map(|iface| {
                        let is_selected = selected.as_deref() == Some(iface.name.as_str());
                        refresh_interface(&iface.name, is_selected)
                    })
                    .collect();
                AgentResponse::State(guard.snapshot())
            } else {
                AgentResponse::Error {
                    message: format!("interface '{name}' is not available"),
                }
            }
        }
        Err(err) => AgentResponse::Error { message: err },
    };

    stream.write_all(encode_response(&response).as_bytes())?;
    let _ = stream.shutdown(Shutdown::Write);
    Ok(())
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
    let output = std::process::Command::new("ip")
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
