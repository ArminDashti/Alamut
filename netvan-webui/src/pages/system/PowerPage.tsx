import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  Power,
  PowerOff,
  RotateCcw,
  Zap,
} from "lucide-react";
import { HistoryFilter, customToTs } from "@/components/HistoryFilter";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { rpc, type HistoryRange, type PowerEvent, type PowerEventKind } from "@/lib/api";
import { cn } from "@/lib/utils";

const KIND_META: Record<
  PowerEventKind,
  { label: string; icon: typeof Power; accent: string; badge: string }
> = {
  boot: {
    label: "Turn on",
    icon: Zap,
    accent: "text-emerald-400",
    badge: "bg-emerald-500/15 text-emerald-300",
  },
  shutdown: {
    label: "Shutdown",
    icon: PowerOff,
    accent: "text-slate-300",
    badge: "bg-slate-500/15 text-slate-200",
  },
  restart: {
    label: "Restart",
    icon: RotateCcw,
    accent: "text-sky-400",
    badge: "bg-sky-500/15 text-sky-300",
  },
  crash: {
    label: "Crash",
    icon: AlertTriangle,
    accent: "text-rose-400",
    badge: "bg-rose-500/15 text-rose-300",
  },
};

function formatTs(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86_400);
  const h = Math.floor((secs % 86_400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function PowerPage() {
  const [range, setRange] = useState<HistoryRange>("months");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [filter, setFilter] = useState<PowerEventKind | "all">("all");
  const [uptimeSecs, setUptimeSecs] = useState(0);
  const [events, setEvents] = useState<PowerEvent[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const start_ts = range === "custom" ? customToTs(customStart) : null;
    const end_ts = range === "custom" ? customToTs(customEnd) : null;
    rpc<{
      type: "PowerEvents";
      data: { uptime_secs: number; events: PowerEvent[] };
    }>({
      method: "GetPowerEvents",
      params: { range, start_ts, end_ts },
    })
      .then((r) => {
        if (!alive) return;
        setUptimeSecs(r.data.uptime_secs);
        setEvents(r.data.events);
        setError(null);
      })
      .catch((e) => {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [range, customStart, customEnd]);

  const counts = useMemo(() => {
    const c: Record<PowerEventKind, number> = {
      boot: 0,
      shutdown: 0,
      restart: 0,
      crash: 0,
    };
    for (const e of events) c[e.kind] += 1;
    return c;
  }, [events]);

  const visible = useMemo(
    () => (filter === "all" ? events : events.filter((e) => e.kind === filter)),
    [events, filter],
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-lg font-semibold">Power &amp; uptime</h1>
          <p className="text-sm text-[var(--color-muted-foreground)]">
            Boot, shutdown, restart, and unexpected stops from the Windows System event log.
          </p>
        </div>
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-muted)]/30 px-3 py-2 text-sm">
          <span className="text-[var(--color-muted-foreground)]">Uptime </span>
          <span className="font-semibold tabular-nums">{formatUptime(uptimeSecs)}</span>
        </div>
      </div>

      {error && <p className="text-sm text-red-400">{error}</p>}

      <HistoryFilter
        value={range}
        onChange={setRange}
        customStart={customStart}
        customEnd={customEnd}
        onCustomStart={setCustomStart}
        onCustomEnd={setCustomEnd}
      />

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {(Object.keys(KIND_META) as PowerEventKind[]).map((kind) => {
          const meta = KIND_META[kind];
          const Icon = meta.icon;
          return (
            <button
              key={kind}
              type="button"
              onClick={() => setFilter(filter === kind ? "all" : kind)}
              className={cn(
                "rounded-lg border border-[var(--color-border)] p-3 text-left transition-colors hover:bg-[var(--color-muted)]/50",
                filter === kind && "border-[var(--color-primary)] bg-[var(--color-muted)]/60",
              )}
            >
              <div className="flex items-center gap-2">
                <Icon className={cn("h-4 w-4", meta.accent)} />
                <span className="text-sm font-medium">{meta.label}</span>
              </div>
              <div className="mt-2 text-2xl font-semibold tabular-nums">{counts[kind]}</div>
              <div className="text-[11px] text-[var(--color-muted-foreground)]">in selected range</div>
            </button>
          );
        })}
      </div>

      <Card>
        <CardHeader className="flex-row items-center justify-between gap-2 py-3">
          <CardTitle className="text-sm font-medium">Event timeline</CardTitle>
          {filter !== "all" && (
            <button
              type="button"
              className="text-xs text-[var(--color-primary)] hover:underline"
              onClick={() => setFilter("all")}
            >
              Show all
            </button>
          )}
        </CardHeader>
        <CardContent className="space-y-2 p-0 pb-2">
          {visible.length === 0 ? (
            <p className="px-4 py-6 text-sm text-[var(--color-muted-foreground)]">
              No power events in this range.
            </p>
          ) : (
            visible.map((ev, i) => {
              const meta = KIND_META[ev.kind];
              const Icon = meta.icon;
              return (
                <div
                  key={`${ev.ts}-${ev.event_id}-${i}`}
                  className="flex gap-3 border-b border-[var(--color-border)]/70 px-4 py-3 last:border-b-0"
                >
                  <div className="mt-0.5">
                    <Icon className={cn("h-4 w-4", meta.accent)} aria-hidden />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className={cn("rounded-full px-2 py-0.5 text-[11px] font-medium", meta.badge)}>
                        {meta.label}
                      </span>
                      <span className="text-xs text-[var(--color-muted-foreground)] tabular-nums">
                        {formatTs(ev.ts)}
                      </span>
                      <span className="text-[11px] text-[var(--color-muted-foreground)]">
                        ID {ev.event_id} · {ev.source}
                      </span>
                    </div>
                    <p className="mt-1 text-sm text-[var(--color-foreground)]">{ev.message}</p>
                  </div>
                </div>
              );
            })
          )}
        </CardContent>
      </Card>

      <p className="text-[11px] text-[var(--color-muted-foreground)]">
        <Power className="mr-1 inline h-3 w-3" />
        Remote power-on is not available from the OS while the machine is off; this page records history only.
      </p>
    </div>
  );
}
