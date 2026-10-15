import { useEffect, useMemo, useRef, useState } from "react";
import { Activity, AppWindow, ArrowUpDown, Clock, Shield } from "lucide-react";
import { AppProcessIcon } from "@/components/AppProcessIcon";
import { HistoryFilter, customToTs } from "@/components/HistoryFilter";
import { SummaryCard } from "@/components/SummaryCard";
import { UsageHeatmap } from "@/components/UsageHeatmap";
import {
  rpc,
  type AppSettings,
  type AppUsageRow,
  type HistoryRange,
} from "@/lib/api";
import {
  aggregateGridSum,
  buildFullDayHourColumns,
  buildPeriodColumns,
} from "@/lib/latencyPeriod";
import { formatUsageGb } from "@/lib/utils";
import {
  formatPeakHourLabel,
  pctChange,
  peakHourFromSeries,
  sumSeriesBytes,
} from "@/lib/usageStats";

type SeriesPoint = {
  hour_ts: number;
  process_name: string;
  remote_ip: string | null;
  host: string | null;
  bytes_in: number;
  bytes_out: number;
};

type CtxMenu = { x: number; y: number; processName: string };

function uniqueApps(points: SeriesPoint[]): Set<string> {
  const s = new Set<string>();
  for (const p of points) {
    const name = p.process_name?.trim();
    if (name) s.add(name);
  }
  return s;
}

function activeAppCount(points: SeriesPoint[]): number {
  const totals = new Map<string, number>();
  for (const p of points) {
    const name = p.process_name?.trim() || "(unknown)";
    totals.set(name, (totals.get(name) ?? 0) + p.bytes_in + p.bytes_out);
  }
  let n = 0;
  for (const v of totals.values()) if (v > 0) n += 1;
  return n;
}

export function AppsPage() {
  const [range, setRange] = useState<HistoryRange>("today");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [series, setSeries] = useState<SeriesPoint[]>([]);
  const [yesterdaySeries, setYesterdaySeries] = useState<SeriesPoint[]>([]);
  const [usageRows, setUsageRows] = useState<AppUsageRow[]>([]);
  const [blockedCount, setBlockedCount] = useState(0);
  const [ctxMenu, setCtxMenu] = useState<CtxMenu | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const ctxRef = useRef<HTMLDivElement>(null);

  const load = async () => {
    const start_ts = range === "custom" ? customToTs(customStart) : null;
    const end_ts = range === "custom" ? customToTs(customEnd) : null;
    const params = {
      range,
      start_ts,
      end_ts,
      group_by: "app",
      nic_id: null,
    };
    const [usage, appUsage, settings] = await Promise.all([
      rpc<{ type: "AppUsageSeries"; data: SeriesPoint[] }>({
        method: "GetAppUsageSeries",
        params,
      }),
      rpc<{ type: "AppUsage"; data: AppUsageRow[] }>({
        method: "GetAppUsage",
        params,
      }),
      rpc<{ type: "Settings"; data: AppSettings }>({ method: "GetSettings" }),
    ]);
    setSeries(usage.data);
    setUsageRows(appUsage.data);
    setBlockedCount(settings.data.ignored_apps?.length ?? 0);

    if (range === "today") {
      const y = await rpc<{ type: "AppUsageSeries"; data: SeriesPoint[] }>({
        method: "GetAppUsageSeries",
        params: {
          range: "yesterday",
          start_ts: null,
          end_ts: null,
          group_by: "app",
          nic_id: null,
        },
      });
      setYesterdaySeries(y.data);
    } else {
      setYesterdaySeries([]);
    }
  };

  useEffect(() => {
    load().catch(console.error);
    const id = setInterval(() => load().catch(console.error), 10000);
    return () => clearInterval(id);
  }, [range, customStart, customEnd]);

  useEffect(() => {
    if (!ctxMenu) return;
    const close = (e: MouseEvent) => {
      if (ctxRef.current?.contains(e.target as Node)) return;
      setCtxMenu(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [ctxMenu]);

  const ignoreApp = async () => {
    if (!ctxMenu) return;
    try {
      const s = await rpc<{ type: "Settings"; data: AppSettings }>({ method: "GetSettings" });
      const settings = {
        ...s.data,
        ignored_apps: s.data.ignored_apps ?? [],
        ignored_ips: s.data.ignored_ips ?? [],
        ignored_urls: s.data.ignored_urls ?? [],
      };
      const name = ctxMenu.processName.trim();
      if (
        name &&
        !settings.ignored_apps.some((x) => x.toLowerCase() === name.toLowerCase())
      ) {
        settings.ignored_apps = [...settings.ignored_apps, name];
      }
      await rpc({ method: "SetSettings", params: { settings } });
      setMsg(`Ignored ${name}`);
      setCtxMenu(null);
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
      setCtxMenu(null);
    }
  };

  const columns = useMemo(() => {
    const now = new Date();
    if (range === "today") return buildFullDayHourColumns(now);
    if (range === "yesterday") {
      const y = new Date(now);
      y.setDate(y.getDate() - 1);
      return buildFullDayHourColumns(y);
    }
    return buildPeriodColumns(range, customStart, customEnd);
  }, [range, customStart, customEnd]);

  const rows = useMemo(() => {
    const totals = new Map<string, number>();
    for (const p of series) {
      const key = p.process_name || "(unknown)";
      totals.set(key, (totals.get(key) ?? 0) + p.bytes_in + p.bytes_out);
    }
    return [...totals.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 50)
      .map(([k]) => k);
  }, [series]);

  const cells = useMemo(
    () =>
      aggregateGridSum(
        rows,
        columns,
        series.map((p) => ({
          ts: p.hour_ts,
          value: p.bytes_in + p.bytes_out,
          rowKey: p.process_name || "(unknown)",
        })),
      ),
    [rows, columns, series],
  );

  const stats = useMemo(() => {
    const totalApps = usageRows.length > 0 ? usageRows.length : uniqueApps(series).size;
    const activeApps = activeAppCount(series);
    const totalTraffic = sumSeriesBytes(series);
    const peak = peakHourFromSeries(series);
    const yTraffic = sumSeriesBytes(yesterdaySeries);
    const yApps = uniqueApps(yesterdaySeries).size;
    const yActive = activeAppCount(yesterdaySeries);
    const showTrend = range === "today" && yesterdaySeries.length > 0;
    return {
      totalApps,
      activeApps,
      totalTraffic,
      peak,
      trends: showTrend
        ? {
            totalApps: pctChange(totalApps, yApps),
            activeApps: pctChange(activeApps, yActive),
            totalTraffic: pctChange(totalTraffic, yTraffic),
            blocked: null as number | null,
          }
        : null,
    };
  }, [series, yesterdaySeries, usageRows, range]);

  return (
    <div className="space-y-5">
      {msg && (
        <div className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-3 py-2 text-sm">
          {msg}
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
        <SummaryCard
          label="Total Apps"
          value={String(stats.totalApps)}
          icon={Activity}
          iconClass="bg-blue-500/15 text-blue-400"
          trend={
            stats.trends?.totalApps != null ? { pct: stats.trends.totalApps } : null
          }
        />
        <SummaryCard
          label="Active Apps"
          value={String(stats.activeApps)}
          icon={AppWindow}
          iconClass="bg-emerald-500/15 text-emerald-400"
          trend={
            stats.trends?.activeApps != null ? { pct: stats.trends.activeApps } : null
          }
        />
        <SummaryCard
          label="Total Traffic"
          value={formatUsageGb(stats.totalTraffic)}
          icon={ArrowUpDown}
          iconClass="bg-violet-500/15 text-violet-400"
          trend={
            stats.trends?.totalTraffic != null ? { pct: stats.trends.totalTraffic } : null
          }
        />
        <SummaryCard
          label="Peak Hour"
          value={stats.peak ? formatPeakHourLabel(stats.peak.hour) : "—"}
          sub={stats.peak ? formatUsageGb(stats.peak.bytes) : undefined}
          icon={Clock}
          iconClass="bg-amber-500/15 text-amber-400"
        />
        <SummaryCard
          label="Blocked Apps"
          value={String(blockedCount)}
          icon={Shield}
          iconClass="bg-cyan-500/15 text-cyan-400"
          trend={null}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <HistoryFilter
          value={range}
          onChange={setRange}
          customStart={customStart}
          customEnd={customEnd}
          onCustomStart={setCustomStart}
          onCustomEnd={setCustomEnd}
        />
      </div>

      <UsageHeatmap
        title="App traffic heatmap"
        rowHeader="App"
        rowLead={(row) => <AppProcessIcon name={row} />}
        rows={rows}
        columns={columns}
        cells={cells}
        onRowContextMenu={(row, e) =>
          setCtxMenu({ x: e.clientX, y: e.clientY, processName: row })
        }
      />

      {ctxMenu && (
        <div
          ref={ctxRef}
          className="fixed z-50 min-w-[140px] rounded-md border border-[var(--color-border)] bg-[var(--color-card)] p-1 shadow-lg"
          style={{ left: ctxMenu.x, top: ctxMenu.y }}
        >
          <button
            type="button"
            className="block w-full rounded px-3 py-1.5 text-left text-sm hover:bg-[var(--color-muted)]"
            onClick={() => void ignoreApp()}
          >
            Ignore
          </button>
        </div>
      )}
    </div>
  );
}
