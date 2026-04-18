# syntax=docker/dockerfile:1.7

FROM ghcr.io/boldsoftware/exeuntu:latest

USER root

COPY rust-toolchain.toml /tmp/rust-toolchain.toml

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN toolchain="$(sed -n 's/^channel = "\\(.*\\)"/\\1/p' /tmp/rust-toolchain.toml)" && \
    sudo -u exedev bash -lc "curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal" && \
    sudo -u exedev bash -lc "source ~/.cargo/env && rustup toolchain install ${toolchain} --profile minimal && rustup default ${toolchain}" && \
    rm -f /tmp/rust-toolchain.toml

ENV PATH="/home/exedev/.cargo/bin:${PATH}"

USER exedev
WORKDIR /home/exedev

RUN bash -lc "cargo --version && rustc --version"
