import type { HistoryRange } from "@/lib/api";

const WEEKDAY_SAT_START = ["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"] as const;

export type PeriodColumn = {
  key: string;
  label: string;
  /** Inclusive local-time bounds for bucketing samples (ms epoch). */
  startMs: number;
  endMs: number;
};

function startOfLocalDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function addDays(d: Date, n: number): Date {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
}

/** Saturday-start week containing `d` (local). */
function startOfSatWeek(d: Date): Date {
  const day = startOfLocalDay(d);
  // JS: 0=Sun … 6=Sat. Distance back to Saturday:
  const dow = day.getDay();
  const back = (dow + 1) % 7;
  return addDays(day, -back);
}

export function buildPeriodColumns(
  range: HistoryRange,
  customStart?: string,
  customEnd?: string,
  now = new Date(),
): PeriodColumn[] {
  const today = startOfLocalDay(now);
  const currentHour = now.getHours();

  if (range === "today") {
    return Array.from({ length: currentHour + 1 }, (_, h) => {
      const start = new Date(today);
      start.setHours(h, 0, 0, 0);
      const end = new Date(start);
      end.setHours(h + 1, 0, 0, 0);
      return {
        key: `h-${h}`,
        label: String(h),
        startMs: start.getTime(),
        endMs: end.getTime(),
      };
    });
  }

  if (range === "yesterday") {
    const y = addDays(today, -1);
    return Array.from({ length: 24 }, (_, h) => {
      const start = new Date(y);
      start.setHours(h, 0, 0, 0);
      const end = new Date(start);
      end.setHours(h + 1, 0, 0, 0);
      return {
        key: `yh-${h}`,
        label: String(h),
        startMs: start.getTime(),
        endMs: end.getTime(),
      };
    });
  }

  if (range === "week") {
    const weekStart = startOfSatWeek(today);
    const cols: PeriodColumn[] = [];
    for (let i = 0; i < 7; i++) {
      const day = addDays(weekStart, i);
      if (day > today) break;
      const next = addDays(day, 1);
      const label = WEEKDAY_SAT_START[i]!;
      cols.push({
        key: `wd-${day.toISOString().slice(0, 10)}`,
        label,
        startMs: day.getTime(),
        endMs: next.getTime(),
      });
    }
    return cols;
  }

  if (range === "last_week") {
    const weekStart = addDays(startOfSatWeek(today), -7);
    return Array.from({ length: 7 }, (_, i) => {
      const day = addDays(weekStart, i);
      const next = addDays(day, 1);
      const label = WEEKDAY_SAT_START[i]!;
      return {
        key: `lwd-${day.toISOString().slice(0, 10)}`,
        label,
        startMs: day.getTime(),
        endMs: next.getTime(),
      };
    });
  }

  if (range === "months") {
    const dayNum = today.getDate();
    const year = today.getFullYear();
    const month = today.getMonth();
    return Array.from({ length: dayNum }, (_, i) => {
      const d = i + 1;
      const start = new Date(year, month, d);
      const end = new Date(year, month, d + 1);
      return {
        key: `md-${d}`,
        label: String(d),
        startMs: start.getTime(),
        endMs: end.getTime(),
      };
    });
  }

  // custom / all → day buckets (or hours if span ≤ 2 days)
  const startMs = customStart ? new Date(customStart).getTime() : today.getTime() - 7 * 86400000;
  const endMs = customEnd ? new Date(customEnd).getTime() : now.getTime();
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs <= startMs) {
    return [];
  }
  const spanDays = (endMs - startMs) / 86400000;
  if (spanDays <= 2) {
    const cols: PeriodColumn[] = [];
    let t = new Date(startMs);
    t.setMinutes(0, 0, 0);
    while (t.getTime() < endMs) {
      const start = new Date(t);
      const end = new Date(t);
      end.setHours(end.getHours() + 1);
      cols.push({
        key: `ch-${start.getTime()}`,
        label: `${start.getMonth() + 1}/${start.getDate()} ${start.getHours()}`,
        startMs: start.getTime(),
        endMs: end.getTime(),
      });
      t = end;
    }
    return cols;
  }

  const cols: PeriodColumn[] = [];
  let day = startOfLocalDay(new Date(startMs));
  const last = startOfLocalDay(new Date(endMs));
  while (day <= last) {
    const next = addDays(day, 1);
    cols.push({
      key: `cd-${day.toISOString().slice(0, 10)}`,
      label: `${day.getMonth() + 1}/${day.getDate()}`,
      startMs: day.getTime(),
      endMs: next.getTime(),
    });
    day = next;
  }
  return cols;
}

/** 12-hour clock label for heatmap headers (e.g. `12 AM`, `2 PM`). */
export function formatHour12Label(hour: number): string {
  const h = ((hour % 24) + 24) % 24;
  if (h === 0) return "12 AM";
  if (h < 12) return `${h} AM`;
  if (h === 12) return "12 PM";
  return `${h - 12} PM`;
}

/** Full local calendar day split into 24 hourly buckets. */
export function buildFullDayHourColumns(day: Date): PeriodColumn[] {
  const startDay = startOfLocalDay(day);
  return Array.from({ length: 24 }, (_, hour) => {
    const start = new Date(startDay);
    start.setHours(hour, 0, 0, 0);
    const end = new Date(start);
    end.setHours(hour + 1, 0, 0, 0);
    return {
      key: `h-${hour}`,
      label: formatHour12Label(hour),
      startMs: start.getTime(),
      endMs: end.getTime(),
    };
  });
}

export type SamplePoint = {
  ts: number; // unix seconds
  value: number | null;
  rowKey: string;
};

export type CellAgg = {
  avg: number | null;
  count: number;
  min?: number | null;
  max?: number | null;
  /** Present when using sum aggregation (usage grids). */
  sum?: number | null;
};

export function aggregateGrid(
  rows: string[],
  columns: PeriodColumn[],
  samples: SamplePoint[],
): Map<string, CellAgg> {
  const map = new Map<string, CellAgg>();
  for (const row of rows) {
    for (const col of columns) {
      map.set(`${row}::${col.key}`, { avg: null, min: null, max: null, count: 0 });
    }
  }
  const sums = new Map<string, { sum: number; count: number; min: number; max: number }>();
  for (const s of samples) {
    if (s.value == null || !Number.isFinite(s.value)) continue;
    const ms = s.ts * 1000;
    const col = columns.find((c) => ms >= c.startMs && ms < c.endMs);
    if (!col) continue;
    const key = `${s.rowKey}::${col.key}`;
    const cur = sums.get(key);
    if (!cur) {
      sums.set(key, { sum: s.value, count: 1, min: s.value, max: s.value });
    } else {
      cur.sum += s.value;
      cur.count += 1;
      cur.min = Math.min(cur.min, s.value);
      cur.max = Math.max(cur.max, s.value);
    }
  }
  for (const [key, { sum, count, min, max }] of sums) {
    map.set(key, { avg: count ? sum / count : null, min, max, count });
  }
  return map;
}

/** Sum samples into period cells (for usage GB grids). */
export function aggregateGridSum(
  rows: string[],
  columns: PeriodColumn[],
  samples: SamplePoint[],
): Map<string, CellAgg> {
  const map = new Map<string, CellAgg>();
  for (const row of rows) {
    for (const col of columns) {
      map.set(`${row}::${col.key}`, { avg: null, sum: null, count: 0 });
    }
  }
  const sums = new Map<string, { sum: number; count: number }>();
  for (const s of samples) {
    if (s.value == null || !Number.isFinite(s.value)) continue;
    const ms = s.ts * 1000;
    const col = columns.find((c) => ms >= c.startMs && ms < c.endMs);
    if (!col) continue;
    const key = `${s.rowKey}::${col.key}`;
    const cur = sums.get(key) ?? { sum: 0, count: 0 };
    cur.sum += s.value;
    cur.count += 1;
    sums.set(key, cur);
  }
  for (const [key, { sum, count }] of sums) {
    map.set(key, { avg: sum, sum, count });
  }
  return map;
}
