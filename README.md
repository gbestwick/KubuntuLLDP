# KubuntuLLDP

KubuntuLLDP is a fullscreen Kubuntu application for network bring-up and switch discovery.
The first version is structured around two processes:

- a privileged system agent that watches a chosen interface, listens for CDP/LLDP, and coordinates DHCP and interface configuration
- a fullscreen UI that presents link status, discovered neighbors, and DHCP details

## Target

- Kubuntu 26.xx
- boot-time startup
- privileged operation allowed
- future support for additional interface and provisioning workflows

## Repository layout

- `crates/kubuntu-lldp-core`: shared domain types and protocol models
- `crates/kubuntu-lldp-agent`: privileged daemon that owns interface monitoring and network setup
- `crates/kubuntu-lldp-ui`: fullscreen application that presents runtime state
- `.devcontainer`: local development environment definition
- `systemd/system`: boot-time system service for the privileged agent
- `systemd/user`: user-session service for the fullscreen UI

## Development environment

This repo is set up to be developed in a container with the following toolchain:

- Rust stable
- `clippy`
- `rustfmt`
- build essentials and common Linux development libraries
- `pkg-config`
- `libdbus-1-dev`
- `libpcap-dev`
- `libudev-dev`
- `libwayland-dev`
- `libxkbcommon-dev`

The container is the fastest way to keep the environment reproducible while the app is still being built.

## Runtime shape

The first implementation should keep these responsibilities separated:

1. Agent: interface monitoring, packet capture, DHCP negotiation, and privileged configuration
2. UI: fullscreen status display and user interaction
3. Shared core: state models, message formats, and protocol parsing helpers

## Startup plan

The long-term boot path should be:

1. systemd starts the privileged agent at boot
2. the UI launches in the user session or kiosk session
3. the UI connects to the agent over a local IPC channel

That split keeps the networking work isolated from the display stack.
