FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    clang \
    curl \
    dbus \
    git \
    libdbus-1-dev \
    libx11-dev \
    libxcb1-dev \
    libpcap-dev \
    libudev-dev \
    libwayland-dev \
    libxkbcommon-dev \
    pkg-config \
    sudo \
    xz-utils \
 && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash vscode \
 && echo "vscode ALL=(ALL) NOPASSWD:ALL" >/etc/sudoers.d/vscode \
 && chmod 0440 /etc/sudoers.d/vscode

USER vscode
WORKDIR /workspaces/KubuntuLLDP

ENV CARGO_HOME=/home/vscode/.cargo
ENV RUSTUP_HOME=/home/vscode/.rustup
ENV PATH=/home/vscode/.cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
RUN rustup component add clippy rustfmt
