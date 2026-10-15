import { useMemo, useState } from "react";
import { type CellAgg, type PeriodColumn } from "@/lib/latencyPeriod";
import { compareValues, type SortDir } from "@/lib/sortable";
import { cn, formatUsageGb } from "@/lib/utils";

function formatCellGb(bytes: number): string {
  const gb = bytes / 1024 ** 3;
  if (gb >= 10) return gb.toFixed(1);
  if (gb >= 1) return gb.toFixed(1);
  if (gb >= 0.1) return gb.toFixed(1);
  if (bytes > 0) return "<0.1";
  return "0";
}

function heatBg(intensity: number): string {
  const pct = Math.round(12 + intensity * 68);
  return `color-mix(in srgb, #3b82f6 ${pct}%, transparent)`;
}

export function UsageHeatmap({
  title,
  rowHeader,
  rowLead,
  rows,
  columns,
  cells,
  onRowContextMenu,
}: {
  title: string;
  rowHeader: string;
  /** Optional node shown left of the row label (e.g. app icon, country flag). */
  rowLead?: (row: string) => React.ReactNode;
  rows: string[];
  columns: PeriodColumn[];
  cells: Map<string, CellAgg>;
  onRowContextMenu?: (row: string, e: React.MouseEvent) => void;
}) {
  const [sortKey, setSortKey] = useState<string | null>("__total__");
  const [sortDir, setSortDir] = useState<SortDir>("desc");

  const rowTotals = useMemo(() => {
    const totals = new Map<string, number>();
    for (const row of rows) {
      let sum = 0;
      for (const col of columns) {
        const cell = cells.get(`${row}::${col.key}`);
        sum += cell?.sum ?? 0;
      }
      totals.set(row, sum);
    }
    return totals;
  }, [rows, columns, cells]);

  const sortedRows = useMemo(() => {
    if (!sortKey) return rows;
    const dir = sortDir === "asc" ? 1 : -1;
    return [...rows].sort((a, b) => {
      if (sortKey === "__row__") return dir * compareValues(a, b);
      if (sortKey === "__total__") {
        return dir * compareValues(rowTotals.get(a) ?? 0, rowTotals.get(b) ?? 0);
      }
      const av = cells.get(`${a}::${sortKey}`)?.sum ?? null;
      const bv = cells.get(`${b}::${sortKey}`)?.sum ?? null;
      return dir * compareValues(av, bv);
    });
  }, [rows, cells, sortKey, sortDir, rowTotals]);

  const toggle = (key: string) => {
    if (sortKey !== key) {
      setSortKey(key);
      setSortDir(key === "__total__" ? "desc" : "asc");
      return;
    }
    if (sortDir === "asc") {
      setSortDir("desc");
      return;
    }
    setSortKey(null);
    setSortDir("asc");
  };

  const marker = (key: string) =>
    sortKey === key ? (sortDir === "asc" ? " ▲" : " ▼") : "";

  const values = [...cells.values()]
    .map((c) => c.sum)
    .filter((v): v is number => v != null && Number.isFinite(v) && v > 0);
  const max = values.length ? Math.max(...values) : 0;

  if (columns.length === 0) {
    return (
      <p className="text-sm text-[var(--color-muted-foreground)]">
        No period columns for this range.
      </p>
    );
  }

  return (
    <div className="overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/90">
      <div className="flex flex-wrap items-center justify-end gap-2 border-b border-[var(--color-border)] px-4 py-2.5">
        <span className="mr-auto text-xs text-[var(--color-muted-foreground)]">{title}</span>
        <span className="text-[10px] text-[var(--color-muted-foreground)]">Less Traffic</span>
        {[0.15, 0.35, 0.55, 0.75, 1].map((i) => (
          <span
            key={i}
            className="h-3 w-5 rounded-sm"
            style={{ backgroundColor: heatBg(i) }}
          />
        ))}
        <span className="text-[10px] text-[var(--color-muted-foreground)]">More Traffic</span>
        <span className="mx-1 text-[var(--color-muted-foreground)]">·</span>
        <span className="h-3 w-5 rounded-sm bg-[var(--color-muted)]/80" title="No data" />
        <span className="text-[10px] text-[var(--color-muted-foreground)]">No Data</span>
      </div>
      <div className="overflow-auto">
        <table className="min-w-full border-collapse text-[11px]">
          <thead>
            <tr className="bg-[var(--color-muted)]/40">
              <th
                className="sticky left-0 z-10 min-w-[140px] cursor-pointer select-none bg-[var(--color-card)] px-3 py-2 text-left text-xs font-medium"
                onClick={() => toggle("__row__")}
              >
                {rowHeader}
                <span className="opacity-70">{marker("__row__")}</span>
              </th>
              {columns.map((c) => (
                <th
                  key={c.key}
                  className="min-w-[2.75rem] cursor-pointer select-none px-0.5 py-2 text-center text-[10px] font-medium text-[var(--color-muted-foreground)]"
                  onClick={() => toggle(c.key)}
                >
                  {c.label}
                  <span className="opacity-70">{marker(c.key)}</span>
                </th>
              ))}
              <th
                className="sticky right-0 z-10 min-w-[4.5rem] cursor-pointer select-none bg-[var(--color-card)] px-2 py-2 text-right text-xs font-medium"
                onClick={() => toggle("__total__")}
              >
                Total
                <span className="opacity-70">{marker("__total__")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {sortedRows.map((row) => {
              const total = rowTotals.get(row) ?? 0;
              return (
                <tr
                  key={row}
                  className="border-t border-[var(--color-border)]/80"
                  onContextMenu={(e) => {
                    if (!onRowContextMenu) return;
                    e.preventDefault();
                    onRowContextMenu(row, e);
                  }}
                >
                  <td className="sticky left-0 z-10 bg-[var(--color-card)] px-3 py-1.5">
                    <div className="flex max-w-[160px] items-center gap-2">
                      {rowLead?.(row)}
                      <span className="truncate font-medium">{row}</span>
                    </div>
                  </td>
                  {columns.map((col) => {
                    const cell = cells.get(`${row}::${col.key}`);
                    const sum = cell?.sum ?? null;
                    const count = cell?.count ?? 0;
                    const intensity =
                      sum != null && max > 0 ? Math.min(1, sum / max) : 0;
                    const show = sum != null && sum > 0;
                    return (
                      <td
                        key={col.key}
                        title={
                          sum != null
                            ? `${formatCellGb(sum)} GB · ${count} sample${count === 1 ? "" : "s"}`
                            : "No data"
                        }
                        className={cn(
                          "px-0.5 py-1 text-center tabular-nums",
                          !show && "text-[var(--color-muted-foreground)]/35",
                        )}
                        style={show ? { backgroundColor: heatBg(intensity) } : undefined}
                      >
                        {show ? formatCellGb(sum) : "—"}
                      </td>
                    );
                  })}
                  <td className="sticky right-0 z-10 bg-[var(--color-card)] px-2 py-1.5 text-right font-medium tabular-nums">
                    {total > 0 ? formatUsageGb(total) : "—"}
                  </td>
                </tr>
              );
            })}
            {rows.length === 0 && (
              <tr>
                <td
                  colSpan={columns.length + 2}
                  className="px-3 py-8 text-center text-sm text-[var(--color-muted-foreground)]"
                >
                  No usage samples yet
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
