import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { LauncherSettings } from "@/types";
import { DEFAULT_SETTINGS } from "@/types";
import { getSetting, setSetting } from "@/lib/tauri";

const SETTINGS_KEY = "launcher_settings";

interface SettingsState {
  settings: LauncherSettings;
  loaded: boolean;

  load: () => Promise<void>;
  update: (partial: Partial<LauncherSettings>) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>()(
  subscribeWithSelector((set, get) => ({
    settings: DEFAULT_SETTINGS,
    loaded: false,

    load: async () => {
      try {
        const raw = await getSetting(SETTINGS_KEY);
        if (raw) {
          const parsed: Partial<LauncherSettings> = JSON.parse(raw);
          set({ settings: { ...DEFAULT_SETTINGS, ...parsed }, loaded: true });
        } else {
          set({ loaded: true });
        }
      } catch {
        // Settings file might be corrupted — fall back to defaults silently.
        set({ loaded: true });
      }
    },

    update: async (partial) => {
      const next = { ...get().settings, ...partial };
      set({ settings: next });
      try {
        await setSetting(SETTINGS_KEY, JSON.stringify(next));
      } catch (err) {
        console.error("[settings] failed to persist:", err);
      }
    },
  })),
);
