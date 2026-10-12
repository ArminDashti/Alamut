import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
    <div className="space-y-2">
      <div className="text-sm text-[var(--color-muted-foreground)]">{label}</div>
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
            className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-2 py-0.5 text-xs hover:border-[var(--color-destructive)]/50"
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

  return (
    <div className="space-y-5">
      {msg && (
        <div className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-3 py-2 text-sm">
          {msg}
        </div>
      )}

      <div className="flex min-h-[28rem] gap-4">
        <nav className="flex w-44 shrink-0 flex-col gap-0.5">
          {CATEGORIES.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => setCategory(c.id)}
              className={cn(
                "rounded-md px-3 py-2 text-left text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                category === c.id &&
                  "bg-[var(--color-muted)] font-medium text-[var(--color-foreground)]",
              )}
            >
              {c.label}
            </button>
          ))}
        </nav>

        <div className="min-w-0 flex-1">
          {category === "appearance" && (
            <Card>
              <CardHeader>
                <CardTitle>Theme</CardTitle>
                <CardDescription>Choose a color theme for the app</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                  {THEMES.map((t) => {
                    const active = (settings.theme ?? "dark-plus") === t.id;
                    return (
                      <button
                        key={t.id}
                        type="button"
                        onClick={() => void setTheme(t.id)}
                        className={cn(
                          "rounded-lg border p-3 text-left transition-colors",
                          active
                            ? "border-[var(--color-primary)] bg-[var(--color-muted)]"
                            : "border-[var(--color-border)] hover:border-[var(--color-primary)]/50",
                        )}
                      >
                        <div className="mb-2 flex gap-1">
                          {t.swatch.map((color) => (
                            <span
                              key={color}
                              className="h-6 w-6 rounded-md border border-[var(--color-border)]"
                              style={{ backgroundColor: color }}
                            />
                          ))}
                        </div>
                        <div className="text-sm font-medium">{t.label}</div>
                      </button>
                    );
                  })}
                </div>
              </CardContent>
            </Card>
          )}

          {category === "general" && (
            <Card>
              <CardHeader>
                <CardTitle>Startup</CardTitle>
                <CardDescription>
                  Background collection runs in the <code className="text-xs">alamut-api</code>{" "}
                  Windows service. This PWA only displays results.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="text-sm font-medium">Start UI with Windows</div>
                    <div className="text-xs text-[var(--color-muted-foreground)]">
                      Saves the autostart flag used by the API service companion
                    </div>
                  </div>
                  <Switch
                    checked={settings.start_ui_with_windows}
                    onCheckedChange={() => void toggleAutostart()}
                    aria-label="Start UI with Windows"
                  />
                </div>
              </CardContent>
            </Card>
          )}

          {category === "capture" && (
            <Card>
              <CardHeader>
                <CardTitle>Traffic capture</CardTitle>
                <CardDescription>Process-level TCP ESTATS capture</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-3 py-2 text-sm">
                  Process (app + IP)
                </div>
              </CardContent>
            </Card>
          )}

          {category === "strip" && (
            <Card>
              <CardHeader>
                <CardTitle>Taskbar speed strip</CardTitle>
                <CardDescription>
                  Compact download/upload on the taskbar. Prefers the gap between Widgets and
                  Start; otherwise docks near the tray.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">Network interface</span>
                  <select
                    className="mt-1 flex h-9 w-full rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-2 text-sm"
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
                </label>
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">Strip location</span>
                  <select
                    className="mt-1 flex h-9 w-full rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-2 text-sm"
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
                </label>
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">
                    Horizontal offset (px)
                  </span>
                  <Input
                    type="number"
                    min={-40}
                    max={200}
                    step={2}
                    className="mt-1"
                    value={settings.strip_offset_px}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        strip_offset_px: Number(e.target.value) || 0,
                      })
                    }
                  />
                </label>
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">Strip width (px)</span>
                  <Input
                    type="number"
                    min={80}
                    max={200}
                    step={4}
                    className="mt-1"
                    value={settings.strip_width_px}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        strip_width_px: Number(e.target.value) || 110,
                      })
                    }
                  />
                </label>
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">Font size (px)</span>
                  <Input
                    type="number"
                    min={8}
                    max={14}
                    step={1}
                    className="mt-1"
                    value={settings.strip_font_px}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        strip_font_px: Number(e.target.value) || 10,
                      })
                    }
                  />
                </label>
                <Button onClick={() => void save()}>Save</Button>
              </CardContent>
            </Card>
          )}

          {category === "intervals" && (
            <Card>
              <CardHeader>
                <CardTitle>Intervals</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="grid gap-3 sm:grid-cols-2">
                  <label className="text-sm">
                    <span className="text-[var(--color-muted-foreground)]">Ping interval (s)</span>
                    <Input
                      type="number"
                      className="mt-1"
                      value={settings.ping_interval_secs}
                      onChange={(e) =>
                        setSettings({
                          ...settings,
                          ping_interval_secs: Number(e.target.value) || 5,
                        })
                      }
                    />
                  </label>
                  <label className="text-sm">
                    <span className="text-[var(--color-muted-foreground)]">HTTP interval (s)</span>
                    <Input
                      type="number"
                      className="mt-1"
                      value={settings.http_interval_secs}
                      onChange={(e) =>
                        setSettings({
                          ...settings,
                          http_interval_secs: Number(e.target.value) || 30,
                        })
                      }
                    />
                  </label>
                </div>
                <Button onClick={() => void save()}>Save</Button>
              </CardContent>
            </Card>
          )}

          {category === "ignore" && (
            <Card>
              <CardHeader>
                <CardTitle>Ignore lists</CardTitle>
                <CardDescription>
                  Hidden from Apps and IP/Host stats. Traffic is still captured.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={settings.ignore_private_ips}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        ignore_private_ips: e.target.checked,
                      })
                    }
                  />
                  <span>Ignore private IPs in IP/Host stats</span>
                </label>
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
                <Button onClick={() => void save()}>Save</Button>
              </CardContent>
            </Card>
          )}

          {category === "data" && (
            <Card>
              <CardHeader>
                <CardTitle>Data</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">Retention (days)</span>
                  <Input
                    type="number"
                    className="mt-1"
                    value={settings.retention_raw_days}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        retention_raw_days: Number(e.target.value) || 14,
                      })
                    }
                  />
                </label>
                <label className="block text-sm">
                  <span className="text-[var(--color-muted-foreground)]">
                    Speedtest CLI path (optional)
                  </span>
                  <Input
                    className="mt-1"
                    value={settings.speedtest_cli_path ?? ""}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        speedtest_cli_path: e.target.value || null,
                      })
                    }
                  />
                </label>
                <div className="text-xs text-[var(--color-muted-foreground)]">
                  Data directory: {dataDir}
                </div>
                <div className="flex gap-2">
                  <Button onClick={() => void save()}>Save</Button>
                  <OpenDataDirButton />
                </div>
              </CardContent>
            </Card>
          )}

          {category === "about" && (
            <Card>
              <CardHeader>
                <CardTitle>About me</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3 text-sm">
                <p className="text-lg font-semibold">Armin Dashti</p>
                <p className="text-[var(--color-muted-foreground)]">
                  Software engineer · Network monitoring &amp; systems tooling
                </p>
                <p>Created by Armin Dashti</p>
                <div className="space-y-1 text-[var(--color-muted-foreground)]">
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
                  <p>Alamut — local network monitor for Windows</p>
                </div>
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
