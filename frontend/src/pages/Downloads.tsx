import { Download } from "lucide-react";
import { EmptyState } from "@/components/shared/Feedback";

/**
 * Downloads page — shows active / queued download progress.
 * Milestone 1 shows a placeholder; Milestone 2 (mod installs + Modrinth)
 * populates this with live download tasks from the download manager.
 */
export default function DownloadsPage() {
  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 px-5 py-3 border-b border-border flex-shrink-0">
        <Download size={16} className="text-muted" />
        <span className="text-xs font-semibold text-muted uppercase tracking-wider">Downloads</span>
      </div>

      <div className="flex-1 flex items-center justify-center">
        <EmptyState
          icon={<Download size={32} />}
          title="No active downloads"
          description="When you install mods or start an instance that needs files, download progress will appear here."
        />
      </div>
    </div>
  );
}
