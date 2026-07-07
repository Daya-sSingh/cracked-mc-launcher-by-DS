import * as React from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "./Button";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
  /** Max width tailwind class e.g. "max-w-lg" */
  maxWidth?: string;
  /** If true, clicking the backdrop does not dismiss the modal */
  persistent?: boolean;
}

export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  className,
  maxWidth = "max-w-lg",
  persistent = false,
}: ModalProps) {
  // Close on Escape
  React.useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !persistent) onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose, persistent]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm animate-fade-in"
        onClick={() => !persistent && onClose()}
      />

      {/* Panel */}
      <div
        className={cn(
          "relative w-full max-h-[85vh] flex flex-col bg-surface border border-border rounded-xl shadow-xl overflow-hidden",
          "animate-scale-in",
          maxWidth,
          className,
        )}
      >
        {/* Header */}
        {(title || description) && (
          <div className="flex items-start justify-between p-5 pb-4 border-b border-border shrink-0">
            <div className="flex-1 min-w-0">
              {title && (
                <h2 className="text-base font-semibold text-primary">{title}</h2>
              )}
              {description && (
                <p className="text-sm text-secondary mt-0.5">{description}</p>
              )}
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="ml-3 shrink-0 -mr-1 -mt-1 h-7 w-7"
              onClick={onClose}
              aria-label="Close"
            >
              <X size={14} />
            </Button>
          </div>
        )}

        {/* Content — scrolls internally once it exceeds the panel's capped
            height, rather than the panel itself growing past the viewport
            and ending up flush against the window's top/bottom edges. */}
        <div className="p-5 overflow-y-auto scrollbar-thin flex-1 min-h-0">{children}</div>
      </div>
    </div>
  );
}

export function ModalFooter({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-center justify-end gap-2.5 pt-4 mt-4 border-t border-border",
        className,
      )}
    >
      {children}
    </div>
  );
}
