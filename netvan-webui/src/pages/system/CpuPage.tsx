import { useEffect, useMemo, useState } from "react";
import ReactECharts from "echarts-for-react";
import { ArrowDown, ArrowUp, Clock } from "lucide-react";
import { HwVendorLogo } from "@/components/HwVendorLogo";
import { matchHwVendor } from "@/lib/hwVendor";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { LatencyPeriodGrid } from "@/components/LatencyPeriodGrid";
import {
  rpc,
  type CpuSnapshot,
  type HardwareInventory,
  type HistoryRange,
  type MetricSummary,
  type SystemMetricPoint,
  type ThermalSnapshot,
} from "@/lib/api";
import { aggregateGrid, buildPeriodColumns } from "@/lib/latencyPeriod";
import { formatPercent } from "@/lib/systemMetrics";
import { formatCelsius, pickCpuSensor } from "@/lib/thermal";
import { cn } from "@/lib/utils";

const CHART_RANGES: { id: HistoryRange; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "yesterday", label: "Yesterday" },
  { id: "week", label: "7D" },
  { id: "months", label: "30D" },
];

const emptySummary = (): MetricSummary => ({
  avg: null,
  min: null,
  max: null,
  sample_count: 0,
});

function clampPct(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

function formatBrand(brand: string): string {
  return brand.replace(/\(R\)/gi, "®").replace(/\(TM\)/gi, "™");
}

type CoreBand = "low" | "medium" | "high";

function coreBand(pct: number): CoreBand {
  if (pct >= 55) return "high";
  if (pct >= 35) return "medium";
  return "low";
}

const CORE_BAR: Record<CoreBand, string> = {
  high: "bg-blue-500",
  medium: "bg-sky-400",
  low: "bg-emerald-400",
};

function axisBounds(range: HistoryRange): { min?: number; max?: number } {
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const day = 86_400_000;
  if (range === "today") return { min: startOfToday, max: now.getTime() };
  if (range === "yesterday") return { min: startOfToday - day, max: startOfToday };
  if (range === "week") return { min: startOfToday - 7 * day, max: now.getTime() };
  if (range === "months") return { min: startOfToday - 30 * day, max: now.getTime() };
  return {};
}

function seriesWithGaps(points: SystemMetricPoint[], range: HistoryRange) {
  const gapMs =
    range === "today" || range === "yesterday"
      ? 20 * 60 * 1000
      : range === "week"
        ? 6 * 60 * 60 * 1000
        : 36 * 60 * 60 * 1000;
  const data: [number, number | null][] = [];
  for (let i = 0; i < points.length; i++) {
    const point = points[i]!;
    const prev = points[i - 1];
    if (prev && (point.ts - prev.ts) * 1000 > gapMs) {
      data.push([prev.ts * 1000 + 1000, null]);
    }
    data.push([point.ts * 1000, Number(point.value.toFixed(2))]);
  }
  return data;
}

function chartOption(points: SystemMetricPoint[], range: HistoryRange) {
  const bounds = axisBounds(range);
  return {
    backgroundColor: "transparent",
    textStyle: { color: "#8b9bb4" },
    grid: { left: 48, right: 12, top: 16, bottom: 28 },
    tooltip: { trigger: "axis" },
    xAxis: {
      type: "time",
      min: bounds.min,
      max: bounds.max,
      axisLabel: {
        color: "#8b9bb4",
        hideOverlap: true,
        formatter: (value: number) => {
          const d = new Date(value);
          if (range === "today" || range === "yesterday") {
            return `${String(d.getHours()).padStart(2, "0")}:00`;
          }
          if (range === "week") {
            return d.toLocaleDateString(undefined, { weekday: "short" });
          }
          return String(d.getDate());
        },
      },
      axisLine: { lineStyle: { color: "#243049" } },
      splitLine: { show: false },
    },
    yAxis: {
      type: "value",
      min: 0,
      max: 100,
      interval: 25,
      axisLabel: {
        color: "#8b9bb4",
        formatter: (v: number) => `${v}%`,
      },
      splitLine: { lineStyle: { color: "#243049" } },
    },
    series: [
      {
        name: "CPU",
        type: "line",
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        data: seriesWithGaps(points, range),
        color: "#3d9cf0",
      },
    ],
  };
}

function loadHistory(range: HistoryRange) {
  return rpc<{
    type: "CpuHistory";
    data: { series: SystemMetricPoint[]; summary: MetricSummary };
  }>({
    method: "GetCpuHistory",
    params: { range, start_ts: null, end_ts: null },
  });
}

function ProcessorMark({ brand, vendor }: { brand: string; vendor: string }) {
  return (
    <HwVendorLogo vendor={matchHwVendor(brand, vendor)} className="h-10 w-14 shrink-0 object-contain" />
  );
}

function Donut({ value }: { value: number }) {
  const pct = clampPct(value);
  const r = 42;
  const c = 2 * Math.PI * r;
  const dash = (pct / 100) * c;
  return (
    <div className="relative h-28 w-28 shrink-0">
      <svg viewBox="0 0 120 120" className="h-full w-full">
        <circle
          cx="60"
          cy="60"
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="10"
          className="text-[var(--color-muted)]"
        />
        <circle
          cx="60"
          cy="60"
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="10"
          strokeLinecap="round"
          strokeDasharray={`${dash} ${c - dash}`}
          transform="rotate(-90 60 60)"
          className="text-blue-500"
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <div className="text-lg font-semibold tabular-nums leading-none">
          {formatPercent(pct, 1)}
        </div>
        <div className="mt-1 text-[10px] text-[var(--color-muted-foreground)]">Current</div>
      </div>
    </div>
  );
}

function Sparkline({ points }: { points: number[] }) {
  if (points.length < 2) {
    return <div className="h-12 min-w-[8rem] flex-1" />;
  }
  const w = 160;
  const h = 48;
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const d = points
    .map((v, i) => {
      const x = (i / (points.length - 1)) * w;
      const y = h - ((v - min) / span) * (h - 6) - 3;
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="h-12 min-w-[8rem] flex-1" aria-hidden>
      <path d={d} fill="none" stroke="#7eb6ff" strokeWidth="1.6" />
    </svg>
  );
}

function MetricStat({
  value,
  label,
}: {
  value: string;
  label: string;
}) {
  return (
    <div className="min-w-0">
      <div className="truncate text-lg font-semibold tabular-nums">{value}</div>
      <div className="text-[11px] leading-tight text-[var(--color-muted-foreground)]">{label}</div>
    </div>
  );
}

function CoreTile({ index, value }: { index: number; value: number }) {
  const pct = clampPct(value);
  const band = coreBand(pct);
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-muted)]/25 px-3 py-2.5">
      <div className="text-[11px] text-[var(--color-muted-foreground)]">Core {index}</div>
      <div className="mt-1 text-sm font-semibold tabular-nums">{formatPercent(pct, 0)}</div>
      <div className="mt-2 h-1 overflow-hidden rounded-full bg-[var(--color-muted)]">
        <div
          className={cn("h-full rounded-full", CORE_BAR[band])}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

export function CpuPage() {
  const [snap, setSnap] = useState<CpuSnapshot | null>(null);
  const [baseMhz, setBaseMhz] = useState<number | null>(null);
  const [cpuTemp, setCpuTemp] = useState("—");
  const [error, setError] = useState<string | null>(null);
  const [range, setRange] = useState<HistoryRange>("today");
  const [series, setSeries] = useState<SystemMetricPoint[]>([]);
  const [todaySummary, setTodaySummary] = useState<MetricSummary>(emptySummary);
  const [todaySeries, setTodaySeries] = useState<SystemMetricPoint[]>([]);
  const [yesterdayAvg, setYesterdayAvg] = useState<number | null>(null);

  useEffect(() => {
    let alive = true;
    rpc<{ type: "HardwareInventory"; data: HardwareInventory }>({
      method: "GetHardwareInventory",
    })
      .then((r) => {
        if (alive) setBaseMhz(r.data.cpu.base_speed_mhz);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const [r, t] = await Promise.all([
          rpc<{ type: "CpuSnapshot"; data: CpuSnapshot }>({
            method: "GetCpuSnapshot",
          }),
          rpc<{ type: "ThermalSnapshot"; data: ThermalSnapshot }>({
            method: "GetThermalSnapshot",
          }),
        ]);
        if (!alive) return;
        setSnap(r.data);
        setCpuTemp(formatCelsius(pickCpuSensor(t.data.sensors)?.celsius));
        setError(null);
      } catch (e) {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      }
    };
    load();
    const id = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const selected = await loadHistory(range);
        if (!alive) return;
        setSeries(selected.data.series);
        if (range === "today") {
          setTodaySeries(selected.data.series);
          setTodaySummary(selected.data.summary);
        } else {
          const today = await loadHistory("today");
          if (!alive) return;
          setTodaySeries(today.data.series);
          setTodaySummary(today.data.summary);
        }
        if (range === "yesterday") {
          setYesterdayAvg(selected.data.summary.avg);
        } else {
          const yday = await loadHistory("yesterday");
          if (alive) setYesterdayAvg(yday.data.summary.avg);
        }
      } catch (e) {
        console.error(e);
      }
    };
    load();
    const id = setInterval(load, 30_000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [range]);

  const columns = useMemo(() => buildPeriodColumns(range), [range]);
  const cells = useMemo(
    () =>
      aggregateGrid(
        ["CPU"],
        columns,
        series.map((p) => ({ ts: p.ts, value: p.value, rowKey: "CPU" })),
      ),
    [columns, series],
  );
  const option = useMemo(() => chartOption(series, range), [series, range]);

  const brand = formatBrand(snap?.brand || "—");
  const vendor = snap?.vendor_id || "—";
  const mhz = baseMhz ?? snap?.frequency_mhz ?? null;
  const liveAvg = todaySummary.avg;
  const delta =
    liveAvg != null && yesterdayAvg != null ? liveAvg - yesterdayAvg : null;
  const spark = todaySeries.slice(-36).map((p) => p.value);
  const cores = snap?.per_core ?? [];

  return (
    <div className="space-y-4">
      {error && <p className="text-sm text-red-400">{error}</p>}

      <div className="grid gap-4 xl:grid-cols-[1.35fr_1fr]">
        <Card>
          <CardContent className="pt-4">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="flex min-w-0 items-center gap-3">
                <ProcessorMark brand={snap?.brand ?? ""} vendor={vendor} />
                <div className="min-w-0">
                  <div className="truncate text-base font-semibold">{brand}</div>
                  <div className="text-xs text-[var(--color-muted-foreground)]">Processor</div>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-x-8 gap-y-2 text-sm">
                <div>
                  <div className="text-[11px] text-[var(--color-muted-foreground)]">Vendor</div>
                  <div className="font-medium">{vendor}</div>
                </div>
                <div>
                  <div className="text-[11px] text-[var(--color-muted-foreground)]">Utilization</div>
                  <div className="font-semibold tabular-nums">
                    {formatPercent(snap?.utilization, 1)}
                  </div>
                  <div className="text-[10px] text-[var(--color-muted-foreground)]">Overall</div>
                </div>
              </div>
            </div>
            <div className="mt-4 grid grid-cols-2 gap-3 border-t border-[var(--color-border)] pt-4 sm:grid-cols-4">
              <MetricStat
                value={snap?.physical_cores != null ? String(snap.physical_cores) : "—"}
                label="Physical Cores"
              />
              <MetricStat
                value={snap ? String(snap.logical_cores) : "—"}
                label="Logical Cores"
              />
              <MetricStat
                value={mhz != null ? `${mhz} MHz` : "—"}
                label="Base Frequency"
              />
              <MetricStat value={cpuTemp} label="Temperature" />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-0">
            <CardTitle className="text-sm font-medium">Live Utilization</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-4">
              <Donut value={snap?.utilization ?? 0} />
              <div className="min-w-0 flex-1">
                {delta != null && (
                  <div
                    className={cn(
                      "mb-2 flex items-center gap-1 text-xs font-medium",
                      delta <= 0 ? "text-emerald-400" : "text-amber-400",
                    )}
                  >
                    {delta <= 0 ? (
                      <ArrowDown className="h-3.5 w-3.5" />
                    ) : (
                      <ArrowUp className="h-3.5 w-3.5" />
                    )}
                    <span>{formatPercent(Math.abs(delta), 1)}</span>
                    <span className="font-normal text-[var(--color-muted-foreground)]">
                      vs yesterday
                    </span>
                  </div>
                )}
                <Sparkline points={spark} />
              </div>
            </div>
            <div className="mt-3 grid grid-cols-2 gap-3 border-t border-[var(--color-border)] pt-3">
              <MetricStat value={formatPercent(todaySummary.max, 0)} label="Peak" />
              <MetricStat value={formatPercent(todaySummary.avg, 1)} label="Average" />
            </div>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="flex-row items-center justify-between gap-3 py-3">
          <CardTitle className="text-sm font-medium">Per-core Utilization</CardTitle>
          <div className="flex flex-wrap items-center gap-3 text-[11px] text-[var(--color-muted-foreground)]">
            <span className="inline-flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-blue-500" /> High
            </span>
            <span className="inline-flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-sky-400" /> Medium
            </span>
            <span className="inline-flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-emerald-400" /> Low
            </span>
          </div>
        </CardHeader>
        <CardContent>
          {cores.length === 0 ? (
            <p className="py-6 text-center text-sm text-[var(--color-muted-foreground)]">
              No per-core samples yet.
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-8">
              {cores.map((u, i) => (
                <CoreTile key={i} index={i} value={u} />
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <div className="grid gap-4 xl:grid-cols-[1.35fr_1fr]">
        <Card>
          <CardHeader className="flex-row items-center justify-between gap-3 py-3">
            <CardTitle className="text-sm font-medium">Utilization Over Time</CardTitle>
            <div className="flex items-center gap-1 rounded-full bg-[var(--color-muted)]/40 p-1">
              {CHART_RANGES.map((r) => (
                <button
                  key={r.id}
                  type="button"
                  onClick={() => setRange(r.id)}
                  className={cn(
                    "rounded-full px-3 py-1 text-xs font-medium",
                    range === r.id
                      ? "bg-blue-500 text-white"
                      : "text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]",
                  )}
                >
                  {r.label}
                </button>
              ))}
            </div>
          </CardHeader>
          <CardContent>
            {series.length === 0 ? (
              <p className="py-10 text-center text-sm text-[var(--color-muted-foreground)]">
                No samples for this range yet.
              </p>
            ) : (
              <ReactECharts option={option} style={{ height: 240 }} />
            )}
            <p className="mt-1 flex items-center gap-1.5 text-[11px] text-[var(--color-muted-foreground)]">
              <Clock className="h-3 w-3" />
              All times are shown in your local timezone
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="py-3">
            <CardTitle className="text-sm font-medium">Period Averages (CPU %)</CardTitle>
          </CardHeader>
          <CardContent>
            <LatencyPeriodGrid
              rows={["CPU"]}
              columns={columns}
              cells={cells}
              unit="%"
              rowHeader="Metric"
            />
            <div className="mt-3 flex items-center gap-2 text-[10px] text-[var(--color-muted-foreground)]">
              <span>Lower</span>
              <div className="h-1.5 flex-1 rounded-full bg-gradient-to-r from-blue-500/15 to-blue-500" />
              <span>Higher</span>
            </div>
          </CardContent>
        </Card>
      </div>

      <p className="flex items-center justify-end gap-2 text-[11px] text-[var(--color-muted-foreground)]">
        <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
        Auto-refresh: On
      </p>
    </div>
  );
}
