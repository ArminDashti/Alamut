api.ts 603L cognitive
// C:/Users/armin/GitHub/Alamut/netvan-webui/src/lib/api.ts
§ function resolveApiBase (L2-L20)
function resolveApiBase(): string {
  const env = (import.meta.env.VITE_ALAMUT_API_URL as string | undefined)?.trim();
  if (env) return env.replace(/\/$/, "");

  // Electron desktop wrapper: ask the main process for the configured API URL
  // (respects --api-url=<host:port> and ALAMUT_API_URL). Falls back to the
  // default if the bridge is unavailable (e.g. running the built PWA in a
  // browser).
  if (typeof window !== "undefined" && window.alamut?.getApiUrl) {
    try {
      const cached = window.alamut.getApiUrlSync?.();
      if (cached) return cached.replace(/\/$/, "");
    } catch {
      // ignore synchronous bridge failures; async fallback below
    }
  }

  return (import.meta.env.BASE_URL || "/").replace(/\/$/, "");
}
// ... 358 lines omitted
§ type RpcResponse (L379-L534)
export type RpcResponse =
  | { type: "Ok" }
  | { type: "Pong" }
  | { type: "Status"; data: ServiceStatus }
  | { type: "Settings"; data: AppSettings }
  | { type: "Nics"; data: NicInfo[] }
  | { type: "Nic"; data: NicInfo }
  | { type: "BandwidthHistory"; data: { ts: number; rx_bytes: number; tx_bytes: number }[] }
  | {
      type: "PingHistory";
      data: {
        nic_id: string | null;
        target: string;
        ts: number;
        rtt_ms: number | null;
        success: boolean;
        error: string | null;
      }[];
    }
  | {
      type: "HttpLatencyHistory";
      data: {
        nic_id: string | null;
        url: string;
        ts: number;
        dns_ms: number | null;
        connect_ms: number | null;
        tls_ms: number | null;
        ttfb_ms: number | null;
        total_ms: number | null;
        status_code: number | null;
        success: boolean;
        error: string | null;
      }[];
    }
  | {
      type: "LinkEvents";
      data: {
        nic_id: string;
        ts: number;
        event: string;
        detail: string | null;
      }[];
    }
  | {
      type: "AppUsage";
      data: {
        process_name: string;
        process_path: string | null;
        remote_ip: string | null;
        host: string | null;
        bytes_in: number;
        bytes_out: number;
        nic_id: string | null;
      }[];
    }
  | {
      type: "AppUsageSeries";
      data: {
        hour_ts: number;
        process_name: string;
        remote_ip: string | null;
        host: string | null;
        bytes_in: number;
        bytes_out: number;
      }[];
    }
  | {
      type: "PingResult";
      data: {
        nic_id: string | null;
        target: string;
        ts: number;
        rtt_ms: number | null;
        success: boolean;
        error: string | null;
      };
    }
  | {
      type: "HttpLatencyResult";
      data: {
        nic_id: string | null;
        url: string;
        ts: number;
        dns_ms: number | null;
        connect_ms: number | null;
        tls_ms: number | null;
        ttfb_ms: number | null;
        total_ms: number | null;
        status_code: number | null;
        success: boolean;
        error: string | null;
      };
    }
  | {
      type: "Traceroute";
      data: {
        id: number;
        nic_id: string | null;
        target: string;
        ts: number;
        hops: {
          hop: number;
          address: string | null;
          hostname: string | null;
          rtt_ms: number | null;
        }[];
      };
    }
  | {
      type: "Nslookup";
      data: { query: string; records: string[]; raw: string };
    }
  | {
      type: "Speedtest";
      data: {
        id: number;
        nic_id: string | null;
        ts: number;
        server_id: string | null;
        server_name: string | null;
        download_mbps: number;
        upload_mbps: number;
        ping_ms: number;
        jitter_ms: number | null;
        packet_loss: number | null;
        raw_json: string | null;
      };
    }
  | {
      type: "SpeedtestHistory";
      data: {
        id: number;
        nic_id: string | null;
        ts: number;
        server_id: string | null;
        server_name: string | null;
        download_mbps: number;
        upload_mbps: number;
        ping_ms: number;
        jitter_ms: number | null;
        packet_loss: number | null;
        raw_json: string | null;
      }[];
    }
  | { type: "CpuSnapshot"; data: CpuSnapshot }
  | { type: "MemorySnapshot"; data: MemorySnapshot }
  | { type: "Disks"; data: DiskSnapshot[] }
  | { type: "HardwareInventory"; data: HardwareInventory }
  | { type: "ThermalSnapshot"; data: ThermalSnapshot }
  | { type: "CpuHistory"; data: SystemMetricHistory }
  | { type: "MemoryHistory"; data: SystemMetricHistory }
  | { type: "DiskHistory"; data: SystemMetricHistory }
  | { type: "EnvVars"; data: EnvVar[] }
  | { type: "AppIcon"; data: string | null }
  | { type: "Error"; data: { message: string } };
// ... 1 lines omitted
§ function rpc<T (L536-L551)
export async function rpc<T = RpcResponse>(req: RpcRequest): Promise<T> {
  const res = await fetch(`${API_BASE}/api/rpc`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(req),
  });
  if (!res.ok) {
    throw new Error(`API HTTP ${res.status}`);
  }
  const raw = await res.json();
  const normalized = normalizeResponse(raw);
  if (normalized.type === "Error") {
    throw new Error(normalized.data.message);
  }
  return normalized as T;
}
// ... 1 lines omitted
§ function getDataDir (L553-L559)
export async function getDataDir(): Promise<string> {
  const res = await fetch(`${API_BASE}/api/data-dir`);
  if (!res.ok) throw new Error(`API HTTP ${res.status}`);
  const body = (await res.json()) as { path?: string; error?: string };
  if (body.error) throw new Error(body.error);
  return body.path ?? "";
}
// ... 1 lines omitted
§ function healthCheck (L561-L568)
export async function healthCheck(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/api/health`);
    return res.ok;
  } catch {
    return false;
  }
}
// ... 1 lines omitted
§ function toolsWsUrl (L570-L580)
export function toolsWsUrl(): string {
  // In the browser PWA the API is same-origin (proxied /api), so use the
  // page origin. In Electron the API is on a separate host:port, so use
  // API_BASE directly (it already resolves to http://127.0.0.1:28437).
  const base = API_BASE && /^https?:\/\//.test(API_BASE) ? API_BASE : "";
  const origin =
    base || (typeof window !== "undefined" ? window.location.origin : "http://127.0.0.1:28438");
  const u = new URL("/api/ws/tools", origin);
  u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
  return u.toString();
}
// ... 1 lines omitted
§ function normalizeResponse(raw (L582-L603)
function normalizeResponse(raw: unknown): RpcResponse {
  if (!raw || typeof raw !== "object") {
    return { type: "Error", data: { message: "empty response" } };
  }
  const obj = raw as Record<string, unknown>;
  if ("type" in obj) {
    return raw as RpcResponse;
  }
  // Externally tagged: { "Nics": [...] }
  const key = Object.keys(obj)[0];
  if (!key) {
    return { type: "Error", data: { message: "empty response" } };
  }
  if (key === "Ok" || key === "Pong") {
    return { type: key } as RpcResponse;
  }
  if (key === "Error") {
    const data = obj[key] as { message: string };
    return { type: "Error", data };
  }
  return { type: key, data: obj[key] } as RpcResponse;
}
7/37 chunks shown (1859 tokens)
[lean-ctx] full source: read "C:/Users/armin/GitHub/Alamut/netvan-webui/src/lib/api.ts" directly (no MCP)  ·  or ctx_read("C:/Users/armin/GitHub/Alamut/netvan-webui/src/lib/api.ts", mode="full")

