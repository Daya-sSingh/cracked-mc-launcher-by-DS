import { useState, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Plus, Search, Library as LibraryIcon } from "lucide-react";
import { useInstances } from "@/hooks/useInstances";
import { useInstanceStore, selectFilteredInstances } from "@/state/instances";
import { useShallow } from "zustand/react/shallow";
import { InstanceCard } from "@/components/instances/InstanceCard";
import { CreateInstanceModal } from "@/components/instances/CreateInstanceModal";
import { Button } from "@/components/shared/Button";
import { Input, Select } from "@/components/shared/FormControls";
import { Spinner, EmptyState } from "@/components/shared/Feedback";
import type { InstanceSort } from "@/types";

const SORT_OPTIONS: { value: InstanceSort; label: string }[] = [
  { value: "recently_played", label: "Recently Played" },
  { value: "name_ascending",  label: "Name A → Z" },
  { value: "name_descending", label: "Name Z → A" },
  { value: "favorites_first", label: "Favorites First" },
];

export default function LibraryPage() {
  const navigate = useNavigate();
  const { isLoading } = useInstances();
  const filteredInstances = useInstanceStore(useShallow(selectFilteredInstances));
  const { searchQuery, sortOrder, setSearchQuery, setSortOrder } = useInstanceStore();

  const [createOpen, setCreateOpen] = useState(false);

  // Launching an instance no longer opens a blocking overlay from the grid
  // — Play just launches inline (the card shows its own mini progress bar),
  // and the full stage/log view lives on the instance's own page, reachable
  // any time by clicking the card. Creating a new instance lands you there
  // directly since there's nothing useful to see on the grid yet.
  const handleCreated = useCallback(
    (id: string) => navigate(`/instance/${id}`),
    [navigate],
  );

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-3 px-5 py-3 border-b border-border flex-shrink-0">
        <div className="flex items-center gap-2 text-muted mr-1">
          <LibraryIcon size={16} />
          <span className="text-xs font-semibold uppercase tracking-wider">Library</span>
        </div>

        <div className="flex-1 max-w-xs">
          <Input
            placeholder="Search instances…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            leftAdornment={<Search size={13} />}
            className="h-8 text-xs"
          />
        </div>

        <Select
          options={SORT_OPTIONS}
          value={sortOrder}
          onChange={(e) => setSortOrder(e.target.value as InstanceSort)}
          className="h-8 text-xs w-44"
        />

        <Button
          variant="primary"
          size="sm"
          leftIcon={<Plus size={14} />}
          onClick={() => setCreateOpen(true)}
        >
          New Instance
        </Button>
      </div>

      {/* Grid */}
      <div className="flex-1 overflow-y-auto scrollbar-thin p-5">
        {isLoading ? (
          <div className="flex items-center justify-center h-full">
            <Spinner size={28} label="Loading instances…" />
          </div>
        ) : filteredInstances.length === 0 ? (
          <EmptyState
            icon={<LibraryIcon size={32} />}
            title={searchQuery ? "No instances match your search" : "No instances yet"}
            description={
              searchQuery
                ? "Try a different name or version."
                : "Create your first instance to start playing."
            }
            action={
              !searchQuery && (
                <Button
                  variant="primary"
                  size="sm"
                  leftIcon={<Plus size={14} />}
                  onClick={() => setCreateOpen(true)}
                >
                  New Instance
                </Button>
              )
            }
          />
        ) : (
          <div className="grid gap-4 grid-cols-[repeat(auto-fill,minmax(200px,1fr))]">
            {filteredInstances.map((instance) => (
              <InstanceCard key={instance.id} instance={instance} />
            ))}
          </div>
        )}
      </div>

      <CreateInstanceModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={handleCreated}
      />
    </div>
  );
}
