import { cn } from "@/lib/utils";
import { Loader2 } from "lucide-react";

// ─── ProgressBar ──────────────────────────────────────────────────────────────

interface ProgressBarProps {
  value: number;          // 0..1
  className?: string;
  animated?: boolean;
  color?: "accent" | "success" | "danger";
  height?: string;        // Tailwind height class, default "h-1.5"
  label?: string;         // Screen-reader label
}

export function ProgressBar({
  value,
  className,
  animated = true,
  color = "accent",
  height = "h-1.5",
  label,
}: ProgressBarProps) {
  const colorClass = {
    accent:  "bg-accent",
    success: "bg-success",
    danger:  "bg-danger",
  }[color];

  const percentage = Math.min(Math.max(value * 100, 0), 100);

  return (
    <div
      role="progressbar"
      aria-valuenow={Math.round(percentage)}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={label}
      className={cn("w-full bg-overlay rounded-full overflow-hidden", height, className)}
    >
      <div
        className={cn(
          "h-full rounded-full",
          colorClass,
          animated && "transition-[width] duration-300 ease-out",
        )}
        style={{ width: `${percentage}%` }}
      />
    </div>
  );
}

// ─── Spinner ──────────────────────────────────────────────────────────────────

interface SpinnerProps {
  size?: number;
  className?: string;
  label?: string;
}

export function Spinner({ size = 20, className, label = "Loading…" }: SpinnerProps) {
  return (
    <span
      role="status"
      aria-label={label}
      className={cn("inline-flex text-accent", className)}
    >
      <Loader2 size={size} className="animate-spin" />
    </span>
  );
}

// ─── EmptyState ───────────────────────────────────────────────────────────────

interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
}

import type React from "react";

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-4 py-16 px-8 text-center">
      {icon && (
        <div className="w-16 h-16 rounded-2xl bg-elevated flex items-center justify-center text-muted">
          {icon}
        </div>
      )}
      <div>
        <p className="text-base font-semibold text-primary">{title}</p>
        {description && <p className="text-sm text-secondary mt-1 max-w-xs">{description}</p>}
      </div>
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}

// ─── Badge ────────────────────────────────────────────────────────────────────

interface BadgeProps {
  children: React.ReactNode;
  color?: "accent" | "fabric" | "success" | "warning" | "danger" | "muted";
  className?: string;
}

export function Badge({ children, color = "muted", className }: BadgeProps) {
  const colorClass = {
    accent:  "bg-accent/15 text-accent border-accent/25",
    fabric:  "bg-fabric/15 text-fabric border-fabric/25",
    success: "bg-success/15 text-success border-success/25",
    warning: "bg-warning/15 text-warning border-warning/25",
    danger:  "bg-danger/15 text-danger border-danger/25",
    muted:   "bg-elevated text-secondary border-border",
  }[color];

  return (
    <span
      className={cn(
        "inline-flex items-center px-1.5 py-0.5 rounded-md text-[10px] font-semibold border tracking-wide uppercase",
        colorClass,
        className,
      )}
    >
      {children}
    </span>
  );
}
