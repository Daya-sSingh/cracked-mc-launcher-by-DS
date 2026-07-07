# Architecture

## Overview

```
launcher/
├── Cargo.toml                  Workspace root
├── src-tauri/                  Tauri application shell (Rust)
│   └── src/
│       ├── lib.rs              App setup, state wiring, command registration
│       ├── state.rs            AppState: shared repos, paths, HTTP, DL manager
│       └── commands/           One file per concern
│           ├── instances.rs    Instance CRUD
│           ├── launch.rs       Game launch lifecycle + IPC event forwarding
│           ├── versions.rs     Mojang version manifest (with SQLite cache)
│           └── settings.rs     Generic key/value settings persistence
├── crates/
│   ├── database/               SQLite schema, migrations, repository pattern
│   ├── downloads/              Parallel download manager (generic)
│   └── minecraft/              Mojang protocol, Java management, launcher
└── frontend/                   Tauri v2 webview (React + TypeScript)
    └── src/
        ├── types/index.ts      TypeScript mirror of Rust serde types
        ├── lib/tauri.ts        All invoke/listen calls — only IPC entry point
        ├── state/              Zustand stores (instances, launch, settings)
        ├── hooks/              React Query + Tauri hooks
        ├── components/         Shared UI + page-specific components
        └── pages/              Route-level page components
```

## Crate Dependency Graph

```
src-tauri
  ├── database   (repositories, schema, models)
  ├── downloads  (DownloadManager, DownloadTask, DownloadEvent)
  └── minecraft  (manifest, assets, libraries, java, launch)
        └── downloads  (builds DownloadTask lists, delegates actual I/O)
```

`minecraft` never imports `database` — the launcher core knows nothing about
how instances are persisted. `src-tauri` is the only crate that holds both.

## Key Design Decisions

### Repository Pattern (database crate)
Commands depend on `Arc<dyn InstanceRepository>`, not on
`SqliteInstanceRepository`. This means:
- Swapping the backend (Postgres, cloud sync) only requires a new impl.
- Unit-testing commands with a mock repository requires no `#[cfg(test)]`
  hacks in production code.

### Shared vs Isolated on Disk
| Path | Shared across all instances? | Why |
|---|---|---|
| `cache/libraries/` | Yes | Maven JARs are content-addressed; sharing is free. |
| `cache/assets/objects/` | Yes | SHA-1 content-addressed; identical objects appear in many versions. |
| `cache/java/` | Yes | 200-400 MB per runtime; duplicating per instance is wasteful. |
| `instances/<id>/minecraft/` | No | Saves, mods, config, options.txt — must be isolated per the spec. |
| `instances/<id>/natives/<ver>/` | No | Cheap to extract; avoids version-to-version conflicts. |

### Event-Driven Launch Flow
`minecraft::launch` returns a `LaunchHandle` almost immediately (as soon as
the OS has accepted the `fork()`/`CreateProcess` call). Every subsequent event
— download progress, log lines, exit code — arrives asynchronously via an
`UnboundedSender<LaunchEvent>`. The Tauri command layer maps these to
`launch:<instance_id>` Tauri events, which the frontend's `useLaunch` hook
subscribes to via `listen()`.

This decoupling means:
- The Tauri command returns in milliseconds, never holding up IPC.
- The frontend can always be rebuilt/reconnected without killing the game.
- Multiple UI components can independently listen to the same launch.

### Why No Electron
Tauri's webview shell uses the OS-native renderer (WebKit on macOS/Linux,
WebView2 on Windows), with the Rust backend instead of a Node.js process.
The result is ~10-20× lower idle RAM than an equivalent Electron app and a
noticeably faster cold-start — both explicit goals from the spec.

### Version Manifest Caching
The backend caches the Mojang version manifest in `settings` (SQLite) with a
1-hour TTL. The frontend's React Query layer adds a second client-side cache
with the same TTL. Stale-while-revalidate means the UI is never blocked
waiting for a network round-trip when the user opens the create-instance
dialog.

## Fabric Loader Support

Fabric's `.../profile/json` endpoint doesn't return a self-contained version
JSON — it returns a *delta* on top of the vanilla version it targets: a
different main class, an appended library list (the loader itself,
intermediary mappings, ASM, Mixin, ...), and optionally extra JVM/game
arguments. `crates/minecraft/src/fabric.rs` fetches that delta and merges it
onto the already-resolved vanilla `VersionDetail`, producing an ordinary
`VersionDetail` that `launch::launch` consumes identically to a plain
vanilla one — the launch pipeline has no loader-specific branching beyond
the single merge step at the top of `launch()`.

Two correctness details worth calling out:
- **The client jar is always cached under the vanilla version id**, never
  the merged Fabric id — the jar itself is unmodified vanilla bytes, so
  every Fabric instance (at any loader version) shares one cached copy with
  every other instance on that Minecraft version, vanilla or Fabric alike.
- **Library version conflicts are resolved by last-write-wins**:
  `libraries::dedupe_libraries_preferring_last` collapses the combined
  vanilla+Fabric library list down to one entry per Maven `group:artifact`,
  keeping whichever version appears later (Fabric's, since its libraries are
  appended after vanilla's). Without this, a library present in both lists
  at different versions would put two jars of the same class on the
  classpath, and which one the JVM actually loads would depend on classpath
  order — a bug that would only sometimes manifest.
  **The dedup key must preserve platform classifiers, not just
  group:artifact.** Minecraft 1.19+ lists each platform's native LWJGL
  variant as its own library entry with the classifier baked into the
  coordinate (`org.lwjgl:lwjgl-glfw:3.3.3:natives-windows`, `...:natives-linux`,
  `...:natives-macos`, ...). An earlier version of the key derivation
  discarded the classifier along with the version, which collapsed every
  platform variant of a module into whichever happened to be listed last —
  silently dropping every other platform's native jar from the classpath,
  including the one the current OS actually needed. This only affected
  Fabric instances, since vanilla-only launches never run the merge/dedup
  step at all.

`minecraft::Loader` is a separate enum from `database::Loader` (same two
variants) rather than a shared type, since `minecraft` must not depend on
`database` — `src-tauri/src/commands/launch.rs` is the one place that
converts between them.

## Future Loader Extension Point (Forge / NeoForge / Quilt)

Adding another loader later requires:
1. A new migration adding the variant to the `instances.loader` CHECK constraint.
2. A new arm in `database::Loader::from_str` / `Loader::as_str`.
3. A new module in `crates/minecraft/src/` (e.g. `forge.rs`) mirroring
   `fabric.rs`'s fetch-and-merge shape for that loader's install logic.
4. A new arm in `minecraft::Loader` and in `launch::launch`'s loader match,
   and in `src-tauri`'s `to_minecraft_loader` boundary conversion.

Nothing else changes — the frontend's loader picker, the database schema
beyond the CHECK constraint, and the download/launch machinery are already
loader-agnostic.

## Instance Navigation Model

Instances are no longer managed through modals opened from the library grid
— clicking an instance card navigates to `/instance/:id`, a dedicated page
with `Logs` and `Content` tabs; its gear icon (or the card's own gear
button) navigates to `/instance/:id/settings`, a sidebar-tabbed settings
page (General / Installation / Window / Java and memory / Game Arguments).

This replaced an earlier version where launching opened a blocking
`LaunchOverlay` modal and settings opened in a scrolling drawer modal. The
modal approach meant a launch's progress/log view had to be dismissed
before you could do anything else in the app. Now:
- `LaunchStatusPanel` (`components/instances/LaunchStatusPanel.tsx`) renders
  the same stage-tracker/progress-bar/log-viewer UI as a plain embeddable
  panel, living inside the instance page's Logs tab — non-blocking, and
  reachable any time by navigating back to that instance.
- Playing from the Library grid or Home's "Continue Playing" list still
  launches inline (the card shows its own small progress bar) without
  forcing navigation anywhere; the full log view is opt-in.
- Deleting an instance (`ConfirmDialog` + the already-existing
  `delete_instance` command) lives on the General tab of the settings page.

### Mod folder access (pre-mod-browser)

Until Modrinth/CurseForge integration lands, `Content` tab gives three ways
to get a mod into an instance: a real drag-and-drop zone, an "Open Folder"
button, and a read-only list of what's already there (`list_instance_mods`).
Drag-and-drop is implemented via Tauri's own `onDragDropEvent` window event
(`lib/tauri.ts`'s `listenFileDragDrop`), not the browser's native
`onDrop`/`ondragover` — the OS-level drag is intercepted by Tauri's window
layer before the webview's DOM ever sees a `drop` event, so the browser API
silently never fires for real file drops in this environment. The native
event's payload carries real filesystem paths, which `import_mod_files`
copies directly into the instance's `mods` folder without ever touching
file contents in JS.

## Mojang Protocol Reference

- Version manifest v2: `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json`
- Asset objects: `https://resources.download.minecraft.net/<hh>/<hash>`
- Java runtime manifest: `https://launchermeta.mojang.com/v1/products/java-runtime/...`
- Library host: `https://libraries.minecraft.net/`

The `version_detail.rs` structs were built against the live JSON for versions
`1.21.11` (modern structured arguments), `1.12.2` (legacy `minecraftArguments`
flat string), and `1.7.10` (legacy asset layout + LWJGL2 native classifiers).
