import { useCallback, useEffect, useRef } from "react";
import { useMutation } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";

import * as ipc from "@/lib/tauri";
import { useLaunchStore } from "@/state/launch";
import { useSettingsStore } from "@/state/settings";

/**
 * Returns a `launch` function and the current launch state for a given
 * instance. Also sets up (and tears down) the Tauri event listener that
 * drives the launch store.
 */
export function useLaunch(instanceId: string) {
  const { initLaunch, handleEvent, clearLaunch } = useLaunchStore();
  const launchState = useLaunchStore((s) => s.launches[instanceId] ?? null);
  const offlineUsername = useSettingsStore((s) => s.settings.offlineUsername);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // Attach the event listener as soon as we know this instance might launch.
  useEffect(() => {
    let mounted = true;

    ipc.listenLaunchEvents(instanceId, (event) => {
      if (!mounted) return;
      handleEvent(instanceId, event);
    }).then((unlisten) => {
      if (!mounted) {
        unlisten();
        return;
      }
      unlistenRef.current = unlisten;
    });

    return () => {
      mounted = false;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, [instanceId, handleEvent]);

  const mutation = useMutation({
    mutationFn: async (username?: string) => {
      const name = username ?? offlineUsername;
      if (!name.trim()) throw new Error("Enter a username to play in offline mode.");
      initLaunch(instanceId);
      await ipc.launchInstance(instanceId, name.trim());
    },
  });

  const launch = useCallback(
    (username?: string) => mutation.mutate(username),
    [mutation],
  );

  const stop = useCallback(() => ipc.stopInstance(instanceId), [instanceId]);

  const clear = useCallback(() => clearLaunch(instanceId), [instanceId, clearLaunch]);

  return { launch, stop, clear, launchState, isPending: mutation.isPending, error: mutation.error };
}
