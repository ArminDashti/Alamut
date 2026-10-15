export type UsagePoint = {
  hour_ts: number;
  bytes_in: number;
  bytes_out: number;
};

export function pctChange(current: number, previous: number): number | null {
  if (!Number.isFinite(current) || !Number.isFinite(previous)) return null;
  if (previous <= 0) return current > 0 ? 100 : null;
  return ((current - previous) / previous) * 100;
}

export function formatPeakHourLabel(hour: number): string {
  if (hour === 0) return "12 AM";
  if (hour < 12) return `${hour} AM`;
  if (hour === 12) return "12 PM";
  return `${hour - 12} PM`;
}

export function sumSeriesBytes(points: UsagePoint[]): number {
  let n = 0;
  for (const p of points) n += p.bytes_in + p.bytes_out;
  return n;
}

export function peakHourFromSeries(points: UsagePoint[]): { hour: number; bytes: number } | null {
  const byHour = new Map<number, number>();
  for (const p of points) {
    const h = new Date(p.hour_ts * 1000).getHours();
    byHour.set(h, (byHour.get(h) ?? 0) + p.bytes_in + p.bytes_out);
  }
  let bestHour = -1;
  let bestBytes = 0;
  for (const [h, b] of byHour) {
    if (b > bestBytes) {
      bestBytes = b;
      bestHour = h;
    }
  }
  if (bestHour < 0 || bestBytes <= 0) return null;
  return { hour: bestHour, bytes: bestBytes };
}
