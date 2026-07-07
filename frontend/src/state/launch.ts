import { create } from "zustand";
import type { InstanceLaunchState, LaunchEventPayload, LaunchStage, LogLine } from "@/types";
import { humanLaunchStage } from "@/types";

let logLineCounter = 0;

interface LaunchStoreState {
  // Keyed by instance id; absent = never launched this session
  launches: Record<string, InstanceLaunchState>;
  // Instance ids that currently have a live game process
  runningIds: Set<string>;

  // Actions
  initLaunch: (instanceId: string) => void;
  handleEvent: (instanceId: string, event: LaunchEventPayload) => void;
  clearLaunch: (instanceId: string) => void;
  setRunning: (ids: string[]) => void;
}

export const useLaunchStore = create<LaunchStoreState>()((set) => ({
  launches: {},
  runningIds: new Set(),

  initLaunch: (instanceId) =>
    set((state) => ({
      launches: {
        ...state.launches,
        [instanceId]: {
          instanceId,
          stage: "resolving_version",
          stageLabel: humanLaunchStage("resolving_version"),
          overallProgress: 0,
          bytesPerSec: 0,
          completedTasks: 0,
          totalTasks: 0,
          logLines: [],
          errorMessage: null,
          exitCode: null,
        } satisfies InstanceLaunchState,
      },
    })),

  handleEvent: (instanceId, event) =>
    set((state) => {
      const current = state.launches[instanceId];
      if (!current) return state;

      let patch: Partial<InstanceLaunchState> = {};

      switch (event.type) {
        case "Stage": {
          const stage = event.stage as LaunchStage;
          patch = { stage, stageLabel: humanLaunchStage(stage) };
          break;
        }

        case "AggregateProgress":
          patch = {
            overallProgress:
              event.total_tasks > 0 ? event.completed_tasks / event.total_tasks : 0,
            bytesPerSec: event.bytes_per_sec,
            completedTasks: event.completed_tasks,
            totalTasks: event.total_tasks,
          };
          break;

        case "ProcessOutput": {
          const newLine: LogLine = {
            id: ++logLineCounter,
            text: event.line,
            isStderr: event.is_stderr,
            timestamp: Date.now(),
          };
          // Keep the last 2000 lines to avoid unbounded memory growth on a
          // very long session.
          const logLines = [...current.logLines, newLine].slice(-2000);
          patch = { logLines };
          // Once we're getting process output, the stage is "running" — and
          // since there's no dedicated "process spawned" event on the wire,
          // this is also the only reliable signal the frontend gets that the
          // child process is actually alive. This is why it's also where the
          // instance joins runningIds: every other Play/Stop control in the
          // app (InstanceCard, the Home page's recent list, the instance
          // header, the sidebar's running count) reads runningIds rather
          // than this launch's own stage, so without this they'd never
          // notice a launch that started this session — staying stuck on
          // "Play" and, if clicked, resetting this very launch state back to
          // "resolving_version" via initLaunch (the backend rejects the
          // resulting duplicate launch_instance call, so no second game
          // process actually spawns, but the reset stage never reaches
          // "starting" again on its own, leaving the Logs tab frozen while
          // real output from the still-running game keeps appending below).
          if (current.stage === "starting") {
            patch.stage = "running";
            patch.stageLabel = humanLaunchStage("running");
            return {
              launches: { ...state.launches, [instanceId]: { ...current, ...patch } },
              runningIds: new Set([...state.runningIds, instanceId]),
            };
          }
          break;
        }

        case "Exited":
          patch = {
            stage: "running",
            exitCode: event.exit_code,
            stageLabel: `Exited (code ${event.exit_code ?? "?"})`,
          };
          return {
            launches: { ...state.launches, [instanceId]: { ...current, ...patch } },
            runningIds: new Set([...state.runningIds].filter((id) => id !== instanceId)),
          };

        case "Failed":
          patch = {
            stage: "failed",
            stageLabel: humanLaunchStage("failed"),
            errorMessage: event.message,
          };
          return {
            launches: { ...state.launches, [instanceId]: { ...current, ...patch } },
            runningIds: new Set([...state.runningIds].filter((id) => id !== instanceId)),
          };

        default:
          // DownloadStarted / DownloadProgress / DownloadSkipped / ... —
          // currently only AggregateProgress is rendered in the progress bar,
          // so individual file events are intentionally ignored here. A
          // future "download details" panel can subscribe to them.
          break;
      }

      return {
        launches: { ...state.launches, [instanceId]: { ...current, ...patch } },
      };
    }),

  clearLaunch: (instanceId) =>
    set((state) => {
      const next = { ...state.launches };
      delete next[instanceId];
      return { launches: next };
    }),

  setRunning: (ids) => set({ runningIds: new Set(ids) }),
}));
