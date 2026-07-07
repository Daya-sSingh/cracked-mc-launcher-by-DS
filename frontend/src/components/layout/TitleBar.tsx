import { useEffect, useState, useCallback } from "react";
import { Minus, Square, X, Maximize2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { minimizeWindow, maximizeWindow, closeWindow, isWindowMaximized } from "@/lib/tauri";

interface TitleBarProps {
  title?: string;
}

export function TitleBar({ title = "Launcher" }: TitleBarProps) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    isWindowMaximized().then(setMaximized).catch(() => {});
  }, []);

  const handleMaximize = useCallback(async () => {
    await maximizeWindow();
    setMaximized(await isWindowMaximized());
  }, []);

  return (
    <header
      className="flex items-center justify-between h-10 px-3 flex-shrink-0 bg-base border-b border-border"
      data-tauri-drag-region
    >
      {/* App identity */}
      <div className="flex items-center gap-2.5" data-tauri-drag-region>
        <div className="w-5 h-5 rounded-sm overflow-hidden flex-shrink-0">
          <img src="/icon.png" alt="" className="w-full h-full object-cover" />
        </div>
        <span className="text-xs font-semibold text-secondary tracking-wide uppercase select-none">
          {title}
        </span>
      </div>

      {/* Window controls */}
      <div className="flex items-center gap-0.5">
        <TitleBarButton
          onClick={minimizeWindow}
          label="Minimize"
          hoverColor="hover:bg-overlay"
        >
          <Minus size={12} strokeWidth={2.5} />
        </TitleBarButton>

        <TitleBarButton
          onClick={handleMaximize}
          label={maximized ? "Restore" : "Maximize"}
          hoverColor="hover:bg-overlay"
        >
          {maximized ? (
            <Maximize2 size={11} strokeWidth={2.5} />
          ) : (
            <Square size={11} strokeWidth={2.5} />
          )}
        </TitleBarButton>

        <TitleBarButton
          onClick={closeWindow}
          label="Close"
          hoverColor="hover:bg-danger/80"
          hoverText="hover:text-white"
        >
          <X size={12} strokeWidth={2.5} />
        </TitleBarButton>
      </div>
    </header>
  );
}

interface TitleBarButtonProps {
  onClick: () => void;
  label: string;
  children: React.ReactNode;
  hoverColor?: string;
  hoverText?: string;
}

function TitleBarButton({
  onClick,
  label,
  children,
  hoverColor = "hover:bg-elevated",
  hoverText = "",
}: TitleBarButtonProps) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      className={cn(
        "w-8 h-7 flex items-center justify-center rounded",
        "text-muted transition-colors duration-100",
        hoverColor,
        hoverText,
      )}
    >
      {children}
    </button>
  );
}
