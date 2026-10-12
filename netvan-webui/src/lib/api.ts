/** Empty = same origin (Vite proxies /api → alamut-api in dev). Override with VITE_ALAMUT_API_URL. */
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

export const API_BASE = resolveApiBase();

export type HistoryRange =
  | "today"
  | "yesterday"
  | "week"
  | "months"
  | "all"
  | "custom";

export type CaptureMode = "process";
export type StripSlot = "widgets_start" | "near_tray";
export type AppTheme =
  | "dark-plus"
  | "midnight"
  | "light"
  | "nord"
  | "dracula"
  | "tokyo-night";

export interface NicInfo {
  id: string;
  guid: string;
  name: string;
  description: string;
  interface_index: number;
  mac: string | null;
  mtu: number | null;
  media_type: string;
  oper_status: string;
  admin_status: string;
  link_speed_bps: number | null;
  ipv4_addresses: string[];
  ipv6_addresses: string[];
  gateways: string[];
  dns_servers: string[];
  dhcp_enabled: boolean;
  wifi_ssid: string | null;
  wifi_bssid: string | null;
  wifi_signal: number | null;
  driver: string | null;
  rx_bytes: number;
  tx_bytes: number;
  rx_bps: number;
  tx_bps: number;
}

export interface AppSettings {
  capture_mode: CaptureMode;
  ping_targets: string[];
  http_targets: string[];
  ping_interval_secs: number;
  http_interval_secs: number;
  bandwidth_interval_ms: number;
  retention_raw_days: number;
  start_ui_with_windows: boolean;
  speedtest_cli_path: string | null;
  speedtest_eula_accepted: boolean;
  default_nic_id: string | null;
  strip_width_px: number;
  strip_font_px: number;
  strip_slot: StripSlot;
  strip_offset_px: number;
  ignore_private_ips: boolean;
  ignored_apps: string[];
  ignored_ips: string[];
  ignored_urls: string[];
  theme?: AppTheme;
  system_interval_ms?: number;
  system_persist_interval_ms?: number;
  system_raw_retention_days?: number;
  system_hourly_retention_days?: number;
}

export type DiskKind = "ssd" | "hdd";

export interface CpuSnapshot {
  brand: string;
  vendor_id: string;
  physical_cores: number | null;
  logical_cores: number;
  frequency_mhz: number | null;
  utilization: number;
  per_core: number[];
}

export interface MemorySnapshot {
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  utilization: number;
}

export interface DiskSnapshot {
  id: string;
  name: string;
  mount_point: string;
  file_system: string;
  kind: DiskKind;
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  utilization: number;
}

export interface SystemMetricPoint {
  ts: number;
  value: number;
}

export interface MetricSummary {
  avg: number | null;
  min: number | null;
  max: number | null;
  sample_count: number;
}

export interface SystemMetricHistory {
  series: SystemMetricPoint[];
  summary: MetricSummary;
}

export interface CpuInfo {
  brand: string;
  model: string;
  base_speed_mhz: number | null;
  physical_cores: number | null;
  logical_processors: number;
}

export interface MemoryInfo {
  brand: string;
  model: string;
  size_bytes: number;
  speed_mhz: number | null;
  modules?: number;
  module_size_bytes?: number | null;
  memory_type?: string;
  form_factor?: string;
  configured_speed_mhz?: number | null;
}

export interface DiskInfo {
  brand: string;
  model: string;
  capacity_bytes: number;
  kind: DiskKind;
  interface_type?: string;
  media_type?: string;
  serial_number?: string;
  partitions?: number | null;
  firmware_revision?: string;
}

export interface GpuInfo {
  brand: string;
  model: string;
  memory_bytes: number | null;
  cuda_cores: number | null;
  tensor_cores: number | null;
}

export interface MotherboardInfo {
  brand: string;
  model: string;
  ram_slots: number | null;
}

export interface HardwareInventory {
  cpu: CpuInfo;
  memory: MemoryInfo;
  disks: DiskInfo[];
  gpus: GpuInfo[];
  motherboard: MotherboardInfo;
}

export type HardwareKind =
  | "cpu"
  | "gpu"
  | "motherboard"
  | "storage"
  | "memory"
  | "other";

export interface ThermalSensor {
  id: string;
  hardware_kind: HardwareKind;
  hardware_name: string;
  sensor_name: string;
  celsius: number | null;
}

export interface ThermalSnapshot {
  sensors: ThermalSensor[];
}

export interface AppUsageRow {
  process_name: string;
  process_path: string | null;
  remote_ip: string | null;
  host: string | null;
  bytes_in: number;
  bytes_out: number;
  nic_id: string | null;
}

export interface ServiceStatus {
  running: boolean;
  pipe_connected: boolean;
  capture_mode: string;
  message: string;
}

export type RpcRequest =
  | { method: "Ping" }
  | { method: "GetStatus" }
  | { method: "GetSettings" }
  | { method: "SetSettings"; params: { settings: AppSettings } }
  | { method: "SetCaptureMode"; params: { mode: CaptureMode } }
  | { method: "ListNics" }
  | { method: "GetNic"; params: { nic_id: string } }
  | { method: "SetNicEnabled"; params: { nic_id: string; enabled: boolean } }
  | {
      method: "GetBandwidthHistory";
      params: {
        nic_id: string | null;
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | {
      method: "GetPingHistory";
      params: {
        nic_id: string | null;
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | {
      method: "GetHttpLatencyHistory";
      params: {
        nic_id: string | null;
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | {
      method: "GetLinkEvents";
      params: {
        nic_id: string | null;
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | {
      method: "GetAppUsage";
      params: {
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
        group_by: string;
        nic_id?: string | null;
      };
    }
  | {
      method: "GetAppUsageSeries";
      params: {
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
        group_by: string;
        nic_id?: string | null;
      };
    }
  | { method: "RunPing"; params: { target: string; nic_id: string | null; count?: number; packet_size?: number | null } }
  | {
      method: "RunHttpLatency";
      params: { url: string; nic_id: string | null };
    }
  | {
      method: "RunTraceroute";
      params: { target: string; nic_id: string | null; max_hops: number | null };
    }
  | { method: "RunNslookup"; params: { query: string } }
  | {
      method: "RunSpeedtest";
      params: {
        nic_id: string | null;
        server_id: string | null;
        accept_eula: boolean;
      };
    }
  | {
      method: "GetSpeedtestHistory";
      params: {
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | { method: "AcceptSpeedtestEula" }
  | { method: "GetCpuSnapshot" }
  | { method: "GetMemorySnapshot" }
  | { method: "GetDisks"; params: { kind: DiskKind } }
  | { method: "GetHardwareInventory" }
  | { method: "GetThermalSnapshot" }
  | {
      method: "GetCpuHistory";
      params: {
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | {
      method: "GetMemoryHistory";
      params: {
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    }
  | {
      method: "GetDiskHistory";
      params: {
        disk_id: string | null;
        kind: DiskKind;
        range: HistoryRange;
        start_ts: number | null;
        end_ts: number | null;
      };
    };

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
  | { type: "Error"; data: { message: string } };

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

export async function getDataDir(): Promise<string> {
  const res = await fetch(`${API_BASE}/api/data-dir`);
  if (!res.ok) throw new Error(`API HTTP ${res.status}`);
  const body = (await res.json()) as { path?: string; error?: string };
  if (body.error) throw new Error(body.error);
  return body.path ?? "";
}

export async function healthCheck(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/api/health`);
    return res.ok;
  } catch {
    return false;
  }
}

export function toolsWsUrl(): string {
  // In the browser PWA the API is same-origin (proxied /api), so use the
  // page origin. In Electron the API is on a separate host:port, so use
  // API_BASE directly (it already resolves to http://127.0.0.1:8000).
  const base = API_BASE && /^https?:\/\//.test(API_BASE) ? API_BASE : "";
  const origin =
    base || (typeof window !== "undefined" ? window.location.origin : "http://127.0.0.1:8001");
  const u = new URL("/api/ws/tools", origin);
  u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
  return u.toString();
}

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
