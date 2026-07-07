import { NavLink } from "react-router-dom";
import {
  Home,
  Library,
  Download,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useLaunchStore } from "@/state/launch";

const navItems: { to: string; label: string; Icon: LucideIcon }[] = [
  { to: "/",        label: "Home",     Icon: Home    },
  { to: "/library", label: "Library",  Icon: Library },
  { to: "/downloads", label: "Downloads", Icon: Download },
  { to: "/settings", label: "Settings", Icon: Settings },
];

export function Sidebar() {
  const runningCount = useLaunchStore((s) => s.runningIds.size);

  return (
    <aside className="w-[68px] flex flex-col items-center py-3 gap-1 bg-base border-r border-border flex-shrink-0">
      {navItems.map(({ to, label, Icon }) => (
        <SidebarItem key={to} to={to} label={label} Icon={Icon} />
      ))}

      {/* Running instances badge — appears at bottom when any game is live */}
      {runningCount > 0 && (
        <div className="mt-auto mb-1 flex flex-col items-center">
          <div className="relative">
            <div className="w-8 h-8 rounded-full bg-accent/10 flex items-center justify-center">
              <span className="text-accent text-xs font-bold">{runningCount}</span>
            </div>
            <span className="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 rounded-full bg-success border-2 border-base animate-pulse" />
          </div>
          <span className="text-muted text-[9px] mt-1">Running</span>
        </div>
      )}
    </aside>
  );
}

interface SidebarItemProps {
  to: string;
  label: string;
  Icon: LucideIcon;
}

function SidebarItem({ to, label, Icon }: SidebarItemProps) {
  return (
    <NavLink
      to={to}
      end={to === "/"}
      className={({ isActive }) =>
        cn(
          "group flex flex-col items-center gap-1 w-12 py-2 rounded-lg transition-all duration-150",
          isActive
            ? "bg-accent/10 text-accent"
            : "text-muted hover:text-secondary hover:bg-elevated",
        )
      }
      title={label}
    >
      {({ isActive }) => (
        <>
          <Icon
            size={18}
            strokeWidth={isActive ? 2.5 : 2}
            className="transition-transform duration-150 group-hover:scale-110"
          />
          <span className="text-[9px] font-medium leading-none">{label}</span>
        </>
      )}
    </NavLink>
  );
}
