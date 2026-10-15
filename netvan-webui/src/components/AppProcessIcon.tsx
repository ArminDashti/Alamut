import { useEffect, useState } from "react";
import { rpc } from "@/lib/api";

const KNOWN_APP_COLORS: Record<string, string> = {
  chrome: "#4285F4",
  slack: "#4A154B",
  teams: "#6264A7",
  spotify: "#1DB954",
  code: "#007ACC",
  vscode: "#007ACC",
  git: "#F05032",
  docker: "#2496ED",
  node: "#339933",
  postman: "#FF6C37",
  zoom: "#2D8CFF",
};

// ponytail: one request per app name per page session; no eviction or retry on failure.
const iconCache = new Map<string, Promise<string | null>>();

function appIconSrc(name: string): Promise<string | null> {
  let src = iconCache.get(name);
  if (!src) {
    src = rpc<{ type: "AppIcon"; data: string | null }>({
      method: "GetAppIcon",
      params: { process_name: name },
    }).then((r) => (r.data ? `data:image/png;base64,${r.data}` : null));
    iconCache.set(name, src);
  }
  return src;
}

export function AppProcessIcon({ name }: { name: string }) {
  const [icon, setIcon] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    appIconSrc(name)
      .then((src) => alive && setIcon(src))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [name]);
  if (icon) {
    return <img src={icon} alt="" className="h-6 w-6 shrink-0" aria-hidden draggable={false} />;
  }
  const base = name.replace(/\.exe$/i, "").toLowerCase();
  const key = Object.keys(KNOWN_APP_COLORS).find((k) => base.includes(k));
  const bg = key ? KNOWN_APP_COLORS[key]! : "var(--color-muted)";
  const label = base.slice(0, 2).toUpperCase() || "?";
  return (
    <span
      className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-[10px] font-semibold text-white"
      style={{ backgroundColor: bg }}
      aria-hidden
    >
      {key ? label.charAt(0) : label}
    </span>
  );
}
