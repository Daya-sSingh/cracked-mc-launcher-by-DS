import { useEffect } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AppRouter } from "./router";
import { useSettingsStore } from "@/state/settings";
import { useLaunchStore } from "@/state/launch";
import { listRunningInstances } from "@/lib/tauri";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,
      refetchOnWindowFocus: false,
    },
    mutations: {
      onError: (error) => {
        console.error("[mutation error]", error);
      },
    },
  },
});

/**
 * Runs once on mount: loads persisted settings from SQLite and syncs the
 * running-instance set from the backend (relevant after a hot-reload in dev
 * mode where the frontend re-initialises but the backend process stays up).
 */
function AppInit() {
  const loadSettings = useSettingsStore((s) => s.load);
  const setRunning   = useLaunchStore((s) => s.setRunning);

  useEffect(() => {
    loadSettings();
    listRunningInstances()
      .then(setRunning)
      .catch(() => {});
  }, []);  // eslint-disable-line react-hooks/exhaustive-deps

  return null;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppInit />
      <AppRouter />
    </QueryClientProvider>
  );
}
