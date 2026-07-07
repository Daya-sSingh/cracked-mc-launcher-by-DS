import { useState, useMemo, useEffect } from "react";
import { PackagePlus } from "lucide-react";
import { Modal, ModalFooter } from "@/components/shared/Modal";
import { Button } from "@/components/shared/Button";
import { Input, Select } from "@/components/shared/FormControls";
import { Spinner, Badge } from "@/components/shared/Feedback";
import { useCreateInstance, useVersionManifest, useFabricLoaderVersions } from "@/hooks/useInstances";
import { recommendedFabricLoaderVersion } from "@/types";
import type { Loader, VersionType } from "@/types";

interface CreateInstanceModalProps {
  open: boolean;
  onClose: () => void;
  onCreated?: (id: string) => void;
}

type VersionFilter = "all" | "release" | "snapshot";

const VERSION_TYPE_LABELS: Record<VersionType, string> = {
  release:   "Release",
  snapshot:  "Snapshot",
  old_beta:  "Beta",
  old_alpha: "Alpha",
};

export function CreateInstanceModal({ open, onClose, onCreated }: CreateInstanceModalProps) {
  const [name, setName]                 = useState("");
  const [loader, setLoader]             = useState<Loader>("vanilla");
  const [mcVersion, setMcVersion]       = useState("");
  const [loaderVersion, setLoaderVersion] = useState("");
  const [filter, setFilter]             = useState<VersionFilter>("release");

  const { data: manifest, isLoading: manifestLoading } = useVersionManifest(open);
  const {
    data: fabricVersions,
    isLoading: fabricVersionsLoading,
  } = useFabricLoaderVersions(mcVersion, open && loader === "fabric");
  const createMutation = useCreateInstance();

  // Auto-select the latest release once the manifest first loads.
  useEffect(() => {
    if (manifest && !mcVersion) {
      setMcVersion(manifest.latest.release);
    }
  }, [manifest, mcVersion]);

  // Whenever the target Minecraft version or loader changes, any
  // previously-chosen Fabric build may no longer even be a valid option —
  // clear it so a stale selection can never be submitted alongside a
  // different Minecraft version than the one it was checked against.
  useEffect(() => {
    setLoaderVersion("");
  }, [mcVersion, loader]);

  // Once Fabric's compatible-builds list arrives for the current selection,
  // preselect the recommended (newest stable) build.
  useEffect(() => {
    if (loader === "fabric" && fabricVersions && !loaderVersion) {
      const recommended = recommendedFabricLoaderVersion(fabricVersions);
      if (recommended) setLoaderVersion(recommended.loader.version);
    }
  }, [fabricVersions, loader, loaderVersion]);

  const filteredVersions = useMemo(() => {
    if (!manifest) return [];
    return manifest.versions.filter((v) => {
      if (filter === "all") return true;
      if (filter === "release")  return v.type === "release";
      if (filter === "snapshot") return v.type === "snapshot";
      return false;
    });
  }, [manifest, filter]);

  const versionOptions = filteredVersions.map((v) => ({
    value: v.id,
    label: `${v.id}  [${VERSION_TYPE_LABELS[v.type]}]`,
  }));

  const fabricVersionOptions = (fabricVersions ?? []).map((v) => ({
    value: v.loader.version,
    label: v.loader.stable ? v.loader.version : `${v.loader.version}  (unstable)`,
  }));

  const loaderOptions = [
    { value: "vanilla", label: "Vanilla" },
    { value: "fabric",  label: "Fabric" },
  ];

  // Auto-generate a name from the selection if the user hasn't typed one yet
  function autoName(): string {
    if (loader === "vanilla") return `Minecraft ${mcVersion}`;
    return `${loader[0].toUpperCase()}${loader.slice(1)} ${mcVersion}`;
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const finalName = name.trim() || autoName();
    const result = await createMutation.mutateAsync({
      name: finalName,
      loader,
      loader_version: loader === "fabric" ? loaderVersion : null,
      minecraft_version: mcVersion,
      icon: null,
    });
    onCreated?.(result.id);
    handleClose();
  }

  function handleClose() {
    if (createMutation.isPending) return;
    setName("");
    setLoader("vanilla");
    setMcVersion(manifest?.latest.release ?? "");
    setLoaderVersion("");
    setFilter("release");
    createMutation.reset();
    onClose();
  }

  const fabricHasNoBuilds =
    loader === "fabric" && !fabricVersionsLoading && fabricVersionOptions.length === 0;

  const isValid = !!mcVersion && (loader !== "fabric" || !!loaderVersion);

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title="New Instance"
      description="Choose a Minecraft version and give your instance a name."
      maxWidth="max-w-md"
      persistent={createMutation.isPending}
    >
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        {/* Name */}
        <Input
          label="Name"
          placeholder={mcVersion ? autoName() : "e.g. Survival World"}
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={64}
          hint="Leave blank to generate a name automatically."
        />

        {/* Version picker */}
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-secondary">Minecraft Version</span>
            {/* Filter pills */}
            <div className="flex gap-1">
              {(["release", "snapshot", "all"] as VersionFilter[]).map((f) => (
                <button
                  key={f}
                  type="button"
                  onClick={() => setFilter(f)}
                  className={`text-[10px] px-2 py-0.5 rounded-md border transition-colors duration-100 ${
                    filter === f
                      ? "bg-accent/15 border-accent/30 text-accent"
                      : "bg-elevated border-border text-muted hover:text-secondary"
                  }`}
                >
                  {f[0].toUpperCase() + f.slice(1)}
                </button>
              ))}
            </div>
          </div>

          {manifestLoading ? (
            <div className="h-9 bg-elevated rounded-lg flex items-center justify-center gap-2">
              <Spinner size={14} />
              <span className="text-xs text-muted">Fetching versions…</span>
            </div>
          ) : (
            <Select
              options={versionOptions}
              value={mcVersion}
              onChange={(e) => setMcVersion(e.target.value)}
              disabled={versionOptions.length === 0}
            />
          )}

          {manifest && mcVersion && (
            <div className="flex items-center gap-1.5">
              {(() => {
                const v = manifest.versions.find((x) => x.id === mcVersion);
                if (!v) return null;
                return (
                  <Badge
                    color={
                      v.type === "release"  ? "success"
                      : v.type === "snapshot" ? "warning"
                      : "muted"
                    }
                  >
                    {VERSION_TYPE_LABELS[v.type]}
                  </Badge>
                );
              })()}
            </div>
          )}
        </div>

        {/* Loader */}
        <Select
          label="Mod Loader"
          options={loaderOptions}
          value={loader}
          onChange={(e) => setLoader(e.target.value as Loader)}
        />

        {/* Fabric loader version picker — only shown once Fabric is selected */}
        {loader === "fabric" && (
          <div className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-secondary">Fabric Loader Version</span>

            {fabricVersionsLoading ? (
              <div className="h-9 bg-elevated rounded-lg flex items-center justify-center gap-2">
                <Spinner size={14} />
                <span className="text-xs text-muted">Fetching Fabric builds…</span>
              </div>
            ) : fabricHasNoBuilds ? (
              <p className="text-xs text-warning bg-warning/10 border border-warning/20 rounded-lg px-3 py-2">
                No Fabric Loader builds are available for Minecraft {mcVersion} yet.
              </p>
            ) : (
              <Select
                options={fabricVersionOptions}
                value={loaderVersion}
                onChange={(e) => setLoaderVersion(e.target.value)}
              />
            )}
          </div>
        )}

        {/* Error */}
        {createMutation.error && (
          <p className="text-sm text-danger bg-danger/10 border border-danger/20 rounded-lg px-3 py-2">
            {createMutation.error.message}
          </p>
        )}

        <ModalFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={handleClose}
            disabled={createMutation.isPending}
          >
            Cancel
          </Button>
          <Button
            type="submit"
            variant="primary"
            isLoading={createMutation.isPending}
            disabled={!isValid}
            leftIcon={<PackagePlus size={15} />}
          >
            Create Instance
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
