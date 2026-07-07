import { useRef, useEffect, useState } from "react";
import { CheckCircle, AlertCircle, Terminal, Copy, Check, Play } from "lucide-react";
import { ProgressBar, Spinner, EmptyState } from "@/components/shared/Feedback";
import { Button } from "@/components/shared/Button";
import { formatSpeed, humanLaunchStage } from "@/types";
import type { InstanceLaunchState, LaunchStage } from "@/types";

const STAGE_ORDER: LaunchStage[] = [
  "resolving_version",
  "downloading_files",
  "installing_java",
  "extracting_natives",
  "starting",
];

interface LaunchStatusPanelProps {
  /** `null` means this instance hasn't been launched this session — shows an empty state with its own Play button instead of stage/log UI. */
  state: InstanceLaunchState | null;
  onStop: () => void;
  onLaunch: () => void;
  isLaunching: boolean;
}

/**
 * Everything about watching a launch happen — stage progress, the overall
 * download bar, the outcome banner, and the live/finished game log — as a
 * plain embeddable panel rather than a `Modal`. This is what replaced the
 * old launch overlay: logs now live inside the instance detail page's Logs
 * tab, where they persist and stay reachable for as long as the instance is
 * open, instead of popping up as a dialog that has to be dismissed before
 * you can do anything else in the app.
 */
export function LaunchStatusPanel({ state, onStop, onLaunch, isLaunching }: LaunchStatusPanelProps) {
  const logRef = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [state?.logLines.length]);

  async function handleCopyLog() {
    if (!state) return;
    const text = state.logLines.map((line) => line.text).join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // The log viewer's text is still fully selectable as a fallback if
      // the Clipboard API is unavailable in this context.
    }
  }

  if (!state) {
    return (
      <EmptyState
        icon={<Terminal size={32} />}
        title="No logs yet"
        description="Launch this instance to see live progress and game output here."
        action={
          <Button
            variant="primary"
            isLoading={isLaunching}
            leftIcon={<Play size={13} fill="currentColor" strokeWidth={0} />}
            onClick={onLaunch}
          >
            Play
          </Button>
        }
      />
    );
  }

  const isDone = state.stage === "running" && state.exitCode !== null;
  const isFailed = state.stage === "failed";
  const isRunning = state.stage === "running" && state.exitCode === null;
  const currentStageIndex = STAGE_ORDER.indexOf(state.stage as LaunchStage);

  return (
    <div className="flex flex-col gap-4 h-full">
      {/* Stage track */}
      {!isRunning && !isDone && !isFailed && (
        <div className="flex items-center gap-2 flex-shrink-0">
          {STAGE_ORDER.map((stage, i) => {
            const isCompleted = i < currentStageIndex;
            const isCurrent = i === currentStageIndex;
            return (
              <div key={stage} className="flex items-center gap-2 flex-1 last:flex-none">
                <div className="flex flex-col items-center gap-1">
                  <div
                    className={`w-5 h-5 rounded-full flex items-center justify-center transition-colors ${
                      isCompleted ? "bg-success text-white"
                      : isCurrent  ? "bg-accent text-[#1a0e05]"
                      : "bg-overlay text-muted"
                    }`}
                  >
                    {isCompleted ? (
                      <CheckCircle size={12} strokeWidth={2.5} />
                    ) : isCurrent ? (
                      <Spinner size={10} />
                    ) : (
                      <span className="w-1.5 h-1.5 rounded-full bg-current" />
                    )}
                  </div>
                  <span className="text-[9px] text-muted hidden sm:block text-center w-16 leading-tight">
                    {humanLaunchStage(stage)}
                  </span>
                </div>
                {i < STAGE_ORDER.length - 1 && (
                  <div className={`flex-1 h-px mb-4 ${isCompleted ? "bg-success/40" : "bg-border"}`} />
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Overall progress bar (while downloading) */}
      {state.stage === "downloading_files" && state.totalTasks > 0 && (
        <div className="flex flex-col gap-1.5 flex-shrink-0">
          <ProgressBar value={state.overallProgress} label="Overall download progress" />
          <div className="flex justify-between text-[10px] text-muted">
            <span>{state.completedTasks} / {state.totalTasks} files</span>
            <span>{formatSpeed(state.bytesPerSec)}</span>
          </div>
        </div>
      )}

      {/* Outcome */}
      {isDone && (
        <div className="flex items-center justify-between gap-2.5 p-3 bg-elevated border border-border rounded-lg flex-shrink-0">
          <div className="flex items-center gap-2.5">
            <CheckCircle size={18} className="text-success shrink-0" />
            <div>
              <p className="text-sm font-medium text-primary">Session ended</p>
              <p className="text-xs text-muted">Exit code: {state.exitCode ?? "?"}</p>
            </div>
          </div>
          <Button
            variant="primary"
            size="sm"
            isLoading={isLaunching}
            leftIcon={<Play size={12} fill="currentColor" strokeWidth={0} />}
            onClick={onLaunch}
          >
            Play Again
          </Button>
        </div>
      )}

      {isFailed && (
        <div className="flex items-start gap-2.5 p-3 bg-danger/10 border border-danger/25 rounded-lg flex-shrink-0">
          <AlertCircle size={18} className="text-danger shrink-0 mt-0.5" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-semibold text-danger">Launch failed</p>
            <p className="text-xs text-secondary mt-0.5 break-words selectable">
              {state.errorMessage ?? "An unknown error occurred."}
            </p>
          </div>
          <Button variant="secondary" size="sm" isLoading={isLaunching} onClick={onLaunch}>
            Retry
          </Button>
        </div>
      )}

      {isRunning && (
        <div className="flex items-center justify-between gap-2.5 p-3 bg-success/10 border border-success/25 rounded-lg flex-shrink-0">
          <div className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
            <p className="text-sm font-medium text-success">Running</p>
          </div>
          <Button variant="danger" size="sm" onClick={onStop}>
            Stop Game
          </Button>
        </div>
      )}

      {/* Log viewer — takes up the remaining space in the tab */}
      {state.logLines.length > 0 && (
        <div className="flex flex-col gap-1.5 flex-1 min-h-0">
          <div className="flex items-center justify-between flex-shrink-0">
            <div className="flex items-center gap-1.5 text-muted">
              <Terminal size={12} />
              <span className="text-[10px] font-medium">Game Log ({state.logLines.length} lines)</span>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-[10px]"
              onClick={handleCopyLog}
              leftIcon={copied ? <Check size={11} /> : <Copy size={11} />}
            >
              {copied ? "Copied!" : "Copy Log"}
            </Button>
          </div>
          <div
            ref={logRef}
            className="flex-1 min-h-0 overflow-y-auto scrollbar-thin bg-base border border-border rounded-lg p-2 font-mono text-[10px] leading-relaxed selectable"
          >
            {state.logLines.map((line) => (
              <div key={line.id} className={line.isStderr ? "text-danger/80" : "text-secondary"}>
                {line.text}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
