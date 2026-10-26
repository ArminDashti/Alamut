import { type HistoryRange } from "@/lib/api";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

const RANGES: { id: HistoryRange; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "yesterday", label: "Yesterday" },
  { id: "week", label: "Week" },
  { id: "last_week", label: "Last week" },
  { id: "months", label: "Months" },
  { id: "custom", label: "Custom" },
];

export function HistoryFilter({
  value,
  onChange,
  customStart,
  customEnd,
  onCustomStart,
  onCustomEnd,
  labels,
  className,
}: {
  value: HistoryRange;
  onChange: (v: HistoryRange) => void;
  customStart?: string;
  customEnd?: string;
  onCustomStart?: (v: string) => void;
  onCustomEnd?: (v: string) => void;
  labels?: Partial<Record<HistoryRange, string>>;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <div className="flex flex-wrap items-center gap-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-muted)]/30 p-1">
        {RANGES.map((r) => (
          <button
            key={r.id}
            type="button"
            onClick={() => onChange(r.id)}
            className={cn(
              "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
              value === r.id
                ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
                : "text-[var(--color-muted-foreground)] hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
            )}
          >
            {labels?.[r.id] ?? r.label}
          </button>
        ))}
      </div>
      {value === "custom" && (
        <>
          <Input
            type="datetime-local"
            className="w-auto"
            value={customStart ?? ""}
            onChange={(e) => onCustomStart?.(e.target.value)}
          />
          <span className="text-[var(--color-muted-foreground)] text-sm">to</span>
          <Input
            type="datetime-local"
            className="w-auto"
            value={customEnd ?? ""}
            onChange={(e) => onCustomEnd?.(e.target.value)}
          />
        </>
      )}
    </div>
  );
}

export function customToTs(local: string): number | null {
  if (!local) return null;
  const t = new Date(local).getTime();
  return Number.isFinite(t) ? Math.floor(t / 1000) : null;
}
