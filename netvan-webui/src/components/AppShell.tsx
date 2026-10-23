import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import alamutMark from "@/assets/brand/alamut-mark.png";
import {
  AppWindow,
  ChevronDown,
  Cpu,
  Gauge,
  Globe,
  Info,
  Network,
  Settings,
  Timer,
  Variable,
  Wrench,
  Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { healthCheck, rpc, type AppSettings } from "@/lib/api";
import { applyTheme } from "@/lib/theme";
import { Dialog } from "@/components/ui/dialog";
import { SettingsPage } from "@/pages/SettingsPage";

const NAV_BEFORE_SYSTEM = [
  { to: "/", label: "Dashboard", icon: Gauge },
  { to: "/info", label: "Info", icon: Info },
  { to: "/nics", label: "NICs", icon: Network },
];

const NAV_AFTER_LATENCY = [
  { to: "/speedtest", label: "Speed Test", icon: Zap },
  { to: "/apps", label: "Apps", icon: AppWindow },
  { to: "/hosts", label: "IP / Host", icon: Globe },
  { to: "/env", label: "Environment", icon: Variable },
];

const TOOL_CHILDREN = [
  { to: "/tools/nslookup", label: "nslookup" },
  { to: "/tools/latency", label: "Latency" },
  { to: "/tools/traceroute", label: "Traceroute" },
];

const LATENCY_CHILDREN = [
  { to: "/latency/icmp", label: "ICMP" },
  { to: "/latency/http", label: "HTTP" },
];

const SYSTEM_CHILDREN = [
  { to: "/system/cpu", label: "CPU" },
  { to: "/system/thermal", label: "Thermal" },
  { to: "/system/memory", label: "Memory" },
  { to: "/system/ssd", label: "SSD" },
  { to: "/system/hdd", label: "HDD" },
  { to: "/system/power", label: "Power" },
];

function NavItem({
  to,
  label,
  icon: Icon,
  end,
}: {
  to: string;
  label: string;
  icon: typeof Gauge;
  end?: boolean;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-2 rounded-md px-3 py-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
          isActive && "bg-[var(--color-muted)] text-[var(--color-foreground)] font-medium",
        )
      }
    >
      <Icon className="h-4 w-4" />
      {label}
    </NavLink>
  );
}

export function AppShell() {
  const location = useLocation();
  const onToolsRoute = location.pathname.startsWith("/tools");
  const onLatencyRoute = location.pathname.startsWith("/latency");
  const onSystemRoute = location.pathname.startsWith("/system");
  const [toolsOpen, setToolsOpen] = useState(onToolsRoute);
  const [latencyOpen, setLatencyOpen] = useState(onLatencyRoute);
  const [systemOpen, setSystemOpen] = useState(onSystemRoute);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [apiOk, setApiOk] = useState<boolean | null>(null);
  const [hasHdd, setHasHdd] = useState(false);

  useEffect(() => {
    if (onToolsRoute) setToolsOpen(true);
  }, [onToolsRoute]);

  useEffect(() => {
    if (onLatencyRoute) setLatencyOpen(true);
  }, [onLatencyRoute]);

  useEffect(() => {
    if (onSystemRoute) setSystemOpen(true);
  }, [onSystemRoute]);

  useEffect(() => {
    const checkHdd = () => {
      rpc<{ type: "Disks"; data: { id: string }[] }>({
        method: "GetDisks",
        params: { kind: "hdd" },
      })
        .then((r) => setHasHdd(r.data.length > 0))
        .catch(() => setHasHdd(false));
    };
    checkHdd();
    const id = setInterval(checkHdd, 30_000);
    return () => clearInterval(id);
  }, []);

  const systemChildren = SYSTEM_CHILDREN.filter(
    (c) => c.to !== "/system/hdd" || hasHdd,
  );
  useEffect(() => {
    const load = () => {
      rpc<{ type: "Settings"; data: AppSettings }>({ method: "GetSettings" })
        .then((r) => {
          applyTheme(r.data.theme ?? "dark-plus");
          setApiOk(true);
        })
        .catch(() => setApiOk(false));
      void healthCheck().then(setApiOk);
    };
    load();
    const id = setInterval(load, 2000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="flex h-full min-h-0">
      <aside
        className="flex w-56 shrink-0 flex-col border-r border-[var(--color-border)] bg-[var(--color-card)]/70 backdrop-blur"
      >
        <div className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-4">
          <span
            aria-hidden="true"
            className="block h-6 w-8 shrink-0 bg-[var(--color-primary)]"
            style={{
              WebkitMaskImage: `url(${alamutMark})`,
              maskImage: `url(${alamutMark})`,
              WebkitMaskSize: "contain",
              maskSize: "contain",
              WebkitMaskRepeat: "no-repeat",
              maskRepeat: "no-repeat",
              WebkitMaskPosition: "center",
              maskPosition: "center",
            }}
          />
          <div>
            <div className="text-sm font-semibold tracking-wide">Alamut</div>
            <div className="text-[11px] text-[var(--color-muted-foreground)]">
              {apiOk === false ? "API offline" : "Network Monitor"}
            </div>
          </div>
        </div>
        <nav className="flex flex-1 flex-col gap-1 p-2">
          {NAV_BEFORE_SYSTEM.map((item) => (
            <NavItem key={item.to} {...item} end={item.to === "/"} />
          ))}

          <div>
            <button
              type="button"
              onClick={() => setSystemOpen((o) => !o)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                onSystemRoute &&
                  "bg-[var(--color-muted)] text-[var(--color-foreground)] font-medium",
              )}
            >
              <Cpu className="h-4 w-4" />
              <span className="flex-1 text-left">System</span>
              <ChevronDown
                className={cn("h-3.5 w-3.5 transition-transform", systemOpen && "rotate-180")}
              />
            </button>
            {systemOpen && (
              <div className="ml-4 mt-0.5 flex flex-col gap-0.5 border-l border-[var(--color-border)] pl-2">
                {systemChildren.map(({ to, label }) => (
                  <NavLink
                    key={to}
                    to={to}
                    className={({ isActive }) =>
                      cn(
                        "rounded-md px-2 py-1.5 text-xs text-[var(--color-muted-foreground)] hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                        isActive && "bg-[var(--color-muted)] text-[var(--color-foreground)]",
                      )
                    }
                  >
                    {label}
                  </NavLink>
                ))}
              </div>
            )}
          </div>

          <div>
            <button
              type="button"
              onClick={() => setLatencyOpen((o) => !o)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                onLatencyRoute &&
                  "bg-[var(--color-muted)] text-[var(--color-foreground)] font-medium",
              )}
            >
              <Timer className="h-4 w-4" />
              <span className="flex-1 text-left">Latency</span>
              <ChevronDown
                className={cn("h-3.5 w-3.5 transition-transform", latencyOpen && "rotate-180")}
              />
            </button>
            {latencyOpen && (
              <div className="ml-4 mt-0.5 flex flex-col gap-0.5 border-l border-[var(--color-border)] pl-2">
                {LATENCY_CHILDREN.map(({ to, label }) => (
                  <NavLink
                    key={to}
                    to={to}
                    className={({ isActive }) =>
                      cn(
                        "rounded-md px-2 py-1.5 text-xs text-[var(--color-muted-foreground)] hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                        isActive && "bg-[var(--color-muted)] text-[var(--color-foreground)]",
                      )
                    }
                  >
                    {label}
                  </NavLink>
                ))}
              </div>
            )}
          </div>

          {NAV_AFTER_LATENCY.map((item) => (
            <NavItem key={item.to} {...item} />
          ))}

          <div>
            <button
              type="button"
              onClick={() => setToolsOpen((o) => !o)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                onToolsRoute &&
                  "bg-[var(--color-muted)] text-[var(--color-foreground)] font-medium",
              )}
            >
              <Wrench className="h-4 w-4" />
              <span className="flex-1 text-left">Tools</span>
              <ChevronDown
                className={cn("h-3.5 w-3.5 transition-transform", toolsOpen && "rotate-180")}
              />
            </button>
            {toolsOpen && (
              <div className="ml-4 mt-0.5 flex flex-col gap-0.5 border-l border-[var(--color-border)] pl-2">
                {TOOL_CHILDREN.map(({ to, label }) => (
                  <NavLink
                    key={to}
                    to={to}
                    className={({ isActive }) =>
                      cn(
                        "rounded-md px-2 py-1.5 text-xs text-[var(--color-muted-foreground)] hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]",
                        isActive && "bg-[var(--color-muted)] text-[var(--color-foreground)]",
                      )
                    }
                  >
                    {label}
                  </NavLink>
                ))}
              </div>
            )}
          </div>

          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            className="flex items-center gap-2 rounded-md px-3 py-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]"
          >
            <Settings className="h-4 w-4" />
            Settings
          </button>
        </nav>
        <div className="border-t border-[var(--color-border)] p-3 text-[11px] text-[var(--color-muted-foreground)]">
          <div className="mb-1 flex items-center gap-1.5">
            <span
              className={cn(
                "inline-block h-1.5 w-1.5 rounded-full",
                apiOk ? "bg-[var(--color-success)]" : "bg-[var(--color-destructive)]",
              )}
            />
            {apiOk ? "alamut-api connected" : "waiting for alamut-api"}
          </div>
          Created by Armin Dashti
        </div>
      </aside>
      <main className="min-w-0 flex-1 overflow-auto p-5">
        <Outlet />
      </main>
      <Dialog open={settingsOpen} onOpenChange={setSettingsOpen} className="max-w-4xl">
        <SettingsPage />
      </Dialog>
    </div>
  );
}
