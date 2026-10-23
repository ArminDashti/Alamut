## Learned User Preferences

- Prefer Cursor multi-root workspace files in the dedicated `cursor-workspaces` GitHub repo, not beside individual project folders.
- When running the local alamut stack, enable hot reload on both backend and frontend.
- Prefer heavy-cache PWA as app-shell only: precache UI assets for offline load; do not cache API/RPC responses.

## Learned Workspace Facts

- `alamut-api` and `alamut-webui` are a split web stack (Windows service API + browser PWA), inspired by but not the same as the monolithic Tauri desktop `alamut` repo.
- The multi-root workspace file is `alamut.code-workspace` in `C:\Users\armin\GitHub\cursor-workspaces` (GitHub: `ArminDashti/cursor-workspaces`).
- Local defaults: API at `127.0.0.1:28437`, Vite web UI at `127.0.0.1:28438`; API data under `%ProgramData%\Alamut\AlamutApi\`.
- `alamut-api` `.cargo/config.toml` prefers `windows-gnu`; if only MSVC is installed, run with `--target x86_64-pc-windows-msvc`.
- `alamut-webui` uses `vite-plugin-pwa` with custom `src/sw.ts`; app-shell caching keeps cross-origin localhost API/WebSocket traffic NetworkOnly.
