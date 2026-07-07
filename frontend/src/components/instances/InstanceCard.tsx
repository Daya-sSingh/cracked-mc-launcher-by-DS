import { useNavigate } from "react-router-dom";
import { Play, Square, Settings as SettingsIcon, Star } from "lucide-react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/shared/Feedback";
import { Button } from "@/components/shared/Button";
import { useLaunchStore } from "@/state/launch";
import { useToggleFavorite } from "@/hooks/useInstances";
import { useLaunch } from "@/hooks/useLaunch";
import { formatPlaytime } from "@/types";
import type { Instance } from "@/types";

interface InstanceCardProps {
  instance: Instance;
}

/**
 * Visual grid card for one instance in the Library. Clicking the card body
 * navigates to that instance's detail page (Logs/Content tabs); clicking
 * the gear icon jumps straight to its settings page. Play/Stop and
 * Favorite are independent click targets that never trigger navigation.
 */
export function InstanceCard({ instance }: InstanceCardProps) {
  const navigate = useNavigate();
  const { launch, stop } = useLaunch(instance.id);
  const launchState = useLaunchStore((s) => s.launches[instance.id]);
  const isRunning = useLaunchStore((s) => s.runningIds.has(instance.id));
  const toggleFavorite = useToggleFavorite();

  const isLaunching = launchState != null && launchState.stage !== "running" && launchState.stage !== "failed";

  function handlePlayStop(e: React.MouseEvent) {
    e.stopPropagation();
    if (isRunning) {
      stop();
    } else {
      launch();
    }
  }

  function handleFavorite(e: React.MouseEvent) {
    e.stopPropagation();
    toggleFavorite.mutate({ id: instance.id, favorite: !instance.favorite });
  }

  function handleSettings(e: React.MouseEvent) {
    e.stopPropagation();
    navigate(`/instance/${instance.id}/settings`);
  }

  // Derive a deterministic card accent from the version string so each
  // vanilla world card has a slightly different hue even without a custom icon.
  const cardHue = stringToHue(instance.minecraft_version);

  return (
    <article
      onClick={() => navigate(`/instance/${instance.id}`)}
      className={cn(
        "group relative flex flex-col bg-surface border rounded-xl overflow-hidden cursor-pointer",
        "transition-all duration-150 hover:border-white/12 hover:-translate-y-0.5 hover:shadow-lg border-border",
      )}
    >
      {/* Card banner / icon area */}
      <div
        className="h-24 flex items-center justify-center relative overflow-hidden"
        style={{
          background: instance.icon
            ? undefined
            : `linear-gradient(135deg, hsl(${cardHue},25%,12%) 0%, hsl(${cardHue},20%,9%) 100%)`,
        }}
      >
        {instance.icon ? (
          <img
            src={instance.icon}
            alt=""
            className="w-full h-full object-cover"
          />
        ) : (
          <span
            className="text-3xl font-black select-none opacity-30"
            style={{ color: `hsl(${cardHue},50%,60%)` }}
          >
            {instance.name.slice(0, 2).toUpperCase()}
          </span>
        )}

        {/* Running indicator */}
        {isRunning && (
          <span className="absolute top-2 right-2 flex items-center gap-1 bg-success/20 border border-success/30 rounded-full px-1.5 py-0.5">
            <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
            <span className="text-success text-[9px] font-semibold">Running</span>
          </span>
        )}

        {/* Launching progress */}
        {isLaunching && launchState && (
          <div className="absolute inset-x-0 bottom-0 h-0.5 bg-overlay">
            <div
              className="h-full bg-accent transition-[width] duration-300"
              style={{ width: `${launchState.overallProgress * 100}%` }}
            />
          </div>
        )}

        {/* Favorite button — top-left, visible on hover */}
        <button
          onClick={handleFavorite}
          className={cn(
            "absolute top-2 left-2 w-6 h-6 rounded-full flex items-center justify-center",
            "transition-all duration-150",
            instance.favorite
              ? "opacity-100 text-amber-400"
              : "opacity-0 group-hover:opacity-100 text-muted hover:text-amber-400",
          )}
          aria-label={instance.favorite ? "Remove from favorites" : "Add to favorites"}
        >
          <Star
            size={12}
            strokeWidth={2}
            fill={instance.favorite ? "currentColor" : "none"}
          />
        </button>
      </div>

      {/* Content */}
      <div className="flex flex-col gap-1.5 p-3 flex-1">
        <div className="flex items-start justify-between gap-1">
          <h3 className="text-sm font-semibold text-primary leading-snug line-clamp-1 flex-1">
            {instance.name}
          </h3>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 shrink-0 opacity-0 group-hover:opacity-100 -mr-1 -mt-0.5"
            onClick={handleSettings}
            aria-label="Instance settings"
          >
            <SettingsIcon size={14} />
          </Button>
        </div>

        <div className="flex items-center gap-1.5 flex-wrap">
          <Badge color="muted">{instance.minecraft_version}</Badge>
          {instance.loader !== "vanilla" && (
            <Badge color="fabric">{instance.loader}</Badge>
          )}
        </div>

        <div className="flex items-center justify-between mt-auto pt-2">
          <span className="text-[10px] text-muted">
            {formatPlaytime(instance.total_playtime_seconds)}
          </span>
          <Button
            variant={isRunning ? "danger" : "primary"}
            size="sm"
            isLoading={isLaunching}
            onClick={handlePlayStop}
            className="h-7 px-3 text-xs"
            aria-label={isRunning ? "Stop instance" : "Play instance"}
          >
            {isRunning ? (
              <>
                <Square size={10} strokeWidth={2.5} className="mr-1" />
                Stop
              </>
            ) : (
              <>
                <Play size={10} strokeWidth={0} fill="currentColor" className="mr-1" />
                Play
              </>
            )}
          </Button>
        </div>
      </div>
    </article>
  );
}

function stringToHue(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash * 31 + str.charCodeAt(i)) >>> 0;
  }
  return hash % 360;
}
