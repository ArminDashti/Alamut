import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/AppShell";
import { DashboardPage } from "@/pages/DashboardPage";
import { InfoPage } from "@/pages/InfoPage";
import { NicsPage } from "@/pages/NicsPage";
import { LatencyLayout } from "@/pages/latency/LatencyLayout";
import { IcmpLatencyPage } from "@/pages/latency/IcmpLatencyPage";
import { HttpLatencyPage } from "@/pages/latency/HttpLatencyPage";
import { SpeedtestPage } from "@/pages/SpeedtestPage";
import { EnvVarsPage } from "@/pages/EnvVarsPage";
import { AppsPage } from "@/pages/AppsPage";
import { IpHostPage } from "@/pages/IpHostPage";
import { ToolsLayout } from "@/pages/tools/ToolsLayout";
import { NslookupToolPage } from "@/pages/tools/NslookupToolPage";
import { LatencyToolPage } from "@/pages/tools/LatencyToolPage";
import { TracerouteToolPage } from "@/pages/tools/TracerouteToolPage";
import { SystemLayout } from "@/pages/system/SystemLayout";
import { CpuPage } from "@/pages/system/CpuPage";
import { MemoryPage } from "@/pages/system/MemoryPage";
import { HddPage, SsdPage } from "@/pages/system/DiskPage";
import { ThermalPage } from "@/pages/system/ThermalPage";

export default function App() {
  const basename = (import.meta.env.BASE_URL || "/").replace(/\/$/, "") || undefined;
  return (
    <BrowserRouter basename={basename}>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="info" element={<InfoPage />} />
          <Route path="nics" element={<NicsPage />} />
          <Route path="system" element={<SystemLayout />}>
            <Route index element={<Navigate to="cpu" replace />} />
            <Route path="cpu" element={<CpuPage />} />
            <Route path="thermal" element={<ThermalPage />} />
            <Route path="memory" element={<MemoryPage />} />
            <Route path="ssd" element={<SsdPage />} />
            <Route path="hdd" element={<HddPage />} />
          </Route>
          <Route path="latency" element={<LatencyLayout />}>
            <Route index element={<Navigate to="icmp" replace />} />
            <Route path="icmp" element={<IcmpLatencyPage />} />
            <Route path="http" element={<HttpLatencyPage />} />
          </Route>
          <Route path="speedtest" element={<SpeedtestPage />} />
          <Route path="apps" element={<AppsPage />} />
          <Route path="hosts" element={<IpHostPage />} />
          <Route path="env" element={<EnvVarsPage />} />
          <Route path="tools" element={<ToolsLayout />}>
            <Route index element={<Navigate to="nslookup" replace />} />
            <Route path="nslookup" element={<NslookupToolPage />} />
            <Route path="latency" element={<LatencyToolPage />} />
            <Route path="http-latency" element={<Navigate to="/tools/latency" replace />} />
            <Route path="ping" element={<Navigate to="/tools/latency" replace />} />
            <Route path="traceroute" element={<TracerouteToolPage />} />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
