import { toolsWsUrl } from "@/lib/api";

export type PingLineEvent = { phase: string; line: string | null };
export type TracerouteHopEvent = {
  phase: string;
  hop: number | null;
  address: string | null;
  rtt_ms: number | null;
};
export type SpeedtestProgress = {
  phase: string;
  download_mbps: number | null;
  upload_mbps: number | null;
  ping_ms: number | null;
  server_name: string | null;
};

type ServerMsg =
  | { type: "ping_line"; event: PingLineEvent }
  | { type: "ping_done"; result: unknown }
  | { type: "traceroute_hop"; event: TracerouteHopEvent }
  | { type: "traceroute_done"; result: unknown }
  | { type: "speedtest_progress"; event: SpeedtestProgress }
  | { type: "speedtest_done"; result: unknown }
  | { type: "ip_info"; ip: string; info: unknown }
  | { type: "error"; message: string };

function openToolsSocket(): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(toolsWsUrl());
    ws.onopen = () => resolve(ws);
    ws.onerror = () => reject(new Error("WebSocket connection failed"));
  });
}

function send(ws: WebSocket, msg: unknown) {
  ws.send(JSON.stringify(msg));
}

function parseMsg(data: string): ServerMsg {
  return JSON.parse(data) as ServerMsg;
}

/** Display-only: request live ping from alamut-api and stream lines. */
export async function runPingLive(opts: {
  target: string;
  count?: number;
  packetSize?: number;
  onLine: (ev: PingLineEvent) => void;
}): Promise<unknown> {
  const ws = await openToolsSocket();
  return new Promise((resolve, reject) => {
    ws.onmessage = (ev) => {
      const msg = parseMsg(String(ev.data));
      if (msg.type === "ping_line") opts.onLine(msg.event);
      else if (msg.type === "ping_done") {
        ws.close();
        resolve(msg.result);
      } else if (msg.type === "error") {
        ws.close();
        reject(new Error(msg.message));
      }
    };
    ws.onerror = () => reject(new Error("WebSocket error"));
    send(ws, {
      type: "ping_live",
      target: opts.target,
      count: opts.count,
      packet_size: opts.packetSize,
    });
  });
}

export async function runTracerouteLive(opts: {
  target: string;
  maxHops?: number;
  onHop: (ev: TracerouteHopEvent) => void;
}): Promise<unknown> {
  const ws = await openToolsSocket();
  return new Promise((resolve, reject) => {
    ws.onmessage = (ev) => {
      const msg = parseMsg(String(ev.data));
      if (msg.type === "traceroute_hop") opts.onHop(msg.event);
      else if (msg.type === "traceroute_done") {
        ws.close();
        resolve(msg.result);
      } else if (msg.type === "error") {
        ws.close();
        reject(new Error(msg.message));
      }
    };
    ws.onerror = () => reject(new Error("WebSocket error"));
    send(ws, {
      type: "traceroute_live",
      target: opts.target,
      max_hops: opts.maxHops ?? 20,
    });
  });
}

export async function runSpeedtestLive(opts: {
  nicId?: string | null;
  serverId?: string | null;
  acceptEula: boolean;
  onProgress: (ev: SpeedtestProgress) => void;
}): Promise<unknown> {
  const ws = await openToolsSocket();
  return new Promise((resolve, reject) => {
    ws.onmessage = (ev) => {
      const msg = parseMsg(String(ev.data));
      if (msg.type === "speedtest_progress") opts.onProgress(msg.event);
      else if (msg.type === "speedtest_done") {
        ws.close();
        resolve(msg.result);
      } else if (msg.type === "error") {
        ws.close();
        reject(new Error(msg.message));
      }
    };
    ws.onerror = () => reject(new Error("WebSocket error"));
    send(ws, {
      type: "speedtest_live",
      nic_id: opts.nicId ?? null,
      server_id: opts.serverId ?? null,
      accept_eula: opts.acceptEula,
    });
  });
}

export async function cancelSpeedtest(): Promise<void> {
  const ws = await openToolsSocket();
  send(ws, { type: "cancel_speedtest" });
  // brief wait then close; server cancels in-process CLI
  await new Promise((r) => setTimeout(r, 100));
  ws.close();
}

export async function lookupIpInfoViaApi(ip: string): Promise<unknown> {
  const ws = await openToolsSocket();
  return new Promise((resolve, reject) => {
    ws.onmessage = (ev) => {
      const msg = parseMsg(String(ev.data));
      if (msg.type === "ip_info") {
        ws.close();
        resolve(msg.info);
      } else if (msg.type === "error") {
        ws.close();
        reject(new Error(msg.message));
      }
    };
    ws.onerror = () => reject(new Error("WebSocket error"));
    send(ws, { type: "lookup_ip_info", ip });
  });
}
