/**
 * Global types for the Alamut Electron preload bridge.
 *
 * The preload exposes `window.alamut` only when running inside the Electron
 * desktop wrapper; in the browser PWA it is undefined. Declaring the shape
 * here lets src/lib/api.ts and SettingsPage read it without `any` casts.
 */

export interface AlamutElectronBridge {
  /** Synchronous API base URL (argv/env resolved in the preload). */
  getApiUrlSync?: () => string;
  /** Async API base URL (re-queries the main process). */
  getApiUrl?: () => Promise<string>;
  /** Open the API data directory in the OS file manager. */
  openDataDir?: () => Promise<void>;
  /** Open an external URL in the system browser. */
  openExternal?: (url: string) => void;
  /** Quit the desktop app. */
  quit?: () => void;
}

declare global {
  interface Window {
    alamut?: AlamutElectronBridge;
  }
}

export {};