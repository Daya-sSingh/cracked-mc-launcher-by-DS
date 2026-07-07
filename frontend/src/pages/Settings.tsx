import { useState, useEffect } from "react";
import { Settings, Palette, User, MemoryStick, Download, Save } from "lucide-react";
import { useSettingsStore } from "@/state/settings";
import { Button } from "@/components/shared/Button";
import { Input, Select, Slider } from "@/components/shared/FormControls";
import { Badge } from "@/components/shared/Feedback";
import type { LauncherSettings } from "@/types";

const ACCENT_PRESETS = [
  { label: "Ember",   value: "#ff9a57" },
  { label: "Sky",     value: "#5b8af5" },
  { label: "Jade",    value: "#4caf84" },
  { label: "Rose",    value: "#f55b8a" },
  { label: "Violet",  value: "#9b72f5" },
  { label: "Gold",    value: "#f5a623" },
];

const THEME_OPTIONS = [
  { value: "dark",   label: "Dark" },
  { value: "light",  label: "Light (coming soon)", disabled: true },
  { value: "system", label: "System (coming soon)", disabled: true },
];

export default function SettingsPage() {
  const { settings, update, loaded } = useSettingsStore();
  const [form, setForm] = useState<LauncherSettings>(settings);
  const [saved, setSaved] = useState(false);

  // Sync form when settings load from disk
  useEffect(() => {
    if (loaded) setForm(settings);
  }, [loaded]);  // eslint-disable-line react-hooks/exhaustive-deps

  const set = <K extends keyof LauncherSettings>(key: K, value: LauncherSettings[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  async function handleSave(e: React.FormEvent) {
    e.preventDefault();
    await update(form);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  const isDirty = JSON.stringify(form) !== JSON.stringify(settings);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-border flex-shrink-0">
        <div className="flex items-center gap-2">
          <Settings size={16} className="text-muted" />
          <span className="text-xs font-semibold text-muted uppercase tracking-wider">Settings</span>
        </div>
        <div className="flex items-center gap-2">
          {saved && (
            <span className="text-xs text-success animate-fade-in">Saved!</span>
          )}
          <Button
            form="settings-form"
            type="submit"
            variant="primary"
            size="sm"
            isLoading={false}
            disabled={!isDirty}
            leftIcon={<Save size={13} />}
          >
            Save
          </Button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto scrollbar-thin">
        <form id="settings-form" onSubmit={handleSave} className="max-w-xl mx-auto px-6 py-6 flex flex-col gap-7">

          {/* Appearance */}
          <Section icon={<Palette size={14} />} title="Appearance">
            <Select
              label="Theme"
              options={THEME_OPTIONS}
              value={form.theme}
              onChange={(e) => set("theme", e.target.value as LauncherSettings["theme"])}
            />

            <div className="flex flex-col gap-1.5">
              <span className="text-xs font-medium text-secondary">Accent Color</span>
              <div className="flex items-center gap-2 flex-wrap">
                {ACCENT_PRESETS.map((preset) => (
                  <button
                    key={preset.value}
                    type="button"
                    title={preset.label}
                    onClick={() => set("accentColor", preset.value)}
                    className="group flex flex-col items-center gap-1"
                  >
                    <div
                      className="w-7 h-7 rounded-full border-2 transition-all duration-150"
                      style={{
                        backgroundColor: preset.value,
                        borderColor:
                          form.accentColor === preset.value
                            ? "white"
                            : "transparent",
                        boxShadow:
                          form.accentColor === preset.value
                            ? `0 0 8px ${preset.value}80`
                            : "none",
                      }}
                    />
                    <span className="text-[9px] text-muted group-hover:text-secondary transition-colors">
                      {preset.label}
                    </span>
                  </button>
                ))}
                {/* Custom hex input */}
                <div className="flex items-center gap-1.5 ml-1">
                  <div
                    className="w-7 h-7 rounded-full border border-border"
                    style={{ backgroundColor: form.accentColor }}
                  />
                  <input
                    type="text"
                    value={form.accentColor}
                    onChange={(e) => set("accentColor", e.target.value)}
                    maxLength={7}
                    className="w-20 h-7 bg-elevated border border-border rounded-md text-xs text-primary px-2 font-mono focus:outline-none focus:border-accent/60 selectable"
                    placeholder="#ff9a57"
                  />
                </div>
              </div>
            </div>
          </Section>

          {/* Account */}
          <Section icon={<User size={14} />} title="Account">
            <Input
              label="Offline Mode Username"
              placeholder="Steve"
              value={form.offlineUsername}
              onChange={(e) => set("offlineUsername", e.target.value)}
              maxLength={16}
              hint="Used when launching in offline mode. Must be 3–16 characters."
            />
            <div className="flex items-center gap-2 p-3 rounded-lg bg-elevated border border-border">
              <Badge color="muted">Offline Mode</Badge>
              <p className="text-xs text-secondary">
                Microsoft authentication coming in a future update.
              </p>
            </div>
          </Section>

          {/* Memory */}
          <Section icon={<MemoryStick size={14} />} title="Default Memory">
            <Slider
              label="Default Minimum RAM"
              valueLabel={`${form.defaultMemoryMinMb} MB`}
              min={256}
              max={form.defaultMemoryMaxMb - 256}
              step={256}
              value={form.defaultMemoryMinMb}
              onChange={(e) => set("defaultMemoryMinMb", Number(e.target.value))}
              hint="Applied when creating a new instance."
            />
            <Slider
              label="Default Maximum RAM"
              valueLabel={`${form.defaultMemoryMaxMb} MB`}
              min={form.defaultMemoryMinMb + 256}
              max={32768}
              step={256}
              value={form.defaultMemoryMaxMb}
              onChange={(e) => set("defaultMemoryMaxMb", Number(e.target.value))}
            />
          </Section>

          {/* Downloads */}
          <Section icon={<Download size={14} />} title="Downloads">
            <Slider
              label="Concurrent Downloads"
              valueLabel={form.maxConcurrentDownloads.toString()}
              min={1}
              max={32}
              step={1}
              value={form.maxConcurrentDownloads}
              onChange={(e) => set("maxConcurrentDownloads", Number(e.target.value))}
              hint="Higher values use more bandwidth and CPU but finish faster."
            />
          </Section>

          {/* Version */}
          <div className="flex items-center justify-between pt-2 border-t border-border">
            <span className="text-xs text-muted">Launcher version</span>
            <span className="text-xs text-secondary font-mono">0.1.0</span>
          </div>
        </form>
      </div>
    </div>
  );
}

function Section({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <span className="text-muted">{icon}</span>
        <h2 className="text-xs font-semibold text-muted uppercase tracking-wider">{title}</h2>
        <div className="flex-1 h-px bg-border" />
      </div>
      <div className="flex flex-col gap-4">{children}</div>
    </section>
  );
}
