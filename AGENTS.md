# KubuntuLLDP Agent Handoff

This file is intended for future Codex or human contributors resuming the project. Keep it current when behavior, setup, or priorities change.

## Project Goal

KubuntuLLDP is a Rust application targeting Kubuntu 26.xx. The goal is to emulate the basic field workflow of a Fluke LinkRunner 2000:

- run automatically at boot
- provide a fullscreen, touch-friendly UI
- monitor a selected network interface
- detect link state, MAC address, and IP address
- capture LLDP/CDP packets from the connected switch
- display clear switch/port discovery information
- attempt DHCP/network configuration
- display DHCP/network configuration details

The intended architecture is split into a privileged backend agent and an unprivileged fullscreen UI.

## Repository

GitHub remote:

```bash
https://github.com/gbestwick/KubuntuLLDP.git
```

Workspace layout:

- `crates/kubuntu-lldp-core`: shared domain types and text IPC protocol.
- `crates/kubuntu-lldp-agent`: privileged network agent.
- `crates/kubuntu-lldp-ui`: fullscreen native X11 UI.
- `systemd/system/kubuntu-lldp-agent.service`: draft system service for the agent.
- `systemd/user/kubuntu-lldp-ui.service`: draft user service for the UI.
- `.devcontainer`: development container definition.

## Current Runtime Model

The UI talks to the agent over a Unix socket:

```text
/tmp/kubuntu-lldp/kubuntu-lldp.sock
```

The agent creates the socket with mode `0666` so an unprivileged UI can connect. The agent should normally run as root because live LLDP/CDP capture uses `tcpdump`, which requires packet capture privileges such as `CAP_NET_RAW`.

Current default/test interface behavior:

- The agent accepts `--interface <name>`.
- If no interface is supplied, it prefers `enxc8a3623fbbbe` when present.
- Otherwise it selects the first non-loopback interface it finds.

Typical manual test startup:

```bash
cd /home/pcadmin/KubuntuLLDP

sudo target/debug/kubuntu-lldp-agent \
  --interface enxc8a3623fbbbe \
  --socket-path /tmp/kubuntu-lldp/kubuntu-lldp.sock
```

In another terminal:

```bash
cd /home/pcadmin/KubuntuLLDP
target/debug/kubuntu-lldp-ui
```

Useful socket query:

```bash
printf 'LIST_STATE\n' | socat - UNIX-CONNECT:/tmp/kubuntu-lldp/kubuntu-lldp.sock
```

## Build And Verification Commands

The installed rustup toolchain has been used directly in this environment:

```bash
RUSTC=/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc \
RUSTDOC=/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustdoc \
/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo test --workspace

RUSTC=/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc \
RUSTDOC=/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustdoc \
/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo check --workspace

RUSTC=/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc \
RUSTDOC=/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustdoc \
/home/pcadmin/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo build --workspace
```

The shell often prints this warning under Codex sandboxing:

```text
Failed to create stream fd: Operation not permitted
```

This has been benign when the command still exits successfully.

## Implemented Features

### Core IPC

The shared core crate defines:

- `AgentRequest::ListState`
- `AgentRequest::SelectInterface { name }`
- `AgentRequest::RetryProvisioning`
- `AgentResponse::State`
- `AgentResponse::Error`

The wire format is a simple line-oriented text format over the Unix socket. A round-trip unit test exists for extended LLDP/CDP neighbor fields.

### Agent

The agent currently:

- lists local interfaces from `/sys/class/net`
- reads link state, MAC address, and IPv4 address
- maintains selected-interface state
- clears stale neighbor and DHCP data when interface selection changes
- monitors the selected interface and starts discovery when link is up
- captures one incoming LLDP/CDP packet using `tcpdump`
- supports replay parsing with `--pcap-file <path>`
- filters local/self LLDP advertisements by local MAC address
- parses LLDP Ethernet frames
- parses CDP SNAP frames
- runs DHCP/network configuration probing after discovery
- falls back to active network configuration when `dhclient` is missing or fails

Current DHCP fallback sources:

- current IPv4 address from `ip`
- NetworkManager device data from `nmcli`
- default route from `ip route`
- existing dhclient lease files when present

### LLDP/CDP Fields

The current neighbor model can carry:

- protocol, `LLDP` or `CDP`
- chassis ID
- remote port ID
- port description
- device/system name
- system description
- management addresses
- capabilities
- TTL
- native VLAN
- duplex/media where advertised

Known parsed LLDP TLVs:

- Chassis ID
- Port ID
- TTL
- Port Description
- System Name
- System Description
- System Capabilities
- Management Address
- selected organizational TLVs for VLAN and MAC/PHY/media information

Known parsed CDP TLVs:

- Device ID
- Address
- Port ID
- Capabilities
- Software Version
- Platform
- Native VLAN
- Duplex

### UI

The UI is a native X11 fullscreen-style dashboard using direct Xlib FFI. It currently provides:

- large touch-friendly layout
- status cards for selected interface, discovery, DHCP, and last error
- large Retry and Quit buttons
- left-side interface selector
- right-side LLDP/CDP neighbor panel
- lower-right DHCP/network configuration panel
- labeled LLDP/CDP rows so the meaning of each field is clear

The LLDP/CDP panel currently receives most of the right-side vertical space because the expanded neighbor fields need more room.

## Known Issues And Constraints

### Privileges

Live LLDP/CDP capture requires root or packet capture capability. Running the agent as a normal user causes errors like:

```text
tcpdump: <interface>: You don't have permission to perform this capture on that device
(Attempt to create packet socket failed - CAP_NET_RAW may be required)
```

The intended fix is to run the agent as a systemd system service as root. The UI should stay unprivileged.

### DHCP

`dhclient` is not installed on the current test machine. Earlier builds reported DHCP as `error` because the agent attempted to run `dhclient` directly. The current code falls back to active network configuration and should show `network configuration detected` when it can read useful data.

Open question: decide whether KubuntuLLDP should actually control DHCP itself or ask NetworkManager to configure the selected interface. On Kubuntu, NetworkManager integration is probably the better long-term path.

### UI Font Rendering

The current UI uses X11 core font APIs (`XLoadQueryFont`, `XDrawString`). This limits typography and makes modern TrueType fonts difficult. Font candidates were improved, but fully modern anti-aliased text likely requires one of:

- Xft/fontconfig rendering
- a GUI toolkit such as Qt, GTK, Slint, Iced, or egui
- moving UI rendering to a more capable graphics stack

Given the target is Kubuntu, Qt/QML may be a strong fit for the final UI.

### UI Capacity

The LLDP/CDP information panel is now larger, but there is no scrolling yet. If multiple neighbors or many DHCP options are present, content can still overflow. Touch-friendly scrolling should be added.

### Socket Lifecycle

If the agent exits unexpectedly, the UI may show connection errors. Restarting the agent recreates the socket. A future systemd service should handle restart and socket cleanup.

## Important Test Observations

`lldpcli` previously confirmed that the real switch advertises LLDP on `enxc8a3623fbbbe`, including data similar to:

- system name: `S108EF5918007679`
- system description: `FortiSwitch-108E-FPOE v7.4.8...`
- management IP: `10.255.1.8`
- port ID/description: `port3`
- capabilities: Bridge and Router
- TTL: `120`
- MAU/media: `1000BaseT full duplex`

This is the type of detail the app should display clearly.

## Near-Term Roadmap

Recommended next work, in order:

1. Run the updated agent as root and UI normally, then verify the expanded LLDP/CDP panel against the FortiSwitch.
2. Add scrolling to the LLDP/CDP and DHCP panels.
3. Replace `dhclient` execution with proper NetworkManager integration, or make DHCP mode selectable:
   - observe only
   - request/configure via NetworkManager
   - direct standalone DHCP client
4. Improve DHCP display semantics:
   - distinguish offered options from active configuration
   - show lease server, lease time, renewal/rebinding time, router, DNS, domain, subnet mask, MTU
5. Improve UI font rendering with Xft or migrate UI to a toolkit.
6. Add installer/service setup:
   - install binaries into `/usr/local/bin` or package path
   - install and enable systemd agent service
   - install and enable user/kiosk UI service
7. Add better diagnostics:
   - show whether capture privileges are available
   - show whether `tcpdump`, `nmcli`, and DHCP tooling are available
   - show the exact selected interface and active socket path
8. Add tests for LLDP/CDP parser sample frames.
9. Add a release build/package workflow.

## Coding Notes For Future Agents

- Prefer narrow, behavior-focused changes.
- Do not revert user or unrelated work in the repo.
- Use `rg` for searching.
- Use `apply_patch` for manual edits.
- Run `rustfmt` on touched Rust files.
- Run at least `cargo test --workspace` before committing behavior changes.
- Commit and push completed project changes to `main` unless the user says otherwise.

## Current Git State At Handoff

At the time this handoff file was created, the project had recent commits for:

- improved touchscreen UI layout
- stale state clearing on interface selection
- richer LLDP/CDP neighbor parsing and display
- DHCP fallback to active network configuration when `dhclient` is unavailable

After updating this file, verify with:

```bash
git status --short
git log --oneline -5
```
