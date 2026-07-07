import { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Info, Wrench, Monitor, Coffee, Code2, Trash2, ArrowLeft, Save } from "lucide-react";
import { useInstance, useUpdateInstance, useDeleteInstance } from "@/hooks/useInstances";
import { Button } from "@/components/shared/Button";
import { Input, Slider } from "@/components/shared/FormControls";
import { ConfirmDialog } from "@/components/shared/ConfirmDialog";
import { Spinner } from "@/components/shared/Feedback";
import type { InstanceUpdate } from "@/types";

type SettingsTab = "general" | "installation" | "window" | "java" | "arguments";

const TABS: { id: SettingsTab; label: string; icon: React.ReactNode }[] = [
  { id: "general", label: "General", icon: <Info size={15} /> },
  { id: "installation", label: "Installation", icon: <Wrench size={15} /> },
  { id: "window", label: "Window", icon: <Monitor size={15} /> },
  { id: "java", label: "Java and memory", icon: <Coffee size={15} /> },
  { id: "arguments", label: "Game Arguments", icon: <Code2 size={15} /> },
];

interface FormState {
  name: string;
  java_path: string;
  java_args: string;
  memory_min_mb: number;
  memory_max_mb: number;
  window_width: number;
  window_height: number;
  fullscreen: boolean;
  game_args: string;
}

export default function InstanceSettingsPage() {
  const { id = "" } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [tab, setTab] = useState<SettingsTab>("general");
  const [confirmDeleteOpen, setConfirmDeleteOpen] = useState(false);
  const [saved, setSaved] = useState(false);

  const { data: instance, isLoading } = useInstance(id);
  const updateMutation = useUpdateInstance();
  const deleteMutation = useDeleteInstance();

  const [form, setForm] = useState<FormState | null>(null);

  useEffect(() => {
    if (instance) {
      setForm({
        name: instance.name,
        java_path: instance.java_path ?? "",
        java_args: instance.java_args ?? "",
        memory_min_mb: instance.memory_min_mb,
        memory_max_mb: instance.memory_max_mb,
        window_width: instance.window_width,
        window_height: instance.window_height,
        fullscreen: instance.fullscreen,
        game_args: instance.game_args ?? "",
      });
    }
  }, [instance]);

  if (isLoading || !instance || !form) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner size={24} />
      </div>
    );
  }

  const set = <K extends keyof FormState>(key: K, value: FormState[K]) =>
    setForm((f) => (f ? { ...f, [key]: value } : f));

  async function handleSave() {
    if (!form) return;
    const update: InstanceUpdate = {
      name: form.name.trim() || instance!.name,
      java_path: form.java_path.trim() || null,
      java_args: form.java_args.trim() || null,
      memory_min_mb: form.memory_min_mb,
      memory_max_mb: form.memory_max_mb,
      window_width: form.window_width,
      window_height: form.window_height,
      fullscreen: form.fullscreen,
      game_args: form.game_args.trim() || null,
    };
    await updateMutation.mutateAsync({ id, update });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  async function handleDelete() {
    await deleteMutation.mutateAsync(id);
    navigate("/library");
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header / breadcrumb */}
      <div className="flex items-center gap-3 px-5 py-4 border-b border-border flex-shrink-0">
        <Button variant="ghost" size="icon" onClick={() => navigate(`/instance/${id}`)} aria-label="Back">
          <ArrowLeft size={16} />
        </Button>
        <div>
          <p className="text-xs text-muted">{instance.name}</p>
          <h1 className="text-base font-semibold text-primary">Settings</h1>
        </div>
        <div className="flex-1" />
        {saved && <span className="text-xs text-success animate-fade-in">Saved!</span>}
        <Button variant="primary" size="sm" leftIcon={<Save size={13} />} isLoading={updateMutation.isPending} onClick={handleSave}>
          Save Changes
        </Button>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <nav className="w-52 border-r border-border p-3 flex flex-col gap-1 flex-shrink-0 overflow-y-auto scrollbar-thin">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm font-medium transition-colors text-left ${
                tab === t.id ? "bg-success/15 text-success" : "text-secondary hover:bg-elevated hover:text-primary"
              }`}
            >
              {t.icon}
              {t.label}
            </button>
          ))}
        </nav>

        {/* Content */}
        <div className="flex-1 overflow-y-auto scrollbar-thin p-6">
          {tab === "general" && (
            <div className="flex flex-col gap-6 max-w-xl">
              <Input label="Name" value={form.name} onChange={(e) => set("name", e.target.value)} maxLength={64} />

              <div className="flex flex-col gap-2 pt-4 border-t border-border">
                <h3 className="text-sm font-semibold text-danger">Delete Instance</h3>
                <p className="text-xs text-secondary">
                  Permanently deletes this instance from your device, including its worlds, configs, and mods.
                  Once deleted, there is no way to recover it.
                </p>
                <Button
                  variant="danger"
                  className="self-start mt-1"
                  leftIcon={<Trash2 size={14} />}
                  onClick={() => setConfirmDeleteOpen(true)}
                >
                  Delete Instance
                </Button>
              </div>
            </div>
          )}

          {tab === "installation" && (
            <div className="flex flex-col gap-4 max-w-xl">
              <h3 className="text-sm font-semibold text-secondary">Installation Info</h3>
              <div className="bg-elevated border border-border rounded-lg divide-y divide-border">
                <InfoRow label="Platform" value={instance.loader === "vanilla" ? "Vanilla" : "Fabric"} />
                <InfoRow label="Game version" value={instance.minecraft_version} />
                {instance.loader_version && <InfoRow label="Loader version" value={instance.loader_version} />}
              </div>
            </div>
          )}

          {tab === "window" && (
            <div className="flex flex-col gap-4 max-w-xl">
              <div className="flex gap-3">
                <Input
                  label="Width"
                  type="number"
                  min={320}
                  max={7680}
                  value={form.window_width}
                  onChange={(e) => set("window_width", Number(e.target.value))}
                  className="font-mono"
                />
                <Input
                  label="Height"
                  type="number"
                  min={200}
                  max={4320}
                  value={form.window_height}
                  onChange={(e) => set("window_height", Number(e.target.value))}
                  className="font-mono"
                />
              </div>
              <label className="flex items-center gap-2.5 cursor-pointer group">
                <input
                  type="checkbox"
                  checked={form.fullscreen}
                  onChange={(e) => set("fullscreen", e.target.checked)}
                  className="w-4 h-4 rounded accent-accent cursor-pointer"
                />
                <span className="text-sm text-secondary group-hover:text-primary transition-colors">
                  Start in fullscreen
                </span>
              </label>
            </div>
          )}

          {tab === "java" && (
            <div className="flex flex-col gap-6 max-w-xl">
              <Input
                label="Java Executable Path"
                placeholder="Detect automatically"
                value={form.java_path}
                onChange={(e) => set("java_path", e.target.value)}
                hint="Leave blank to use the bundled or system Java."
              />
              <Input
                label="Extra JVM Arguments"
                placeholder="-XX:+UseG1GC -XX:MaxGCPauseMillis=50"
                value={form.java_args}
                onChange={(e) => set("java_args", e.target.value)}
                className="font-mono"
              />
              <Slider
                label="Minimum RAM"
                valueLabel={`${form.memory_min_mb} MB`}
                min={256}
                max={form.memory_max_mb - 256}
                step={256}
                value={form.memory_min_mb}
                onChange={(e) => set("memory_min_mb", Number(e.target.value))}
              />
              <Slider
                label="Maximum RAM"
                valueLabel={`${form.memory_max_mb} MB`}
                min={form.memory_min_mb + 256}
                max={32768}
                step={256}
                value={form.memory_max_mb}
                onChange={(e) => set("memory_max_mb", Number(e.target.value))}
              />
            </div>
          )}

          {tab === "arguments" && (
            <div className="flex flex-col gap-4 max-w-xl">
              <Input
                label="Game Arguments"
                placeholder="--server play.example.com --port 25565"
                value={form.game_args}
                onChange={(e) => set("game_args", e.target.value)}
                className="font-mono"
                hint="Appended to the Minecraft launch command."
              />
            </div>
          )}
        </div>
      </div>

      <ConfirmDialog
        open={confirmDeleteOpen}
        onClose={() => setConfirmDeleteOpen(false)}
        onConfirm={handleDelete}
        title="Delete Instance?"
        description={`This will permanently delete "${instance.name}", including its worlds, configs, and mods. This cannot be undone.`}
        confirmLabel="Delete"
        isDangerous
        isLoading={deleteMutation.isPending}
      />
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between px-4 py-3">
      <span className="text-sm text-secondary">{label}</span>
      <span className="text-sm font-medium text-primary">{value}</span>
    </div>
  );
}
