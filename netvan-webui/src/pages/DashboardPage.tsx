import { useEffect, useMemo, useRef, useState } from "react";
import ReactECharts from "echarts-for-react";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  Cpu,
  Database,
  Globe,
  MemoryStick,
  Network,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  rpc,
  type AppUsageRow,
  type CpuSnapshot,
  type DiskSnapshot,
  type GpuInfo,
  type HardwareInventory,
  type HistoryRange,
  type MemorySnapshot,
  type NicInfo,
  type SystemMetricPoint,
  type ThermalSnapshot,
} from "@/lib/api";
import { formatBytes, formatPercent } from "@/lib/systemMetrics";
import { formatCelsius, pickCpuSensor, pickGpuSensors } from "@/lib/thermal";
import { cn, formatHistoryAxisLabel } from "@/lib/utils";

const GAUGE_COLORS = {
  cpu: "#3ecf8e",
  memory: "#3d9cf0",
  disk: "#8b5cf6",
  network: "#2dd4bf",
} as const;

const SPARK_POINTS = 48;

function toMb(bps: number): number {
  return Math.max(0, bps) / 1_000_000;
}

function formatMbpsValue(bps: number): { value: string; unit: string } {
  const mb = toMb(bps);
  if (mb >= 1000) return { value: (mb / 1000).toFixed(2), unit: "Gbps" };
  if (mb >= 100) return { value: mb.toFixed(0), unit: "Mbps" };
  return { value: mb.toFixed(1), unit: "Mbps" };
}

function gibShort(bytes: number | null): string {
  if (bytes == null || !Number.isFinite(bytes)) return "—";
  const gib = bytes / 1024 ** 3;
  return gib >= 10 ? gib.toFixed(0) : gib.toFixed(1);
}

function GaugeRing({
  label,
  icon: Icon,
  color,
  percent,
  display,
  sub,
  children,
}: {
  label: string;
  icon: typeof Cpu;
  color: string;
  percent: number | null;
  display: string;
  sub?: React.ReactNode;
  children?: React.ReactNode;
}) {
  const size = 168;
  const stroke = 11;
  const r = (size - stroke) / 2;
  const circ = 2 * Math.PI * r;
  const clamped = Math.max(0, Math.min(100, percent ?? 0));
  const offset = circ * (1 - clamped / 100);
  return (
    <div className="flex flex-col items-center">
      <div className="relative" style={{ width: size, height: size }}>
        <svg width={size} height={size} className="-rotate-90">
          <circle
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke="var(--color-border)"
            strokeWidth={stroke}
            opacity={0.55}
          />
          <circle
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke={color}
            strokeWidth={stroke}
            strokeLinecap="round"
            strokeDasharray={circ}
            strokeDashoffset={offset}
            style={{
              transition: "stroke-dashoffset 0.8s ease",
              filter: `drop-shadow(0 0 6px ${color}66)`,
            }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-0.5">
          <div className="flex items-center gap-1.5 text-sm" style={{ color }}>
            <Icon className="h-4 w-4" />
            <span>{label}</span>
          </div>
          <div className="text-[26px] font-semibold leading-tight tabular-nums">
            {display}
          </div>
          {sub != null && (
            <div className="text-xs text-[var(--color-muted-foreground)]">{sub}</div>
          )}
          {children}
        </div>
      </div>
    </div>
  );
}

function sparkOption(points: number[], color: string) {
  return {
    backgroundColor: "transparent",
    animation: false,
    grid: { left: 2, right: 2, top: 2, bottom: 2 },
    xAxis: { type: "category", show: true, boundaryGap: false, axisLine: false, axisTick: false, axisLabel: { show: false } },
    yAxis: { type: "value", show: false, min: 0 },
    tooltip: { show: false },
    series: [
      {
        type: "line",
        smooth: true,
        showSymbol: false,
        data: points,
        lineStyle: { color, width: 1.5 },
        areaStyle: { color, opacity: 0.12 },
      },
    ],
  };
}

function areaChartOption(
  points: SystemMetricPoint[],
  range: HistoryRange,
  color: string,
  unit: string,
  maxPercent = false,
) {
  return {
    backgroundColor: "transparent",
    textStyle: { color: "#8b9bb4" },
    grid: { left: 44, right: 16, top: 16, bottom: 24 },
    tooltip: { trigger: "axis", valueFormatter: (v: number | null) => `${v ?? 0}${unit}` },
    xAxis: {
      type: "category",
      data: points.map((p) => formatHistoryAxisLabel(p.ts, range)),
      boundaryGap: false,
      axisLabel: { color: "#8b9bb4", hideOverlap: true },
      axisLine: { lineStyle: { color: "#243049" } },
    },
    yAxis: {
      type: "value",
      max: maxPercent ? 100 : undefined,
      axisLabel: { color: "#8b9bb4", formatter: `{value}${unit}` },
      splitLine: { lineStyle: { color: "#243049" } },
    },
    series: [
      {
        type: "line",
        smooth: true,
        showSymbol: false,
        data: points.map((p) => p.value),
        lineStyle: { color, width: 2 },
        areaStyle: {
          color: {
            type: "linear",
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: `${color}59` },
              { offset: 1, color: `${color}05` },
            ],
          },
        },
      },
    ],
  };
}

function dualAreaChartOption(
  points: { ts: number; rx: number; tx: number }[],
  range: HistoryRange,
) {
  return {
    backgroundColor: "transparent",
    textStyle: { color: "#8b9bb4" },
    grid: { left: 48, right: 16, top: 16, bottom: 24 },
    tooltip: { trigger: "axis" },
    legend: {
      bottom: 0,
      icon: "circle",
      itemWidth: 8,
      itemHeight: 8,
      textStyle: { color: "#8b9bb4" },
    },
    xAxis: {
      type: "category",
      data: points.map((p) => formatHistoryAxisLabel(p.ts, range)),
      boundaryGap: false,
      axisLabel: { color: "#8b9bb4", hideOverlap: true },
      axisLine: { lineStyle: { color: "#243049" } },
    },
    yAxis: {
      type: "value",
      axisLabel: { color: "#8b9bb4", formatter: "{value} Mbps" },
      splitLine: { lineStyle: { color: "#243049" } },
    },
    series: [
      {
        name: "Download",
        type: "line",
        smooth: true,
        showSymbol: false,
        data: points.map((p) => p.rx),
        lineStyle: { color: GAUGE_COLORS.network, width: 2 },
        itemStyle: { color: GAUGE_COLORS.network },
        areaStyle: {
          color: {
            type: "linear",
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: `${GAUGE_COLORS.network}59` },
              { offset: 1, color: `${GAUGE_COLORS.network}05` },
            ],
          },
        },
      },
      {
        name: "Upload",
        type: "line",
        smooth: true,
        showSymbol: false,
        data: points.map((p) => p.tx),
        lineStyle: { color: GAUGE_COLORS.disk, width: 2 },
        itemStyle: { color: GAUGE_COLORS.disk },
        areaStyle: {
          color: {
            type: "linear",
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: `${GAUGE_COLORS.disk}40` },
              { offset: 1, color: `${GAUGE_COLORS.disk}05` },
            ],
          },
        },
      },
    ],
  };
}

function UsageBar({
  percent,
  color,
}: {
  percent: number | null;
  color: string;
}) {
  const pct = Math.max(0, Math.min(100, percent ?? 0));
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--color-muted)]">
      <div
        className="h-full rounded-full transition-[width] duration-700"
        style={{ width: `${pct}%`, backgroundColor: color }}
      />
    </div>
  );
}

function PanelHeader({
  icon: Icon,
  color,
  title,
  trailing,
}: {
  icon: typeof Cpu;
  color: string;
  title: string;
  trailing?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex items-center gap-2">
        <span
          className="flex h-7 w-7 items-center justify-center rounded-md"
          style={{ backgroundColor: `${color}22`, color }}
        >
          <Icon className="h-4 w-4" />
        </span>
        <CardTitle>{title}</CardTitle>
      </div>
      {trailing}
    </div>
  );
}

export function DashboardPage() {
  const [nics, setNics] = useState<NicInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [cpu, setCpu] = useState<CpuSnapshot | null>(null);
  const [mem, setMem] = useState<MemorySnapshot | null>(null);
  const [disks, setDisks] = useState<DiskSnapshot[]>([]);
  const [cpuTemp, setCpuTemp] = useState<number | null>(null);
  const [gpuTemps, setGpuTemps] = useState<{ label: string; value: string }[]>([]);
  const [inventory, setInventory] = useState<HardwareInventory | null>(null);
  const [gpuBrief, setGpuBrief] = useState<string>("—");
  const [spark, setSpark] = useState({
    cpu: [] as number[],
    memory: [] as number[],
    disk: [] as number[],
    network: [] as number[],
  });
  const [cpuHist, setCpuHist] = useState<SystemMetricPoint[]>([]);
  const [netHist, setNetHist] = useState<{ ts: number; rx: number; tx: number }[]>([]);
  const [topApps, setTopApps] = useState<AppUsageRow[]>([]);
  const primaryIdRef = useRef<string | null>(null);

  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const [nicRes, cpuRes, memRes, ssdRes, hddRes, thermalRes] = await Promise.all([
          rpc<{ type: "Nics"; data: NicInfo[] }>({ method: "ListNics" }),
          rpc<{ type: "CpuSnapshot"; data: CpuSnapshot }>({ method: "GetCpuSnapshot" }),
          rpc<{ type: "MemorySnapshot"; data: MemorySnapshot }>({
            method: "GetMemorySnapshot",
          }),
          rpc<{ type: "Disks"; data: DiskSnapshot[] }>({
            method: "GetDisks",
            params: { kind: "ssd" },
          }),
          rpc<{ type: "Disks"; data: DiskSnapshot[] }>({
            method: "GetDisks",
            params: { kind: "hdd" },
          }).catch(() => ({ type: "Disks" as const, data: [] as DiskSnapshot[] })),
          rpc<{ type: "ThermalSnapshot"; data: ThermalSnapshot }>({
            method: "GetThermalSnapshot",
          }).catch(() => ({ type: "ThermalSnapshot" as const, data: { sensors: [] } })),
        ]);
        if (!alive) return;
        const list = nicRes.data.filter((n) => n.media_type !== "Loopback");
        setNics(list);
        setError(null);
        const primary = list.find((n) => n.oper_status === "Up") ?? list[0];
        primaryIdRef.current = primary?.id ?? null;
        setCpu(cpuRes.data);
        setMem(memRes.data);
        const allDisks = [...ssdRes.data, ...hddRes.data];
        setDisks(allDisks);
        const sensors = thermalRes.data.sensors;
        setCpuTemp(pickCpuSensor(sensors)?.celsius ?? null);
        setGpuTemps(
          pickGpuSensors(sensors).map((s) => ({
            label: s.hardware_name || s.sensor_name,
            value: formatCelsius(s.celsius),
          })),
        );
        const totalBps = list.reduce((s, n) => s + n.rx_bps + n.tx_bps, 0);
        const diskUtil =
          allDisks.length > 0
            ? allDisks.reduce((s, d) => s + d.utilization, 0) / allDisks.length
            : null;
        setSpark((prev) => ({
          cpu: [...prev.cpu.slice(-(SPARK_POINTS - 1)), cpuRes.data.utilization],
          memory: [...prev.memory.slice(-(SPARK_POINTS - 1)), memRes.data.utilization],
          disk:
            diskUtil == null
              ? prev.disk
              : [...prev.disk.slice(-(SPARK_POINTS - 1)), diskUtil],
          network: [...prev.network.slice(-(SPARK_POINTS - 1)), toMb(totalBps)],
        }));
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
    rpc<{ type: "HardwareInventory"; data: HardwareInventory }>({
      method: "GetHardwareInventory",
    })
      .then((r) => {
        setInventory(r.data);
        const g: GpuInfo | undefined = r.data.gpus[0];
        if (!g) {
          setGpuBrief("None");
          return;
        }
        const memStr =
          g.memory_bytes != null ? ` · ${formatBytes(g.memory_bytes)}` : "";
        setGpuBrief(`${g.brand || g.model}${memStr}`);
      })
      .catch(() => setGpuBrief("—"));
  }, []);

  useEffect(() => {
    const loadSlow = async () => {
      const primaryId = primaryIdRef.current;
      const [cpuH, bwH, appsH] = await Promise.all([
        rpc<{ type: "CpuHistory"; data: { series: SystemMetricPoint[] } }>({
          method: "GetCpuHistory",
          params: { range: "today", start_ts: null, end_ts: null },
        }).catch(() => null),
        primaryId
          ? rpc<{
              type: "BandwidthHistory";
              data: { ts: number; rx_bytes: number; tx_bytes: number }[];
            }>({
              method: "GetBandwidthHistory",
              params: { nic_id: primaryId, range: "today", start_ts: null, end_ts: null },
            }).catch(() => null)
          : Promise.resolve(null),
        rpc<{ type: "AppUsage"; data: AppUsageRow[] }>({
          method: "GetAppUsage",
          params: { range: "today", start_ts: null, end_ts: null, group_by: "process" },
        }).catch(() => null),
      ]);
      if (cpuH) setCpuHist(cpuH.data.series);
      if (bwH) {
        const pts: { ts: number; rx: number; tx: number }[] = [];
        for (let i = 1; i < bwH.data.length; i++) {
          const prev = bwH.data[i - 1]!;
          const cur = bwH.data[i]!;
          pts.push({
            ts: cur.ts,
            rx: toMb(Math.max(0, cur.rx_bytes - prev.rx_bytes)),
            tx: toMb(Math.max(0, cur.tx_bytes - prev.tx_bytes)),
          });
        }
        setNetHist(pts);
      }
      if (appsH) {
        setTopApps(
          [...appsH.data]
            .sort(
              (a, b) =>
                b.bytes_in + b.bytes_out - (a.bytes_in + a.bytes_out),
            )
            .slice(0, 5),
        );
      }
    };
    loadSlow();
    const id = setInterval(loadSlow, 30_000);
    return () => clearInterval(id);
  }, []);

  const totalDown = nics.reduce((s, n) => s + n.rx_bps, 0);
  const totalUp = nics.reduce((s, n) => s + n.tx_bps, 0);
  const nicUp = nics.filter((n) => n.oper_status === "Up").length;

  const diskTotal = disks.reduce((s, d) => s + d.total_bytes, 0);
  const diskUsed = disks.reduce((s, d) => s + d.used_bytes, 0);
  const diskUtil = disks.length
    ? disks.reduce((s, d) => s + d.utilization, 0) / disks.length
    : null;
  const netMb = formatMbpsValue(totalDown);

  const cpuChart = useMemo(
    () => areaChartOption(cpuHist, "today", GAUGE_COLORS.cpu, "%", true),
    [cpuHist],
  );
  const netChart = useMemo(
    () => dualAreaChartOption(netHist, "today"),
    [netHist],
  );

  const health =
    cpuTemp == null
      ? { ok: nicUp > 0, label: nicUp > 0 ? "System Healthy" : "No link", note: nicUp > 0 ? "Everything looks good." : "No network adapters are up." }
      : cpuTemp >= 90
        ? { ok: false, label: "Running Hot", note: `CPU at ${formatCelsius(cpuTemp)}` }
        : cpuTemp >= 75
          ? { ok: true, label: "Warm", note: `CPU at ${formatCelsius(cpuTemp)}` }
          : { ok: true, label: "System Healthy", note: `CPU at ${formatCelsius(cpuTemp)}` };

  const sparkCpu = useMemo(() => sparkOption(spark.cpu, GAUGE_COLORS.cpu), [spark.cpu]);
  const sparkMem = useMemo(() => sparkOption(spark.memory, GAUGE_COLORS.memory), [spark.memory]);
  const sparkDisk = useMemo(() => sparkOption(spark.disk, GAUGE_COLORS.disk), [spark.disk]);
  const sparkNet = useMemo(() => sparkOption(spark.network, GAUGE_COLORS.network), [spark.network]);

  return (
    <div className="space-y-5">
      {error && (
        <div className="rounded-md border border-[var(--color-destructive)]/40 bg-[var(--color-destructive)]/10 px-3 py-2 text-sm">
          {error}
        </div>
      )}

      <div className="grid gap-5 xl:grid-cols-3">
        <div className="space-y-5 xl:col-span-2">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/90 p-6 backdrop-blur">
            <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
              <div className="flex flex-col items-center gap-2">
                <GaugeRing
                  label="CPU"
                  icon={Cpu}
                  color={GAUGE_COLORS.cpu}
                  percent={cpu?.utilization ?? null}
                  display={formatPercent(cpu?.utilization, 0)}
                  sub={
                    cpu
                      ? `${cpu.frequency_mhz != null ? (cpu.frequency_mhz / 1000).toFixed(1) : "—"} / ${cpu.logical_cores} cores`
                      : "—"
                  }
                />
                <ReactECharts option={sparkCpu} style={{ height: 36, width: "80%" }} />
              </div>
              <div className="flex flex-col items-center gap-2">
                <GaugeRing
                  label="Memory"
                  icon={MemoryStick}
                  color={GAUGE_COLORS.memory}
                  percent={mem?.utilization ?? null}
                  display={formatPercent(mem?.utilization, 0)}
                  sub={
                    mem
                      ? `${gibShort(mem.used_bytes)} / ${gibShort(mem.total_bytes)} GB`
                      : "—"
                  }
                />
                <ReactECharts option={sparkMem} style={{ height: 36, width: "80%" }} />
              </div>
              <div className="flex flex-col items-center gap-2">
                <GaugeRing
                  label="Disk"
                  icon={Database}
                  color={GAUGE_COLORS.disk}
                  percent={diskUtil}
                  display={formatPercent(diskUtil, 0)}
                  sub={
                    diskTotal > 0
                      ? `${gibShort(diskUsed)} / ${gibShort(diskTotal)} GB`
                      : "No disks"
                  }
                />
                <ReactECharts option={sparkDisk} style={{ height: 36, width: "80%" }} />
              </div>
              <div className="flex flex-col items-center gap-2">
                <GaugeRing
                  label="Network"
                  icon={Globe}
                  color={GAUGE_COLORS.network}
                  percent={null}
                  display={`${netMb.value} ${netMb.unit}`}
                  sub={
                    <span className="inline-flex items-center gap-2 tabular-nums">
                      <span className="inline-flex items-center gap-0.5 text-[var(--color-success)]">
                        <ArrowDown className="h-3 w-3" />
                        {formatMbpsValue(totalDown).value}
                      </span>
                      <span className="inline-flex items-center gap-0.5 text-[var(--color-warning)]">
                        <ArrowUp className="h-3 w-3" />
                        {formatMbpsValue(totalUp).value}
                      </span>
                    </span>
                  }
                />
                <ReactECharts option={sparkNet} style={{ height: 36, width: "80%" }} />
              </div>
            </div>
          </div>

          <div className="grid gap-5 lg:grid-cols-2">
            <Card>
              <CardHeader className="pb-1">
                <PanelHeader
                  icon={Activity}
                  color={GAUGE_COLORS.cpu}
                  title="CPU Usage"
                  trailing={
                    <span
                      className="text-lg font-semibold tabular-nums"
                      style={{ color: GAUGE_COLORS.cpu }}
                    >
                      {formatPercent(cpu?.utilization, 0)}
                    </span>
                  }
                />
              </CardHeader>
              <CardContent>
                <ReactECharts option={cpuChart} style={{ height: 220 }} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-1">
                <PanelHeader
                  icon={Network}
                  color={GAUGE_COLORS.network}
                  title="Network Traffic"
                  trailing={
                    <span className="text-xs text-[var(--color-muted-foreground)]">
                      Today
                    </span>
                  }
                />
              </CardHeader>
              <CardContent>
                <ReactECharts option={netChart} style={{ height: 220 }} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-1">
                <PanelHeader
                  icon={Database}
                  color={GAUGE_COLORS.disk}
                  title="Storage"
                />
              </CardHeader>
              <CardContent className="space-y-4">
                {disks.length === 0 && (
                  <p className="text-sm text-[var(--color-muted-foreground)]">
                    No disks detected.
                  </p>
                )}
                {disks.map((d) => (
                  <div key={d.id} className="space-y-1.5">
                    <div className="flex items-center justify-between text-sm">
                      <span className="truncate">
                        {d.name || d.mount_point}
                        <span
                          className={cn(
                            "ml-2 rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide",
                            d.kind === "ssd"
                              ? "bg-[var(--color-primary)]/15 text-[var(--color-primary)]"
                              : "bg-[var(--color-warning)]/15 text-[var(--color-warning)]",
                          )}
                        >
                          {d.kind}
                        </span>
                      </span>
                      <span className="text-[var(--color-muted-foreground)] tabular-nums">
                        {formatPercent(d.utilization, 0)}
                      </span>
                    </div>
                    <UsageBar
                      percent={d.utilization}
                      color={d.utilization >= 90 ? "var(--color-destructive)" : GAUGE_COLORS.cpu}
                    />
                    <div className="text-xs text-[var(--color-muted-foreground)] tabular-nums">
                      {formatBytes(d.used_bytes)} / {formatBytes(d.total_bytes)}
                    </div>
                  </div>
                ))}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-1">
                <PanelHeader
                  icon={MemoryStick}
                  color={GAUGE_COLORS.memory}
                  title="Memory"
                  trailing={
                    <span
                      className="text-lg font-semibold tabular-nums"
                      style={{ color: GAUGE_COLORS.memory }}
                    >
                      {formatPercent(mem?.utilization, 0)}
                    </span>
                  }
                />
              </CardHeader>
              <CardContent className="space-y-3 text-sm">
                <div className="flex justify-between">
                  <span className="text-[var(--color-muted-foreground)]">Used</span>
                  <span className="tabular-nums">
                    {mem ? formatBytes(mem.used_bytes) : "—"}
                  </span>
                </div>
                <UsageBar percent={mem?.utilization ?? null} color={GAUGE_COLORS.memory} />
                <div className="flex justify-between">
                  <span className="text-[var(--color-muted-foreground)]">Available</span>
                  <span className="tabular-nums">
                    {mem ? formatBytes(mem.available_bytes) : "—"}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--color-muted-foreground)]">Total</span>
                  <span className="tabular-nums">
                    {mem ? formatBytes(mem.total_bytes) : "—"}
                  </span>
                </div>
              </CardContent>
            </Card>
          </div>
        </div>

        <div className="space-y-5">
          <Card>
            <CardContent className="pt-5">
              <div className="flex items-start gap-2.5">
                <CheckCircle2
                  className="mt-0.5 h-5 w-5 shrink-0"
                  style={{ color: health.ok ? GAUGE_COLORS.cpu : "var(--color-destructive)" }}
                />
                <div>
                  <div
                    className="font-semibold"
                    style={{ color: health.ok ? GAUGE_COLORS.cpu : "var(--color-destructive)" }}
                  >
                    {health.label}
                  </div>
                  <div className="text-sm text-[var(--color-muted-foreground)]">
                    {health.note}
                  </div>
                </div>
              </div>
              <div className="mt-4 space-y-1.5 border-t border-[var(--color-border)] pt-4 text-sm">
                <div className="flex justify-between">
                  <span className="text-[var(--color-muted-foreground)]">Adapters</span>
                  <span className="tabular-nums">
                    {nicUp}/{nics.length} up
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--color-muted-foreground)]">CPU temp</span>
                  <span className="tabular-nums">
                    {cpuTemp != null ? formatCelsius(cpuTemp) : "—"}
                  </span>
                </div>
                {gpuTemps.map((g) => (
                  <div key={g.label} className="flex justify-between">
                    <span className="truncate text-[var(--color-muted-foreground)]">
                      {g.label}
                    </span>
                    <span className="tabular-nums">{g.value}</span>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <PanelHeader
                icon={Network}
                color={GAUGE_COLORS.network}
                title="Top Apps"
                trailing={
                  <span className="text-xs text-[var(--color-muted-foreground)]">
                    Today · traffic
                  </span>
                }
              />
            </CardHeader>
            <CardContent>
              {topApps.length === 0 ? (
                <p className="text-sm text-[var(--color-muted-foreground)]">
                  No usage recorded yet.
                </p>
              ) : (
                <div className="text-sm">
                  <div className="grid grid-cols-[1fr_auto_auto] gap-x-4 border-b border-[var(--color-border)] pb-2 text-xs text-[var(--color-muted-foreground)]">
                    <span>Process</span>
                    <span className="text-right">↓</span>
                    <span className="text-right">↑</span>
                  </div>
                  {topApps.map((a) => (
                    <div
                      key={a.process_name}
                      className="grid grid-cols-[1fr_auto_auto] items-center gap-x-4 border-b border-[var(--color-border)]/50 py-2 last:border-0"
                    >
                      <span className="truncate">{a.process_name}</span>
                      <span className="text-right tabular-nums text-[var(--color-success)]">
                        {formatBytes(a.bytes_in)}
                      </span>
                      <span className="text-right tabular-nums text-[var(--color-warning)]">
                        {formatBytes(a.bytes_out)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <PanelHeader
                icon={Cpu}
                color={GAUGE_COLORS.memory}
                title="System Info"
              />
            </CardHeader>
            <CardContent className="space-y-2.5 text-sm">
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">Hostname</span>
                <span className="truncate">{navigator.platform || "—"}</span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">CPU</span>
                <span className="max-w-[60%] truncate text-right">
                  {inventory?.cpu.brand || inventory?.cpu.model || "—"}
                </span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">Cores</span>
                <span className="tabular-nums">
                  {cpu ? `${cpu.physical_cores ?? "?"}C / ${cpu.logical_cores}T` : "—"}
                </span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">Memory</span>
                <span className="tabular-nums">
                  {mem ? formatBytes(mem.total_bytes) : "—"}
                </span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">Storage</span>
                <span className="tabular-nums">
                  {diskTotal > 0 ? formatBytes(diskTotal) : "—"}
                </span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">GPU</span>
                <span className="max-w-[60%] truncate text-right">{gpuBrief}</span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-[var(--color-muted-foreground)]">Motherboard</span>
                <span className="max-w-[60%] truncate text-right">
                  {inventory
                    ? [inventory.motherboard.brand, inventory.motherboard.model]
                        .filter(Boolean)
                        .join(" ") || "—"
                    : "—"}
                </span>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
