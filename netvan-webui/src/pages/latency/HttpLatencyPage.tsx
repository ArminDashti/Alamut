import { useEffect, useMemo, useState } from "react";
import { HistoryFilter, customToTs } from "@/components/HistoryFilter";
import { LatencyPeriodGrid } from "@/components/LatencyPeriodGrid";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { rpc, type AppSettings, type HistoryRange, type NicInfo } from "@/lib/api";
import { aggregateGrid, buildPeriodColumns } from "@/lib/latencyPeriod";

export function HttpLatencyPage() {
  const [range, setRange] = useState<HistoryRange>("today");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [nics, setNics] = useState<NicInfo[]>([]);
  const [nicId, setNicId] = useState("");
  const [httpHist, setHttpHist] = useState<
    { ts: number; connect_ms: number | null; url: string }[]
  >([]);
  const [targets, setTargets] = useState<string[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [newUrl, setNewUrl] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  const loadSettings = async () => {
    const r = await rpc<{ type: "Settings"; data: AppSettings }>({ method: "GetSettings" });
    setSettings({
      ...r.data,
      strip_slot: r.data.strip_slot ?? "widgets_start",
      strip_offset_px: r.data.strip_offset_px ?? 0,
    });
    setTargets(r.data.http_targets);
  };

  useEffect(() => {
    rpc<{ type: "Nics"; data: NicInfo[] }>({ method: "ListNics" }).then((r) => {
      setNics(r.data.filter((n) => n.media_type !== "Loopback"));
    });
    loadSettings().catch(console.error);
  }, []);

  useEffect(() => {
    const start_ts = range === "custom" ? customToTs(customStart) : null;
    const end_ts = range === "custom" ? customToTs(customEnd) : null;
    rpc<{
      type: "HttpLatencyHistory";
      data: { ts: number; connect_ms: number | null; url: string }[];
    }>({
      method: "GetHttpLatencyHistory",
      params: { nic_id: nicId || null, range, start_ts, end_ts },
    }).then((h) => setHttpHist(h.data));
  }, [range, customStart, customEnd, nicId]);

  const columns = useMemo(
    () => buildPeriodColumns(range, customStart, customEnd),
    [range, customStart, customEnd],
  );

  const rows = useMemo(() => {
    const fromSettings = targets;
    const fromHist = [...new Set(httpHist.map((p) => p.url))];
    return fromSettings.length ? fromSettings : fromHist;
  }, [targets, httpHist]);

  const cells = useMemo(
    () =>
      aggregateGrid(
        rows,
        columns,
        httpHist.map((p) => ({
          ts: p.ts,
          value: p.connect_ms,
          rowKey: p.url,
        })),
      ),
    [rows, columns, httpHist],
  );

  const addUrl = async () => {
    if (!settings) return;
    const url = newUrl.trim();
    if (!url) return;
    if (settings.http_targets.includes(url)) {
      setMsg("URL already in the list");
      return;
    }
    const next: AppSettings = {
      ...settings,
      http_targets: [...settings.http_targets, url],
    };
    try {
      await rpc({ method: "SetSettings", params: { settings: next } });
      setSettings(next);
      setTargets(next.http_targets);
      setNewUrl("");
      setModalOpen(false);
      setMsg(`Added ${url}`);
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    }
  };

  const removeUrl = async (url: string) => {
    if (!settings) return;
    const next: AppSettings = {
      ...settings,
      http_targets: settings.http_targets.filter((t) => t !== url),
    };
    await rpc({ method: "SetSettings", params: { settings: next } });
    setSettings(next);
    setTargets(next.http_targets);
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">

        <Button onClick={() => setModalOpen(true)}>Add URL</Button>
      </div>

      {msg && (
        <div className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-3 py-2 text-sm">
          {msg}
        </div>
      )}

      <Card>
        <CardHeader className="py-3">
          <CardTitle className="text-base">HTTP targets</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          {targets.map((t) => (
            <span
              key={t}
              className="inline-flex max-w-full items-center gap-1.5 rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-2.5 py-1 text-xs"
            >
              <span className="truncate">{t}</span>
              <button
                type="button"
                className="shrink-0 text-[var(--color-muted-foreground)] hover:text-[var(--color-destructive)]"
                onClick={() => removeUrl(t).catch(console.error)}
                aria-label={`Remove ${t}`}
              >
                ×
              </button>
            </span>
          ))}
          {targets.length === 0 && (
            <span className="text-sm text-[var(--color-muted-foreground)]">No targets yet</span>
          )}
        </CardContent>
      </Card>

      <div className="flex flex-wrap items-center gap-2">
        <select
          className="h-9 rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-2 text-sm"
          value={nicId}
          onChange={(e) => setNicId(e.target.value)}
        >
          <option value="">All NICs</option>
          {nics.map((n) => (
            <option key={n.id} value={n.id}>
              {n.name}
            </option>
          ))}
        </select>
        <HistoryFilter
          value={range}
          onChange={setRange}
          customStart={customStart}
          customEnd={customEnd}
          onCustomStart={setCustomStart}
          onCustomEnd={setCustomEnd}
        />
      </div>

      <LatencyPeriodGrid rows={rows} columns={columns} cells={cells} unit="ms" />

      <Dialog open={modalOpen} onOpenChange={setModalOpen}>
        <DialogHeader>
          <DialogTitle>Add HTTP URL</DialogTitle>
          <DialogDescription>
            URL added to scheduled HTTP latency monitoring.
          </DialogDescription>
        </DialogHeader>
        <Input
          placeholder="https://example.com/"
          value={newUrl}
          onChange={(e) => setNewUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") addUrl().catch(console.error);
          }}
          autoFocus
        />
        <DialogFooter>
          <Button variant="outline" onClick={() => setModalOpen(false)}>
            Cancel
          </Button>
          <Button onClick={() => addUrl().catch(console.error)}>Add</Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}
