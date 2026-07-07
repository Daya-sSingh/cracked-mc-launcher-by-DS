# Launcher

A fast, lightweight, cross-platform Minecraft launcher built with Tauri v2 (Rust backend) and React (TypeScript frontend).

## Features

- **Instance manager** — create, rename, update, delete, favourite instances
- **Vanilla + Fabric launch** — resolves version JSON, merges in Fabric Loader's profile when selected, downloads Java/assets/libraries automatically, verifies checksums, launches the game
- **Fabric Loader picker** — browse every Fabric build compatible with the selected Minecraft version, recommended (stable) build preselected
- **Offline mode** — launch with any username; UUID derived from name the same way vanilla does it
- **Version picker** — every Minecraft version from the Mojang manifest (releases + snapshots + old betas/alphas)
- **Download manager** — parallel transfers, resume, retry with exponential backoff, SHA-1 verification, skip-if-correct
- **Persistent settings** — theme, accent colour, memory defaults, concurrency
- **SQLite persistence** — instances, settings, version manifest cache
- **Frameless window** — custom title bar with minimise / maximise / close

## Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust | 1.77+ | `rustup install stable` |
| Node.js | 20 LTS | `nvm install 20` |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |
| Platform deps | — | See below |

### Platform system libraries

**Linux (Debian/Ubuntu)**
```bash
sudo apt-get install \
  libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

**macOS** — Xcode Command Line Tools (`xcode-select --install`)

**Windows** — Visual Studio Build Tools 2022 with the "Desktop development with C++" workload, and [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/)

## Development

```bash
# Install frontend dependencies (run once)
cd frontend && npm install && cd ..

# Start the dev server (hot-reload both frontend and backend)
cargo tauri dev
```

The app window opens automatically. Rust changes rebuild in the background;
frontend changes update instantly via Vite HMR.

### Useful dev commands

```bash
# Run all Rust tests (no network needed)
cargo test --all

# Run only the ignored integration tests (requires network)
cargo test --all -- --ignored

# Type-check the frontend without building
cd frontend && npm run typecheck

# Run clippy
cargo clippy --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check
```

## Building for distribution

```bash
cargo tauri build
```

Produces native installers in `src-tauri/target/release/bundle/`:
- `*.deb` / `*.AppImage` on Linux
- `*.dmg` / `*.app` on macOS
- `*.msi` / `*.exe` on Windows

## Project structure

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for a full breakdown of every design decision, the crate dependency graph, and the on-disk layout.

```
launcher/
├── Cargo.toml              Workspace root (shared dep versions)
├── crates/
│   ├── database/           SQLite schema, migrations, repositories
│   ├── downloads/          Parallel download manager
│   └── minecraft/          Mojang protocol, Java, launch
├── src-tauri/              Tauri shell: state, commands, IPC
├── frontend/               React + TypeScript UI
│   └── src/
│       ├── types/          TypeScript mirror of Rust serde types
│       ├── lib/tauri.ts    All invoke/listen calls
│       ├── state/          Zustand stores
│       ├── hooks/          React Query + custom hooks
│       ├── components/     UI components
│       └── pages/          Route-level pages
└── .github/workflows/ci.yml  CI: typecheck, clippy, tests, Tauri build
```

## Roadmap

| Milestone | Status | Description |
|-----------|--------|-------------|
| 1 | ✅ Done | Scaffold + vanilla launch + instance manager |
| 2 | Planned | Microsoft OAuth, multiple accounts, session refresh |
| 3 | ✅ Done | Fabric loader install |
| 4 | Planned | Modrinth integration (browse / install / update mods) |
| 5 | Planned | Mod manager UI, crash analyser, log viewer |
| 6 | Planned | CurseForge integration |
| 7 | Planned | Auto-updater, signed installers, full CI release pipeline |

## Licence

MIT
