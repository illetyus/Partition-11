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

export type VolumeEligibilityReason =
  | "eligible"
  | "system_volume"
  | "system_volume_unknown"
  | "no_drive_letter_mount"
  | "ambiguous_mount"
  | "unsupported_drive_type"
  | "unsupported_filesystem"
  | "read_only"
  | "missing_space_metadata"
  | "insufficient_space"
  | "multiple_extents"
  | "missing_extents"
  | "invalid_extent"
  | "unknown_disk"
  | "ambiguous_disk"
  | "boot_disk"
  | "partition_unmapped"
  | "protected_partition"
  | "unsupported_partition";

export type ReadOnlyVolume = Readonly<{
  ordinal: number;
  mountPoint: string | null;
  driveType: "unknown" | "no_root" | "removable" | "fixed" | "remote" | "cdrom" | "ramdisk";
  fileSystem: "NTFS" | "ReFS" | "exFAT" | "FAT" | "FAT32" | null;
  totalBytes: string | null;
  availableBytes: string | null;
  readOnly: boolean;
  eligible: boolean;
  eligibilityReason: VolumeEligibilityReason;
  diskOrdinal: number | null;
  partitionNumber: number | null;
  /** Random token minted by Electron main and bound to the latest scan only. */
  selectionToken: string | null;
}>;

export type ReadOnlyDiskInventory =
  | Readonly<{
      protocolVersion: 2;
      status: "ok";
      scannedAt: string;
      disks: readonly ReadOnlyDisk[];
      volumes: readonly ReadOnlyVolume[];
    }>
  | Readonly<{
      protocolVersion: 2;
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
