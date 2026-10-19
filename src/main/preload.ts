import { contextBridge, ipcRenderer } from "electron";
import type { Partition11Bridge } from "../shared/protocol";

// Sandbox preloads can only require a restricted set of built-in modules.
// Keep channel literals inside this single self-contained emitted script;
// type-only imports are erased by TypeScript during compilation.
const CHANNELS = Object.freeze({
  status: "partition11:status",
  displayInfo: "partition11:display-info",
  displayChanged: "partition11:display-changed",
  titlebarTheme: "partition11:titlebar-theme",
  benchmarkAvailability: "partition11:benchmark-availability",
  readOnlyDiskInventory: "partition11:read-only-disk-inventory"
});

const api: Partition11Bridge = Object.freeze({
  getStatus: () => ipcRenderer.invoke(CHANNELS.status),
  getDisplayInfo: () => ipcRenderer.invoke(CHANNELS.displayInfo),
  getBenchmarkAvailability: () => ipcRenderer.invoke(CHANNELS.benchmarkAvailability),
  getReadOnlyDiskInventory: () => ipcRenderer.invoke(CHANNELS.readOnlyDiskInventory),
  setTitlebarTheme: (theme: "light" | "dark") => ipcRenderer.invoke(CHANNELS.titlebarTheme, theme),
  onDisplayChanged(listener: () => void) {
    const handler = () => listener();
    ipcRenderer.on(CHANNELS.displayChanged, handler);
    return () => ipcRenderer.removeListener(CHANNELS.displayChanged, handler);
  }
});

contextBridge.exposeInMainWorld("partition11", api);
