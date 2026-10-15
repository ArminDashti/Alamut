import { useEffect, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { CircuitBoard, Cpu, Gpu, HardDrive, MemoryStick, type LucideIcon } from "lucide-react";
import { Card } from "@/components/ui/card";
import {
  rpc,
  type GpuInfo,
  type HardwareInventory,
  type ThermalSnapshot,
} from "@/lib/api";
import { formatBytes } from "@/lib/systemMetrics";
import { formatCelsius, pickGpuSensors } from "@/lib/thermal";
import { cn } from "@/lib/utils";

type Accent = {
  icon: string;
  bar: string;
  button: string;
};

const ACCENT = {
  cpu: {
    icon: "bg-sky-500/15 text-sky-400",
    bar: "bg-sky-500",
    button: "bg-sky-600 hover:bg-sky-500",
  },
  memory: {
    icon: "bg-violet-500/15 text-violet-400",
    bar: "bg-violet-500",
    button: "bg-violet-600 hover:bg-violet-500",
  },
  storage: {
    icon: "bg-emerald-500/15 text-emerald-400",
    bar: "bg-emerald-500",
    button: "bg-emerald-600 hover:bg-emerald-500",
  },
  gpu: {
    icon: "bg-blue-500/15 text-blue-400",
    bar: "bg-blue-500",
    button: "bg-blue-600 hover:bg-blue-500",
  },
  board: {
    icon: "bg-amber-500/15 text-amber-400",
    bar: "bg-amber-500",
    button: "bg-amber-600 hover:bg-amber-500",
  },
} satisfies Record<string, Accent>;

function formatMhz(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value) || value <= 0) return "—";
  if (value >= 1000) return `${(value / 1000).toFixed(2)} GHz`;
  return `${value} MHz`;
}

function formatMemMhz(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value) || value <= 0) return "—";
  return `${value} MHz`;
}

function brandModel(brand: string, model: string): string {
  const b = brand.trim();
  const m = model.trim();
  if (b && m) {
    if (m.toLowerCase().includes(b.toLowerCase())) return m;
    return `${b} ${m}`;
  }
  return b || m || "—";
}

function orDash(value: string | null | undefined): string {
  const v = value?.trim();
  return v ? v : "—";
}

function countText(value: number | null | undefined): string {
  return value != null && value > 0 ? String(value) : "—";
}

function gpuMemoryKind(gpu: GpuInfo): "Shared" | "Dedicated" {
  const s = `${gpu.brand} ${gpu.model}`.toLowerCase();
  if (/\barc\b|rtx|gtx|radeon rx|geforce|quadro/.test(s)) return "Dedicated";
  if (/uhd|iris|intel|radeon\(tm\) graphics/.test(s)) return "Shared";
  return "Dedicated";
}

function DetailCard({
  icon: Icon,
  accent,
  title,
  rows,
  action,
}: {
  icon: LucideIcon;
  accent: Accent;
  title: string;
  rows: { label: string; value: ReactNode }[];
  action?: { to: string; label: string };
}) {
  return (
    <Card className="flex h-full flex-col">
      <div className="flex items-center gap-2 px-4 pt-4">
        <div className={cn("flex h-8 w-8 items-center justify-center rounded-lg", accent.icon)}>
          <Icon className="h-4 w-4" aria-hidden />
        </div>
        <h2 className="text-sm font-semibold">{title}</h2>
      </div>
      <div className="flex-1 px-4 py-2">
        {rows.map((row) => (
          <div
            key={row.label}
            className="flex items-center justify-between gap-3 border-b border-[var(--color-border)]/70 py-2.5 text-sm last:border-b-0"
          >
            <span className="text-[var(--color-muted-foreground)]">{row.label}</span>
            <span className="text-right font-medium">{row.value}</span>
          </div>
        ))}
      </div>
      {action ? (
        <div className="p-3 pt-1">
          <Link
            to={action.to}
            className={cn(
              "flex h-9 items-center justify-center rounded-md text-sm font-medium text-white transition-colors",
              accent.button,
            )}
          >
            {action.label}
          </Link>
        </div>
      ) : null}
    </Card>
  );
}

export function InfoPage() {
  const [inv, setInv] = useState<HardwareInventory | null>(null);
  const [gpuTemps, setGpuTemps] = useState<{ label: string; value: string }[]>([]);
  const [cpuTemp, setCpuTemp] = useState("—");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    rpc<{ type: "HardwareInventory"; data: HardwareInventory }>({
      method: "GetHardwareInventory",
    })
      .then((r) => {
        if (!alive) return;
        setInv(r.data);
        setError(null);
      })
      .catch((e) => {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      });
    const loadTemps = () => {
      rpc<{ type: "ThermalSnapshot"; data: ThermalSnapshot }>({
        method: "GetThermalSnapshot",
      })
        .then((r) => {
          if (!alive) return;
          setGpuTemps(
            pickGpuSensors(r.data.sensors).map((s) => ({
              label: s.hardware_name,
              value: formatCelsius(s.celsius),
            })),
          );
          const cpu = r.data.sensors.find((s) => s.hardware_kind === "cpu" && s.celsius != null);
          setCpuTemp(formatCelsius(cpu?.celsius));
        })
        .catch(() => {
          if (alive) setGpuTemps([]);
        });
    };
    loadTemps();
    const id = setInterval(loadTemps, 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  const cpu = inv?.cpu;
  const mem = inv?.memory;
  const mb = inv?.motherboard;
  const disks = inv?.disks ?? [];
  const gpus = inv?.gpus ?? [];

  return (
    <div className="space-y-4">
      {error ? <p className="text-sm text-red-400">{error}</p> : null}

      <div className="grid gap-4 lg:grid-cols-3">
        <DetailCard
          icon={Cpu}
          accent={ACCENT.cpu}
          title="Processor"
          rows={[
            { label: "Model", value: brandModel(cpu?.brand ?? "", cpu?.model ?? "") },
            { label: "Base Speed", value: formatMhz(cpu?.base_speed_mhz) },
            { label: "Cores", value: countText(cpu?.physical_cores) },
            { label: "Logical Processors", value: countText(cpu?.logical_processors) },
            {
              label: "Temperature",
              value:
                cpuTemp === "—" ? (
                  "—"
                ) : (
                  <span className="rounded-full bg-sky-500/20 px-2.5 py-0.5 text-xs font-medium text-sky-300">
                    {cpuTemp}
                  </span>
                ),
            },
          ]}
        />
        <DetailCard
          icon={MemoryStick}
          accent={ACCENT.memory}
          title="Memory"
          action={{ to: "/system/memory", label: "View Memory Details" }}
          rows={[
            { label: "Total Size", value: mem ? formatBytes(mem.size_bytes) : "—" },
            { label: "Type", value: orDash(mem?.memory_type) },
            { label: "Form Factor", value: orDash(mem?.form_factor) },
            { label: "Modules", value: countText(mem?.modules) },
            {
              label: "Capacity per Module",
              value: mem?.module_size_bytes != null ? formatBytes(mem.module_size_bytes) : "—",
            },
            { label: "Configured Speed", value: formatMemMhz(mem?.configured_speed_mhz) },
            { label: "Rated Speed", value: formatMemMhz(mem?.speed_mhz) },
          ]}
        />
        {disks.length === 0 ? (
          <DetailCard
            icon={HardDrive}
            accent={ACCENT.storage}
            title="Storage"
            rows={[
              { label: "Model", value: "—" },
              { label: "Capacity", value: "—" },
            ]}
          />
        ) : (
          disks.map((d, i) => (
            <DetailCard
              key={`${d.kind}-${i}-${d.model}`}
              icon={HardDrive}
              accent={ACCENT.storage}
              title={disks.length > 1 ? `Storage ${i + 1}` : "Storage"}
              action={{
                to: d.kind === "hdd" ? "/system/hdd" : "/system/ssd",
                label: "View Storage Details",
              }}
              rows={[
                { label: "Model", value: brandModel(d.brand, d.model) },
                { label: "Capacity", value: formatBytes(d.capacity_bytes) },
                { label: "Interface", value: orDash(d.interface_type) },
                { label: "Media Type", value: orDash(d.media_type) },
                { label: "Partitions", value: d.partitions != null ? String(d.partitions) : "—" },
                { label: "Firmware", value: orDash(d.firmware_revision) },
                { label: "Serial Number", value: orDash(d.serial_number) },
              ]}
            />
          ))
        )}
        {gpus.length === 0 ? (
          <DetailCard
            icon={Gpu}
            accent={ACCENT.gpu}
            title="Graphics"
            rows={[{ label: "Adapter", value: "No GPU detected" }]}
          />
        ) : (
          gpus.map((g, i) => {
            const kind = gpuMemoryKind(g);
            const memText = g.memory_bytes != null ? formatBytes(g.memory_bytes) : "—";
            return (
              <DetailCard
                key={`gpu-${i}-${g.model}`}
                icon={Gpu}
                accent={ACCENT.gpu}
                title={gpus.length > 1 ? `Graphics ${i + 1}` : "Graphics"}
                rows={[
                  { label: "Adapter", value: brandModel(g.brand, g.model) },
                  { label: "Dedicated Memory", value: kind === "Dedicated" ? memText : "—" },
                  { label: "Shared Memory", value: kind === "Shared" ? memText : "—" },
                  {
                    label: "CUDA Cores",
                    value: g.cuda_cores != null ? g.cuda_cores.toLocaleString() : "—",
                  },
                  {
                    label: "Tensor Cores",
                    value: g.tensor_cores != null ? g.tensor_cores.toLocaleString() : "—",
                  },
                  { label: "Temperature", value: gpuTemps[i]?.value ?? "—" },
                ]}
              />
            );
          })
        )}
        <DetailCard
          icon={CircuitBoard}
          accent={ACCENT.board}
          title="Motherboard"
          rows={[
            { label: "Manufacturer", value: orDash(mb?.brand) },
            { label: "Model", value: orDash(mb?.model) },
            { label: "RAM Slots", value: mb?.ram_slots != null ? String(mb.ram_slots) : "—" },
          ]}
        />
      </div>
    </div>
  );
}
