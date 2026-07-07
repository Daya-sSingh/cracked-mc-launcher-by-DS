/**
 * Typed wrappers for every Tauri command and event channel used by the
 * launcher. Nothing in the frontend should call `invoke` or `listen` directly
 * — all IPC goes through this module so type-checking catches mismatches
 * between the TS types and the Rust command signatures.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  CreateInstanceRequest,
  FabricLoaderForGame,
  Instance,
  InstanceSort,
  InstanceUpdate,
  LaunchEventPayload,
  ModFileInfo,
  TauriDragDropPayload,
  VersionManifest,
} from "@/types";

// ─── Instance commands ────────────────────────────────────────────────────────

export const createInstance = (request: CreateInstanceRequest): Promise<Instance> =>
  invoke("create_instance", { request });

export const listInstances = (sort?: InstanceSort): Promise<Instance[]> =>
  invoke("list_instances", { sort: sort ?? null });

export const getInstance = (instanceId: string): Promise<Instance> =>
  invoke("get_instance", { instanceId });

export const updateInstance = (instanceId: string, update: InstanceUpdate): Promise<Instance> =>
  invoke("update_instance", { instanceId, update });

export const deleteInstance = (instanceId: string): Promise<void> =>
  invoke("delete_instance", { instanceId });

/** Opens an instance's game folder in the OS's file manager (Explorer / Finder / the default file manager on Linux). */
export const openInstanceFolder = (instanceId: string): Promise<void> =>
  invoke("open_instance_folder", { instanceId });

/** Lists every `.jar` currently in an instance's mods folder. */
export const listInstanceMods = (instanceId: string): Promise<ModFileInfo[]> =>
  invoke("list_instance_mods", { instanceId });

/**
 * Copies dropped/browsed `.jar` file paths into an instance's mods folder.
 * Non-jar paths are silently skipped by the backend. Returns how many files
 * were actually imported.
 */
export const importModFiles = (instanceId: string, filePaths: string[]): Promise<number> =>
  invoke("import_mod_files", { instanceId, filePaths });

// ─── Version manifest commands ────────────────────────────────────────────────

export const getVersionManifest = (forceRefresh = false): Promise<VersionManifest> =>
  invoke("get_version_manifest", { forceRefresh });

// ─── Fabric commands ──────────────────────────────────────────────────────────

/** Every Fabric Loader build compatible with the given Minecraft version. */
export const getFabricLoaderVersions = (gameVersion: string): Promise<FabricLoaderForGame[]> =>
  invoke("get_fabric_loader_versions", { gameVersion });

// ─── Launch commands ──────────────────────────────────────────────────────────

/**
 * Triggers a launch. Returns as soon as the background task is running —
 * actual progress comes via `listenLaunchEvents`.
 */
export const launchInstance = (instanceId: string, accountUsername: string): Promise<void> =>
  invoke("launch_instance", { instanceId, accountUsername });

export const stopInstance = (instanceId: string): Promise<void> =>
  invoke("stop_instance", { instanceId });

export const isInstanceRunning = (instanceId: string): Promise<boolean> =>
  invoke("is_instance_running", { instanceId });

export const listRunningInstances = (): Promise<string[]> =>
  invoke("list_running_instances");

// ─── Settings commands ────────────────────────────────────────────────────────

/** Retrieves a settings value. Returns null when the key has never been set. */
export const getSetting = (key: string): Promise<string | null> =>
  invoke("get_setting", { key });

/** Persists a settings value as a JSON string. */
export const setSetting = (key: string, valueJson: string): Promise<void> =>
  invoke("set_setting", { key, valueJson });

// ─── Event listeners ──────────────────────────────────────────────────────────

/**
 * Subscribes to the `launch:<instanceId>` event channel. Returns the
 * `unlisten` function — callers MUST call it when they unmount/stop caring,
 * or the listener leaks across the app lifetime.
 */
export const listenLaunchEvents = (
  instanceId: string,
  handler: (event: LaunchEventPayload) => void,
): Promise<UnlistenFn> =>
  listen<LaunchEventPayload>(`launch:${instanceId}`, (event) => handler(event.payload));

/**
 * Subscribes to OS-level file drag-and-drop over this window. Deliberately
 * NOT implemented with the browser's native `onDrop`/`ondragover` DOM
 * events — those never fire for real OS file drops inside a Tauri webview,
 * since the native window layer intercepts the drag before the webview
 * gets a chance to see it. `onDragDropEvent` is Tauri's own event for this,
 * and its payload carries real filesystem paths rather than browser File
 * objects, which is exactly what a backend command needs to copy the file.
 */
export const listenFileDragDrop = (
  handler: (event: TauriDragDropPayload) => void,
): Promise<UnlistenFn> =>
  getCurrentWebviewWindow().onDragDropEvent((event) => handler(event.payload as unknown as TauriDragDropPayload));

// ─── Window control helpers ───────────────────────────────────────────────────
// Thin wrappers so components import from one place rather than mixing
// @tauri-apps/api/window with @tauri-apps/api/core imports everywhere.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export const minimizeWindow  = (): Promise<void> => getCurrentWindow().minimize();
export const maximizeWindow  = (): Promise<void> => getCurrentWindow().toggleMaximize();
export const closeWindow     = (): Promise<void> => getCurrentWindow().close();
export const isWindowMaximized = (): Promise<boolean> => getCurrentWindow().isMaximized();
