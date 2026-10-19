/**
 * Narrow and versioned API. The renderer never receives Node APIs, physical
 * device paths, durable identifiers or native handles.
 */
export const IPC = Object.freeze({
  STATUS: "partition11:status",
  DISPLAY_INFO: "partition11:display-info",
  DISPLAY_CHANGED: "partition11:display-changed",
  TITLEBAR_THEME: "partition11:titlebar-theme",
  BENCHMARK_AVAILABILITY: "partition11:benchmark-availability",
  READ_ONLY_DISK_INVENTORY: "partition11:read-only-disk-inventory"
} as const);

export type AppStatus = Readonly<{
  protocolVersion: 1;
  appVersion: string;
  phase: "P0";
  agentConnected: false;
  capabilities: Readonly<{
    inspectImages: false;
    inspectPhysicalDisks: boolean;
    writeImages: false;
    writePhysicalDisks: false;
  }>;
}>;

export type InventoryAvailability =
  | "ready"
  | "access_denied"
  | "device_unavailable"
  | "unsupported"
  | "identity_changed"
  | "scan_timeout"
  | "agent_failure";

export type ReadOnlyDisk = Readonly<{
  ordinal: number;
  availability: InventoryAvailability;
  capacityBytes: string | null;
  logicalSectorSize: number | null;
  physicalSectorSize: number | null;
  busType: string | null;
  partitionStyle: "gpt" | "mbr" | "raw" | null;
  partitionCount: number | null;
}>;

export type ReadOnlyDiskInventory =
  | Readonly<{
      protocolVersion: 1;
      status: "ok";
      scannedAt: string;
      disks: readonly ReadOnlyDisk[];
    }>
  | Readonly<{
      protocolVersion: 1;
      status: "error";
      error: "agent_unavailable" | "timeout" | "invalid_response" | "access_denied" | "agent_failure";
    }>;

export type DisplayInfo = Readonly<{
  protocolVersion: 1;
  scaleFactor: number;
  workAreaWidth: number;
  workAreaHeight: number;
  highContrast: boolean;
  platform: "windows" | "other";
}>;

import type { BenchmarkAvailability } from "./benchmark";

export interface Partition11Bridge {
  getStatus(): Promise<AppStatus>;
  getDisplayInfo(): Promise<DisplayInfo>;
  getBenchmarkAvailability(): Promise<BenchmarkAvailability>;
  getReadOnlyDiskInventory(): Promise<ReadOnlyDiskInventory>;
  setTitlebarTheme(theme: "light" | "dark"): Promise<void>;
  onDisplayChanged(listener: () => void): () => void;
}
