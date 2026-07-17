# Building Notare

Verified on Linux (Ubuntu 24.04 / WSL2) 2026-07-14. Windows gotchas and macOS
notes are in their own sections below.

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

## GPU builds (Vulkan)

Default builds run whisper.cpp on CPU. The opt-in `gpu-vulkan` cargo feature
(on the `desktop` crate, forwarding to `tauri-plugin-local-stt/vulkan` →
`whisper-rs/vulkan` → whisper.cpp's `GGML_VULKAN`) offloads transcription to
the GPU via Vulkan — one backend for NVIDIA, AMD and Intel, on both Windows
and Linux. This is the same approach meetily ships as its Windows release
default.

### Build-time requirement: the Vulkan SDK

Vulkan builds need the SDK (headers + the `glslc` shader compiler) at compile
time. Plain CPU builds must never require it — that's why the feature is not
default.

- **Windows:**

  ```powershell
  winget install KhronosGroup.VulkanSDK
  ```

  Then open a fresh shell so `VULKAN_SDK` is set (the whisper.cpp cmake build
  locates the SDK through it, and `glslc.exe` lives in `%VULKAN_SDK%\Bin`).

- **Linux (Ubuntu/Debian):**

  ```sh
  sudo apt-get install -y libvulkan-dev glslc
  ```

  (Or the full `vulkan-sdk` package from the LunarG apt repo if you also want
  the validation layers/tools.)

### Local build commands

Linux:

```sh
pnpm -F desktop tauri build --features gpu-vulkan
```

Windows (PowerShell, from the repo root):

```powershell
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
pnpm -F desktop tauri build --features gpu-vulkan
```

`cargo check`/`cargo build -p desktop --features gpu-vulkan` work the same way
for Rust-only iteration.

CI: the `desktop_test_build` workflow has a `gpu` input — set it to `vulkan`
to get a GPU-enabled artifact (installs the SDK on the runner via
`humbletim/install-vulkan-sdk` on Windows, apt on Linux).

### Runtime: verify the GPU is actually used

At runtime the app only needs the Vulkan *loader* (`vulkan-1.dll` ships with
every Windows GPU driver; `libvulkan.so.1` comes with Mesa/vendor drivers on
Linux) — end users do not need the SDK.

**Do not assume offload happened — check the log.** whisper.cpp prints the
selected device when the model loads, e.g.:

```
ggml_vulkan: Found 1 Vulkan devices:
ggml_vulkan: 0 = NVIDIA GeForce RTX 4080 (NVIDIA) | uma: 0 | fp16: 1 | ...
whisper_init_state: ... backend = Vulkan
```

If instead you see the model land on `CPU`, the Vulkan path silently fell
back. This is a known real-world failure mode — AMD RDNA2 cards in particular
have reports of ggml-vulkan silently falling back to CPU while everything
*appears* to work (just slowly). So after any driver/OS/build change, confirm
the `ggml_vulkan: Found N Vulkan devices` line appears and transcription speed
matches GPU expectations before trusting the build.

## Known environment quirks

- **WSL2 / hosts with broken IPv6:** Node's fetch (and pnpm's) can time out
  where curl works. Fixes: `export NODE_OPTIONS="--no-network-family-autoselection"`;
  if pnpm still times out on registry metadata, pin a reachable IPv4 in
  `/etc/hosts` (e.g. `104.16.1.34 registry.npmjs.org`).
- `crates/api-client` generates code from `crates/api-client/openapi.upstream.json`
  (a snapshot of the upstream cloud API — the live `apps/api` was stripped from
  this fork; the crate disappears with the cloud client code).

### Windows Vulkan build gotchas (all three bite in sequence)

1. **Use the Ninja generator** (`$env:CMAKE_GENERATOR = 'Ninja'`, run inside a
   VS developer shell): whisper.cpp's `vulkan-shaders-gen` subproject fails
   under the default Visual Studio generator (MSBuild `VCTargetsPath` probe).
2. **Use a short cargo target dir** (`$env:CARGO_TARGET_DIR = 'C:\nb'`): the
   shader generator nests its build ~230 chars deep; under the default
   `apps\desktop\src-tauri\target` it hits Windows' 260-char MAX_PATH and
   fails with `ninja: error: mkdir ... No such file or directory`.
3. **If switching generators, delete stale `whisper-rs-sys-*` build dirs**
   first — a CMakeCache from a Visual Studio attempt makes Ninja fail with
   "does not support instance specification".

Verified working recipe (2026-07-16, RTX 4080 machine): VS dev shell +
Ninja + `CARGO_TARGET_DIR=C:\nb` + `CC=cl CXX=cl` + `--features gpu-vulkan`.

## macOS builds (Apple Silicon, unsigned)

Released macOS builds are **Apple Silicon only** (GitHub's `macos-latest`
runners are arm64) and **unsigned** — there are no Apple Developer
certificates yet. Intel Macs must build from source
(`tauri.conf.macos-intel.json` exists for local Intel quirks).

### Prerequisites

- **Xcode** (full toolchain, not just CLT) — `swiftc` compiles the
  `binaries/check-permissions-<triple>` external binary via
  `src-tauri/build.rs` (source: `plugins/permissions/swift/check-permissions.swift`;
  gitignored, rebuilt automatically, nothing to fetch), and
  `crates/transcribe-soniqo` builds its Swift package (mlx-swift et al.)
  through `xcrun`/`swift build`.
- **bindgen needs `libclang.dylib`** (for `libsqlite3-sys` via the mac-only
  legacy importer). If the build panics with "Unable to find libclang", set
  `LIBCLANG_PATH="$(dirname "$(dirname "$(xcrun --find clang)")")/lib"`.
  Never export `LIBCLANG_PATH=""` — a set-but-empty value makes clang-sys
  skip its default search paths entirely (this broke the first macOS CI run).
- **No `--features gpu-vulkan` on macOS.** The default macOS dependency graph
  does not compile whisper.cpp at all: `tauri-plugin-local-stt` gets its
  `whisper-cpp` feature only on Linux/Windows targets (see
  `apps/desktop/src-tauri/Cargo.toml`); macOS uses the upstream Apple-Silicon
  STT runtime (Soniqo/Argmax, Metal via mlx-swift) instead.
- Rust (stable) + Node 22 + pnpm via corepack, as on other platforms.

### Build

```sh
pnpm install
pnpm exec turbo run build --filter=@hypr/ui
pnpm -F desktop tauri build \
  --config ./src-tauri/tauri.conf.stable.json \
  --config ./src-tauri/tauri.conf.stable-macos.json
```

Config layering (tauri v2 merges in order, later wins, arrays replaced):
`tauri.conf.json` → `tauri.macos.conf.json` (auto-merged platform file; adds
the `check-permissions` externalBin) → `tauri.conf.stable.json` (version,
identifier, updater endpoint/pubkey) → `tauri.conf.stable-macos.json`
(only swaps `bundle.targets` to `["app", "dmg"]`).

Outputs land in `apps/desktop/src-tauri/target/release/bundle/`:

- `dmg/Notare_<version>_aarch64.dmg` — what users download.
- `macos/Notare.app` — the bundle itself.
- `macos/Notare.app.tar.gz` + `.sig` — the **updater** artifact (with
  `createUpdaterArtifacts` on and `TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]` set).
  `latest.json` on the release must reference the `.app.tar.gz`, never the
  `.dmg`, under the `darwin-aarch64` platform key.

### Unsigned status, Gatekeeper, and microphone permission

- Because the app is unsigned, Gatekeeper quarantines downloads: first launch
  needs **right-click → Open**, or `xattr -cr /Applications/Notare.app`.
- Microphone (and calendar/contacts) prompts still work unsigned: macOS TCC
  keys off the `Info.plist` usage strings (`NSMicrophoneUsageDescription`,
  `NSAudioCaptureUsageDescription`, …), which Tauri merges from
  `src-tauri/Info.plist` into the bundled app regardless of signing.
- `src-tauri/Entitlements.plist` (audio-input, calendars, addressbook, JIT)
  is only *applied* when the bundle is codesigned with an identity; for the
  unsigned build it is effectively inert. That is fine — entitlements of this
  kind only gate sandboxed / hardened-runtime apps.

### Future signing checklist (when certificates exist)

1. Join the Apple Developer Program; create a **Developer ID Application**
   certificate; export as `.p12`.
2. CI env: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_SIGNING_IDENTITY` (or set `bundle.macOS.signingIdentity` in the
   stable config).
3. Enable hardened runtime + notarization: `APPLE_ID`,
   `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID` — or an App Store Connect
   API key (`APPLE_API_ISSUER`/`APPLE_API_KEY`). Tauri notarizes + staples
   automatically when these are present.
4. Re-check `Entitlements.plist` — once signing with hardened runtime, the
   `com.apple.security.device.audio-input` and JIT entitlements become live
   and required for mic capture and the WebView.
5. Drop the Gatekeeper caveats from README + release body.

