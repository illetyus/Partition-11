import { contextBridge, ipcRenderer } from "electron";
import type { Partition11Bridge } from "../shared/protocol";
import type { BenchmarkStartRequest } from "../shared/benchmark";

// Sandbox preloads can only require a restricted set of built-in modules.
// Keep channel literals inside this single self-contained emitted script;
// type-only imports are erased by TypeScript during compilation.
const CHANNELS = Object.freeze({
  status: "partition11:status",
  displayInfo: "partition11:display-info",
  displayChanged: "partition11:display-changed",
  titlebarTheme: "partition11:titlebar-theme",
  benchmarkAvailability: "partition11:benchmark-availability",
  benchmarkStart: "partition11:benchmark-start",
  benchmarkCancel: "partition11:benchmark-cancel",
  readOnlyDiskInventory: "partition11:read-only-disk-inventory",
  imageOpen: "partition11:image-open",
  imageListDirectory: "partition11:image-list-directory",
  imageNextPage: "partition11:image-next-page",
  imageExportFile: "partition11:image-export-file",
  imageCloseSession: "partition11:image-close-session"
});

const api: Partition11Bridge = Object.freeze({
  getStatus: () => ipcRenderer.invoke(CHANNELS.status),
  getDisplayInfo: () => ipcRenderer.invoke(CHANNELS.displayInfo),
  getBenchmarkAvailability: () => ipcRenderer.invoke(CHANNELS.benchmarkAvailability),
  startBenchmark: (request: BenchmarkStartRequest) => ipcRenderer.invoke(CHANNELS.benchmarkStart, request),
  cancelBenchmark: (requestId: string) => ipcRenderer.invoke(CHANNELS.benchmarkCancel, requestId),
  getReadOnlyDiskInventory: () => ipcRenderer.invoke(CHANNELS.readOnlyDiskInventory),
  openImage: () => ipcRenderer.invoke(CHANNELS.imageOpen),
  listImageDirectory: (sessionId: string, directoryId: string) => ipcRenderer.invoke(CHANNELS.imageListDirectory, sessionId, directoryId),
  nextImagePage: (sessionId: string, pageToken: string) => ipcRenderer.invoke(CHANNELS.imageNextPage, sessionId, pageToken),
  exportImageFile: (sessionId: string, fileId: string) => ipcRenderer.invoke(CHANNELS.imageExportFile, sessionId, fileId),
  closeImageSession: (sessionId: string) => ipcRenderer.invoke(CHANNELS.imageCloseSession, sessionId),
  setTitlebarTheme: (theme: "light" | "dark") => ipcRenderer.invoke(CHANNELS.titlebarTheme, theme),
  onDisplayChanged(listener: () => void) {
    const handler = () => listener();
    ipcRenderer.on(CHANNELS.displayChanged, handler);
    return () => ipcRenderer.removeListener(CHANNELS.displayChanged, handler);
  }
});

contextBridge.exposeInMainWorld("partition11", api);
