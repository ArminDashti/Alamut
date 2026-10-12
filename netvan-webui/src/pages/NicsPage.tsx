import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Bluetooth,
  Cable,
  Copy,
  Gauge,
  Globe,
  Network,
  Power,
  RefreshCw,
  Router,
  Search,
  Shield,
  Wifi,
  type LucideIcon,
} from "lucide-react";
import ReactECharts from "echarts-for-react";
import { HistoryFilter, customToTs } from "@/components/HistoryFilter";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { rpc, type HistoryRange, type NicInfo } from "@/lib/api";
import { aggregateGridSum, buildPeriodColumns } from "@/lib/latencyPeriod";
import { cn, bytesToGb, formatSpeedMb, formatUsageGb } from "@/lib/utils";

const LIVE_POINTS = 60;
const DOWNLOAD_COLOR = "#3d9cf0";
const UPLOAD_COLOR = "#2dd4bf";

function NicTypeIcon({ mediaType }: { mediaType: string }) {
  const cls = "h-4 w-4 shrink-0 text-[var(--color-muted-foreground)]";
  switch (mediaType) {
    case "Wi-Fi":
      return <Wifi className={cls} aria-label="Wi-Fi" />;
    case "Bluetooth":
      return <Bluetooth className={cls} aria-label="Bluetooth" />;
    case "Ethernet":
      return <Cable className={cls} aria-label="LAN" />;
    default:
      return <Network className={cls} aria-label={mediaType} />;
  }
}

function nicPrimaryLabel(n: Pick<NicInfo, "name" | "description">) {
  return n.description?.trim() || n.name;
}

function nicSecondaryLabel(n: Pick<NicInfo, "name" | "description" | "media_type">) {
  if (n.description?.trim()) return n.name;
  return n.media_type;
}

function adminUp(n: Pick<NicInfo, "admin_status">) {
  const a = n.admin_status.toLowerCase();
  return a === "up" || a === "enabled";
}

function usageDeltas(
  usage: { ts: number; rx_bytes: number; tx_bytes: number }[],
): { ts: number; rx: number; tx: number }[] {
  const out: { ts: number; rx: number; tx: number }[] = [];
  for (let i = 1; i < usage.length; i++) {
    const prev = usage[i - 1]!;
    const cur = usage[i]!;
    const rx = cur.rx_bytes >= prev.rx_bytes ? cur.rx_bytes - prev.rx_bytes : 0;
    const tx = cur.tx_bytes >= prev.tx_bytes ? cur.tx_bytes - prev.tx_bytes : 0;
    out.push({ ts: cur.ts, rx, tx });
  }
  return out;
}

function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("min-w-0", className)}>
      <dt className="text-[10px] font-medium uppercase tracking-wide text-[var(--color-muted-foreground)]">
        {label}
      </dt>
      <dd className="mt-0.5 truncate text-sm">{children}</dd>
    </div>
  );
}

function ConfigRow({
  icon: Icon,
  label,
  values,
}: {
  icon: LucideIcon;
  label: string;
  values: string[];
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="flex shrink-0 items-center gap-2 text-sm text-[var(--color-muted-foreground)]">
        <Icon className="h-4 w-4" />
        {label}
      </span>
      <div className="flex min-w-0 flex-wrap justify-end gap-1">
        {values.length === 0 && <span className="text-sm text-[var(--color-muted-foreground)]">—</span>}
        {values.map((v) => (
          <span
            key={v}
            className="truncate rounded-md border border-[var(--color-border)] px-2 py-0.5 font-mono text-xs"
          >
            {v}
          </span>
        ))}
      </div>
    </div>
  );
}

function UsageTile({
  icon: Icon,
  label,
  value,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <div className="space-y-2 rounded-lg border border-[var(--color-border)] p-3">
      <div className="flex items-center gap-1.5 text-xs text-[var(--color-muted-foreground)]">
        <Icon className="h-3.5 w-3.5" />
        {label}
      </div>
      <div className="text-lg font-semibold tabular-nums">{value}</div>
    </div>
  );
}

export function NicsPage() {
  const [nics, setNics] = useState<NicInfo[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [search, setSearch] = useState("");
  const [range, setRange] = useState<HistoryRange>("today");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [usage, setUsage] = useState<{ ts: number; rx_bytes: number; tx_bytes: number }[]>([]);
  const [liveHistory, setLiveHistory] = useState<{ rx: number; tx: number }[]>([]);
  const [ctxMenu, setCtxMenu] = useState<{
    x: number;
    y: number;
    nic: NicInfo;
  } | null>(null);
  const [actionMsg, setActionMsg] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const ctxRef = useRef<HTMLDivElement>(null);

  const refreshNics = async () => {
    const r = await rpc<{ type: "Nics"; data: NicInfo[] }>({ method: "ListNics" });
    const list = r.data.filter((n) => n.media_type !== "Loopback");
    setNics(list);
    return list;
  };

  const mediaTypes = useMemo(() => [...new Set(nics.map((n) => n.media_type))].sort(), [nics]);

  const filteredNics = useMemo(() => {
    const q = search.trim().toLowerCase();
    return nics.filter(
      (n) =>
        (typeFilter === "all" || n.media_type === typeFilter) &&
        (!q ||
          nicPrimaryLabel(n).toLowerCase().includes(q) ||
          nicSecondaryLabel(n).toLowerCase().includes(q)),
    );
  }, [nics, typeFilter, search]);

  const activeCount = nics.filter((n) => n.oper_status === "Up").length;

  useEffect(() => {
    refreshNics().then((list) => {
      if (!selected && list[0]) setSelected(list[0].id);
    });
  }, []);

  useEffect(() => {
    if (!selected) return;
    if (filteredNics.some((n) => n.id === selected)) return;
    setSelected(filteredNics[0]?.id ?? "");
  }, [filteredNics, selected]);

  useEffect(() => {
    const id = setInterval(() => {
      void refreshNics();
    }, 2000);
    return () => clearInterval(id);
  }, []);

  // Rolling live-traffic buffer for the selected adapter; restarted on selection change.
  useEffect(() => {
    setLiveHistory([]);
  }, [selected]);

  useEffect(() => {
    const cur = nics.find((n) => n.id === selected);
    if (!cur) return;
    setLiveHistory((h) => [...h, { rx: cur.rx_bps, tx: cur.tx_bps }].slice(-LIVE_POINTS));
  }, [nics, selected]);

  useEffect(() => {
    if (!selected) return;
    const start_ts = range === "custom" ? customToTs(customStart) : null;
    const end_ts = range === "custom" ? customToTs(customEnd) : null;
    rpc<{
      type: "BandwidthHistory";
      data: { ts: number; rx_bytes: number; tx_bytes: number }[];
    }>({
      method: "GetBandwidthHistory",
      params: { nic_id: selected, range, start_ts, end_ts },
    }).then((u) => setUsage(u.data));
  }, [selected, range, customStart, customEnd]);

  useEffect(() => {
    if (!ctxMenu) return;
    const close = (e: MouseEvent) => {
      if (ctxRef.current && !ctxRef.current.contains(e.target as Node)) {
        setCtxMenu(null);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setCtxMenu(null);
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [ctxMenu]);

  const nic = nics.find((n) => n.id === selected);
  const deltas = useMemo(() => usageDeltas(usage), [usage]);

  const columns = useMemo(
    () => buildPeriodColumns(range, customStart, customEnd),
    [range, customStart, customEnd],
  );

  const cells = useMemo(
    () =>
      aggregateGridSum(
        ["Download", "Upload"],
        columns,
        deltas.flatMap((p) => [
          { ts: p.ts, value: p.rx, rowKey: "Download" },
          { ts: p.ts, value: p.tx, rowKey: "Upload" },
        ]),
      ),
    [columns, deltas],
  );

  const hourly = useMemo(
    () =>
      columns.map((c) => ({
        label: c.label,
        down: cells.get(`Download::${c.key}`)?.sum ?? 0,
        up: cells.get(`Upload::${c.key}`)?.sum ?? 0,
      })),
    [columns, cells],
  );

  const totalDown = hourly.reduce((s, h) => s + h.down, 0);
  const totalUp = hourly.reduce((s, h) => s + h.up, 0);
  const peak = hourly.reduce(
    (p, h) => (h.down + h.up > p.down + p.up ? h : p),
    { label: "", down: 0, up: 0 },
  );

  const usageOption = useMemo(
    () => ({
      backgroundColor: "transparent",
      textStyle: { color: "#8b9bb4" },
      animation: false,
      grid: { left: 48, right: 12, top: 32, bottom: 28 },
      tooltip: { trigger: "axis" },
      legend: { top: 0, right: 0, itemWidth: 10, itemHeight: 10, textStyle: { color: "#8b9bb4" } },
      xAxis: {
        type: "category",
        data: hourly.map((h) => h.label),
        axisLabel: { color: "#8b9bb4", hideOverlap: true },
      },
      yAxis: {
        type: "value",
        axisLabel: { color: "#8b9bb4", formatter: "{value} GB" },
        splitLine: { lineStyle: { color: "#243049" } },
      },
      series: [
        {
          name: "Download",
          type: "bar",
          color: DOWNLOAD_COLOR,
          data: hourly.map((h) => Number(bytesToGb(h.down).toFixed(3))),
        },
        {
          name: "Upload",
          type: "bar",
          color: UPLOAD_COLOR,
          data: hourly.map((h) => Number(bytesToGb(h.up).toFixed(3))),
        },
      ],
    }),
    [hourly],
  );

  const liveOption = useMemo(
    () => ({
      backgroundColor: "transparent",
      textStyle: { color: "#8b9bb4" },
      animation: false,
      grid: { left: 44, right: 8, top: 8, bottom: 8 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", show: false, data: liveHistory.map((_, i) => i) },
      yAxis: {
        type: "value",
        min: 0,
        axisLabel: { color: "#8b9bb4", formatter: "{value} Mb" },
        splitLine: { lineStyle: { color: "#243049" } },
      },
      series: [
        {
          name: "Download",
          type: "line",
          smooth: true,
          showSymbol: false,
          areaStyle: { opacity: 0.15 },
          color: DOWNLOAD_COLOR,
          data: liveHistory.map((p) => Number((p.rx / 1_000_000).toFixed(2))),
        },
        {
          name: "Upload",
          type: "line",
          smooth: true,
          showSymbol: false,
          areaStyle: { opacity: 0.15 },
          color: UPLOAD_COLOR,
          data: liveHistory.map((p) => Number((p.tx / 1_000_000).toFixed(2))),
        },
      ],
    }),
    [liveHistory],
  );

  const setNicEnabled = async (target: NicInfo, enabled: boolean) => {
    setCtxMenu(null);
    setActionBusy(true);
    setActionMsg(null);
    try {
      await rpc({ method: "SetNicEnabled", params: { nic_id: target.id, enabled } });
      setActionMsg(`${target.name}: ${enabled ? "enabled" : "disabled"}`);
      await refreshNics();
    } catch (e) {
      setActionMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setActionBusy(false);
    }
  };

  const copyInfo = async (n: NicInfo) => {
    const text = [
      nicPrimaryLabel(n),
      `GUID: ${n.guid}`,
      `MAC: ${n.mac ?? "—"}`,
      `IPv4: ${n.ipv4_addresses.join(", ") || "—"}`,
      `IPv6: ${n.ipv6_addresses.join(", ") || "—"}`,
      `Gateway: ${n.gateways.join(", ") || "—"}`,
      `DNS: ${n.dns_servers.join(", ") || "—"}`,
    ].join("\n");
    await navigator.clipboard.writeText(text);
    setActionMsg("Adapter info copied");
  };

  return (
    <div className="flex h-[calc(100vh-3rem)] min-h-[480px] flex-col gap-4">
      {actionMsg && (
        <div className="rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-3 py-2 text-sm">
          {actionMsg}
        </div>
      )}

      <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[270px_1fr_1fr]">
        <Card className="flex min-h-0 flex-col overflow-hidden">
          <CardHeader className="shrink-0 space-y-3 py-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">Adapters</CardTitle>
              <span className="rounded-full bg-[var(--color-muted)] px-2 py-0.5 text-xs text-[var(--color-muted-foreground)]">
                {nics.length} total
              </span>
            </div>
            <select
              className="h-8 w-full rounded-md border border-[var(--color-border)] bg-[var(--color-muted)] px-2 text-xs"
              value={typeFilter}
              onChange={(e) => setTypeFilter(e.target.value)}
            >
              <option value="all">All types</option>
              {mediaTypes.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
            <div className="relative">
              <Search className="pointer-events-none absolute top-2.5 left-2.5 h-3.5 w-3.5 text-[var(--color-muted-foreground)]" />
              <Input
                className="h-8 pl-8 text-xs"
                placeholder="Find adapter..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          </CardHeader>
          <CardContent className="min-h-0 flex-1 space-y-1 overflow-auto p-2">
            {filteredNics.map((n) => {
              const up = n.oper_status === "Up";
              return (
                <button
                  key={n.id}
                  type="button"
                  onClick={() => setSelected(n.id)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setCtxMenu({ x: e.clientX, y: e.clientY, nic: n });
                  }}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md border px-2.5 py-2 text-left text-sm transition-colors",
                    selected === n.id
                      ? "border-[var(--color-primary)] bg-[var(--color-muted)]"
                      : "border-transparent hover:bg-[var(--color-muted)]/60",
                  )}
                >
                  <span
                    className={cn(
                      "h-2 w-2 shrink-0 rounded-full",
                      up ? "bg-[var(--color-success)]" : "bg-[var(--color-destructive)]",
                    )}
                    title={n.oper_status}
                  />
                  <NicTypeIcon mediaType={n.media_type} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium leading-tight">
                      {nicPrimaryLabel(n)}
                    </span>
                    <span className="block truncate text-[11px] text-[var(--color-muted-foreground)]">
                      {nicSecondaryLabel(n)}
                    </span>
                  </span>
                </button>
              );
            })}
            {filteredNics.length === 0 && (
              <p className="p-2 text-sm text-[var(--color-muted-foreground)]">No adapters</p>
            )}
          </CardContent>
          <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--color-border)] px-3 py-2 text-xs text-[var(--color-muted-foreground)]">
            <span>
              {activeCount} active • {nics.length - activeCount} inactive
            </span>
            <Button variant="outline" size="sm" onClick={() => void refreshNics()}>
              <RefreshCw className="h-3.5 w-3.5" />
              Refresh
            </Button>
          </div>
        </Card>

        <div className="flex min-h-0 flex-col gap-4">
          <Card className="shrink-0 overflow-hidden">
            {nic ? (
              <>
                <CardHeader className="gap-1 py-4">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 space-y-1">
                      <div className="flex items-center gap-2">
                        <span
                          className={cn(
                            "h-2.5 w-2.5 shrink-0 rounded-full",
                            nic.oper_status === "Up"
                              ? "bg-[var(--color-success)]"
                              : "bg-[var(--color-destructive)]",
                          )}
                        />
                        <CardTitle className="truncate text-base">{nicPrimaryLabel(nic)}</CardTitle>
                      </div>
                      <CardDescription>{nicSecondaryLabel(nic)}</CardDescription>
                    </div>
                    <div className="flex shrink-0 gap-2">
                      <Button variant="outline" size="sm" onClick={() => void copyInfo(nic)}>
                        <Copy className="h-3.5 w-3.5" />
                        Copy info
                      </Button>
                      <Button
                        variant={adminUp(nic) ? "destructive" : "default"}
                        size="sm"
                        disabled={actionBusy}
                        onClick={() => void setNicEnabled(nic, !adminUp(nic))}
                      >
                        <Power className="h-3.5 w-3.5" />
                        {adminUp(nic) ? "Disconnect" : "Connect"}
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="pt-0">
                  <dl className="grid grid-cols-3 gap-x-6 gap-y-3">
                    <Field label="GUID" className="col-span-2">
                      <span className="font-mono text-xs">{nic.guid}</span>
                    </Field>
                    <Field label="Index">{nic.interface_index}</Field>
                    <Field label="MAC address">
                      <span className="font-mono text-xs">{nic.mac ?? "—"}</span>
                    </Field>
                    <Field label="Type">{nic.media_type}</Field>
                    <Field label="MTU">{nic.mtu ?? "—"}</Field>
                    <Field label="Admin status">
                      <span
                        className={cn(
                          "rounded px-1.5 py-0.5 text-xs font-medium",
                          adminUp(nic)
                            ? "bg-[var(--color-success)]/15 text-[var(--color-success)]"
                            : "bg-[var(--color-destructive)]/15 text-[var(--color-destructive)]",
                        )}
                      >
                        {nic.admin_status}
                      </span>
                    </Field>
                    <Field label="Link speed">
                      {nic.link_speed_bps ? formatSpeedMb(nic.link_speed_bps) : "—"}
                    </Field>
                    <Field label="DHCP">{nic.dhcp_enabled ? "Enabled" : "Disabled"}</Field>
                  </dl>
                </CardContent>
              </>
            ) : (
              <CardContent className="flex items-center justify-center py-10 text-sm text-[var(--color-muted-foreground)]">
                Select an adapter
              </CardContent>
            )}
          </Card>

          <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
            {nic && (
              <>
                <CardContent className="grid min-h-0 flex-1 gap-0 p-0 md:grid-cols-2">
                  <section className="min-w-0 space-y-4 p-4">
                    <h3 className="text-sm font-medium">Network configuration</h3>
                    <ConfigRow icon={Globe} label="IPv4" values={nic.ipv4_addresses} />
                    <ConfigRow icon={Globe} label="IPv6" values={nic.ipv6_addresses} />
                    <ConfigRow icon={Router} label="Gateway" values={nic.gateways} />
                    <ConfigRow icon={Shield} label="DNS" values={nic.dns_servers} />
                  </section>
                  <section className="min-w-0 space-y-3 border-t border-[var(--color-border)] p-4 md:border-t-0 md:border-l">
                    <div className="flex items-center justify-between">
                      <h3 className="text-sm font-medium">Live traffic</h3>
                      <span className="flex items-center gap-1.5 text-xs text-[var(--color-success)]">
                        <span className="h-2 w-2 rounded-full bg-[var(--color-success)]" />
                        Live
                      </span>
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <div className="flex items-center gap-1.5 text-xs text-[var(--color-muted-foreground)]">
                          <ArrowDown className="h-3.5 w-3.5" style={{ color: DOWNLOAD_COLOR }} />
                          Download
                        </div>
                        <div className="text-xl font-semibold tabular-nums" style={{ color: DOWNLOAD_COLOR }}>
                          {formatSpeedMb(nic.rx_bps)}/s
                        </div>
                      </div>
                      <div>
                        <div className="flex items-center gap-1.5 text-xs text-[var(--color-muted-foreground)]">
                          <ArrowUp className="h-3.5 w-3.5" style={{ color: UPLOAD_COLOR }} />
                          Upload
                        </div>
                        <div className="text-xl font-semibold tabular-nums" style={{ color: UPLOAD_COLOR }}>
                          {formatSpeedMb(nic.tx_bps)}/s
                        </div>
                      </div>
                    </div>
                    <ReactECharts option={liveOption} style={{ height: 140 }} />
                  </section>
                </CardContent>
                <div className="flex shrink-0 items-center justify-between border-t border-[var(--color-border)] px-4 py-2.5 text-xs">
                  <span className="text-[var(--color-muted-foreground)]">Counters</span>
                  <span className="tabular-nums">
                    {formatUsageGb(nic.rx_bytes)} ↓ • {formatUsageGb(nic.tx_bytes)} ↑
                  </span>
                </div>
              </>
            )}
          </Card>
        </div>

        <Card className="flex min-h-0 flex-col overflow-hidden">
          <CardHeader className="shrink-0 space-y-3 py-3">
            <CardTitle className="text-base">Usage history</CardTitle>
            <HistoryFilter
              value={range}
              onChange={setRange}
              customStart={customStart}
              customEnd={customEnd}
              onCustomStart={setCustomStart}
              onCustomEnd={setCustomEnd}
              labels={{ months: "Month" }}
            />
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto">
            <div className="grid grid-cols-3 gap-3">
              <UsageTile icon={ArrowDown} label="Total download" value={formatUsageGb(totalDown)} />
              <UsageTile icon={ArrowUp} label="Total upload" value={formatUsageGb(totalUp)} />
              <UsageTile
                icon={Gauge}
                label="Peak"
                value={peak.down + peak.up > 0 ? `${formatUsageGb(peak.down + peak.up)} at ${peak.label}` : "—"}
              />
            </div>
            <div className="min-h-0 flex-1">
              <div className="mb-1 text-xs text-[var(--color-muted-foreground)]">Bandwidth per period</div>
              {usage.length === 0 ? (
                <p className="py-10 text-center text-sm text-[var(--color-muted-foreground)]">
                  No usage samples yet
                </p>
              ) : (
                <ReactECharts option={usageOption} style={{ height: 280 }} />
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {ctxMenu && (
        <div
          ref={ctxRef}
          className="fixed z-50 min-w-[140px] rounded-md border border-[var(--color-border)] bg-[var(--color-card)] p-1 shadow-lg"
          style={{ left: ctxMenu.x, top: ctxMenu.y }}
        >
          <button
            type="button"
            disabled={actionBusy || adminUp(ctxMenu.nic)}
            className="block w-full rounded px-3 py-1.5 text-left text-sm hover:bg-[var(--color-muted)] disabled:opacity-40"
            onClick={() => setNicEnabled(ctxMenu.nic, true)}
          >
            Enable
          </button>
          <button
            type="button"
            disabled={actionBusy || !adminUp(ctxMenu.nic)}
            className="block w-full rounded px-3 py-1.5 text-left text-sm hover:bg-[var(--color-muted)] disabled:opacity-40"
            onClick={() => setNicEnabled(ctxMenu.nic, false)}
          >
            Disable
          </button>
        </div>
      )}
    </div>
  );
}
