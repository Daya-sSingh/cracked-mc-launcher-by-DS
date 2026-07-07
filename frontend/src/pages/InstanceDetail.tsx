import { useState, useEffect, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import {
  Play,
  Square,
  Settings as SettingsIcon,
  ArrowLeft,
  Terminal,
  Package,
  FolderOpen,
  Compass,
  UploadCloud,
} from "lucide-react";
import { useInstance, useInstanceMods, useImportModFiles } from "@/hooks/useInstances";
import { useLaunch } from "@/hooks/useLaunch";
import { useLaunchStore } from "@/state/launch";
import { LaunchStatusPanel } from "@/components/instances/LaunchStatusPanel";
import { Button } from "@/components/shared/Button";
import { Badge, Spinner, EmptyState } from "@/components/shared/Feedback";
import { openInstanceFolder, listenFileDragDrop } from "@/lib/tauri";
import { formatBytes, formatPlaytime } from "@/types";

type Tab = "logs" | "content";

export default function InstanceDetailPage() {
  const { id = "" } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>("logs");
  const [isDragOver, setIsDragOver] = useState(false);
  const [importMessage, setImportMessage] = useState<string | null>(null);
  const [comingSoonMessage, setComingSoonMessage] = useState(false);

  const { data: instance, isLoading } = useInstance(id);
  const { launch, stop, launchState, isPending } = useLaunch(id);
  const isRunning = useLaunchStore((s) => s.runningIds.has(id));

  const { data: mods, isLoading: modsLoading, refetch: refetchMods } = useInstanceMods(id, tab === "content");
  const importModsMutation = useImportModFiles();

  // Drag-and-drop import — only armed while the Content tab is open. Uses
  // Tauri's native window-level drag-drop event rather than the browser's
  // onDrop, which never fires for OS file drops inside a Tauri webview.
  useEffect(() => {
    if (tab !== "content" || !id) return;
    let unlisten: (() => void) | undefined;
    let mounted = true;

    listenFileDragDrop((event) => {
      if (!mounted) return;

      if (event.type === "enter" || event.type === "over") {
        setIsDragOver(true);
      } else if (event.type === "leave") {
        setIsDragOver(false);
      } else if (event.type === "drop") {
        setIsDragOver(false);
        const jarPaths = (event.paths ?? []).filter((p) => p.toLowerCase().endsWith(".jar"));
        if (jarPaths.length === 0) return;

        importModsMutation.mutate(
          { instanceId: id, filePaths: jarPaths },
          {
            onSuccess: (count) => {
              setImportMessage(count === 1 ? "1 mod added" : `${count} mods added`);
              setTimeout(() => setImportMessage(null), 2500);
            },
          },
        );
      }
    }).then((fn) => {
      if (!mounted) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [tab, id, importModsMutation]);

  const handleOpenFolder = useCallback(async () => {
    if (!id) return;
    await openInstanceFolder(id);
  }, [id]);

  const handleBrowseModsClick = useCallback(() => {
    setComingSoonMessage(true);
    setTimeout(() => setComingSoonMessage(false), 3000);
  }, []);

  if (isLoading || !instance) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner size={24} />
      </div>
    );
  }

  function handlePlayStop() {
    if (isRunning) stop();
    else launch();
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-4 px-5 py-4 border-b border-border flex-shrink-0">
        <Button variant="ghost" size="icon" onClick={() => navigate("/library")} aria-label="Back to Library">
          <ArrowLeft size={16} />
        </Button>

        <div className="w-12 h-12 rounded-lg bg-elevated flex items-center justify-center text-muted font-black text-sm shrink-0 overflow-hidden">
          {instance.icon ? (
            <img src={instance.icon} alt="" className="w-full h-full object-cover" />
          ) : (
            instance.name.slice(0, 2).toUpperCase()
          )}
        </div>

        <div className="flex-1 min-w-0">
          <h1 className="text-lg font-bold text-primary truncate">{instance.name}</h1>
          <div className="flex items-center gap-2 mt-0.5">
            <span className="text-xs text-muted">{instance.minecraft_version}</span>
            {instance.loader !== "vanilla" && <Badge color="fabric">{instance.loader}</Badge>}
            <span className="text-xs text-muted">· {formatPlaytime(instance.total_playtime_seconds)}</span>
          </div>
        </div>

        <Button
          variant={isRunning ? "danger" : "primary"}
          onClick={handlePlayStop}
          isLoading={isPending && !isRunning}
          leftIcon={
            isRunning ? <Square size={13} strokeWidth={2.5} /> : <Play size={13} fill="currentColor" strokeWidth={0} />
          }
        >
          {isRunning ? "Stop" : "Play"}
        </Button>

        <Button
          variant="secondary"
          size="icon"
          onClick={() => navigate(`/instance/${id}/settings`)}
          aria-label="Instance Settings"
        >
          <SettingsIcon size={16} />
        </Button>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1 px-5 pt-3 border-b border-border flex-shrink-0">
        <TabButton active={tab === "logs"} onClick={() => setTab("logs")} icon={<Terminal size={13} />} label="Logs" />
        <TabButton active={tab === "content"} onClick={() => setTab("content")} icon={<Package size={13} />} label="Content" />
      </div>

      {/* Tab body */}
      <div className="flex-1 overflow-hidden p-5">
        {tab === "logs" && (
          <LaunchStatusPanel state={launchState} onStop={stop} onLaunch={() => launch()} isLaunching={isPending} />
        )}

        {tab === "content" && (
          <div className="flex flex-col h-full gap-3">
            {/* Toolbar */}
            <div className="flex items-center justify-between flex-shrink-0">
              <span className="text-xs font-semibold text-muted uppercase tracking-wider">
                Mods {mods ? `(${mods.length})` : ""}
              </span>
              <div className="flex items-center gap-2">
                {importMessage && (
                  <span className="text-xs text-success animate-fade-in">{importMessage}</span>
                )}
                <Button variant="ghost" size="sm" onClick={() => refetchMods()}>
                  Refresh
                </Button>
                <Button variant="secondary" size="sm" leftIcon={<FolderOpen size={13} />} onClick={handleOpenFolder}>
                  Open Folder
                </Button>
                <Button variant="primary" size="sm" leftIcon={<Compass size={13} />} onClick={handleBrowseModsClick}>
                  Browse Mods
                </Button>
              </div>
            </div>

            {comingSoonMessage && (
              <p className="text-xs text-accent bg-accent/10 border border-accent/25 rounded-lg px-3 py-2 flex-shrink-0 animate-fade-in">
                Browsing and installing mods from Modrinth &amp; CurseForge is coming in a future update.
                For now, drop a mod .jar below or use Open Folder.
              </p>
            )}

            {/* Drop zone */}
            <div
              className={`flex-shrink-0 flex items-center justify-center gap-2.5 rounded-xl border-2 border-dashed px-4 py-5 transition-colors duration-150 ${
                isDragOver
                  ? "border-accent bg-accent/10 text-accent"
                  : "border-border text-muted"
              }`}
            >
              <UploadCloud size={18} />
              <span className="text-sm font-medium">
                {isDragOver ? "Release to add mod" : "Drag & drop a mod .jar here to install it"}
              </span>
            </div>

            {/* Mod list */}
            <div className="flex-1 overflow-y-auto scrollbar-thin min-h-0">
              {modsLoading ? (
                <div className="flex items-center justify-center h-full">
                  <Spinner size={20} />
                </div>
              ) : !mods || mods.length === 0 ? (
                <EmptyState
                  icon={<Package size={32} />}
                  title="No mods installed"
                  description="Drag a mod .jar file into the drop zone above, or use Open Folder to add one manually."
                />
              ) : (
                <div className="flex flex-col gap-1.5">
                  {mods.map((mod) => (
                    <div
                      key={mod.file_name}
                      className="flex items-center justify-between px-3 py-2 bg-surface border border-border rounded-lg"
                    >
                      <span className="text-sm text-primary truncate selectable">{mod.file_name}</span>
                      <span className="text-xs text-muted shrink-0 ml-3">{formatBytes(mod.size_bytes)}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
        active ? "border-accent text-accent" : "border-transparent text-muted hover:text-secondary"
      }`}
    >
      {icon}
      {label}
    </button>
  );
}
