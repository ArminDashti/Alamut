# alamut-webui

Display-only **PWA** for Alamut. Inspired by the desktop Alamut UI, rebuilt with React + Tailwind + Shadcn-style components.

**Does not** run collectors, ping, traceroute, or speedtest in the browser. All work happens in **[alamut-api](https://github.com/ArminDashti/alamut-api)** (`127.0.0.1:28437`).

## Requirements

- Node.js 20+
- Running `alamut-api` (`alamut-api.exe run` or Windows service `alamut-api`)

## Setup

```powershell
npm install
npm run dev
```

Open `http://127.0.0.1:28438`.

Dev proxies `/api` (and `/api/ws/tools`) to `http://127.0.0.1:28437`, so the UI works over a single forwarded port.

Optional absolute API origin: `VITE_ALAMUT_API_URL=http://127.0.0.1:28437`

## PWA

Production build registers a service worker that **heavily precaches** the app shell and static assets. API / WebSocket traffic to localhost is **NetworkOnly** (never stale metrics).

```powershell
npm run build
npm run preview
```

Install from the browser (“Install Alamut”) when served over localhost/HTTPS.

## Scripts

| Script | Purpose |
|--------|---------|
| `npm run dev` | Vite on `127.0.0.1:28438` |
| `npm run build` | Typecheck + production PWA build |
| `npm run preview` | Preview production build |

## Architecture

```
Browser (this repo)  --POST /api/rpc-->  alamut-api service
                     --WS /api/ws/tools-->  (live tool streams)
```
