import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import type { HistoryRange } from "@/lib/api";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Live speed: bits/sec → megabits labeled "Mb", or "GB" when ≥ 1000 Mb. */
export function formatSpeedMb(bps: number): string {
  if (!Number.isFinite(bps) || bps < 0) return "0.0 Mb";
  const mb = bps / 1_000_000;
  if (mb >= 1000) {
    const gb = mb / 1000;
    return `${gb.toFixed(gb >= 10 ? 0 : 1)} GB`;
  }
  return `${mb.toFixed(mb >= 100 ? 0 : 1)} Mb`;
}

/** Volume / usage: binary units (KiB/MiB/…) labeled KB/MB/GB/TB. */
export function formatUsageGb(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
  const abs = bytes;
  if (abs < 1024) return `${Math.round(abs)} B`;
  if (abs < 1024 ** 2) return `${(abs / 1024).toFixed(1)} KB`;
  if (abs < 1024 ** 3) return `${(abs / 1024 ** 2).toFixed(2)} MB`;
  if (abs < 1024 ** 4) return `${(abs / 1024 ** 3).toFixed(2)} GB`;
  return `${(abs / 1024 ** 4).toFixed(2)} TB`;
}

/** Bytes → gibibytes for chart series (axis labeled GB). */
export function bytesToGb(bytes: number): number {
  if (!Number.isFinite(bytes) || bytes < 0) return 0;
  return bytes / 1024 ** 3;
}

/** @deprecated Prefer formatSpeedMb for live rates. */
export function formatBps(bps: number): string {
  return formatSpeedMb(bps);
}

/** @deprecated Prefer formatUsageGb for usage counters. */
export function formatBytes(bytes: number): string {
  return formatUsageGb(bytes);
}

export function formatMs(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "—";
  return `${ms.toFixed(1)} ms`;
}

export function formatTs(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

const WEEKDAYS = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"] as const;

/** X-axis label for history charts based on selected range. */
export function formatHistoryAxisLabel(ts: number, range: HistoryRange): string {
  const d = new Date(ts * 1000);
  switch (range) {
    case "today":
    case "yesterday":
      return String(d.getHours()).padStart(2, "0");
    case "week":
    case "last_week":
      return WEEKDAYS[d.getDay()] ?? "";
    case "months":
      return String(d.getDate());
    case "custom":
    case "all":
    default:
      return d.toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
  }
}
