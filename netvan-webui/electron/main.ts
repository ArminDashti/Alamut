/**
 * Alamut Electron main process.
 *
 * Wraps the existing Vite/PWA renderer (the alamut-webui React app) in a
 * desktop window. The renderer talks to alamut-api over HTTP/WebSocket just
 * like the browser PWA; this file only manages the window, the API health
 * probe, and a couple of IPC helpers used by Settings ("open data dir",
 * "open external URL").
 *
 * API origin is discovered in this order:
 *   1. --api-url=<host:port>  (passed by the CLI wrapper)
 *   2. ALAMUT_API_URL env var
 *   3. http://127.0.0.1:8000
 */

import { app, BrowserWindow, ipcMain, shell, dialog } from "electron";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function resolveApiUrl(): string {
  const argv = process.argv.filter((a) => a.startsWith("--api-url="));
  if (argv.length) {
    const v = argv[argv.length - 1].split("=", 2)[1];
    if (v) return v.replace(/\/$/, "");
  }
  const env = process.env.ALAMUT_API_URL || process.env.ALAMUT_API_BASE;
  if (env && env.trim()) return env.trim().replace(/\/$/, "");
  return "http://127.0.0.1:8000";
}

function resolveRendererPath(): string {
  // Production: built dist/ folder served by the API, or bundled index.html.
  if (app.isPackaged) {
    const dist = path.join(__dirname, "..", "dist", "index.html");
    return dist;
  }
  // Dev: Vite dev server (started by vite-plugin-electron onstart).
  const url = process.env.VITE_DEV_URL || "http://localhost:8001";
  return url;
}

let win: BrowserWindow | null = null;

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 820,
    minWidth: 980,
    minHeight: 640,
    title: "Alamut",
    backgroundColor: "#0b1220",
    trafficLightPosition: { x: 12, y: 12 },
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  const target = resolveRendererPath();
  if (typeof target === "string" && target.startsWith("http")) {
    win.loadURL(target);
  } else {
    win.loadFile(target);
  }

  win.on("closed", () => {
    win = null;
  });

  // Open external links in the system browser, never in-app.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith("http://") || url.startsWith("https://")) {
      shell.openExternal(url);
      return { action: "deny" };
    }
    return { action: "allow" };
  });
}

app.whenReady().then(() => {
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  // Keep running in the tray on Windows; quit explicitly on mac.
  if (process.platform !== "darwin") app.quit();
});

// ---- IPC helpers (renderer calls these via window.alamut.*) ----

ipcMain.on("alamut:get-api-url", (e) => {
  e.reply("alamut:api-url", resolveApiUrl());
});

ipcMain.on("alamut:open-data-dir", async (e) => {
  const api = resolveApiUrl();
  try {
    const res = await fetch(`${api}/api/data-dir`);
    const body = (await res.json()) as { path?: string; error?: string };
    if (body.path) {
      shell.showItemInFolder(body.path);
      return;
    }
    if (body.error) throw new Error(body.error);
  } catch (err) {
    dialog.showErrorBox(
      "Alamut",
      `Could not resolve data dir: ${(err as Error).message}`,
    );
  }
  e.reply("alamut:data-dir-error");
});

ipcMain.on("alamut:open-external", (_e, url: string) => {
  if (typeof url !== "string") return;
  if (url.startsWith("http://") || url.startsWith("https://")) {
    shell.openExternal(url);
  }
});

ipcMain.on("alamut:quit", () => app.quit());