import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  getDataDir,
  rpc,
  type AppSettings,
  type AppTheme,
  type NicInfo,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { applyTheme } from "@/lib/theme";

type CategoryId =
  | "appearance"
  | "general"
  | "capture"
  | "strip"
  | "intervals"
  | "ignore"
  | "data"
  | "about";

const CATEGORIES: { id: CategoryId; label: string }[] = [
  { id: "appearance", label: "Appearance" },
  { id: "general", label: "General" },
  { id: "capture", label: "Capture" },
  { id: "strip", label: "Taskbar strip" },
  { id: "intervals", label: "Intervals" },
  { id: "ignore", label: "Ignore lists" },
  { id: "data", label: "Data" },
  { id: "about", label: "About me" },
];

const THEMES: { id: AppTheme; label: string; swatch: [string, string, string] }[] = [
  { id: "dark-plus", label: "Dark+", swatch: ["#1e1e1e", "#3794ff", "#252526"] },
  { id: "midnight", label: "Midnight", swatch: ["#0b1220", "#3d9cf0", "#121a2b"] },
  { id: "light", label: "Light", swatch: ["#f4f6fa", "#2563eb", "#ffffff"] },
  { id: "nord", label: "Nord", swatch: ["#2e3440", "#88c0d0", "#3b4252"] },
  { id: "dracula", label: "Dracula", swatch: ["#282a36", "#bd93f9", "#21222c"] },
  { id: "tokyo-night", label: "Tokyo Night", swatch: ["#1a1b26", "#7aa2f7", "#24283b"] },
];

/** Inset-grouped list container (iOS Settings style). */
function Group({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <section className="apple-group">
      {title && <h3 className="apple-group-title">{title}</h3>}
      <div className="apple-card">{children}</div>
    </section>
  );
}

/** One settings row: label + description on the left, control on the right. */
function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="apple-row">
      <div className="min-w-0">
        <div className="text-sm font-medium">{label}</div>
        {description && (
          <div className="mt-0.5 text-xs leading-relaxed text-[var(--color-muted-foreground)]">
            {description}
          </div>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function IgnoreListEditor({
  label,
  placeholder,
  values,
  onChange,
}: {
  label: string;
  placeholder: string;
  values: string[];
  onChange: (next: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  const add = () => {
    const v = draft.trim();
    if (!v) return;
    if (values.some((x) => x.toLowerCase() === v.toLowerCase())) {
      setDraft("");
      return;
    }
    onChange([...values, v]);
    setDraft("");
  };
  return (
    <div className="apple-row flex-col items-stretch gap-2">
      <div className="text-sm font-medium">{label}</div>
      <div className="flex gap-2">
        <Input
          value={draft}
          placeholder={placeholder}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              add();
            }
          }}
        />
        <Button type="button" variant="outline" onClick={add}>
          Add
        </Button>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {values.length === 0 && (
          <span className="text-xs text-[var(--color-muted-foreground)]">None</span>
        )}
        {values.map((v) => (
          <button
            key={v}
            type="button"
            className="apple-chip"
            onClick={() => onChange(values.filter((x) => x !== v))}
            title="Remove"
          >
            {v} ×
          </button>
        ))}
      </div>
    </div>
  );
}

function OpenDataDirButton() {
  const alamut = typeof window !== "undefined" ? (window as unknown as { alamut?: { openDataDir?: () => Promise<void> } }).alamut : undefined;
  if (!alamut?.openDataDir) return null;
  return (
    <Button type="button" variant="outline" onClick={() => void alamut.openDataDir?.()}>
      Open data directory
    </Button>
  );
}

function normalizeSettings(data: AppSettings): AppSettings {
  return {
    ...data,
    strip_width_px: data.strip_width_px ?? 110,
    strip_font_px: data.strip_font_px ?? 10,
    strip_slot: data.strip_slot ?? "widgets_start",
    strip_offset_px: data.strip_offset_px ?? 0,
    ignore_private_ips: data.ignore_private_ips ?? false,
    ignored_apps: data.ignored_apps ?? [],
    ignored_ips: data.ignored_ips ?? [],
    ignored_urls: data.ignored_urls ?? [],
    theme: data.theme ?? "dark-plus",
    capture_mode: "process",
  };
}

export function SettingsPage() {
  const [category, setCategory] = useState<CategoryId>("appearance");
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [nics, setNics] = useState<NicInfo[]>([]);
  const [dataDir, setDataDir] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  const reload = async () => {
    const [s, dir, nicRes] = await Promise.all([
      rpc<{ type: "Settings"; data: AppSettings }>({ method: "GetSettings" }),
      getDataDir(),
      rpc<{ type: "Nics"; data: NicInfo[] }>({ method: "ListNics" }),
    ]);
    const next = normalizeSettings(s.data);
    setSettings(next);
    applyTheme(next.theme ?? "dark-plus");
    setDataDir(dir);
    setNics(nicRes.data.filter((n) => n.media_type !== "Loopback"));
  };

  useEffect(() => {
    reload().catch((e) => setMsg(String(e)));
  }, []);

  const save = async (patch?: Partial<AppSettings>) => {
    if (!settings) return;
    setMsg(null);
    try {
      const next = normalizeSettings({ ...settings, ...patch });
      await rpc({ method: "SetSettings", params: { settings: next } });
      setSettings(next);
      if (next.theme) applyTheme(next.theme);
      setMsg("Settings saved");
      await reload();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    }
  };

  const setTheme = async (theme: AppTheme) => {
    if (!settings) return;
    applyTheme(theme);
    const next = { ...settings, theme };
    setSettings(next);
    try {
      await rpc({ method: "SetSettings", params: { settings: next } });
      setMsg("Theme applied");
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    }
  };

  const toggleAutostart = async () => {
    if (!settings) return;
    try {
      const next = {
        ...settings,
        start_ui_with_windows: !settings.start_ui_with_windows,
      };
      await rpc({ method: "SetSettings", params: { settings: next } });
      setSettings(next);
      setMsg(
        next.start_ui_with_windows
          ? "Flag saved (UI autostart is managed by the alamut-api Windows service / Phase 2 companion)"
          : "UI autostart flag cleared",
      );
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    }
  };

  if (!settings) {
    return <div className="text-sm text-[var(--color-muted-foreground)]">Loading…</div>;
  }

  const selectClass =
    "apple-select mt-1 w-full";

  return (
    <div className="space-y-5">
      {msg && (
        <div className="apple-toast" role="status">
          {msg}
        </div>
      )}

      <div className="flex min-h-[28rem] gap-6">
        {/* Translucent sidebar — content scrolls beneath the material */}
        <nav className="apple-sidebar flex w-44 shrink-0 flex-col gap-0.5 self-start rounded-xl p-1.5">
          {CATEGORIES.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => setCategory(c.id)}
              className={cn(
                "apple-nav-item rounded-lg px-3 py-2 text-left text-sm text-[var(--color-muted-foreground)]",
                category === c.id && "is-active font-medium text-[var(--color-foreground)]",
              )}
            >
              {c.label}
            </button>
          ))}
        </nav>

        <div key={category} className="apple-panel min-w-0 flex-1 space-y-5">
          {category === "appearance" && (
            <Group title="Theme">
              <div className="grid gap-3 p-3 sm:grid-cols-2 lg:grid-cols-3">
                {THEMES.map((t) => {
                  const active = (settings.theme ?? "dark-plus") === t.id;
                  return (
                    <button
                      key={t.id}
                      type="button"
                      onClick={() => void setTheme(t.id)}
                      className={cn(
                        "apple-press rounded-xl border p-3 text-left",
                        active
                          ? "border-[var(--color-primary)] bg-[var(--color-muted)] ring-1 ring-[var(--color-primary)]"
                          : "border-[var(--color-border)]",
                      )}
                    >
                      <div className="mb-2 flex gap-1">
                        {t.swatch.map((color) => (
                          <span
                            key={color}
                            className="h-6 w-6 rounded-full border border-[var(--color-border)]"
                            style={{ backgroundColor: color }}
                          />
                        ))}
                      </div>
                      <div className="text-sm font-medium">{t.label}</div>
                    </button>
                  );
                })}
              </div>
            </Group>
          )}

          {category === "general" && (
            <Group title="Startup">
              <Row
                label="Start UI with Windows"
                description="Background collection runs in the alamut-api Windows service. This PWA only displays results."
              >
                <Switch
                  checked={settings.start_ui_with_windows}
                  onCheckedChange={() => void toggleAutostart()}
                  aria-label="Start UI with Windows"
                />
              </Row>
            </Group>
          )}

          {category === "capture" && (
            <Group title="Traffic capture">
              <Row label="Capture mode" description="Process-level TCP ESTATS capture">
                <span className="rounded-full bg-[var(--color-muted)] px-3 py-1 text-xs font-medium">
                  Process (app + IP)
                </span>
              </Row>
            </Group>
          )}

          {category === "strip" && (
            <Group title="Taskbar speed strip">
              <p className="px-1 pb-1 text-xs leading-relaxed text-[var(--color-muted-foreground)]">
                Compact download/upload on the taskbar. Prefers the gap between Widgets and
                Start; otherwise docks near the tray.
              </p>
              <Row label="Network interface">
                <select
                  className={selectClass}
                  value={settings.default_nic_id ?? ""}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      default_nic_id: e.target.value || null,
                    })
                  }
                >
                  <option value="">All NICs</option>
                  {nics.map((n) => (
                    <option key={n.id} value={n.id}>
                      {n.name}
                      {n.oper_status !== "Up" ? ` (${n.oper_status})` : ""}
                    </option>
                  ))}
                </select>
              </Row>
              <Row label="Strip location">
                <select
                  className={selectClass}
                  value={settings.strip_slot}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      strip_slot: e.target.value as "widgets_start" | "near_tray",
                    })
                  }
                >
                  <option value="widgets_start">Between weather and Start</option>
                  <option value="near_tray">Near system tray</option>
                </select>
              </Row>
              <Row label="Horizontal offset (px)">
                <Input
                  type="number"
                  min={-40}
                  max={200}
                  step={2}
                  className="w-24"
                  value={settings.strip_offset_px}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      strip_offset_px: Number(e.target.value) || 0,
                    })
                  }
                />
              </Row>
              <Row label="Strip width (px)">
                <Input
                  type="number"
                  min={80}
                  max={200}
                  step={4}
                  className="w-24"
                  value={settings.strip_width_px}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      strip_width_px: Number(e.target.value) || 110,
                    })
                  }
                />
              </Row>
              <Row label="Font size (px)">
                <Input
                  type="number"
                  min={8}
                  max={14}
                  step={1}
                  className="w-24"
                  value={settings.strip_font_px}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      strip_font_px: Number(e.target.value) || 10,
                    })
                  }
                />
              </Row>
              <div className="flex justify-end px-3 pb-3 pt-1">
                <Button onClick={() => void save()}>Save</Button>
              </div>
            </Group>
          )}

          {category === "intervals" && (
            <Group title="Intervals">
              <Row label="Ping interval (s)">
                <Input
                  type="number"
                  className="w-24"
                  value={settings.ping_interval_secs}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      ping_interval_secs: Number(e.target.value) || 5,
                    })
                  }
                />
              </Row>
              <Row label="HTTP interval (s)">
                <Input
                  type="number"
                  className="w-24"
                  value={settings.http_interval_secs}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      http_interval_secs: Number(e.target.value) || 30,
                    })
                  }
                />
              </Row>
              <div className="flex justify-end px-3 pb-3 pt-1">
                <Button onClick={() => void save()}>Save</Button>
              </div>
            </Group>
          )}

          {category === "ignore" && (
            <Group title="Ignore lists">
              <p className="px-1 pb-1 text-xs leading-relaxed text-[var(--color-muted-foreground)]">
                Hidden from Apps and IP/Host stats. Traffic is still captured.
              </p>
              <Row label="Ignore private IPs" description="In IP/Host stats">
                <Switch
                  checked={settings.ignore_private_ips}
                  onCheckedChange={(checked) =>
                    setSettings({ ...settings, ignore_private_ips: checked })
                  }
                  aria-label="Ignore private IPs"
                />
              </Row>
              {(
                [
                  ["ignored_apps", "Apps (process names)", "chrome.exe"] as const,
                  ["ignored_ips", "IPs", "1.1.1.1"] as const,
                  ["ignored_urls", "URLs / hosts", "cdn.example.com"] as const,
                ] as const
              ).map(([key, label, placeholder]) => (
                <IgnoreListEditor
                  key={key}
                  label={label}
                  placeholder={placeholder}
                  values={settings[key]}
                  onChange={(values) => setSettings({ ...settings, [key]: values })}
                />
              ))}
              <div className="flex justify-end px-3 pb-3 pt-1">
                <Button onClick={() => void save()}>Save</Button>
              </div>
            </Group>
          )}

          {category === "data" && (
            <Group title="Data">
              <Row label="Retention (days)">
                <Input
                  type="number"
                  className="w-24"
                  value={settings.retention_raw_days}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      retention_raw_days: Number(e.target.value) || 14,
                    })
                  }
                />
              </Row>
              <Row label="Speedtest CLI path" description="Optional">
                <Input
                  className="w-64"
                  value={settings.speedtest_cli_path ?? ""}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      speedtest_cli_path: e.target.value || null,
                    })
                  }
                />
              </Row>
              <Row label="Data directory" description={dataDir || undefined}>
                <div className="flex gap-2">
                  <Button onClick={() => void save()}>Save</Button>
                  <OpenDataDirButton />
                </div>
              </Row>
            </Group>
          )}

          {category === "about" && (
            <Group title="About me">
              <div className="apple-row flex-col items-start gap-1">
                <p className="text-lg font-semibold tracking-tight">Armin Dashti</p>
                <p className="text-sm text-[var(--color-muted-foreground)]">
                  Software engineer · Network monitoring &amp; systems tooling
                </p>
                <p className="text-sm">Created by Armin Dashti</p>
              </div>
              <div className="apple-row flex-col items-start gap-1 text-sm">
                <p>
                  GitHub:{" "}
                  <a
                    className="text-[var(--color-primary)] underline-offset-2 hover:underline"
                    href="https://github.com/armindashti"
                    target="_blank"
                    rel="noreferrer"
                  >
                    github.com/armindashti
                  </a>
                </p>
                <p className="text-[var(--color-muted-foreground)]">
                  Alamut — local network monitor for Windows
                </p>
              </div>
            </Group>
          )}
        </div>
      </div>
    </div>
  );
}
