import { ArrowDown, ArrowUp, type LucideIcon } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function SummaryCard({
  label,
  value,
  sub,
  trend,
  icon: Icon,
  iconClass,
}: {
  label: string;
  value: string;
  sub?: string;
  trend?: { pct: number; invert?: boolean } | null;
  icon: LucideIcon;
  iconClass: string;
}) {
  const up = trend != null && trend.pct > 0;
  const down = trend != null && trend.pct < 0;
  const good = trend?.invert ? down : up;
  const bad = trend?.invert ? up : down;
  return (
    <Card className="border-[var(--color-border)]/80">
      <CardContent className="flex items-start gap-3 p-4">
        <div
          className={cn(
            "flex h-10 w-10 shrink-0 items-center justify-center rounded-lg",
            iconClass,
          )}
        >
          <Icon className="h-5 w-5" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-xs text-[var(--color-muted-foreground)]">{label}</div>
          <div className="truncate text-2xl font-semibold tabular-nums leading-tight">{value}</div>
          {sub && (
            <div className="mt-0.5 text-xs text-[var(--color-muted-foreground)]">{sub}</div>
          )}
          {trend != null && trend.pct != null && Number.isFinite(trend.pct) && (
            <div
              className={cn(
                "mt-1 flex items-center gap-0.5 text-xs font-medium",
                good && "text-emerald-400",
                bad && "text-emerald-400",
                !good && !bad && "text-[var(--color-muted-foreground)]",
              )}
            >
              {up ? <ArrowUp className="h-3 w-3" /> : down ? <ArrowDown className="h-3 w-3" /> : null}
              <span>{Math.abs(trend.pct).toFixed(0)}%</span>
              <span className="font-normal text-[var(--color-muted-foreground)]">vs yesterday</span>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
