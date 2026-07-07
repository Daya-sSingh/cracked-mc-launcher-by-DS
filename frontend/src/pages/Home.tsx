import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Play, Plus, Clock, Cpu, Package } from "lucide-react";
import { useInstanceStore, selectFilteredInstances } from "@/state/instances";
import { useShallow } from "zustand/react/shallow";
import { useInstances } from "@/hooks/useInstances";
import { useLaunch } from "@/hooks/useLaunch";
import { useLaunchStore } from "@/state/launch";
import { CreateInstanceModal } from "@/components/instances/CreateInstanceModal";
import { Button } from "@/components/shared/Button";
import { Badge } from "@/components/shared/Feedback";
import { cn } from "@/lib/utils";
import { formatPlaytime } from "@/types";
import type { Instance } from "@/types";

// ─── Recent instance card ──────────────────────────────────────────────────────

function RecentCard({ instance }: { instance: Instance }) {
  const navigate = useNavigate();
  const { launch, launchState, isPending } = useLaunch(instance.id);
  const isRunning = useLaunchStore((s) => s.runningIds.has(instance.id));
  const isLaunching = launchState != null && launchState.stage !== "running" && launchState.stage !== "failed";

  function handlePlay(e: React.MouseEvent) {
    e.stopPropagation();
    if (!isRunning) launch();
    // Launching stays inline here (mirrors the Library grid's cards) — the
    // full stage/log view is always one click away on the instance's own
    // page, reachable via the row click below, so Play never has to pop
    // anything up in front of the rest of the app.
  }

  return (
    <div
      className={cn(
        "group relative flex items-center gap-3 p-3 rounded-xl border bg-surface",
        "transition-all duration-150 hover:border-white/10 hover:bg-elevated cursor-pointer",
        isRunning ? "border-success/30" : "border-border",
      )}
      onClick={() => navigate(`/instance/${instance.id}`)}
    >
      {/* Icon */}
      <div className="w-10 h-10 rounded-lg bg-elevated flex items-center justify-center flex-shrink-0 text-muted font-black text-sm">
        {instance.icon
          ? <img src={instance.icon} alt="" className="w-full h-full object-cover rounded-lg" />
          : instance.name.slice(0, 2).toUpperCase()
        }
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <p className="text-sm font-semibold text-primary truncate">{instance.name}</p>
        <div className="flex items-center gap-1.5 mt-0.5">
          <span className="text-xs text-muted">{instance.minecraft_version}</span>
          {instance.loader !== "vanilla" && (
            <Badge color="fabric" className="text-[9px]">{instance.loader}</Badge>
          )}
        </div>
      </div>

      {/* Play / running indicator */}
      <div className="flex items-center gap-2 shrink-0">
        {isRunning && (
          <span className="flex items-center gap-1 text-[10px] text-success font-medium">
            <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
            Running
          </span>
        )}
        <Button
          variant={isRunning ? "secondary" : "primary"}
          size="sm"
          isLoading={(isLaunching || isPending) && !isRunning}
          onClick={handlePlay}
          className="h-7 w-7 p-0"
          aria-label={isRunning ? "View" : "Play"}
        >
          <Play size={10} fill="currentColor" strokeWidth={0} />
        </Button>
      </div>
    </div>
  );
}

// ─── Stat card ────────────────────────────────────────────────────────────────

function StatCard({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="flex items-center gap-3 p-3.5 rounded-xl bg-surface border border-border">
      <div className="w-8 h-8 rounded-lg bg-elevated flex items-center justify-center text-muted shrink-0">
        {icon}
      </div>
      <div>
        <p className="text-[10px] text-muted uppercase tracking-wide">{label}</p>
        <p className="text-sm font-semibold text-primary">{value}</p>
      </div>
    </div>
  );
}

// ─── Page ────────────────────────────────────────────────────────────────────

export default function HomePage() {
  useInstances();
  const allInstances = useInstanceStore(useShallow(selectFilteredInstances));
  const runningCount = useLaunchStore((s) => s.runningIds.size);
  const [createOpen, setCreateOpen] = useState(false);

  // Instances with last_played_at, sorted newest-first — the "continue playing" section
  const recentlyPlayed = allInstances
    .filter((i) => i.last_played_at !== null)
    .slice(0, 4);

  const totalPlaytime = allInstances.reduce(
    (acc, i) => acc + i.total_playtime_seconds,
    0,
  );

  const fabricCount = allInstances.filter((i) => i.loader === "fabric").length;

  return (
    <div className="h-full overflow-y-auto scrollbar-thin">
      <div className="max-w-2xl mx-auto px-6 py-8 flex flex-col gap-8">

        {/* Welcome header */}
        <div>
          <h1 className="text-2xl font-bold text-primary">
            {recentlyPlayed.length > 0 ? "Welcome back" : "Welcome to Launcher"}
          </h1>
          <p className="text-sm text-secondary mt-1">
            {runningCount > 0
              ? `${runningCount} instance${runningCount > 1 ? "s" : ""} currently running`
              : allInstances.length > 0
                ? `${allInstances.length} instance${allInstances.length > 1 ? "s" : ""} in your library`
                : "Create your first instance to get started."}
          </p>
        </div>

        {/* Continue playing */}
        {recentlyPlayed.length > 0 && (
          <section className="flex flex-col gap-3">
            <SectionHeading>Continue Playing</SectionHeading>
            <div className="flex flex-col gap-2">
              {recentlyPlayed.map((instance) => (
                <RecentCard key={instance.id} instance={instance} />
              ))}
            </div>
          </section>
        )}

        {/* Quick actions */}
        {allInstances.length === 0 && (
          <div className="flex flex-col items-center gap-4 py-10 text-center">
            <div className="w-16 h-16 rounded-2xl bg-elevated flex items-center justify-center">
              <Package size={32} className="text-muted" />
            </div>
            <div>
              <p className="font-semibold text-primary">No instances yet</p>
              <p className="text-sm text-secondary mt-1">
                Create an instance to start playing Minecraft.
              </p>
            </div>
            <Button
              variant="primary"
              leftIcon={<Plus size={15} />}
              onClick={() => setCreateOpen(true)}
            >
              Create Instance
            </Button>
          </div>
        )}

        {/* Stats row */}
        {allInstances.length > 0 && (
          <section className="flex flex-col gap-3">
            <SectionHeading>Overview</SectionHeading>
            <div className="grid grid-cols-3 gap-3">
              <StatCard
                icon={<Package size={16} />}
                label="Instances"
                value={allInstances.length.toString()}
              />
              <StatCard
                icon={<Clock size={16} />}
                label="Total Playtime"
                value={formatPlaytime(totalPlaytime)}
              />
              <StatCard
                icon={<Cpu size={16} />}
                label="Fabric Instances"
                value={fabricCount.toString()}
              />
            </div>
          </section>
        )}

        <CreateInstanceModal
          open={createOpen}
          onClose={() => setCreateOpen(false)}
        />
      </div>
    </div>
  );
}

function SectionHeading({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <h2 className="text-xs font-semibold text-muted uppercase tracking-wider">{children}</h2>
      <div className="flex-1 h-px bg-border" />
    </div>
  );
}
