import { useEffect, useMemo, useRef, useState } from "react";
import { Activity, ArrowUpDown, Clock, Globe } from "lucide-react";
import { HistoryFilter, customToTs } from "@/components/HistoryFilter";
import { SummaryCard } from "@/components/SummaryCard";
import { UsageHeatmap } from "@/components/UsageHeatmap";
import { Button } from "@/components/ui/button";
import { rpc, type AppSettings, type HistoryRange, type NicInfo } from "@/lib/api";
import { aggregateGridSum, buildPeriodColumns } from "@/lib/latencyPeriod";
import { countryToIso2, fetchIpInfoBatch, type IpInfo } from "@/lib/ipInfo";
import {
  formatPeakHourLabel,
  pctChange,
  peakHourFromSeries,
  sumSeriesBytes,
} from "@/lib/usageStats";
import { cn, formatUsageGb } from "@/lib/utils";

type SeriesPoint = {
  hour_ts: number;
  process_name: string;
  remote_ip: string | null;
  host: string | null;
  bytes_in: number;
  bytes_out: number;
};

type CtxMenu = { x: number; y: number; label: string; kind: "ip" | "host" };

type GroupBy = "ip" | "host";

const ROW_LIMIT = 200;

function rowLabelOf(p: SeriesPoint, groupBy: GroupBy): string {
  return groupBy === "ip"
    ? p.remote_ip || p.process_name || "(unknown)"
    : p.host || p.process_name || "(unknown)";
}

function totalsByLabel(points: SeriesPoint[], groupBy: GroupBy): Map<string, number> {
  const totals = new Map<string, number>();
  for (const p of points) {
    const key = rowLabelOf(p, groupBy);
    totals.set(key, (totals.get(key) ?? 0) + p.bytes_in + p.bytes_out);
  }
  return totals;
}

export function IpHostPage() {
  const [range, setRange] = useState<HistoryRange>("today");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [groupBy, setGroupBy] = useState<GroupBy>("ip");
  const [nics, setNics] = useState<NicInfo[]>([]);
  const [nicId, setNicId] = useState("");
  const [series, setSeries] = useState<SeriesPoint[]>([]);
  const [yesterdaySeries, setYesterdaySeries] = useState<SeriesPoint[]>([]);
  const [ctxMenu, setCtxMenu] = useState<CtxMenu | null>(null);
  const [geo, setGeo] = useState<Map<string, IpInfo | null>>(new Map());
  const [msg, setMsg] = useState<string | null>(null);
  const ctxRef = useRef<HTMLDivElement>(null);

  const load = async () => {
    const start_ts = range === "custom" ? customToTs(customStart) : null;
    const end_ts = range === "custom" ? customToTs(customEnd) : null;
    const nic_id = nicId || null;
    const [usage, yesterday] = await Promise.all([
      rpc<{ type: "AppUsageSeries"; data: SeriesPoint[] }>({
        method: "GetAppUsageSeries",
        params: { range, start_ts, end_ts, group_by: groupBy, nic_id },
      }),
      range === "today"
        ? rpc<{ type: "AppUsageSeries"; data: SeriesPoint[] }>({
            method: "GetAppUsageSeries",
            params: {
              range: "yesterday",
              start_ts: null,
              end_ts: null,
              group_by: groupBy,
              nic_id,
            },
          })
        : null,
    ]);
    setSeries(usage.data);
    setYesterdaySeries(yesterday?.data ?? []);
  };

  useEffect(() => {
    rpc<{ type: "Nics"; data: NicInfo[] }>({ method: "ListNics" }).then((r) => {
      setNics(r.data.filter((n) => n.media_type !== "Loopback"));
    });
  }, []);

  useEffect(() => {
    load().catch(console.error);
    const id = setInterval(() => load().catch(console.error), 10000);
    return () => clearInterval(id);
  }, [range, customStart, customEnd, groupBy, nicId]);

  useEffect(() => {
    if (!ctxMenu) return;
    const close = (e: MouseEvent) => {
      if (ctxRef.current?.contains(e.target as Node)) return;
      setCtxMenu(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [ctxMenu]);

  const ignoreEntry = async () => {
    if (!ctxMenu) return;
    try {
      const s = await rpc<{ type: "Settings"; data: AppSettings }>({ method: "GetSettings" });
      const settings = {
        ...s.data,
        ignored_apps: s.data.ignored_apps ?? [],
        ignored_ips: s.data.ignored_ips ?? [],
        ignored_urls: s.data.ignored_urls ?? [],
      };
      const value = ctxMenu.label.trim();
      if (!value) return;
      if (ctxMenu.kind === "ip") {
        if (!settings.ignored_ips.some((x) => x.toLowerCase() === value.toLowerCase())) {
          settings.ignored_ips = [...settings.ignored_ips, value];
        }
      } else if (!settings.ignored_urls.some((x) => x.toLowerCase() === value.toLowerCase())) {
        settings.ignored_urls = [...settings.ignored_urls, value];
      }
      await rpc({ method: "SetSettings", params: { settings } });
      setMsg(`Ignored ${value}`);
      setCtxMenu(null);
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
      setCtxMenu(null);
    }
  };

  const columns = useMemo(
    () => buildPeriodColumns(range, customStart, customEnd),
    [range, customStart, customEnd],
  );

  const totals = useMemo(() => totalsByLabel(series, groupBy), [series, groupBy]);

  const rows = useMemo(
    () =>
      [...totals.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, ROW_LIMIT)
        .map(([k]) => k),
    [totals],
  );

  const stats = useMemo(() => {
    const yTotals = totalsByLabel(yesterdaySeries, groupBy);
    const totalTraffic = sumSeriesBytes(series);
    const showTrend = range === "today" && yesterdaySeries.length > 0;
    const top = [...totals.entries()].sort((a, b) => b[1] - a[1])[0] ?? null;
    return {
      count: totals.size,
      totalTraffic,
      peak: peakHourFromSeries(series),
      top,
      trends: showTrend
        ? {
            count: pctChange(totals.size, yTotals.size),
            totalTraffic: pctChange(totalTraffic, sumSeriesBytes(yesterdaySeries)),
          }
        : null,
    };
  }, [series, yesterdaySeries, totals, groupBy, range]);

  useEffect(() => {
    if (groupBy !== "ip" || rows.length === 0) return;
    let alive = true;
    fetchIpInfoBatch(rows).then((m) => alive && setGeo(m)).catch(console.error);
    return () => {
      alive = false;
    };
  }, [rows, groupBy]);

  const countryFlag = (ip: string) => {
    const info = geo.get(ip.trim());
    const cc = countryToIso2(info?.country);
    if (!cc) return null;
    return (
      <span
        className={cn("fi", `fi-${cc}`, "shrink-0 text-[11px]")}
        title={info?.country ?? undefined}
        aria-hidden
      />
    );
  };

  const cells = useMemo(
    () =>
      aggregateGridSum(
        rows,
        columns,
        series.map((p) => ({
          ts: p.hour_ts,
          value: p.bytes_in + p.bytes_out,
          rowKey: rowLabelOf(p, groupBy),
        })),
      ),
    [rows, columns, series, groupBy],
  );

  return (
    <div className="space-y-5">
      {msg && (
        <div className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-3 py-2 text-sm">
          {msg}
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <SummaryCard
          label={groupBy === "ip" ? "Total IPs" : "Total Hosts"}
          value={String(stats.count)}
          icon={Globe}
          iconClass="bg-blue-500/15 text-blue-400"
          trend={stats.trends?.count != null ? { pct: stats.trends.count } : null}
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
          label="Top Talker"
          value={stats.top ? stats.top[0] : "—"}
          sub={stats.top ? formatUsageGb(stats.top[1]) : undefined}
          icon={Activity}
          iconClass="bg-emerald-500/15 text-emerald-400"
        />
      </div>

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
        <div className="ml-auto flex gap-1">
          <Button
            size="sm"
            variant={groupBy === "ip" ? "default" : "outline"}
            onClick={() => setGroupBy("ip")}
          >
            IP
          </Button>
          <Button
            size="sm"
            variant={groupBy === "host" ? "default" : "outline"}
            onClick={() => setGroupBy("host")}
          >
            Host / URL
          </Button>
        </div>
      </div>

      <UsageHeatmap
        title={groupBy === "ip" ? "IP traffic heatmap" : "Host traffic heatmap"}
        rowHeader={groupBy === "ip" ? "Address" : "Host"}
        rowLead={groupBy === "ip" ? countryFlag : undefined}
        rows={rows}
        columns={columns}
        cells={cells}
        onRowContextMenu={(row, e) =>
          setCtxMenu({
            x: e.clientX,
            y: e.clientY,
            label: row,
            kind: groupBy === "ip" ? "ip" : "host",
          })
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
            onClick={() => void ignoreEntry()}
          >
            Ignore
          </button>
        </div>
      )}
    </div>
  );
}
