# Building Notare

Verified on Linux (Ubuntu 24.04 / WSL2) 2026-07-14. macOS/Windows instructions
will follow as those pipelines are exercised.

## Prerequisites

- **Rust** (stable, via rustup) — the workspace pins its toolchain in
  `rust-toolchain.toml`.
- **Node 22+** and **pnpm 11** (`corepack enable` uses the pinned version from
  `package.json`).
- **Linux system packages** (Ubuntu/Debian):

  ```sh
  sudo apt-get install -y \
    libwebkit2gtk-4.1-dev build-essential file libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev xdg-utils \
    libgtk-3-dev libgtk-4-dev libasound2-dev libudev-dev libpulse-dev \
    libpipewire-0.3-dev libgraphene-1.0-dev pkg-config patchelf cmake \
    libcurl4-openssl-dev libclang-dev clang
  ```

  (`scripts/setup-linux.sh` is the upstream equivalent; the list above adds
  `libclang-dev`/`clang`, required by bindgen, and works without the PipeWire
  PPA on Ubuntu 24.04.)

## Build

```sh
pnpm install
pnpm exec turbo run build --filter=@hypr/desktop   # frontend (builds @hypr/ui first)
cargo check -p desktop                              # Rust workspace sanity
cargo build -p desktop                              # debug binary
```

Note: `pnpm -F desktop build` alone fails (`@hypr/ui/globals.css` unresolved) —
workspace packages must be built first, which turbo handles.

## Known environment quirks

- **WSL2 / hosts with broken IPv6:** Node's fetch (and pnpm's) can time out
  where curl works. Fixes: `export NODE_OPTIONS="--no-network-family-autoselection"`;
  if pnpm still times out on registry metadata, pin a reachable IPv4 in
  `/etc/hosts` (e.g. `104.16.1.34 registry.npmjs.org`).
- `crates/api-client` generates code from `crates/api-client/openapi.upstream.json`
  (a snapshot of the upstream cloud API — the live `apps/api` was stripped from
  this fork; the crate disappears with the cloud client code).
