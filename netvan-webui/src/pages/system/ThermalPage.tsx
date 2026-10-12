import { useEffect, useMemo, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { rpc, type ThermalSensor, type ThermalSnapshot } from "@/lib/api";
import { formatCelsius, groupByHardware, kindLabel } from "@/lib/thermal";

export function ThermalPage() {
  const [sensors, setSensors] = useState<ThermalSensor[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const r = await rpc<{ type: "ThermalSnapshot"; data: ThermalSnapshot }>({
          method: "GetThermalSnapshot",
        });
        if (!alive) return;
        setSensors(r.data.sensors);
        setError(null);
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

  const groups = useMemo(() => groupByHardware(sensors), [sensors]);

  return (
    <div className="space-y-6">
      {error && <p className="text-sm text-red-400">{error}</p>}
      {groups.length === 0 && !error && (
        <p className="text-sm text-[var(--color-muted-foreground)]">
          No temperature sensors reported. Alamut tried LibreHardwareMonitor and the native Windows
          ACPI thermal interface, but this machine did not expose a readable temperature sensor.
        </p>
      )}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {groups.map((g) => (
          <Card key={g.key}>
            <CardHeader className="py-3">
              <CardTitle className="text-base">{g.hardwareName}</CardTitle>
              <p className="text-xs text-[var(--color-muted-foreground)]">{kindLabel(g.kind)}</p>
            </CardHeader>
            <CardContent className="space-y-1.5">
              {g.sensors.map((s) => (
                <div key={s.id} className="flex justify-between gap-3 text-sm">
                  <span className="text-[var(--color-muted-foreground)]">{s.sensor_name}</span>
                  <span className="font-medium tabular-nums">{formatCelsius(s.celsius)}</span>
                </div>
              ))}
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
