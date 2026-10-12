/**
 * Build the Electron desktop wrapper for alamut-webui.
 *
 * Runs three phases sequentially so failures are visible:
 *   1. tsc (typecheck renderer + electron entries)
 *   2. Vite build with VITE_ELECTRON=1 (produces dist/ + dist-electron/)
 *   3. electron-builder --publish never (packager)
 */
import { spawnSync } from "node:child_process";
import { cwd } from "node:process";

const log = (label: string, msg: string) =>
  console.log(`[build-electron] ${label}: ${msg}`);

function run(label: string, cmd: string, args: string[]) {
  log(label, `$ ${cmd} ${args.join(" ")}`);
  const res = spawnSync(cmd, args, {
    cwd: cwd(),
    stdio: "inherit",
    env: { ...process.env, VITE_ELECTRON: "1" },
  });
  if (res.status !== 0) {
    log(label, `FAILED (exit ${res.status})`);
    process.exit(res.status ?? 1);
  }
  log(label, "OK");
}

run("1/3 typecheck", "npx", ["tsc"]);
run("2/3 vite", "npx", ["vite", "build"]);
run("3/3 package", "npx", [
  "electron-builder",
  "--linux",
  "--win",
  "--publish",
  "never",
]);
log("done", "Electron build complete");