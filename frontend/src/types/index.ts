/**
 * Types that exactly mirror the serde serialization of the Rust structs.
 *
 * Rules followed:
 *  - Rust `Option<T>` → TypeScript `T | null`
 *  - `#[serde(rename_all = "lowercase")]` enum → lowercase string literals
 *  - `#[serde(rename_all = "snake_case")]` enum → snake_case string literals
 *  - `#[serde(rename = "...")]` field → the renamed JSON key
 *  - `#[serde(tag = "type")]` enum → `{ type: "VariantName", ...fields }`
 *    (Rust PascalCase variant names are NOT transformed by default)
 *  - Rust `DateTime<Utc>` → ISO 8601 string
 *  - Rust `Uuid` → hyphenated UUID string
 *
 * Never import from here in the Rust code — this file is the TS-side
 * transcription of what serde produces.
 */

// ─── database::Loader ─────────────────────────────────────────────────────────
// #[serde(rename_all = "lowercase")]
export type Loader = "vanilla" | "fabric";

// ─── database::Instance ───────────────────────────────────────────────────────
// All fields snake_case (serde default), Uuid → string, DateTime → ISO string
export interface Instance {
  id: string;
  name: string;
  loader: Loader;
  loader_version: string | null;
  minecraft_version: string;
  icon: string | null;
  group_name: string | null;
  favorite: boolean;

  java_path: string | null;
  java_args: string | null;
  memory_min_mb: number;
  memory_max_mb: number;
  window_width: number;
  window_height: number;
  fullscreen: boolean;
  game_args: string | null;

  last_played_at: string | null;  // ISO 8601 | null
  total_playtime_seconds: number;
  created_at: string;
  updated_at: string;
}

// ─── commands/instances.rs CreateInstanceRequest ─────────────────────────────
// Plain Deserialize, no rename — all snake_case
export interface CreateInstanceRequest {
  name: string;
  loader: Loader;
  loader_version: string | null;
  minecraft_version: string;
  icon: string | null;
}

// ─── database::InstanceUpdate ─────────────────────────────────────────────────
// #[derive(Default, Deserialize)] — all fields optional, all snake_case
export interface InstanceUpdate {
  name?: string | null;
  icon?: string | null;
  group_name?: string | null;
  favorite?: boolean | null;
  java_path?: string | null;
  java_args?: string | null;
  memory_min_mb?: number | null;
  memory_max_mb?: number | null;
  window_width?: number | null;
  window_height?: number | null;
  fullscreen?: boolean | null;
  game_args?: string | null;
}

// ─── commands/instances.rs InstanceSortRequest ───────────────────────────────
// #[serde(rename_all = "snake_case")]
export type InstanceSort =
  | "recently_played"
  | "name_ascending"
  | "name_descending"
  | "favorites_first";

// ─── minecraft::VersionType ───────────────────────────────────────────────────
// #[serde(rename_all = "snake_case")]
export type VersionType = "release" | "snapshot" | "old_beta" | "old_alpha";

// ─── minecraft::VersionSummary ────────────────────────────────────────────────
// #[serde(rename = "type")] on version_type,
// #[serde(rename = "releaseTime")] on release_time
export interface VersionSummary {
  id: string;
  type: VersionType;        // Rust: version_type → JSON: "type"
  url: string;
  releaseTime: string;      // Rust: release_time → JSON: "releaseTime"
}

// ─── minecraft::LatestVersions ───────────────────────────────────────────────
export interface LatestVersions {
  release: string;
  snapshot: string;
}

// ─── commands/instances.rs ModFileInfo ───────────────────────────────────────
export interface ModFileInfo {
  file_name: string;
  size_bytes: number;
  modified_at: string | null;
}

// ─── Tauri's native drag-and-drop event payload ──────────────────────────────
// Defined locally (not imported from @tauri-apps/api) so this code doesn't
// depend on the exact type export name, which has moved between package
// versions. The runtime shape is stable and documented by Tauri v2's
// `onDragDropEvent` API: { type: "enter"|"over"|"drop"|"leave", paths?, position? }.
// Browser-native HTML5 drag-and-drop (onDrop/ondragover) does NOT fire for
// OS file drops inside a Tauri webview — the native window layer intercepts
// the drag before the webview ever sees it — so this event is the only way
// to receive real filesystem paths for a dropped file.
export interface TauriDragDropPayload {
  type: "enter" | "over" | "drop" | "leave";
  paths?: string[];
  position?: { x: number; y: number };
}

// ─── minecraft::VersionManifest ──────────────────────────────────────────────
export interface VersionManifest {
  latest: LatestVersions;
  versions: VersionSummary[];
}

// ─── minecraft::fabric::FabricLoaderVersion ──────────────────────────────────
// Plain Deserialize+Serialize struct, no rename — all fields snake_case-free
// (every field name is already a single lowercase word, so serde's default
// output matches these exactly).
export interface FabricLoaderVersion {
  separator: string;
  build: number;
  maven: string;
  version: string;
  stable: boolean;
}

// ─── minecraft::fabric::FabricIntermediary ───────────────────────────────────
export interface FabricIntermediary {
  maven: string;
  version: string;
  stable: boolean;
}

// ─── minecraft::fabric::FabricLoaderForGame ──────────────────────────────────
// One entry from the Fabric loader-list command — nested loader + intermediary
// details for a single compatible build.
export interface FabricLoaderForGame {
  loader: FabricLoaderVersion;
  intermediary: FabricIntermediary;
}

// ─── commands/launch.rs LaunchEventPayload ───────────────────────────────────
// #[serde(tag = "type")] — variant name is the "type" field value (PascalCase)
export type LaunchEventPayload =
  | { type: "Stage"; stage: LaunchStage }
  | { type: "DownloadStarted"; label: string; total_bytes: number | null }
  | { type: "DownloadProgress"; label: string; bytes_downloaded: number; total_bytes: number | null }
  | { type: "DownloadSkipped"; label: string }
  | { type: "DownloadRetrying"; label: string; attempt: number; error: string }
  | { type: "DownloadCompleted"; label: string }
  | { type: "DownloadFailed"; label: string; error: string }
  | {
      type: "AggregateProgress";
      completed_tasks: number;
      total_tasks: number;
      bytes_downloaded: number;
      total_bytes: number;
      bytes_per_sec: number;
    }
  | { type: "ProcessOutput"; line: string; is_stderr: boolean }
  | { type: "Exited"; exit_code: number | null; was_stopped_by_user: boolean }
  | { type: "Failed"; message: string };

// The string values match what the Rust code puts in the `stage` field of
// `LaunchEventPayload::Stage` — these are plain string literals, not an enum.
export type LaunchStage =
  | "resolving_version"
  | "downloading_files"
  | "installing_java"
  | "extracting_natives"
  | "starting";

// ─── Derived / UI-only types ──────────────────────────────────────────────────

/** Aggregated per-instance launch state maintained by the launchStore Zustand
 *  store. Not serialised over IPC — computed from a stream of events. */
export interface InstanceLaunchState {
  instanceId: string;
  stage: LaunchStage | "running" | "failed";
  stageLabel: string;
  overallProgress: number;   // 0..1 — derived from AggregateProgress events
  bytesPerSec: number;
  completedTasks: number;
  totalTasks: number;
  logLines: LogLine[];
  errorMessage: string | null;
  exitCode: number | null;
}

export interface LogLine {
  id: number;
  text: string;
  isStderr: boolean;
  timestamp: number;  // Date.now() at receipt
}

/** The subset of launcher-wide settings we persist to the `settings` table.
 *  Stored as a single JSON value under the key `"launcher_settings"`. */
export interface LauncherSettings {
  theme: "dark" | "light" | "system";
  accentColor: string;         // hex, default "#ff9a57"
  defaultMemoryMinMb: number;  // default 1024
  defaultMemoryMaxMb: number;  // default 4096
  maxConcurrentDownloads: number; // default 12
  offlineUsername: string;     // persisted for offline-mode convenience
  instanceSortOrder: InstanceSort;
}

export const DEFAULT_SETTINGS: LauncherSettings = {
  theme: "dark",
  accentColor: "#ff9a57",
  defaultMemoryMinMb: 1024,
  defaultMemoryMaxMb: 4096,
  maxConcurrentDownloads: 12,
  offlineUsername: "",
  instanceSortOrder: "recently_played",
};

// ─── Utility helpers ──────────────────────────────────────────────────────────

/** Formats `total_playtime_seconds` as "Xh Ym" or "Xm" (< 1 hour). */
export function formatPlaytime(seconds: number): string {
  if (seconds < 60) return seconds > 0 ? `${seconds}s` : "Never played";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours === 0) return `${minutes}m`;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

/** Formats bytes/sec as a human-readable speed string. */
export function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`;
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
}

/** Formats bytes as a human-readable size string. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/**
 * Picks which Fabric loader build to preselect: the newest build marked
 * `stable`, or — if a Minecraft version genuinely has no stable build yet
 * (common in the first days after a new release) — the first entry, on the
 * assumption the API returns builds newest-first. Mirrors
 * `minecraft::fabric::recommended_loader_version` on the Rust side so the
 * default shown in the UI always matches what a bare `null` loader_version
 * would have resolved to server-side.
 */
export function recommendedFabricLoaderVersion(
  candidates: FabricLoaderForGame[],
): FabricLoaderForGame | null {
  return candidates.find((c) => c.loader.stable) ?? candidates[0] ?? null;
}

export function humanLaunchStage(stage: LaunchStage | "running" | "failed"): string {
  switch (stage) {
    case "resolving_version": return "Resolving version…";
    case "downloading_files": return "Downloading files…";
    case "installing_java":   return "Installing Java…";
    case "extracting_natives":return "Extracting natives…";
    case "starting":          return "Starting game…";
    case "running":           return "Running";
    case "failed":            return "Failed";
  }
}
